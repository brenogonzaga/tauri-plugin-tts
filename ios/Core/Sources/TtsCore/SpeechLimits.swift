import Foundation

/// Input limits, mirroring the constants in the Rust plugin.
public enum SpeechLimits {
    /// Maximum `text` length, in UTF-8 bytes.
    public static let maxTextBytes = 10_000
    /// Maximum `voiceId` length, in characters.
    public static let maxVoiceIdLength = 256
    /// Maximum `language` length, in characters.
    public static let maxLanguageLength = 35

    /// Accepted `rate`, where 1.0 is the platform's normal speed.
    public static let rateRange: ClosedRange<Float> = 0.1...4.0
    /// Accepted `pitch`, where 1.0 is the platform's normal pitch.
    public static let pitchRange: ClosedRange<Float> = 0.5...2.0
    public static let volumeRange: ClosedRange<Float> = 0.0...1.0

    public static let defaultSampleText = "Hello! This is a sample of how this voice sounds."
}

public enum TtsValidationError: Error, LocalizedError, Equatable {
    case emptyText
    case textTooLong(length: Int, max: Int)
    case voiceIdTooLong(length: Int, max: Int)
    case invalidVoiceId
    case languageTooLong(length: Int, max: Int)

    public var code: String {
        switch self {
        case .emptyText: return "EMPTY_TEXT"
        case .textTooLong: return "TEXT_TOO_LONG"
        case .voiceIdTooLong: return "VOICE_ID_TOO_LONG"
        case .invalidVoiceId: return "INVALID_VOICE_ID"
        case .languageTooLong: return "LANGUAGE_TOO_LONG"
        }
    }

    public var errorDescription: String? {
        switch self {
        case .emptyText:
            return "Text cannot be empty"
        case let .textTooLong(length, max):
            return "Text too long: \(length) bytes (max: \(max))"
        case let .voiceIdTooLong(length, max):
            return "Voice ID too long: \(length) characters (max: \(max))"
        case .invalidVoiceId:
            return "Invalid voice ID - control characters are not allowed"
        case let .languageTooLong(length, max):
            return "Language code too long: \(length) characters (max: \(max))"
        }
    }

    /// The message the plugin rejects with, carrying the code so JavaScript can classify it.
    public var rejection: String {
        "\(code): \(errorDescription ?? "Invalid input")"
    }
}

/// Second line of defence behind the shared Rust validation, which every call already
/// passes through.
public enum SpeechValidator {

    public static func validateText(_ text: String) throws {
        guard !text.isEmpty else { throw TtsValidationError.emptyText }

        let bytes = text.utf8.count
        guard bytes <= SpeechLimits.maxTextBytes else {
            throw TtsValidationError.textTooLong(length: bytes, max: SpeechLimits.maxTextBytes)
        }
    }

    /// Voice IDs are matched by exact equality against the engine's own voices, so this only
    /// has to reject pathological input.
    public static func validateVoiceId(_ voiceId: String) throws {
        guard voiceId.count <= SpeechLimits.maxVoiceIdLength else {
            throw TtsValidationError.voiceIdTooLong(
                length: voiceId.count,
                max: SpeechLimits.maxVoiceIdLength
            )
        }
        guard !voiceId.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
        else {
            throw TtsValidationError.invalidVoiceId
        }
    }

    public static func validateLanguage(_ language: String) throws {
        guard language.count <= SpeechLimits.maxLanguageLength else {
            throw TtsValidationError.languageTooLong(
                length: language.count,
                max: SpeechLimits.maxLanguageLength
            )
        }
    }
}
