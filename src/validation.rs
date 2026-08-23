use serde::{Deserialize, Deserializer};

use crate::models::{PreviewVoiceRequest, QueueMode, SpeakRequest};

/// Maximum `text` length, in UTF-8 bytes.
pub const MAX_TEXT_LENGTH: usize = 10_000;
/// Maximum `voiceId` length, in characters.
pub const MAX_VOICE_ID_LENGTH: usize = 256;
/// Maximum `language` length, in characters.
pub const MAX_LANGUAGE_LENGTH: usize = 35;

/// Accepted `rate`, as the user sees it. 1.0 is the platform's normal speed.
pub const RATE_RANGE: (f32, f32) = (0.1, 4.0);
/// Accepted `pitch`. 1.0 is the platform's normal pitch.
pub const PITCH_RANGE: (f32, f32) = (0.5, 2.0);
/// Accepted `volume`.
pub const VOLUME_RANGE: (f32, f32) = (0.0, 1.0);

/// The sample spoken by `previewVoice` when the caller supplies no text.
pub const DEFAULT_SAMPLE_TEXT: &str = "Hello! This is a sample of how this voice sounds.";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
            Self::EmptyText => "EMPTY_TEXT",
            Self::TextTooLong { .. } => "TEXT_TOO_LONG",
            Self::VoiceIdTooLong { .. } => "VOICE_ID_TOO_LONG",
            Self::InvalidVoiceId => "INVALID_VOICE_ID",
            Self::LanguageTooLong { .. } => "LANGUAGE_TOO_LONG",
        }
    }
}

type Result<T> = std::result::Result<T, ValidationError>;

/// A [`SpeakRequest`] whose text, voice and language passed validation and whose numeric
/// parameters are already clamped to the documented ranges.
#[derive(Debug, Clone, PartialEq)]
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
    pub fn validate(&self) -> Result<ValidatedSpeakRequest> {
        validate_text(&self.text)?;
        if let Some(language) = &self.language {
            validate_language(language)?;
        }
        if let Some(voice_id) = &self.voice_id {
            validate_voice_id(voice_id)?;
        }

        Ok(ValidatedSpeakRequest {
            text: self.text.clone(),
            language: self.language.clone(),
            voice_id: self.voice_id.clone(),
            rate: clamp(self.rate, RATE_RANGE),
            pitch: clamp(self.pitch, PITCH_RANGE),
            volume: clamp(self.volume, VOLUME_RANGE),
            queue_mode: self.queue_mode,
        })
    }
}

impl PreviewVoiceRequest {
    pub fn validate(&self) -> Result<()> {
        validate_voice_id(&self.voice_id)?;
        if let Some(text) = &self.text {
            validate_text(text)?;
        }
        Ok(())
    }

    pub fn sample_text(&self) -> &str {
        self.text.as_deref().unwrap_or(DEFAULT_SAMPLE_TEXT)
    }
}

/// Rebuilds the wire payload from a validated request so the mobile plugins receive values
/// that went through this clamping rather than clamping differently per platform.
impl From<ValidatedSpeakRequest> for SpeakRequest {
    fn from(request: ValidatedSpeakRequest) -> Self {
        Self {
            text: request.text,
            language: request.language,
            voice_id: request.voice_id,
            rate: request.rate,
            pitch: request.pitch,
            volume: request.volume,
            queue_mode: request.queue_mode,
        }
    }
}

fn clamp(value: f32, (min, max): (f32, f32)) -> f32 {
    value.clamp(min, max)
}

fn validate_text(text: &str) -> Result<()> {
    if text.is_empty() {
        return Err(ValidationError::EmptyText);
    }
    if text.len() > MAX_TEXT_LENGTH {
        return Err(ValidationError::TextTooLong {
            len: text.len(),
            max: MAX_TEXT_LENGTH,
        });
    }
    Ok(())
}

