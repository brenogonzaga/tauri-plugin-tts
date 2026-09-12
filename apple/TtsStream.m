// Buffer-producing speech synthesis over a C ABI (iOS + macOS).
//
// The host app owns audio output (a Rust playback pipeline). Instead of
// letting AVSpeechSynthesizer play into the device, we drive
// `-[AVSpeechSynthesizer writeUtterance:toBufferCallback:]` and hand every
// PCM buffer to Rust through the `rust_tts_stream_*` callbacks the plugin's
// Rust side exports. No WKWebView, no Tauri invoke bridge, no competing
// audio session configuration — synthesis becomes a pure function from text
// to samples the host can queue, pause, visualize and replay like any other
// TTS engine's audio.
//
// Compiled by the crate's build.rs via `cc` for `target_os = "ios"` and
// `"macos"`; the same source serves both.
//
// Threading: `tts_stream_*` may be called from any thread. Job state is
// serialized on a private serial queue; the synthesizer's buffer callback and
// delegate methods arrive on the main queue (the framework requires a running
// main run loop — a Tauri app always has one) and hop onto that queue. Rust
// callbacks are invoked from that queue and must not block (they push into a
// channel).
//
// Lifetime: synthesizers are never deallocated. Each lives in a
// `TtsStreamChannel` that is reused for job after job — see that class for
// the framework behaviour that forces this.

#import <AVFoundation/AVFoundation.h>
#import <Foundation/Foundation.h>
#include <stdint.h>

// MARK: - Rust callbacks (exported by src/apple_stream.rs)

extern void rust_tts_stream_on_audio(uint64_t job_id, const float *samples, uint32_t count,
                                     double sample_rate);
extern void rust_tts_stream_on_error(uint64_t job_id, const char *message);
extern void rust_tts_stream_on_ended(uint64_t job_id);

// Error codes returned by `tts_stream_start`. Mirrored in Rust.
enum TtsStreamStartError {
    TtsStreamStartOk = 0,
    TtsStreamStartEmptyText = 1,
    TtsStreamStartAlreadyRunning = 2,
    TtsStreamStartNoVoice = 3,
};

// MARK: - Shared state

static dispatch_queue_t tts_stream_queue(void) {
    static dispatch_queue_t queue;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        queue = dispatch_queue_create("com.yellowbites.tts-stream", DISPATCH_QUEUE_SERIAL);
    });
    return queue;
}

@class TtsStreamJob;
@class TtsStreamChannel;

/// Live jobs by id. Ids are minted by Rust; a stale id is a no-op.
static NSMutableDictionary<NSNumber *, TtsStreamJob *> *tts_stream_jobs(void) {
    static NSMutableDictionary *jobs;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        jobs = [NSMutableDictionary dictionary];
    });
    return jobs;
}

/// Every synthesizer channel ever created, busy or idle. Channels are never
/// removed — see `TtsStreamChannel` for why they must outlive their jobs.
static NSMutableArray<TtsStreamChannel *> *tts_stream_channels(void) {
    static NSMutableArray *channels;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        channels = [NSMutableArray array];
    });
    return channels;
}

/// Convert whatever PCM layout the synthesizer produced into mono float32.
/// Returns nil if the buffer's format is not understood.
static NSData *mono_float_samples(AVAudioPCMBuffer *pcm) {
    AVAudioFormat *format = pcm.format;
    AVAudioFrameCount frames = pcm.frameLength;
    AVAudioChannelCount channels = format.channelCount;
    if (channels == 0) {
        return nil;
    }
    BOOL interleaved = format.isInterleaved;
    NSMutableData *mono = [NSMutableData dataWithLength:frames * sizeof(float)];
    float *out = (float *)mono.mutableBytes;

    switch (format.commonFormat) {
    case AVAudioPCMFormatFloat32: {
        float *const *data = pcm.floatChannelData;
        if (!data) return nil;
        for (AVAudioFrameCount f = 0; f < frames; f++) {
            float acc = 0;
            for (AVAudioChannelCount c = 0; c < channels; c++) {
                acc += interleaved ? data[0][f * channels + c] : data[c][f];
            }
            out[f] = acc / (float)channels;
        }
        break;
    }
    case AVAudioPCMFormatInt16: {
        int16_t *const *data = pcm.int16ChannelData;
        if (!data) return nil;
        for (AVAudioFrameCount f = 0; f < frames; f++) {
            float acc = 0;
            for (AVAudioChannelCount c = 0; c < channels; c++) {
                int16_t s = interleaved ? data[0][f * channels + c] : data[c][f];
                acc += (float)s / 32768.0f;
            }
            out[f] = acc / (float)channels;
        }
        break;
    }
    case AVAudioPCMFormatInt32: {
        int32_t *const *data = pcm.int32ChannelData;
        if (!data) return nil;
        for (AVAudioFrameCount f = 0; f < frames; f++) {
            float acc = 0;
            for (AVAudioChannelCount c = 0; c < channels; c++) {
                int32_t s = interleaved ? data[0][f * channels + c] : data[c][f];
                acc += (float)s / 2147483648.0f;
            }
            out[f] = acc / (float)channels;
        }
        break;
    }
    default:
        return nil;
    }
    return mono;
}

