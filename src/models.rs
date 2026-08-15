use serde::{Deserialize, Serialize};
use std::borrow::Cow;
#[cfg(feature = "ts-bindings")]
use ts_rs::TS;

/// Maximum text length in bytes (10KB).
pub const MAX_TEXT_LENGTH: usize = 10_000;
/// Maximum voice ID length
pub const MAX_VOICE_ID_LENGTH: usize = 256;
/// Maximum language code length
pub const MAX_LANGUAGE_LENGTH: usize = 35;

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/")
)]
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QueueMode {
    /// Flush any pending speech and start speaking immediately (default)
    #[default]
    Flush,
    /// Add to queue and speak after current speech finishes
    Add,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/")
)]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakOptions {
    /// The text to speak (max 10,000 characters)
    pub text: String,
    /// The language/locale code (e.g., "en-US", "pt-BR", "ja-JP")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Specific voice ID to use (from getVoices). Takes priority over language
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice_id: Option<String>,
    /// Speech rate (0.1 to 4.0, where 1.0 = normal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<f32>,
    /// Pitch (0.5 to 2.0, where 1.0 = normal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f32>,
    /// Volume (0.0 to 1.0, where 1.0 = full volume)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    /// Queue mode: "flush" (default) or "add"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_mode: Option<QueueMode>,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/")
)]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewVoiceOptions {
    /// Voice ID to preview
    pub voice_id: String,
    /// Optional custom sample text (uses default if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakRequest {
    /// The text to speak
    pub text: String,
    /// The language/locale code (e.g., "en-US", "pt-BR", "ja-JP")
    #[serde(default)]
    pub language: Option<String>,
    /// Voice ID to use (from getVoices)
    #[serde(default)]
    pub voice_id: Option<String>,
    /// Speech rate (0.1 to 4.0, where 1.0 = normal, 2.0 = double, 0.5 = half)
    #[serde(default = "default_rate")]
    pub rate: f32,
    /// Pitch (0.5 = low, 1.0 = normal, 2.0 = high)
    #[serde(default = "default_pitch")]
    pub pitch: f32,
    /// Volume (0.0 = silent, 1.0 = full volume)
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Queue mode: "flush" (default) or "add"
    #[serde(default)]
    pub queue_mode: QueueMode,
}

fn default_rate() -> f32 {
    1.0
}
fn default_pitch() -> f32 {
    1.0
}
fn default_volume() -> f32 {
    1.0
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("Text cannot be empty")]
    EmptyText,
    #[error("Text too long: {len} bytes (max: {max})")]
    TextTooLong { len: usize, max: usize },
    #[error("Voice ID too long: {len} chars (max: {max})")]
    VoiceIdTooLong { len: usize, max: usize },
    #[error("Invalid voice ID - control characters are not allowed")]
    InvalidVoiceId,
    #[error("Language code too long: {len} chars (max: {max})")]
    LanguageTooLong { len: usize, max: usize },
}

impl ValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            ValidationError::EmptyText => "EMPTY_TEXT",
            ValidationError::TextTooLong { .. } => "TEXT_TOO_LONG",
            ValidationError::VoiceIdTooLong { .. } => "VOICE_ID_TOO_LONG",
            ValidationError::InvalidVoiceId => "INVALID_VOICE_ID",
            ValidationError::LanguageTooLong { .. } => "LANGUAGE_TOO_LONG",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedSpeakRequest {
    pub text: String,
    pub language: Option<String>,
    pub voice_id: Option<String>,
    pub rate: f32,
    pub pitch: f32,
    pub volume: f32,
    pub queue_mode: QueueMode,
}

