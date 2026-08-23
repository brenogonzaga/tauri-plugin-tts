use std::sync::Mutex;

use serde::de::DeserializeOwned;
use serde::Serialize;
use tauri::{
    ipc::{Channel, InvokeResponseBody},
    plugin::{PluginApi, PluginHandle},
    AppHandle, Emitter, Runtime,
};

use crate::models::*;
use crate::{Error, Result};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_tts);

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<Tts<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("com.tts", "TtsPlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_tts)?;

    Ok(Tts {
        handle,
        app: app.clone(),
        relay: Mutex::new(None),
    })
}

pub struct Tts<R: Runtime> {
    handle: PluginHandle<R>,
    app: AppHandle<R>,
    /// Keeps the channel alive so native code can send at any point after setup. Dropping it
    /// destroys the Rust-side callback and silences every event.
    relay: Mutex<Option<Channel<TtsEventPayload>>>,
}

impl<R: Runtime> Tts<R> {
    pub fn speak(&self, payload: SpeakRequest) -> Result<SpeakResponse> {
        let payload = SpeakRequest::from(payload.validate()?);
        self.ensure_relay_registered()?;
        self.invoke("speak", payload)
    }

    pub fn stop(&self) -> Result<StopResponse> {
        self.invoke("stop", ())
    }

    pub fn get_voices(&self, payload: GetVoicesRequest) -> Result<GetVoicesResponse> {
        self.invoke("getVoices", payload)
    }

    pub fn is_speaking(&self) -> Result<IsSpeakingResponse> {
        self.invoke("isSpeaking", ())
    }

    pub fn is_initialized(&self) -> Result<IsInitializedResponse> {
        self.invoke("isInitialized", ())
    }

    pub fn pause_speaking(&self) -> Result<PauseResumeResponse> {
        self.invoke("pauseSpeaking", ())
    }

    pub fn resume_speaking(&self) -> Result<PauseResumeResponse> {
        self.invoke("resumeSpeaking", ())
    }

    pub fn preview_voice(&self, payload: PreviewVoiceRequest) -> Result<SpeakResponse> {
        payload.validate()?;
        self.ensure_relay_registered()?;
        self.invoke("previewVoice", payload)
    }

    pub fn set_background_behavior(
        &self,
        payload: SetBackgroundBehaviorRequest,
    ) -> Result<SetBackgroundBehaviorResponse> {
        self.invoke("setBackgroundBehavior", payload)
    }

    fn invoke<A: Serialize, T: DeserializeOwned>(&self, command: &str, args: A) -> Result<T> {
        self.handle
            .run_mobile_plugin(command, args)
            .map_err(Into::into)
    }

    /// Registers the event relay once. Safe to call repeatedly, and called automatically by
    /// [`Self::speak`] so Rust-side listeners work without any setup from the caller.
    pub fn ensure_relay_registered(&self) -> Result<()> {
        if self.relay_registered()? {
            return Ok(());
        }

        let app = self.app.clone();
        let channel = Channel::<TtsEventPayload>::new(move |body| {
            if let Some(payload) = decode_event(body) {
                let _ = app.emit(&format!("tts://{}", payload.event_type), &payload);
            }
            Ok(())
        });

        #[derive(Serialize)]
        struct RelayArgs<'a> {
            channel: &'a Channel<TtsEventPayload>,
        }

        self.invoke::<_, serde_json::Value>("setupEventRelay", RelayArgs { channel: &channel })?;

        // Stored only after the native call succeeded, so the channel stays alive for the
        // lifetime of this instance.
        *self.relay.lock().map_err(|_| Error::MutexPoisoned)? = Some(channel);
        Ok(())
    }

    fn relay_registered(&self) -> Result<bool> {
        Ok(self
            .relay
            .lock()
            .map_err(|_| Error::MutexPoisoned)?
            .is_some())
    }
}

/// A malformed event is dropped with a warning: it must not take down the relay, which every
/// later event depends on.
fn decode_event(body: InvokeResponseBody) -> Option<TtsEventPayload> {
    let decoded = match &body {
        InvokeResponseBody::Json(json) => serde_json::from_str(json),
        InvokeResponseBody::Raw(bytes) => serde_json::from_slice(bytes),
    };

    match decoded {
        Ok(payload @ TtsEventPayload { .. }) if !payload.event_type.is_empty() => Some(payload),
        Ok(_) => {
            log::warn!("dropped a TTS relay event with no event_type");
            None
        }
        Err(e) => {
            log::warn!("failed to parse a TTS relay event: {e}");
            None
        }
    }
}
