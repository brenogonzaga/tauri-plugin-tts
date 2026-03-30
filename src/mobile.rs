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
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Tts<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("io.affex.tts", "TtsPlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_tts)?;
    Ok(Tts(handle))
}

pub struct Tts<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Tts<R> {
    /// Set up a persistent event relay: native code calls `channel.send(TtsEventPayload)`,
    /// Rust receives it and re-emits via `app.emit("tts://<event_type>", payload)` so that
    /// JS `listen("tts://speech:finish", ...)` works on mobile exactly like desktop.
    pub fn setup_event_relay(&self, app: &AppHandle<R>) -> crate::Result<()> {
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
        struct RelayArgs {
            channel: Channel<TtsEventPayload>,
        }

        self.0
            .run_mobile_plugin::<serde_json::Value>("setupEventRelay", RelayArgs { channel })
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn speak(&self, payload: SpeakRequest) -> crate::Result<SpeakResponse> {
        self.0
            .run_mobile_plugin("speak", payload)
            .map_err(Into::into)
    }

    pub fn stop(&self) -> crate::Result<StopResponse> {
        self.0.run_mobile_plugin("stop", ()).map_err(Into::into)
    }

    pub fn get_voices(&self, payload: GetVoicesRequest) -> crate::Result<GetVoicesResponse> {
        self.0
            .run_mobile_plugin("getVoices", payload)
            .map_err(Into::into)
    }

    pub fn is_speaking(&self) -> crate::Result<IsSpeakingResponse> {
        self.0
            .run_mobile_plugin("isSpeaking", ())
            .map_err(Into::into)
    }

    pub fn is_initialized(&self) -> crate::Result<IsInitializedResponse> {
        self.0
            .run_mobile_plugin("isInitialized", ())
            .map_err(Into::into)
    }

    pub fn pause_speaking(&self) -> crate::Result<PauseResumeResponse> {
        self.0
            .run_mobile_plugin("pauseSpeaking", ())
            .map_err(Into::into)
    }

    pub fn resume_speaking(&self) -> crate::Result<PauseResumeResponse> {
        self.0
            .run_mobile_plugin("resumeSpeaking", ())
            .map_err(Into::into)
    }

    pub fn preview_voice(&self, payload: PreviewVoiceRequest) -> crate::Result<SpeakResponse> {
        self.0
            .run_mobile_plugin("previewVoice", payload)
            .map_err(Into::into)
    }

    pub fn set_background_behavior(
        &self,
        payload: SetBackgroundBehaviorRequest,
    ) -> crate::Result<SetBackgroundBehaviorResponse> {
        self.0
            .run_mobile_plugin("setBackgroundBehavior", payload)
            .map_err(Into::into)
    }
}