impl SpeakRequest {
    pub fn validate(&self) -> Result<ValidatedSpeakRequest, ValidationError> {
        // Text validation
        if self.text.is_empty() {
            return Err(ValidationError::EmptyText);
        }
        if self.text.len() > MAX_TEXT_LENGTH {
            return Err(ValidationError::TextTooLong {
                len: self.text.len(),
                max: MAX_TEXT_LENGTH,
            });
        }

        // Language validation (if provided)
        let sanitized_language = self
            .language
            .as_ref()
            .map(|lang| Self::validate_language(lang))
            .transpose()?;

        // Voice ID validation (if provided)
        if let Some(ref voice_id) = self.voice_id {
            validate_voice_id(voice_id)?;
        }

        Ok(ValidatedSpeakRequest {
            text: self.text.clone(),
            language: sanitized_language,
            voice_id: self.voice_id.clone(),
            rate: self.rate.clamp(0.1, 4.0),
            pitch: self.pitch.clamp(0.5, 2.0),
            volume: self.volume.clamp(0.0, 1.0),
            queue_mode: self.queue_mode,
        })
    }

    fn validate_language(lang: &str) -> Result<String, ValidationError> {
        if lang.len() > MAX_LANGUAGE_LENGTH {
            return Err(ValidationError::LanguageTooLong {
                len: lang.len(),
                max: MAX_LANGUAGE_LENGTH,
            });
        }
        Ok(lang.to_string())
    }
}

