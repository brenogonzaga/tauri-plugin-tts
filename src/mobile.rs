use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
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
