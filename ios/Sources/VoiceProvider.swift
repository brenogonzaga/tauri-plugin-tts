import AVFoundation
import TtsCore

/// The installed voices, cached because enumerating them is not cheap and voice lists only
/// change when the user installs one.
final class VoiceProvider {
    private static let cacheTTL: TimeInterval = 60

    private var cached: [AVSpeechSynthesisVoice]?
    private var cachedAt: Date?
    private let lock = NSLock()

    func voices() -> [AVSpeechSynthesisVoice] {
        lock.lock()
        defer { lock.unlock() }

        if let cached, let cachedAt, Date().timeIntervalSince(cachedAt) < Self.cacheTTL {
            return cached
        }

        let voices = AVSpeechSynthesisVoice.speechVoices()
        // An empty list means the system is not ready; caching it would blank out getVoices
        // for a full TTL after it is.
        if !voices.isEmpty {
            cached = voices
            cachedAt = Date()
        }
        return voices
    }

    func info() -> [VoiceInfo] {
        voices().map {
            VoiceInfo(id: $0.identifier, name: $0.name, language: $0.language)
        }
    }

    func voice(withId id: String) -> AVSpeechSynthesisVoice? {
        voices().first { $0.identifier == id }
    }
}
