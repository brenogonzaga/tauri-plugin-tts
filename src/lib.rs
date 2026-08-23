mod commands;
mod error;
mod models;
mod validation;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

pub use error::{Error, Result};
pub use models::*;
pub use validation::{
    ValidatedSpeakRequest, ValidationError, MAX_LANGUAGE_LENGTH, MAX_TEXT_LENGTH,
    MAX_VOICE_ID_LENGTH, PITCH_RANGE, RATE_RANGE, VOLUME_RANGE,
};

#[cfg(desktop)]
use desktop::Tts;
#[cfg(mobile)]
use mobile::Tts;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

/// Gives [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] access to the TTS API.
pub trait TtsExt<R: Runtime> {
    fn tts(&self) -> &Tts<R>;
}

impl<R: Runtime, T: Manager<R>> TtsExt<R> for T {
    fn tts(&self) -> &Tts<R> {
        self.state::<Tts<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("tts")
        .invoke_handler(tauri::generate_handler![
            commands::speak,
            commands::stop,
            commands::get_voices,
            commands::is_speaking,
            commands::is_initialized,
            commands::pause_speaking,
            commands::resume_speaking,
            commands::preview_voice,
            commands::set_background_behavior,
            commands::register_listener,
        ])
        .setup(|app, api| {
            #[cfg(desktop)]
            let tts = desktop::init(app, api)?;
            #[cfg(mobile)]
            let tts = mobile::init(app, api)?;

            app.manage(tts);
            Ok(())
        })
        .build()
}