// MARK: - One synthesis job

/// The bookkeeping for one utterance: which channel renders it and whether
/// Rust has been told it ended. Holds no framework object of its own beyond
/// the utterance, so its lifetime is free to end with the job.
@interface TtsStreamJob : NSObject
@property(nonatomic, readonly) uint64_t jobId;
@property(nonatomic, strong) AVSpeechUtterance *utterance;
@property(nonatomic, strong) TtsStreamChannel *channel;
@property(nonatomic, assign) BOOL ended;
@property(nonatomic, assign) BOOL cancelled;
- (instancetype)initWithId:(uint64_t)jobId;
- (void)failWithMessage:(NSString *)message;
- (void)finish;
@end

// MARK: - One synthesizer, reused for job after job

/// An `AVSpeechSynthesizer` plus its permanent delegate.
///
/// Why synthesizers are reused instead of created per job: the framework
/// keeps working on a synthesizer on the main queue after it delivered the
/// last buffer and after the delegate learned the utterance finished
/// (`TTSSpeechManager` → `-[AVSpeechSynthesizer processSpeechJobFinished:]`).
/// A synthesizer released from our queue inside that window is a
/// use-after-free the framework trips over — seen as SIGSEGV in
/// `processSpeechJobFinished:successful:` on iOS 26. Hence channels are
/// process-lifetime objects: created on demand, parked when idle, never
/// deallocated. A channel renders one utterance at a time; it is idle again
/// once its delegate was told that utterance finished or was cancelled — the
/// framework's last word about it. A channel whose delegate never hears back
/// simply stays parked; the next job takes another one.
@interface TtsStreamChannel : NSObject <AVSpeechSynthesizerDelegate>
@property(nonatomic, strong, readonly) AVSpeechSynthesizer *synthesizer;
/// The job being rendered; nil while idle. Written on the stream queue only.
@property(nonatomic, strong) TtsStreamJob *job;
@end

@implementation TtsStreamJob

- (instancetype)initWithId:(uint64_t)jobId {
    self = [super init];
    if (self) {
        _jobId = jobId;
        _ended = NO;
        _cancelled = NO;
    }
    return self;
}

// Helpers below run on the stream queue.

- (void)failWithMessage:(NSString *)message {
    if (self.ended) return;
    rust_tts_stream_on_error(self.jobId, message.UTF8String);
    [self finish];
}

- (void)finish {
    if (self.ended) return;
    self.ended = YES;
    rust_tts_stream_on_ended(self.jobId);
    [tts_stream_jobs() removeObjectForKey:@(self.jobId)];
}

@end

@implementation TtsStreamChannel

- (instancetype)init {
    self = [super init];
    if (self) {
        _synthesizer = [[AVSpeechSynthesizer alloc] init];
        _synthesizer.delegate = self;
    }
    return self;
}

/// Stream queue. Hands the utterance to the synthesizer; buffers and the
/// end of the job come back through `job`.
- (void)startJob:(TtsStreamJob *)job {
    self.job = job;
    job.channel = self;
    uint64_t jobId = job.jobId;
    // The framework hands us one buffer at a time and — per the docs — an
    // empty buffer once the utterance is complete. The buffer is only
    // guaranteed valid during the callback, so convert here and hand the copy
    // to the stream queue. The block carries the job id, not the job: a
    // callback for a job that already ended finds nothing and does nothing.
    [self.synthesizer writeUtterance:job.utterance
                    toBufferCallback:^(AVAudioBuffer *_Nonnull buffer) {
                        AVAudioPCMBuffer *pcm = [buffer isKindOfClass:[AVAudioPCMBuffer class]]
                                                    ? (AVAudioPCMBuffer *)buffer
                                                    : nil;
                        NSString *failure = nil;
                        NSData *mono = nil;
                        double sampleRate = pcm ? pcm.format.sampleRate : 0;
                        BOOL finished = NO;
                        if (!pcm) {
                            failure = @"Synthesizer produced a non-PCM buffer";
                        } else if (pcm.frameLength == 0) {
                            finished = YES;
                        } else {
                            mono = mono_float_samples(pcm);
                            if (!mono) {
                                failure = @"Synthesizer produced an unsupported PCM format";
                            }
                        }
                        dispatch_async(tts_stream_queue(), ^{
                            TtsStreamJob *live = tts_stream_jobs()[@(jobId)];
                            if (!live || live.ended) return;
                            if (failure) {
                                [live failWithMessage:failure];
                            } else if (finished) {
                                [live finish];
                            } else {
                                uint32_t count = (uint32_t)(mono.length / sizeof(float));
                                rust_tts_stream_on_audio(jobId, (const float *)mono.bytes, count,
                                                         sampleRate);
                            }
                        });
                    }];
}

