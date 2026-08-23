mod events;
mod scale;
mod utterance;
mod voice;

use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};
use tts::{Features, Tts as Engine};

use crate::models::*;
use crate::validation::{ValidatedSpeakRequest, PITCH_RANGE, RATE_RANGE};
use crate::{Error, Result};

use events::EventEmitter;
use utterance::UtteranceQueue;
use voice::{unavailable_warning, VoiceCache};

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<Tts<R>> {
    let emitter = EventEmitter::new(app.clone());
    let utterances = Arc::new(UtteranceQueue::default());

    // A missing system synthesizer — most often no speech-dispatcher on Linux — must not
    // take the host application down with it.
    let engine = match Engine::default() {
        Ok(engine) => engine,
        Err(e) => {
            let reason = unavailable_reason(e);
            log::warn!("TTS plugin loaded in a degraded state: {reason}");
            return Ok(Tts::unavailable(emitter, utterances, reason));
        }
    };

    let Features {
        utterance_callbacks,
        ..
    } = engine.supported_features();

    if utterance_callbacks {
        register_callbacks(&engine, &emitter, &utterances);
    } else {
        log::warn!(
            "TTS engine has no utterance callbacks; speech:start and speech:cancel are \
             emitted from speak() and stop() instead, without real timing"
        );
    }

    Ok(Tts {
        engine: Some(Mutex::new(engine)),
        init_error: None,
        emitter,
        utterances,
        voices: VoiceCache::default(),
        has_utterance_callbacks: utterance_callbacks,
    })
}

fn register_callbacks<R: Runtime>(
    engine: &Engine,
    emitter: &Arc<EventEmitter<R>>,
    utterances: &Arc<UtteranceQueue>,
) {
    let (begin_emitter, begin_queue) = (Arc::clone(emitter), Arc::clone(utterances));
    let (end_emitter, end_queue) = (Arc::clone(emitter), Arc::clone(utterances));
    let (stop_emitter, stop_queue) = (Arc::clone(emitter), Arc::clone(utterances));

    let registered = [
        engine.on_utterance_begin(Some(Box::new(move |_| {
            begin_emitter.emit(events::START, begin_queue.begin());
        }))),
        engine.on_utterance_end(Some(Box::new(move |_| {
            end_emitter.emit(events::FINISH, end_queue.finish());
        }))),
        // An empty queue means stop() already drained it and reported the cancels itself,
        // so staying quiet here is what keeps an utterance from being cancelled twice.
        engine.on_utterance_stop(Some(Box::new(move |_| {
            if let Some(id) = stop_queue.finish() {
                stop_emitter.emit(events::CANCEL, Some(id));
            }
        }))),
    ];

    for result in registered {
        if let Err(e) = result {
            log::warn!("failed to register a TTS utterance callback: {e:?}");
        }
    }
}

fn unavailable_reason(e: tts::Error) -> String {
    #[cfg(target_os = "linux")]
    {
        let message = e.to_string();
        if message.contains("speech-dispatcher") || message.contains("Speech Dispatcher") {
            return "Speech Dispatcher not available. Please install it:\n\
                Ubuntu/Debian: sudo apt install speech-dispatcher\n\
                Fedora: sudo dnf install speech-dispatcher\n\
                Arch: sudo pacman -S speech-dispatcher"
                .to_string();
        }
    }
    format!("TTS engine unavailable: {e}")
}

pub struct Tts<R: Runtime> {
    engine: Option<Mutex<Engine>>,
    /// Why the engine is missing, for the error every command returns in that state.
    init_error: Option<String>,
    emitter: Arc<EventEmitter<R>>,
    utterances: Arc<UtteranceQueue>,
    voices: VoiceCache,
    has_utterance_callbacks: bool,
}

impl<R: Runtime> Tts<R> {
    fn unavailable(
        emitter: Arc<EventEmitter<R>>,
        utterances: Arc<UtteranceQueue>,
        reason: String,
    ) -> Self {
        Self {
            engine: None,
            init_error: Some(reason),
            emitter,
            utterances,
            voices: VoiceCache::default(),
            has_utterance_callbacks: false,
        }
    }

    fn with_engine<T>(&self, f: impl FnOnce(&mut Engine) -> Result<T>) -> Result<T> {
        let engine = self.engine.as_ref().ok_or_else(|| {
            Error::OperationFailed(
                self.init_error
                    .clone()
                    .unwrap_or_else(|| "TTS engine unavailable".to_string()),
            )
        })?;
        let mut engine = engine.lock().map_err(|_| Error::MutexPoisoned)?;
        f(&mut engine)
    }

    pub fn speak(&self, payload: SpeakRequest) -> Result<SpeakResponse> {
        let request = payload.validate()?;
        let id = uuid::Uuid::now_v7().to_string();

        let result = self.with_engine(|engine| {
            let installed = self.installed_voices(engine)?;
            let selected = voice::select(
                &installed,
                request.voice_id.as_deref(),
                request.language.as_deref(),
            );

            let warning = match selected {
                Some(selected) => {
                    set_engine_voice(engine, &selected.id);
                    None
                }
                None => {
                    unavailable_warning(request.voice_id.as_deref(), request.language.as_deref())
                }
            };

            self.apply_parameters(engine, &request);

            // Enqueue before speaking. Under `flush`, speak() stops the previous utterance
            // and that stop fires the cancel callback; with this utterance already queued
            // behind it, the cancel resolves to the utterance that was really cancelled no
            // matter which side of the call the callback lands on.
            self.utterances.push(id.clone());

            if let Err(e) = engine.speak(&request.text, request.queue_mode != QueueMode::Add) {
                self.utterances.forget(&id);
                return Err(e.into());
            }

            Ok(SpeakResponse {
                success: true,
                warning,
                utterance_id: Some(id.clone()),
            })
        });

        match &result {
            // With callbacks, speech:start comes from on_utterance_begin, which knows when
            // the utterance really started. Without them this is the only chance to report.
            Ok(_) if !self.has_utterance_callbacks => {
                self.emitter.emit(events::START, Some(id));
            }
            Ok(_) => {}
            Err(e) => self.emitter.emit_error(Some(id), e.to_string()),
        }

        result
    }