fn validate_language(language: &str) -> Result<()> {
    let len = language.chars().count();
    if len > MAX_LANGUAGE_LENGTH {
        return Err(ValidationError::LanguageTooLong {
            len,
            max: MAX_LANGUAGE_LENGTH,
        });
    }
    Ok(())
}

/// Voice IDs are matched by exact equality against the engine's own voices, so this only has
/// to reject pathological input. A charset filter is not usable here: Windows voice IDs are
/// registry tokens containing spaces and backslashes, and rejecting those rejected every
/// selectable voice on the platform.
fn validate_voice_id(voice_id: &str) -> Result<()> {
    let len = voice_id.chars().count();
    if len > MAX_VOICE_ID_LENGTH {
        return Err(ValidationError::VoiceIdTooLong {
            len,
            max: MAX_VOICE_ID_LENGTH,
        });
    }
    if voice_id.chars().any(char::is_control) {
        return Err(ValidationError::InvalidVoiceId);
    }
    Ok(())
}

pub(crate) fn normal() -> f32 {
    1.0
}

/// `JSON.stringify` turns `NaN` and `Infinity` into `null`, and the guest API sends `null`
/// for every option the caller left out. A serde type error here would bypass
/// [`crate::Error`] and reach JavaScript untyped, so fall back to the normal value instead.
pub(crate) fn de_finite_or_normal<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<f32, D::Error> {
    Ok(Option::<f32>::deserialize(deserializer)?
        .filter(|value| value.is_finite())
        .unwrap_or_else(normal))
}

/// A null `text` has to reach [`SpeakRequest::validate`] as empty so the caller gets
/// `EMPTY_TEXT` rather than an untyped parse failure.
pub(crate) fn de_string_or_empty<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