/// Stream queue. Aborts the running utterance; the channel is released for
/// reuse when the delegate reports the cancellation.
- (void)cancelJob:(TtsStreamJob *)job {
    if (job.ended) return;
    job.cancelled = YES;
    [self.synthesizer stopSpeakingAtBoundary:AVSpeechBoundaryImmediate];
    [job finish];
}

/// Stream queue. The framework is done with `utterance`: end its job (a
/// no-op when the empty buffer already did) and park the channel.
- (void)utteranceDone:(AVSpeechUtterance *)utterance cancelled:(BOOL)cancelled {
    TtsStreamJob *job = self.job;
    if (!job || job.utterance != utterance) {
        return;
    }
    if (cancelled && !job.cancelled && !job.ended) {
        [job failWithMessage:@"Synthesis was cancelled by the system"];
    } else {
        [job finish];
    }
    self.job = nil;
    job.channel = nil;
}

// MARK: AVSpeechSynthesizerDelegate (main queue)

// Belt and braces: some OS versions have been seen to skip the terminating
// empty buffer. The delegate's finish/cancel notifications end the job too;
// whichever arrives first wins, the other is a no-op. Either way only the
// delegate frees the channel.
- (void)speechSynthesizer:(AVSpeechSynthesizer *)synthesizer
    didFinishSpeechUtterance:(AVSpeechUtterance *)utterance {
    dispatch_async(tts_stream_queue(), ^{
        [self utteranceDone:utterance cancelled:NO];
    });
}

- (void)speechSynthesizer:(AVSpeechSynthesizer *)synthesizer
    didCancelSpeechUtterance:(AVSpeechUtterance *)utterance {
    dispatch_async(tts_stream_queue(), ^{
        [self utteranceDone:utterance cancelled:YES];
    });
}

@end

/// Stream queue. An idle channel, or a new one when every existing channel
/// is busy. The count settles at the host's synthesis concurrency plus one
/// or two: a channel stays busy until its finish notification has hopped
/// from the main queue, which a back-to-back job does not wait for.
static TtsStreamChannel *tts_stream_idle_channel(void) {
    NSMutableArray<TtsStreamChannel *> *channels = tts_stream_channels();
    for (TtsStreamChannel *channel in channels) {
        if (channel.job == nil) {
            return channel;
        }
    }
    TtsStreamChannel *channel = [[TtsStreamChannel alloc] init];
    [channels addObject:channel];
    NSLog(@"[TtsStream] synthesizer #%lu created", (unsigned long)channels.count);
    return channel;
}

// MARK: - C ABI (called from Rust)

static AVSpeechSynthesisVoice *tts_stream_pick_voice(const char *voiceId, const char *language) {
    if (voiceId && voiceId[0] != '\0') {
        NSString *wanted = [NSString stringWithUTF8String:voiceId];
        for (AVSpeechSynthesisVoice *voice in [AVSpeechSynthesisVoice speechVoices]) {
            if ([voice.identifier isEqualToString:wanted]) {
                return voice;
            }
        }
        NSLog(@"[TtsStream] voice %@ not installed, falling back to language default", wanted);
    }
    if (language && language[0] != '\0') {
        NSString *lang = [NSString stringWithUTF8String:language];
        AVSpeechSynthesisVoice *voice = [AVSpeechSynthesisVoice voiceWithLanguage:lang];
        if (voice) return voice;
        // Bare code ("de"): take the first installed voice of that language.
        NSString *prefix = [lang lowercaseString];
        for (AVSpeechSynthesisVoice *candidate in [AVSpeechSynthesisVoice speechVoices]) {
            if ([[candidate.language lowercaseString] hasPrefix:prefix]) {
                return candidate;
            }
        }
    }
    return [AVSpeechSynthesisVoice voiceWithLanguage:[AVSpeechSynthesisVoice currentLanguageCode]];
}

