use serde::de::DeserializeOwned;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tauri::{plugin::PluginApi, AppHandle, Emitter, Runtime};
use tts::{Features, Tts as TtsEngine};

use crate::models::*;

struct VoiceCache {
    voices: Vec<Voice>,
    cached_at: Instant,
}

impl VoiceCache {
    const TTL: Duration = Duration::from_secs(60);

    fn new(voices: Vec<Voice>) -> Self {
        Self {
            voices,
            cached_at: Instant::now(),
        }
    }

    fn is_valid(&self) -> bool {
        self.cached_at.elapsed() < Self::TTL
    }
}

struct EventEmitter<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> EventEmitter<R> {
    fn emit(&self, event_name: &str, event: TtsEventPayload) {
        let full_event_name = format!("tts://{}", event_name);
        if let Err(e) = self.app.emit(&full_event_name, event) {
            log::warn!("Failed to emit TTS event '{}': {}", event_name, e);
        }
    }
}

/// Map a user value where `1.0` means "the platform's normal" onto that
/// platform's own scale, which differs wildly per backend:
/// - AVFoundation (macOS): rate 0.1-2.0 normal 0.5, pitch 0.5-2.0 normal 1.0
/// - WinRT (Windows): rate 0.5-6.0 normal 1.0, pitch 0-2 normal 1.0
/// - SpeechDispatcher (Linux): rate and pitch -100 to 100, normal 0.0
fn scale_anchored_at_normal(
    user: f32,
    user_min: f32,
    user_max: f32,
    min: f32,
    normal: f32,
    max: f32,
) -> f32 {
    if user <= 1.0 {
        let t = ((user - user_min) / (1.0 - user_min)).clamp(0.0, 1.0);
        min + t * (normal - min)
    } else {
        let t = ((user - 1.0) / (user_max - 1.0)).clamp(0.0, 1.0);
        normal + t * (max - normal)
    }
}

/// Volume is not anchored at "normal": our 0.0-1.0 maps straight across the
/// platform's full range (Linux is -100..100 with normal == max == 100).
fn scale_volume(user: f32, min: f32, max: f32) -> f32 {
    min + user.clamp(0.0, 1.0) * (max - min)
}

/// Whether `filter` is a locale prefix of `voice_language` — both "pt" and
/// "pt-BR" match a "pt-BR" voice. Case-insensitive.
fn language_matches(voice_language: &str, filter: &str) -> bool {
    voice_language
        .to_lowercase()
        .starts_with(&filter.to_lowercase())
}

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Tts<R>> {
    let engine = TtsEngine::default().map_err(|e| {
        // Provide better error message for Linux when speech-dispatcher is not installed
        #[cfg(target_os = "linux")]
        {
            let err_msg = e.to_string();
            if err_msg.contains("speech-dispatcher") || err_msg.contains("Speech Dispatcher") {
                return crate::Error::OperationFailed(
                    "Speech Dispatcher not available. Please install it:\n\
                    Ubuntu/Debian: sudo apt install speech-dispatcher\n\
                    Fedora: sudo dnf install speech-dispatcher\n\
                    Arch: sudo pacman -S speech-dispatcher"
                        .to_string(),
                );
            }
        }
        crate::Error::from(e)
    })?;

    // Set up utterance callbacks if supported
    let Features {
        utterance_callbacks,
        ..
    } = engine.supported_features();

    let emitter = Arc::new(EventEmitter { app: app.clone() });
    // Shared utterance ID: set by speak(), read by callbacks to include in finish/cancel events.
    let current_utterance_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    if utterance_callbacks {
        // Clone emitter and ID slot for each callback
        let end_emitter = Arc::clone(&emitter);
        let stop_emitter = Arc::clone(&emitter);
        let end_id = Arc::clone(&current_utterance_id);
        let stop_id = Arc::clone(&current_utterance_id);

        // Set up on_utterance_end callback (natural completion)
        if let Err(e) = engine.on_utterance_end(Some(Box::new(move |_utterance_id| {
            let id = end_id.lock().ok().and_then(|g| g.clone());
            end_emitter.emit(
                "speech:finish",
                TtsEventPayload {
                    event_type: "speech:finish".to_string(),
                    id,
                    ..Default::default()
                },
            );
        }))) {
            log::warn!("Failed to set on_utterance_end callback: {:?}", e);
        }

        // Set up on_utterance_stop callback (cancelled/interrupted)
        if let Err(e) = engine.on_utterance_stop(Some(Box::new(move |_utterance_id| {
            let id = stop_id.lock().ok().and_then(|g| g.clone());
            stop_emitter.emit(
                "speech:cancel",
                TtsEventPayload {
                    event_type: "speech:cancel".to_string(),
                    id,
                    ..Default::default()
                },
            );
        }))) {
            log::warn!("Failed to set on_utterance_stop callback: {:?}", e);
        }

        log::info!("TTS utterance callbacks enabled for speech:finish events");
    } else {
        log::warn!("TTS engine does not support utterance callbacks - speech:finish events will not be emitted");
    }

    Ok(Tts {
        app: app.clone(),
        engine: Mutex::new(engine),
        voice_cache: RwLock::new(None),
        has_utterance_callbacks: utterance_callbacks,
        current_utterance_id,
    })
}