    fn apply_parameters(&self, engine: &mut Engine, request: &ValidatedSpeakRequest) {
        // All three are set every time: the engine keeps whatever was last set, so skipping
        // one leaks the previous utterance's value into this one.
        let _ = engine.set_rate(scale::anchored_at_normal(
            request.rate,
            RATE_RANGE,
            (engine.min_rate(), engine.normal_rate(), engine.max_rate()),
        ));
        let _ = engine.set_pitch(scale::anchored_at_normal(
            request.pitch,
            PITCH_RANGE,
            (
                engine.min_pitch(),
                engine.normal_pitch(),
                engine.max_pitch(),
            ),
        ));
        let _ = engine.set_volume(scale::across_range(
            request.volume,
            engine.min_volume(),
            engine.max_volume(),
        ));
    }

    pub fn stop(&self) -> Result<StopResponse> {
        // Drain before stopping: a backend may fire the stop callback once for a whole queue
        // rather than once per utterance, leaving the survivors to be mis-attributed later.
        let cancelled = self.utterances.drain();

        if let Err(e) = self.with_engine(|engine| {
            engine.stop()?;
            Ok(())
        }) {
            // Nothing stopped, so those utterances are still in flight and need their IDs.
            self.utterances.restore(cancelled);
            return Err(e);
        }

        for id in cancelled.iter().cloned() {
            self.emitter.emit(events::CANCEL, Some(id));
        }

        if cancelled.is_empty() && !self.has_utterance_callbacks {
            self.emitter.emit(events::CANCEL, None);
        }

        Ok(StopResponse { success: true })
    }

    pub fn get_voices(&self, payload: GetVoicesRequest) -> Result<GetVoicesResponse> {
        let voices = self.with_engine(|engine| self.installed_voices(engine))?;

        Ok(GetVoicesResponse {
            voices: voice::filter_by_language(&voices, payload.language.as_deref()),
        })
    }

    fn installed_voices(&self, engine: &Engine) -> Result<Vec<Voice>> {
        if let Some(cached) = self.voices.get() {
            return Ok(cached);
        }

        let voices: Vec<Voice> = engine
            .voices()?
            .into_iter()
            .map(|voice| Voice {
                id: voice.id().to_string(),
                name: voice.name().to_string(),
                language: voice.language().to_string(),
            })
            .collect();

        self.voices.store(&voices);
        Ok(voices)
    }

    pub fn is_speaking(&self) -> Result<IsSpeakingResponse> {
        self.with_engine(|engine| {
            Ok(IsSpeakingResponse {
                speaking: engine.is_speaking()?,
            })
        })
    }

    pub fn is_initialized(&self) -> Result<IsInitializedResponse> {
        if self.engine.is_none() {
            return Ok(IsInitializedResponse::default());
        }

        Ok(IsInitializedResponse {
            initialized: true,
            voice_count: self
                .get_voices(GetVoicesRequest { language: None })
                .map(|response| response.voices.len() as u32)
                .unwrap_or(0),
        })
    }

    pub fn preview_voice(&self, payload: PreviewVoiceRequest) -> Result<SpeakResponse> {
        payload.validate()?;

        // speak() reports an unknown voice through `warning`, which passes straight through:
        // a preview that silently used a different voice defeats the point of the call.
        self.speak(SpeakRequest {
            text: payload.sample_text().to_string(),
            language: None,
            voice_id: Some(payload.voice_id),
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        })
    }

    pub fn pause_speaking(&self) -> Result<PauseResumeResponse> {
        Ok(unsupported("Pause is not supported on desktop platform"))
    }

    pub fn resume_speaking(&self) -> Result<PauseResumeResponse> {
        Ok(unsupported("Resume is not supported on desktop platform"))
    }

    /// Desktop has no background state, so this succeeds without doing anything.
    pub fn set_background_behavior(
        &self,
        _payload: SetBackgroundBehaviorRequest,
    ) -> Result<SetBackgroundBehaviorResponse> {
        Ok(SetBackgroundBehaviorResponse { success: true })
    }
}

/// Selection happens over the cached voice list, but the engine only accepts its own voice
/// type, so the chosen ID is resolved once more against the live list.
fn set_engine_voice(engine: &mut Engine, id: &str) {
    let native = engine
        .voices()
        .ok()
        .and_then(|voices| voices.into_iter().find(|voice| voice.id() == id));

    if let Some(native) = native {
        let _ = engine.set_voice(&native);
    }
}

fn unsupported(reason: &str) -> PauseResumeResponse {
    PauseResumeResponse {
        success: false,
        reason: Some(reason.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reason has to survive into the message: it is all the caller gets once the plugin
    /// loads without an engine.
    #[test]
    fn the_unavailable_reason_keeps_the_underlying_error() {
        let reason = unavailable_reason(tts::Error::UnsupportedFeature);
        assert!(reason.contains("Unsupported feature"), "{reason}");
    }

    #[test]
    fn pause_and_resume_report_why_they_did_nothing() {
        for response in [
            unsupported("Pause is not supported on desktop platform"),
            unsupported("Resume is not supported on desktop platform"),
        ] {
            assert!(!response.success);
            assert!(response.reason.is_some());
        }
    }
}
