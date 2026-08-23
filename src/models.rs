use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-bindings")]
use ts_rs::TS;

use crate::validation::{de_finite_or_normal, de_queue_mode, de_string_or_empty, normal};

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/")
)]
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QueueMode {
    /// Interrupt whatever is speaking and start immediately.
    #[default]
    Flush,
    /// Speak after the current utterance finishes.
    Add,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/", optional_fields = nullable)
)]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakOptions {
    /// Text to speak, at most [`crate::validation::MAX_TEXT_LENGTH`] UTF-8 bytes.
    pub text: String,
    /// Locale code such as `en-US`, `pt-BR` or `ja-JP`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Voice ID from `getVoices`. Takes priority over `language`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice_id: Option<String>,
    /// Speech rate, 0.1 to 4.0, where 1.0 is the platform's normal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<f32>,
    /// Pitch, 0.5 to 2.0, where 1.0 is the platform's normal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f32>,
    /// Volume, 0.0 to 1.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_mode: Option<QueueMode>,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/", optional_fields = nullable)
)]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewVoiceOptions {
    pub voice_id: String,
    /// Overrides the built-in sample sentence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Incoming `speak` payload. Every numeric field tolerates `null`, which is what
/// `JSON.stringify` produces for `NaN` and `Infinity`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakRequest {
    #[serde(default, deserialize_with = "de_string_or_empty")]
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub voice_id: Option<String>,
    #[serde(default = "normal", deserialize_with = "de_finite_or_normal")]
    pub rate: f32,
    #[serde(default = "normal", deserialize_with = "de_finite_or_normal")]
    pub pitch: f32,
    #[serde(default = "normal", deserialize_with = "de_finite_or_normal")]
    pub volume: f32,
    #[serde(default, deserialize_with = "de_queue_mode")]
    pub queue_mode: QueueMode,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewVoiceRequest {
    pub voice_id: String,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetVoicesRequest {
    /// Locale prefix filter: `pt` matches both `pt-BR` and `pt-PT`.
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBackgroundBehaviorRequest {
    /// When false, mobile pauses speech on background and emits `speech:backgroundPause`.
    /// Ignored on desktop.
    pub continue_in_background: bool,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/", optional_fields = nullable)
)]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakResponse {
    pub success: bool,
    /// Set when the requested voice or language was unavailable and the default was used.
    /// Speech still happens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// Matches the `id` on this utterance's lifecycle events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utterance_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBackgroundBehaviorResponse {
    pub success: bool,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/")
)]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub id: String,
    pub name: String,
    /// Locale code such as `en-US`.
    pub language: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetVoicesResponse {
    pub voices: Vec<Voice>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IsSpeakingResponse {
    pub speaking: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IsInitializedResponse {
    pub initialized: bool,
    pub voice_count: u32,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/", optional_fields = nullable)
)]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseResumeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A lifecycle event on its way to `app.emit("tts://<event_type>", ..)`.
///
/// Desktop emits these directly; mobile sends them over a Tauri `Channel` and the relay in
/// [`crate::mobile`] re-emits them, so JS sees one shape everywhere.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsEventPayload {
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupted: Option<bool>,
    /// Why an automatic event fired, e.g. `audio_focus_lost` or `app_paused`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speak_request_fills_in_the_defaults() {
        let request: SpeakRequest = serde_json::from_str(r#"{"text": "Hello world"}"#).unwrap();

        assert_eq!(request.text, "Hello world");
        assert_eq!(request.language, None);
        assert_eq!(request.voice_id, None);
        assert_eq!(
            (request.rate, request.pitch, request.volume),
            (1.0, 1.0, 1.0)
        );
        assert_eq!(request.queue_mode, QueueMode::Flush);
    }

    #[test]
    fn speak_request_reads_every_field() {
        let request: SpeakRequest = serde_json::from_str(
            r#"{
                "text": "Olá",
                "language": "pt-BR",
                "voiceId": "com.apple.voice.enhanced.pt-BR",
                "rate": 0.8,
                "pitch": 1.2,
                "volume": 0.9,
                "queueMode": "add"
            }"#,
        )
        .unwrap();

        assert_eq!(request.text, "Olá");
        assert_eq!(request.language.as_deref(), Some("pt-BR"));
        assert_eq!(
            request.voice_id.as_deref(),
            Some("com.apple.voice.enhanced.pt-BR")
        );
        assert_eq!(
            (request.rate, request.pitch, request.volume),
            (0.8, 1.2, 0.9)
        );
        assert_eq!(request.queue_mode, QueueMode::Add);
    }

    #[test]
    fn get_voices_language_is_optional() {
        let all: GetVoicesRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(all.language, None);

        let filtered: GetVoicesRequest = serde_json::from_str(r#"{"language": "en"}"#).unwrap();
        assert_eq!(filtered.language.as_deref(), Some("en"));
    }

    #[test]
    fn voice_serializes_with_camel_case_keys() {
        let json = serde_json::to_value(Voice {
            id: "test-voice".into(),
            name: "Test Voice".into(),
            language: "en-US".into(),
        })
        .unwrap();

        assert_eq!(json["id"], "test-voice");
        assert_eq!(json["name"], "Test Voice");
        assert_eq!(json["language"], "en-US");
    }

    /// The mobile plugins answer with `utteranceId`; unset optionals stay absent rather than
    /// serializing as `null`.
    #[test]
    fn speak_response_round_trips_the_mobile_payload() {
        let response: SpeakResponse =
            serde_json::from_str(r#"{"success": true, "utteranceId": "tts_42"}"#).unwrap();
        assert_eq!(response.utterance_id.as_deref(), Some("tts_42"));
        assert_eq!(response.warning, None);

        let json = serde_json::to_value(SpeakResponse::default()).unwrap();
        assert!(json.get("utteranceId").is_none());
        assert!(json.get("warning").is_none());
    }
}