pub(crate) fn de_queue_mode<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<QueueMode, D::Error> {
    Ok(Option::<QueueMode>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speak(text: &str) -> SpeakRequest {
        SpeakRequest {
            text: text.into(),
            language: None,
            voice_id: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            queue_mode: QueueMode::Flush,
        }
    }

    #[test]
    fn empty_text_is_rejected() {
        assert_eq!(speak("").validate(), Err(ValidationError::EmptyText));
    }

    #[test]
    fn text_is_measured_in_utf8_bytes() {
        let at_limit = "a".repeat(MAX_TEXT_LENGTH);
        assert!(speak(&at_limit).validate().is_ok());

        assert_eq!(
            speak(&"a".repeat(MAX_TEXT_LENGTH + 1)).validate(),
            Err(ValidationError::TextTooLong {
                len: MAX_TEXT_LENGTH + 1,
                max: MAX_TEXT_LENGTH,
            })
        );

        // "é" is two bytes but one character: the limit counts bytes.
        let two_byte_chars = "é".repeat(MAX_TEXT_LENGTH / 2 + 1);
        assert!(matches!(
            speak(&two_byte_chars).validate(),
            Err(ValidationError::TextTooLong { .. })
        ));
    }

    #[test]
    fn numeric_parameters_are_clamped_not_rejected() {
        let mut request = speak("Hello");
        request.rate = 99.0;
        request.pitch = -5.0;
        request.volume = 42.0;

        let validated = request.validate().unwrap();
        assert_eq!(validated.rate, RATE_RANGE.1);
        assert_eq!(validated.pitch, PITCH_RANGE.0);
        assert_eq!(validated.volume, VOLUME_RANGE.1);
    }

    /// Windows voice IDs are registry tokens; a charset filter rejected every one of them.
    #[test]
    fn voice_ids_allow_platform_punctuation() {
        for id in [
            r"HKEY_LOCAL_MACHINE\SOFTWARE\Speech\Voices\Tokens\TTS_MS_EN-US_ZIRA_11.0",
            "com.apple.voice.enhanced.pt-BR",
            "pt-br-x-afs#female_1-local",
        ] {
            let mut request = speak("Hello");
            request.voice_id = Some(id.into());
            assert!(request.validate().is_ok(), "{id}");
        }
    }

    #[test]
    fn voice_ids_reject_control_characters_and_overlong_input() {
        let mut request = speak("Hello");
        request.voice_id = Some("voice\u{0}id".into());
        assert_eq!(request.validate(), Err(ValidationError::InvalidVoiceId));

        request.voice_id = Some("v".repeat(MAX_VOICE_ID_LENGTH + 1));
        assert_eq!(
            request.validate(),
            Err(ValidationError::VoiceIdTooLong {
                len: MAX_VOICE_ID_LENGTH + 1,
                max: MAX_VOICE_ID_LENGTH,
            })
        );
    }

    #[test]
    fn language_length_is_measured_in_characters() {
        let mut request = speak("Hello");
        request.language = Some("é".repeat(MAX_LANGUAGE_LENGTH));
        assert!(request.validate().is_ok());

        request.language = Some("é".repeat(MAX_LANGUAGE_LENGTH + 1));
        assert_eq!(
            request.validate(),
            Err(ValidationError::LanguageTooLong {
                len: MAX_LANGUAGE_LENGTH + 1,
                max: MAX_LANGUAGE_LENGTH,
            })
        );
    }

    #[test]
    fn preview_falls_back_to_the_built_in_sample() {
        let request = PreviewVoiceRequest {
            voice_id: "voice".into(),
            text: None,
        };
        assert_eq!(request.sample_text(), DEFAULT_SAMPLE_TEXT);
        assert!(request.validate().is_ok());

        let custom = PreviewVoiceRequest {
            voice_id: "voice".into(),
            text: Some("Testing".into()),
        };
        assert_eq!(custom.sample_text(), "Testing");
    }

    #[test]
    fn preview_rejects_an_empty_custom_sample() {
        let request = PreviewVoiceRequest {
            voice_id: "voice".into(),
            text: Some(String::new()),
        };
        assert_eq!(request.validate(), Err(ValidationError::EmptyText));
    }

    /// The clamped values, not the raw ones, are what reaches the mobile plugins.
    #[test]
    fn the_mobile_payload_is_rebuilt_from_validated_values() {
        let mut request = speak("Hello");
        request.rate = 99.0;

        let payload = SpeakRequest::from(request.validate().unwrap());
        assert_eq!(payload.rate, RATE_RANGE.1);
    }

    #[test]
    fn every_variant_has_a_stable_code() {
        assert_eq!(ValidationError::EmptyText.code(), "EMPTY_TEXT");
        assert_eq!(
            ValidationError::TextTooLong { len: 1, max: 0 }.code(),
            "TEXT_TOO_LONG"
        );
        assert_eq!(
            ValidationError::VoiceIdTooLong { len: 1, max: 0 }.code(),
            "VOICE_ID_TOO_LONG"
        );
        assert_eq!(ValidationError::InvalidVoiceId.code(), "INVALID_VOICE_ID");
        assert_eq!(
            ValidationError::LanguageTooLong { len: 1, max: 0 }.code(),
            "LANGUAGE_TOO_LONG"
        );
    }

    /// `JSON.stringify(NaN)` is `null`; none of these may fail deserialization.
    #[test]
    fn null_numbers_and_text_survive_deserialization() {
        let request: SpeakRequest = serde_json::from_str(
            r#"{"text": null, "rate": null, "pitch": null, "volume": null, "queueMode": null}"#,
        )
        .unwrap();

        assert_eq!(
            (request.rate, request.pitch, request.volume),
            (1.0, 1.0, 1.0)
        );
        assert_eq!(request.queue_mode, QueueMode::Flush);
        assert_eq!(request.validate(), Err(ValidationError::EmptyText));
    }

    #[test]
    fn missing_fields_fall_back_to_the_same_defaults() {
        let request: SpeakRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(
            (request.rate, request.pitch, request.volume),
            (1.0, 1.0, 1.0)
        );
        assert_eq!(request.validate(), Err(ValidationError::EmptyText));
    }
}
