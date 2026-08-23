use std::sync::Arc;
use tauri::{AppHandle, Emitter, Runtime};

use crate::models::TtsEventPayload;

pub const START: &str = "speech:start";
pub const FINISH: &str = "speech:finish";
pub const CANCEL: &str = "speech:cancel";
pub const ERROR: &str = "speech:error";

/// Emits lifecycle events as `tts://<name>`.
///
/// Held behind an [`Arc`] because the engine's utterance callbacks outlive the call that
/// registered them.
pub struct EventEmitter<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> EventEmitter<R> {
    pub fn new(app: AppHandle<R>) -> Arc<Self> {
        Arc::new(Self { app })
    }

    pub fn emit(&self, name: &str, id: Option<String>) {
        self.send(TtsEventPayload {
            event_type: name.to_string(),
            id,
            ..Default::default()
        });
    }

    pub fn emit_error(&self, id: Option<String>, error: String) {
        self.send(TtsEventPayload {
            event_type: ERROR.to_string(),
            id,
            error: Some(error),
            ..Default::default()
        });
    }

    fn send(&self, payload: TtsEventPayload) {
        let name = format!("tts://{}", payload.event_type);
        if let Err(e) = self.app.emit(&name, payload) {
            log::warn!("failed to emit {name}: {e}");
        }
    }
}
