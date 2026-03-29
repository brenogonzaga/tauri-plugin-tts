use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::Result;
use crate::TtsExt;

/// Speak the given text using text-to-speech
#[command]
pub(crate) async fn speak<R: Runtime>(
    app: AppHandle<R>,
    payload: SpeakRequest,
) -> Result<SpeakResponse> {
    app.tts().speak(payload)
}

/// Stop any ongoing speech
#[command]
pub(crate) async fn stop<R: Runtime>(app: AppHandle<R>) -> Result<StopResponse> {
    app.tts().stop()
}

/// Get available voices, optionally filtered by language
#[command]
pub(crate) async fn get_voices<R: Runtime>(
    app: AppHandle<R>,
    payload: GetVoicesRequest,
) -> Result<GetVoicesResponse> {
    app.tts().get_voices(payload)
}

/// Check if TTS is currently speaking
#[command]
pub(crate) async fn is_speaking<R: Runtime>(app: AppHandle<R>) -> Result<IsSpeakingResponse> {
    app.tts().is_speaking()
}

/// Check if TTS engine is initialized and ready
#[command]
pub(crate) async fn is_initialized<R: Runtime>(app: AppHandle<R>) -> Result<IsInitializedResponse> {
    app.tts().is_initialized()
}

/// Pause the current speech (mobile only, desktop will return error)
#[command]
pub(crate) async fn pause_speaking<R: Runtime>(app: AppHandle<R>) -> Result<PauseResumeResponse> {
    app.tts().pause_speaking()
}

/// Resume paused speech (mobile only, desktop will return error)
#[command]
pub(crate) async fn resume_speaking<R: Runtime>(app: AppHandle<R>) -> Result<PauseResumeResponse> {
    app.tts().resume_speaking()
}

/// Preview a voice by speaking a sample text
#[command]
pub(crate) async fn preview_voice<R: Runtime>(
    app: AppHandle<R>,
    payload: PreviewVoiceRequest,
) -> Result<SpeakResponse> {
    app.tts().preview_voice(payload)
}

/// Set whether TTS should continue in background when screen locks (mobile only)
#[command]
pub(crate) async fn set_background_behavior<R: Runtime>(
    app: AppHandle<R>,
    payload: SetBackgroundBehaviorRequest,
) -> Result<SetBackgroundBehaviorResponse> {
    app.tts().set_background_behavior(payload)
}
