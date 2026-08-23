import Foundation

/// One installed voice, decoupled from `AVSpeechSynthesisVoice` so selection and filtering
/// can be exercised without the speech framework.
public struct VoiceInfo: Equatable {
    public let id: String
    public let name: String
    public let language: String

    public init(id: String, name: String, language: String) {
        self.id = id
        self.name = name
        self.language = language
    }
}

public enum VoiceCatalog {

    /// Whether `filter` is a locale prefix of `language`: both "pt" and "pt-BR" match a
    /// "pt-BR" voice, while "US" does not match "en-US".
    public static func languageMatches(_ language: String, filter: String) -> Bool {
        language.lowercased().hasPrefix(filter.lowercased())
    }

    /// Applies the optional locale filter using the same rule `speak` uses to resolve a
    /// language to a voice.
    public static func filter(_ voices: [VoiceInfo], language: String?) -> [VoiceInfo] {
        guard let language else { return voices }
        return voices.filter { languageMatches($0.language, filter: language) }
    }

    /// The voice a request asks for: an explicit ID wins, otherwise the first voice whose
    /// locale matches `language`.
    public static func select(
        _ voices: [VoiceInfo],
        voiceId: String?,
        language: String?
    ) -> VoiceInfo? {
        if let voiceId {
            return voices.first { $0.id == voiceId }
        }
        guard let language else { return nil }
        return voices.first { languageMatches($0.language, filter: language) }
    }

    /// What to tell a caller whose request resolved to no voice. Nil when they asked for
    /// nothing in particular.
    public static func unavailableWarning(voiceId: String?, language: String?) -> String? {
        if let voiceId {
            return "Voice '\(voiceId)' not found, using default voice"
        }
        if let language {
            return "Language '\(language)' not supported, using default language"
        }
        return nil
    }
}
