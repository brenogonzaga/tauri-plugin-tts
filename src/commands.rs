//! IPC surface. Each command validates nothing itself; it forwards to the platform backend.

use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::{Result, TtsExt};

/// Runs `f` where this platform's synthesizer is safe to drive.
///
/// Tauri runs `async` commands on the async runtime's worker threads. On macOS that is the
/// wrong thread: the backend talks to `AVSpeechSynthesizer` through raw `msg_send!` with no
/// dispatch of its own. Every other backend is thread-safe, so they skip the hop.
#[cfg(target_os = "macos")]
fn on_synthesizer_thread<R, T, F>(app: &AppHandle<R>, f: F) -> Result<T>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(&AppHandle<R>) -> Result<T> + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel();
    let app_on_main = app.clone();

    app.run_on_main_thread(move || {
        let _ = sender.send(f(&app_on_main));
    })
    .map_err(|e| crate::Error::OperationFailed(format!("could not reach the main thread: {e}")))?;

    receiver.recv().map_err(|_| {
        crate::Error::OperationFailed("the TTS main-thread task did not run".to_string())
    })?
}

#[cfg(not(target_os = "macos"))]
fn on_synthesizer_thread<R, T, F>(app: &AppHandle<R>, f: F) -> Result<T>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(&AppHandle<R>) -> Result<T> + Send + 'static,
{
    f(app)
}

#[command]
pub(crate) async fn speak<R: Runtime>(
    app: AppHandle<R>,
    payload: SpeakRequest,
) -> Result<SpeakResponse> {
    on_synthesizer_thread(&app, move |app| app.tts().speak(payload))
}

#[command]
pub(crate) async fn stop<R: Runtime>(app: AppHandle<R>) -> Result<StopResponse> {
    on_synthesizer_thread(&app, |app| app.tts().stop())
}

#[command]
pub(crate) async fn get_voices<R: Runtime>(
    app: AppHandle<R>,
    payload: GetVoicesRequest,
) -> Result<GetVoicesResponse> {
    on_synthesizer_thread(&app, move |app| app.tts().get_voices(payload))
}

#[command]
pub(crate) async fn is_speaking<R: Runtime>(app: AppHandle<R>) -> Result<IsSpeakingResponse> {
    on_synthesizer_thread(&app, |app| app.tts().is_speaking())
}

#[command]
pub(crate) async fn is_initialized<R: Runtime>(app: AppHandle<R>) -> Result<IsInitializedResponse> {
    on_synthesizer_thread(&app, |app| app.tts().is_initialized())
}

#[command]
pub(crate) async fn preview_voice<R: Runtime>(
    app: AppHandle<R>,
    payload: PreviewVoiceRequest,
) -> Result<SpeakResponse> {
    on_synthesizer_thread(&app, move |app| app.tts().preview_voice(payload))
}

/// iOS only; desktop and Android answer with `success: false` and a reason.
#[command]
pub(crate) async fn pause_speaking<R: Runtime>(app: AppHandle<R>) -> Result<PauseResumeResponse> {
    on_synthesizer_thread(&app, |app| app.tts().pause_speaking())
}

#[command]
pub(crate) async fn resume_speaking<R: Runtime>(app: AppHandle<R>) -> Result<PauseResumeResponse> {
    on_synthesizer_thread(&app, |app| app.tts().resume_speaking())
}

/// Mobile only; a no-op on desktop, which has no background state.
#[command]
pub(crate) async fn set_background_behavior<R: Runtime>(
    app: AppHandle<R>,
    payload: SetBackgroundBehaviorRequest,
) -> Result<SetBackgroundBehaviorResponse> {
    on_synthesizer_thread(&app, move |app| app.tts().set_background_behavior(payload))
}

/// Registers the native event relay. Mobile calls this once before any event can arrive;
/// on desktop the plugin emits directly and there is nothing to set up.
#[command]
pub(crate) async fn register_listener<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    #[cfg(mobile)]
    app.tts().ensure_relay_registered()?;
    #[cfg(not(mobile))]
    let _ = app;

    Ok(())
}