pub struct Tts<R: Runtime> {
    app: AppHandle<R>,
    engine: Mutex<TtsEngine>,
    voice_cache: RwLock<Option<VoiceCache>>,
    has_utterance_callbacks: bool,
    /// Shared with utterance callbacks so finish/cancel events carry the same ID as start.
    current_utterance_id: Arc<Mutex<Option<String>>>,
}

impl<R: Runtime> Tts<R> {
    /// Helper to acquire engine lock with proper error handling
    fn with_engine<T, F>(&self, f: F) -> crate::Result<T>
    where
        F: FnOnce(&mut TtsEngine) -> crate::Result<T>,
    {
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| crate::Error::MutexPoisoned)?;
        f(&mut engine)
    }

    fn emit_event(&self, event_name: &str, event: TtsEventPayload) {
        let full_event_name = format!("tts://{}", event_name);
        if let Err(e) = self.app.emit(&full_event_name, event) {
            log::warn!("Failed to emit TTS event '{}': {}", event_name, e);
        }
    }

    pub fn speak(&self, payload: SpeakRequest) -> crate::Result<SpeakResponse> {
        // Validate input first (before acquiring lock)
        let validated = payload.validate()?;

        // Generate utterance ID for tracking and share it with the utterance callbacks.
        let utterance_id = uuid::Uuid::now_v7().to_string();
        if let Ok(mut guard) = self.current_utterance_id.lock() {
            *guard = Some(utterance_id.clone());
        }

        let result = self.with_engine(|engine| {
            // An explicit voice id wins; otherwise fall back to the first voice
            // for `language`, which would otherwise be silently ignored here.
            let voice = engine
                .voices()
                .ok()
                .and_then(|voices| match validated.voice_id {
                    Some(ref id) => voices.into_iter().find(|v| v.id() == *id),
                    None => validated.language.as_deref().and_then(|lang| {
                        voices
                            .into_iter()
                            .find(|v| language_matches(v.language().as_ref(), lang))
                    }),
                });
            if let Some(voice) = voice {
                let _ = engine.set_voice(&voice);
            }

            // Always set all three: the engine keeps whatever was last set, so
            // skipping a parameter leaks the previous utterance's value into
            // this one (a 2x-rate call would make every later call 2x too).
            let _ = engine.set_rate(scale_anchored_at_normal(
                validated.rate,
                0.25,
                4.0,
                engine.min_rate(),
                engine.normal_rate(),
                engine.max_rate(),
            ));
            let _ = engine.set_pitch(scale_anchored_at_normal(
                validated.pitch,
                0.5,
                2.0,
                engine.min_pitch(),
                engine.normal_pitch(),
                engine.max_pitch(),
            ));
            let _ = engine.set_volume(scale_volume(
                validated.volume,
                engine.min_volume(),
                engine.max_volume(),
            ));

            // Determine if we should interrupt current speech
            // flush (default) = interrupt, add = queue
            let interrupt = validated.queue_mode != QueueMode::Add;

            engine.speak(&validated.text, interrupt)?;

            Ok(SpeakResponse {
                success: true,
                warning: None,
            })
        });

        // Emit speech:start only after engine.speak() succeeds
        if result.is_ok() {
            self.emit_event(
                "speech:start",
                TtsEventPayload {
                    event_type: "speech:start".to_string(),
                    id: Some(utterance_id),
                    ..Default::default()
                },
            );
        }

        result
    }

    pub fn stop(&self) -> crate::Result<StopResponse> {
        self.with_engine(|engine| {
            engine.stop()?;
            Ok(())
        })?;

        // Only emit speech:cancel as a fallback for engines without utterance callbacks.
        // When callbacks are supported, on_utterance_stop fires and emits the event.
        if !self.has_utterance_callbacks {
            self.emit_event(
                "speech:cancel",
                TtsEventPayload {
                    event_type: "speech:cancel".to_string(),
                    ..Default::default()
                },
            );
        }

        Ok(StopResponse { success: true })
    }

    pub fn get_voices(&self, payload: GetVoicesRequest) -> crate::Result<GetVoicesResponse> {
        // Try to use cached voices first
        {
            let cache = self
                .voice_cache
                .read()
                .map_err(|_| crate::Error::MutexPoisoned)?;
            if let Some(ref c) = *cache {
                if c.is_valid() {
                    return Ok(self.filter_voices(&c.voices, &payload.language));
                }
            }
        }

        // Cache miss or expired - fetch from engine
        let voices = self.with_engine(|engine| {
            let native_voices = engine.voices()?;
            Ok(native_voices
                .into_iter()
                .map(|v| Voice {
                    id: v.id().to_string(),
                    name: v.name().to_string(),
                    language: v.language().to_string(),
                })
                .collect::<Vec<Voice>>())
        })?;

        // Update cache
        {
            let mut cache = self
                .voice_cache
                .write()
                .map_err(|_| crate::Error::MutexPoisoned)?;
            *cache = Some(VoiceCache::new(voices.clone()));
        }

        Ok(self.filter_voices(&voices, &payload.language))
    }

    fn filter_voices(&self, voices: &[Voice], language: &Option<String>) -> GetVoicesResponse {
        let filtered: Vec<Voice> = voices
            .iter()
            .filter(|v| {
                if let Some(ref lang_filter) = language {
                    v.language
                        .to_lowercase()
                        .contains(&lang_filter.to_lowercase())
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        GetVoicesResponse { voices: filtered }
    }

    pub fn is_speaking(&self) -> crate::Result<IsSpeakingResponse> {
        self.with_engine(|engine| {
            let speaking = engine.is_speaking()?;
            Ok(IsSpeakingResponse { speaking })
        })
    }

    pub fn is_initialized(&self) -> crate::Result<IsInitializedResponse> {
        // Desktop TTS is always initialized after construction
        // Get voice count from cache or fetch
        let voice_count = self
            .get_voices(GetVoicesRequest { language: None })
            .map(|r| r.voices.len() as u32)
            .unwrap_or(0);
        Ok(IsInitializedResponse {
            initialized: true,
            voice_count,
        })
    }

    pub fn pause_speaking(&self) -> crate::Result<PauseResumeResponse> {
        // Desktop TTS library (tts-rs) doesn't support pause/resume
        // Return a descriptive error
        Ok(PauseResumeResponse {
            success: false,
            reason: Some("Pause is not supported on desktop platform".to_string()),
        })
    }

    pub fn resume_speaking(&self) -> crate::Result<PauseResumeResponse> {
        // Desktop TTS library (tts-rs) doesn't support pause/resume
        Ok(PauseResumeResponse {
            success: false,
            reason: Some("Resume is not supported on desktop platform".to_string()),
        })
    }

    pub fn preview_voice(&self, payload: PreviewVoiceRequest) -> crate::Result<SpeakResponse> {
        // Validate the preview request
        payload.validate()?;

        // Create a speak request with the sample text and specified voice
        let speak_request = SpeakRequest {
            text: payload.sample_text().into_owned(),
            language: None,
            voice_id: Some(payload.voice_id),
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        };
        self.speak(speak_request)
    }

    pub fn set_background_behavior(
        &self,
        _payload: SetBackgroundBehaviorRequest,
    ) -> crate::Result<SetBackgroundBehaviorResponse> {
        // Desktop has no background/foreground concept — no-op
        Ok(SetBackgroundBehaviorResponse { success: true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real backend ranges, from tts 0.26 (min, normal, max).
    const AV_RATE: (f32, f32, f32) = (0.1, 0.5, 2.0);
    const WINRT_RATE: (f32, f32, f32) = (0.5, 1.0, 6.0);
    const SD_PITCH: (f32, f32, f32) = (-100.0, 0.0, 100.0);

    fn rate(user: f32, (min, normal, max): (f32, f32, f32)) -> f32 {
        scale_anchored_at_normal(user, 0.25, 4.0, min, normal, max)
    }

    #[test]
    fn normal_user_value_maps_to_the_platform_normal() {
        assert_eq!(rate(1.0, AV_RATE), 0.5);
        assert_eq!(rate(1.0, WINRT_RATE), 1.0);
        // The regression that made pitch 1.0 nearly inaudible on Linux: a raw
        // 1.0 on a -100..100 scale is not "normal", 0.0 is.
        assert_eq!(
            scale_anchored_at_normal(1.0, 0.5, 2.0, SD_PITCH.0, SD_PITCH.1, SD_PITCH.2),
            0.0
        );
    }

    #[test]
    fn extremes_saturate_at_the_platform_bounds() {
        assert_eq!(rate(4.0, AV_RATE), 2.0);
        assert_eq!(rate(0.25, AV_RATE), 0.1);
        assert_eq!(rate(99.0, WINRT_RATE), 6.0); // above our range, still clamped
        assert_eq!(rate(0.0, WINRT_RATE), 0.5);
        assert_eq!(
            scale_anchored_at_normal(2.0, 0.5, 2.0, SD_PITCH.0, SD_PITCH.1, SD_PITCH.2),
            100.0
        );
    }

    #[test]
    fn rate_stays_monotonic_and_matches_the_previous_tuning() {
        assert!(rate(0.5, AV_RATE) < rate(1.0, AV_RATE));
        assert!(rate(1.0, AV_RATE) < rate(2.0, AV_RATE));
        // 0.25-1.0 → min-normal, unchanged from the original rate-only mapping.
        assert_eq!(rate(0.625, AV_RATE), 0.1 + 0.5 * (0.5 - 0.1));
    }

    #[test]
    fn volume_spans_the_full_platform_range() {
        assert_eq!(scale_volume(1.0, 0.0, 1.0), 1.0);
        assert_eq!(scale_volume(0.0, 0.0, 1.0), 0.0);
        // Linux: normal == max == 100, silent == -100.
        assert_eq!(scale_volume(1.0, -100.0, 100.0), 100.0);
        assert_eq!(scale_volume(0.0, -100.0, 100.0), -100.0);
        assert_eq!(scale_volume(0.5, -100.0, 100.0), 0.0);
    }

    #[test]
    fn language_filter_matches_bare_and_full_tags() {
        assert!(language_matches("pt-BR", "pt"));
        assert!(language_matches("pt-BR", "pt-BR"));
        assert!(language_matches("pt-BR", "PT-br")); // case-insensitive
        assert!(language_matches("en-US", "en"));
        assert!(!language_matches("en-US", "pt"));
        assert!(!language_matches("pt-BR", "pt-PT"));
    }
}