/// Shared voice ID validation. The id is matched by exact equality against the
/// engine's own voices in `speak()` / `preview_voice()`, so this only guards
/// against pathological input (too long, or containing control characters).
///
/// A stricter charset filter cannot be used here: native Windows SAPI voice ids
/// returned by `get_voices()` are registry tokens that contain spaces and
/// backslashes (e.g. `HKEY_LOCAL_MACHINE\...\Tokens\TTS_MS_EN-US_ZIRA_11.0`), so
/// restricting to `[alphanumeric . _ -]` rejected every selectable voice on
/// Windows.
fn validate_voice_id(voice_id: &str) -> Result<(), ValidationError> {
    if voice_id.len() > MAX_VOICE_ID_LENGTH {
        return Err(ValidationError::VoiceIdTooLong {
            len: voice_id.len(),
            max: MAX_VOICE_ID_LENGTH,
        });
    }
    if voice_id.chars().any(|c| c.is_control()) {
        return Err(ValidationError::InvalidVoiceId);
    }
    Ok(())
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakResponse {
    /// Whether speech was successfully initiated
    pub success: bool,
    /// Optional warning message (e.g., voice not found, using fallback)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBackgroundBehaviorRequest {
    /// Whether TTS should continue speaking when the app goes to background / screen locks.
    /// Defaults to true. When false, speech is paused on background and a `speech:backgroundPause`
    /// event is emitted (matching the previous behavior). Desktop: ignored (no-op).
    pub continue_in_background: bool,
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
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    /// Unique identifier for the voice
    pub id: String,
    /// Display name of the voice
    pub name: String,
    /// Language code (e.g., "en-US")
    pub language: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetVoicesRequest {
    /// Optional language filter
    #[serde(default)]
    pub language: Option<String>,
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
    /// Whether the TTS engine is initialized and ready
    pub initialized: bool,
    /// Number of available voices (0 if not initialized)
    pub voice_count: u32,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(
    feature = "ts-bindings",
    ts(export, export_to = "../guest-js/bindings/")
)]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseResumeResponse {
    pub success: bool,
    /// Reason for failure (if success is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewVoiceRequest {
    /// Voice ID to preview
    pub voice_id: String,
    /// Optional custom sample text (uses default if not provided)
    #[serde(default)]
    pub text: Option<String>,
}

impl PreviewVoiceRequest {
    pub const DEFAULT_SAMPLE_TEXT: &'static str =
        "Hello! This is a sample of how this voice sounds.";

    pub fn sample_text(&self) -> Cow<'_, str> {
        match &self.text {
            Some(text) => Cow::Borrowed(text.as_str()),
            None => Cow::Borrowed(Self::DEFAULT_SAMPLE_TEXT),
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        // Validate voice ID
        validate_voice_id(&self.voice_id)?;

        // Validate custom text if provided
        if let Some(ref text) = self.text {
            if text.is_empty() {
                return Err(ValidationError::EmptyText);
            }
            if text.len() > MAX_TEXT_LENGTH {
                return Err(ValidationError::TextTooLong {
                    len: text.len(),
                    max: MAX_TEXT_LENGTH,
                });
            }
        }

        Ok(())
    }
}

/// On desktop, emitted directly via `app.emit("tts://<event_type>", payload)`.
/// On mobile, native plugins send this through a Tauri `Channel`; the Rust relay
/// deserializes it and re-emits via `app.emit()` so JS `listen("tts://...")` works
/// uniformly on every platform.
///
/// The shape matches the JS `SpeechEvent` interface.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsEventPayload {
    /// The event name, e.g. "speech:finish". Used to build the emit key "tts://<event_type>".
    pub event_type: String,
    /// Unique identifier for the utterance (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Error message (for error events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Whether speech was interrupted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupted: Option<bool>,
    /// Reason for the event (e.g. "audio_focus_lost", "app_paused")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speak_request_defaults() {
        let json = r#"{"text": "Hello world"}"#;
        let request: SpeakRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.text, "Hello world");
        assert!(request.language.is_none());
        assert!(request.voice_id.is_none());
        assert_eq!(request.rate, 1.0);
        assert_eq!(request.pitch, 1.0);
        assert_eq!(request.volume, 1.0);
    }

    #[test]
    fn test_speak_request_full() {
        let json = r#"{
            "text": "Olá",
            "language": "pt-BR",
            "voiceId": "com.apple.voice.enhanced.pt-BR",
            "rate": 0.8,
            "pitch": 1.2,
            "volume": 0.9
        }"#;

        let request: SpeakRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.text, "Olá");
        assert_eq!(request.language, Some("pt-BR".to_string()));
        assert_eq!(
            request.voice_id,
            Some("com.apple.voice.enhanced.pt-BR".to_string())
        );
        assert_eq!(request.rate, 0.8);
        assert_eq!(request.pitch, 1.2);
        assert_eq!(request.volume, 0.9);
    }

    #[test]
    fn test_voice_serialization() {
        let voice = Voice {
            id: "test-voice".to_string(),
            name: "Test Voice".to_string(),
            language: "en-US".to_string(),
        };

        let json = serde_json::to_string(&voice).unwrap();
        assert!(json.contains("\"id\":\"test-voice\""));
        assert!(json.contains("\"name\":\"Test Voice\""));
        assert!(json.contains("\"language\":\"en-US\""));
    }

    #[test]
    fn test_get_voices_request_optional_language() {
        let json1 = r#"{}"#;
        let request1: GetVoicesRequest = serde_json::from_str(json1).unwrap();
        assert!(request1.language.is_none());

        let json2 = r#"{"language": "en"}"#;
        let request2: GetVoicesRequest = serde_json::from_str(json2).unwrap();
        assert_eq!(request2.language, Some("en".to_string()));
    }

    #[test]
    fn test_validation_empty_text() {
        let request = SpeakRequest {
            text: "".to_string(),
            language: None,
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        };

        let result = request.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::EmptyText));
    }

    #[test]
    fn test_validation_text_too_long() {
        let long_text = "x".repeat(MAX_TEXT_LENGTH + 1);
        let request = SpeakRequest {
            text: long_text,
            language: None,
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        };

        let result = request.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::TextTooLong { .. }
        ));
    }

    #[test]
    fn test_validation_valid_voice_id() {
        let request = SpeakRequest {
            text: "Hello".to_string(),
            language: None,
            voice_id: Some("com.apple.voice.enhanced.en-US".to_string()),
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        };

        let result = request.validate();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().voice_id,
            Some("com.apple.voice.enhanced.en-US".to_string())
        );
    }

    #[test]
    fn test_validation_voice_id_too_long() {
        let long_voice_id = "x".repeat(MAX_VOICE_ID_LENGTH + 1);
        let request = SpeakRequest {
            text: "Hello".to_string(),
            language: None,
            voice_id: Some(long_voice_id),
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        };

        let result = request.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::VoiceIdTooLong { .. }
        ));
    }

    #[test]
    fn test_validation_rate_clamping() {
        let request = SpeakRequest {
            text: "Hello".to_string(),
            language: None,
            voice_id: None,
            rate: 999.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        };

        let result = request.validate();
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.rate, 4.0); // Clamped to max
    }

    #[test]
    fn test_validation_pitch_clamping() {
        let request = SpeakRequest {
            text: "Hello".to_string(),
            language: None,
            voice_id: None,
            rate: 1.0,
            pitch: 0.1,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        };

        let result = request.validate();
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.pitch, 0.5); // Clamped to min
    }

    #[test]
    fn test_validation_volume_clamping() {
        let request = SpeakRequest {
            text: "Hello".to_string(),
            language: None,
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 5.0,
            queue_mode: QueueMode::Flush,
        };

        let result = request.validate();
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.volume, 1.0); // Clamped to max
    }

    #[test]
    fn test_preview_voice_validation() {
        // Valid preview
        let valid = PreviewVoiceRequest {
            voice_id: "valid-voice_123".to_string(),
            text: None,
        };
        assert!(valid.validate().is_ok());

        // Control characters are the only rejected shape (see validate_voice_id).
        let invalid = PreviewVoiceRequest {
            voice_id: "voice\u{7}id".to_string(),
            text: None,
        };
        assert!(matches!(
            invalid.validate().unwrap_err(),
            ValidationError::InvalidVoiceId
        ));
    }

    /// Regression guard for the Windows SAPI fix: registry-token voice ids
    /// contain spaces and backslashes and must stay acceptable.
    #[test]
    fn test_native_voice_ids_with_spaces_and_backslashes_are_accepted() {
        for id in [
            r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Speech\Voices\Tokens\TTS_MS_EN-US_ZIRA_11.0",
            "Microsoft Zira Desktop",
            "com.apple.voice.enhanced.pt-BR",
        ] {
            let request = PreviewVoiceRequest {
                voice_id: id.to_string(),
                text: None,
            };
            assert!(request.validate().is_ok(), "rejected {id}");
        }
    }

    #[test]
    fn test_validation_error_codes_are_specific() {
        let cases = [
            (ValidationError::EmptyText, "EMPTY_TEXT"),
            (
                ValidationError::TextTooLong { len: 1, max: 0 },
                "TEXT_TOO_LONG",
            ),
            (
                ValidationError::VoiceIdTooLong { len: 1, max: 0 },
                "VOICE_ID_TOO_LONG",
            ),
            (ValidationError::InvalidVoiceId, "INVALID_VOICE_ID"),
            (
                ValidationError::LanguageTooLong { len: 1, max: 0 },
                "LANGUAGE_TOO_LONG",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.code(), expected);
        }
    }

    #[test]
    fn test_language_too_long_is_rejected() {
        let request = SpeakRequest {
            text: "Hello".to_string(),
            language: Some("x".repeat(MAX_LANGUAGE_LENGTH + 1)),
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        };
        assert!(matches!(
            request.validate().unwrap_err(),
            ValidationError::LanguageTooLong { .. }
        ));
    }

    #[test]
    fn test_preview_voice_sample_text() {
        let without_text = PreviewVoiceRequest {
            voice_id: "voice".to_string(),
            text: None,
        };
        assert_eq!(
            without_text.sample_text(),
            PreviewVoiceRequest::DEFAULT_SAMPLE_TEXT
        );

        let with_text = PreviewVoiceRequest {
            voice_id: "voice".to_string(),
            text: Some("Custom sample".to_string()),
        };
        assert_eq!(with_text.sample_text(), "Custom sample");
    }
}
