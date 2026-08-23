import Tauri
import TtsCore

class SpeakArgs: Decodable {
    let text: String
    let language: String?
    let voiceId: String?
    let rate: Float?
    let pitch: Float?
    let volume: Float?
    let queueMode: String?

    func validate() throws {
        try SpeechValidator.validateText(text)
        if let voiceId { try SpeechValidator.validateVoiceId(voiceId) }
        if let language { try SpeechValidator.validateLanguage(language) }
    }

    var clampedRate: Float { clamp(rate, to: SpeechLimits.rateRange) }
    var clampedPitch: Float { clamp(pitch, to: SpeechLimits.pitchRange) }
    var clampedVolume: Float { clamp(volume, to: SpeechLimits.volumeRange) }

    var flushes: Bool { (queueMode ?? "flush").lowercased() != "add" }

    private func clamp(_ value: Float?, to range: ClosedRange<Float>) -> Float {
        guard let value else { return 1.0 }
        return min(max(value, range.lowerBound), range.upperBound)
    }
}

class PreviewVoiceArgs: Decodable {
    let voiceId: String
    let text: String?

    var sampleText: String { text ?? SpeechLimits.defaultSampleText }

    func validate() throws {
        try SpeechValidator.validateVoiceId(voiceId)
        if let text { try SpeechValidator.validateText(text) }
    }
}

class GetVoicesArgs: Decodable {
    let language: String?
}

class SetBackgroundBehaviorArgs: Decodable {
    let continueInBackground: Bool
}

class SetupEventRelayArgs: Decodable {
    let channel: Channel
}
