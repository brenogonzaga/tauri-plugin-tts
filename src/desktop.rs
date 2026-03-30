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

/// Normalize user rate (1.0 = normal) to platform-specific rate
/// Each platform has different rate scales:
/// - AVFoundation (macOS): 0.1-2.0, normal = 0.5
/// - WinRT (Windows): 0.5-6.0, normal = 1.0
/// - SpeechDispatcher (Linux): -100 to 100, normal = 0.0
/// - AppKit (macOS legacy): 10-500, normal = 175.0
fn normalize_rate_for_platform(engine: &TtsEngine, user_rate: f32) -> f32 {
    let normal = engine.normal_rate();
    let min = engine.min_rate();
    let max = engine.max_rate();

    // User rate: 1.0 = normal, <1 = slower, >1 = faster
    // Map user_rate to platform scale using normal_rate as anchor
    if user_rate <= 1.0 {
        // Map 0.25-1.0 → min-normal
        let t = (user_rate - 0.25) / 0.75; // 0.25→0, 1.0→1
        let t = t.clamp(0.0, 1.0);
        min + t * (normal - min)
    } else {
        // Map 1.0-4.0 → normal-max
        let t = (user_rate - 1.0) / 3.0; // 1.0→0, 4.0→1
        let t = t.clamp(0.0, 1.0);
        normal + t * (max - normal)
    }
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
            // Set voice if specified
            if let Some(ref voice_id) = validated.voice_id {
                if let Ok(voices) = engine.voices() {
                    if let Some(voice) = voices.into_iter().find(|v| v.id() == *voice_id) {
                        let _ = engine.set_voice(&voice);
                    }
                }
            }

            // WORKAROUND: If all values are default (1.0), do not configure anything
            // Some engines (especially Google TTS) have bugs when default values are explicitly set
            let all_defaults =
                validated.rate == 1.0 && validated.pitch == 1.0 && validated.volume == 1.0;

            if !all_defaults {
                if validated.rate != 1.0 {
                    // Normalize user rate (1.0 = normal) to platform-specific scale
                    // Each platform has different rate ranges and normal values:
                    // - AVFoundation (macOS): 0.1-2.0, normal = 0.5
                    // - WinRT (Windows): 0.5-6.0, normal = 1.0
                    // - SpeechDispatcher (Linux): -100 to 100, normal = 0.0
                    let rate_to_set = normalize_rate_for_platform(engine, validated.rate);
                    let _ = engine.set_rate(rate_to_set);
                }

                if validated.pitch != 1.0 {
                    // Pitch: tts library uses 0.5-2.0, same as our API (already validated/clamped)
                    let _ = engine.set_pitch(validated.pitch);
                }

                if validated.volume != 1.0 {
                    // Volume: both use 0.0-1.0 (already validated/clamped)
                    let _ = engine.set_volume(validated.volume);
                }
            }

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
