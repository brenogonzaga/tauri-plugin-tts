use std::sync::Mutex;

use serde::de::DeserializeOwned;
use serde::Serialize;
use tauri::{
    ipc::Channel,
    plugin::{PluginApi, PluginHandle},
    AppHandle, Emitter, Runtime,
};

use crate::models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_tts);

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Tts<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("io.affex.tts", "TtsPlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_tts)?;
    Ok(Tts {
        handle,
        app: app.clone(),
        relay_channel: Mutex::new(None),
    })
}

pub struct Tts<R: Runtime> {
    handle: PluginHandle<R>,
    app: AppHandle<R>,
    /// Keeps the Channel alive so Kotlin can call send() at any point after setup.
    /// Dropping the Channel would destroy the Rust-side callback, silencing all events.
    relay_channel: Mutex<Option<Channel<TtsEventPayload>>>,
}

impl<R: Runtime> Tts<R> {
    /// Ensures the event relay is registered exactly once. Safe to call multiple times.
    /// Called automatically by `speak()` — users do not need to call this manually.
    pub fn ensure_relay_registered(&self) -> crate::Result<()> {
        if self.relay_registered()? {
            return Ok(());
        }
        self.setup_event_relay(&self.app.clone())
    }

    fn relay_registered(&self) -> crate::Result<bool> {
        Ok(self
            .relay_channel
            .lock()
            .map_err(|_| crate::Error::MutexPoisoned)?
            .is_some())
    }

    /// Set up a persistent event relay: native code calls `channel.send(TtsEventPayload)`,
    /// Rust receives it and re-emits via `app.emit("tts://<event_type>", payload)` so that
    /// JS `listen("tts://speech:finish", ...)` works on mobile exactly like desktop.
    pub fn setup_event_relay(&self, app: &AppHandle<R>) -> crate::Result<()> {
        if self.relay_registered()? {
            return Ok(());
        }
        use tauri::ipc::InvokeResponseBody;

        let app_handle = app.clone();
        let channel = Channel::<TtsEventPayload>::new(move |body| {
            let payload: TtsEventPayload = match body {
                InvokeResponseBody::Json(json) => match serde_json::from_str(&json) {
                    Ok(p) => p,
                    Err(e) => {
                        log::warn!("Failed to parse TTS relay event JSON: {e}");
                        return Ok(());
                    }
                },
                InvokeResponseBody::Raw(bytes) => match serde_json::from_slice(&bytes) {
                    Ok(p) => p,
                    Err(e) => {
                        log::warn!("Failed to parse TTS relay event bytes: {e}");
                        return Ok(());
                    }
                },
            };
            if !payload.event_type.is_empty() {
                let event_name = format!("tts://{}", payload.event_type);
                let _ = app_handle.emit(&event_name, &payload);
            }
            Ok(())
        });

        #[derive(Serialize)]
        struct RelayArgs<'a> {
            channel: &'a Channel<TtsEventPayload>,
        }

        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "setupEventRelay",
                RelayArgs { channel: &channel },
            )
            .map_err(crate::Error::from)?;

        // Store AFTER the mobile plugin call succeeds so the Channel is kept alive
        // for the lifetime of this Tts instance.
        *self
            .relay_channel
            .lock()
            .map_err(|_| crate::Error::MutexPoisoned)? = Some(channel);

        Ok(())
    }

    pub fn speak(&self, payload: SpeakRequest) -> crate::Result<SpeakResponse> {
        let payload = SpeakRequest::from(payload.validate()?);
        // Auto-register the event relay on first speak so that Rust-side listeners
        // (e.g. app.listen("tts://speech:finish", ...)) receive events without
        // any manual setup from the caller.
        self.ensure_relay_registered()?;
        self.handle
            .run_mobile_plugin("speak", payload)
            .map_err(Into::into)
    }

    pub fn stop(&self) -> crate::Result<StopResponse> {
        self.handle
            .run_mobile_plugin("stop", ())
            .map_err(Into::into)
    }

    pub fn get_voices(&self, payload: GetVoicesRequest) -> crate::Result<GetVoicesResponse> {
        self.handle
            .run_mobile_plugin("getVoices", payload)
            .map_err(Into::into)
    }

    pub fn is_speaking(&self) -> crate::Result<IsSpeakingResponse> {
        self.handle
            .run_mobile_plugin("isSpeaking", ())
            .map_err(Into::into)
    }

    pub fn is_initialized(&self) -> crate::Result<IsInitializedResponse> {
        self.handle
            .run_mobile_plugin("isInitialized", ())
            .map_err(Into::into)
    }

    pub fn pause_speaking(&self) -> crate::Result<PauseResumeResponse> {
        self.handle
            .run_mobile_plugin("pauseSpeaking", ())
            .map_err(Into::into)
    }

    pub fn resume_speaking(&self) -> crate::Result<PauseResumeResponse> {
        self.handle
            .run_mobile_plugin("resumeSpeaking", ())
            .map_err(Into::into)
    }

    pub fn preview_voice(&self, payload: PreviewVoiceRequest) -> crate::Result<SpeakResponse> {
        payload.validate()?;
        self.ensure_relay_registered()?;
        self.handle
            .run_mobile_plugin("previewVoice", payload)
            .map_err(Into::into)
    }

    pub fn set_background_behavior(
        &self,
        payload: SetBackgroundBehaviorRequest,
    ) -> crate::Result<SetBackgroundBehaviorResponse> {
        self.handle
            .run_mobile_plugin("setBackgroundBehavior", payload)
            .map_err(Into::into)
    }
}
