use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Tts;
#[cfg(mobile)]
use mobile::Tts;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the tts APIs.
pub trait TtsExt<R: Runtime> {
    fn tts(&self) -> &Tts<R>;
}

impl<R: Runtime, T: Manager<R>> crate::TtsExt<R> for T {
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
            commands::set_background_behavior
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let tts = mobile::init(app, api)?;
            #[cfg(desktop)]
            let tts = desktop::init(app, api)?;
            app.manage(tts);
            Ok(())
        })
        .build()
}