/// Start a synthesis job. Returns 0 on success or a `TtsStreamStartError`.
///
/// `rate` is the host's speed (1.0 = the platform's default rate), mapped
/// onto AVSpeechUtterance's [min, max] around `AVSpeechUtteranceDefaultSpeechRate`.
/// `voice_id`/`language` may be NULL or empty; the voice falls back to the
/// language, then to the system language.
int32_t tts_stream_start(uint64_t job_id, const char *text, const char *voice_id,
                         const char *language, float rate, float pitch, float volume) {
    if (!text || text[0] == '\0') {
        return TtsStreamStartEmptyText;
    }
    NSString *string = [NSString stringWithUTF8String:text];
    if (string.length == 0) {
        return TtsStreamStartEmptyText;
    }

    AVSpeechSynthesisVoice *voice = tts_stream_pick_voice(voice_id, language);
    if (!voice) {
        return TtsStreamStartNoVoice;
    }

    AVSpeechUtterance *utterance = [AVSpeechUtterance speechUtteranceWithString:string];
    utterance.voice = voice;
    float mapped = AVSpeechUtteranceDefaultSpeechRate * (rate > 0 ? rate : 1.0f);
    utterance.rate = fminf(fmaxf(mapped, AVSpeechUtteranceMinimumSpeechRate),
                           AVSpeechUtteranceMaximumSpeechRate);
    utterance.pitchMultiplier = fminf(fmaxf(pitch > 0 ? pitch : 1.0f, 0.5f), 2.0f);
    utterance.volume = fminf(fmaxf(volume, 0.0f), 1.0f);

    __block int32_t result = TtsStreamStartOk;
    dispatch_sync(tts_stream_queue(), ^{
        NSMutableDictionary *jobs = tts_stream_jobs();
        if (jobs[@(job_id)] != nil) {
            result = TtsStreamStartAlreadyRunning;
            return;
        }
        TtsStreamJob *job = [[TtsStreamJob alloc] initWithId:job_id];
        job.utterance = utterance;
        jobs[@(job_id)] = job;
        [tts_stream_idle_channel() startJob:job];
    });
    return result;
}

void tts_stream_cancel(uint64_t job_id) {
    dispatch_async(tts_stream_queue(), ^{
        TtsStreamJob *job = tts_stream_jobs()[@(job_id)];
        if (job) {
            [job.channel cancelJob:job];
        }
    });
}

/// Enumerate the installed voices. `visit` is called once per voice with
/// (ctx, identifier, name, language, quality) where quality is 1 default,
/// 2 enhanced, 3 premium (0 unknown).
void tts_stream_list_voices(void *ctx, void (*visit)(void *ctx, const char *identifier,
                                                     const char *name, const char *language,
                                                     int32_t quality)) {
    for (AVSpeechSynthesisVoice *voice in [AVSpeechSynthesisVoice speechVoices]) {
        int32_t quality = 0;
        switch (voice.quality) {
        case AVSpeechSynthesisVoiceQualityDefault:
            quality = 1;
            break;
        case AVSpeechSynthesisVoiceQualityEnhanced:
            quality = 2;
            break;
        default:
            // AVSpeechSynthesisVoiceQualityPremium (iOS 16 / macOS 13+) — compare by
            // raw value so the file still compiles against older SDKs.
            quality = voice.quality == 3 ? 3 : 0;
            break;
        }
        visit(ctx, voice.identifier.UTF8String, voice.name.UTF8String, voice.language.UTF8String,
              quality);
    }
}

/// The voice `tts_stream_start` falls back to for `language` when no voice id
/// resolves. Calls `visit` once (or never, if the platform has no voice at all).
void tts_stream_default_voice(const char *language, void *ctx,
                              void (*visit)(void *ctx, const char *identifier, const char *name,
                                            const char *language, int32_t quality)) {
    AVSpeechSynthesisVoice *voice = tts_stream_pick_voice(NULL, language);
    if (!voice) return;
    visit(ctx, voice.identifier.UTF8String, voice.name.UTF8String, voice.language.UTF8String, 0);
}

/// Test/example helper: pump the main run loop for `seconds`. AVSpeechSynthesizer
/// delivers its buffers via the main queue, which a plain command-line process
/// (cargo example) has to drain explicitly; a Tauri app's main loop already does.
void tts_stream_pump_main_run_loop(double seconds) {
    [[NSRunLoop mainRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:seconds]];
}
