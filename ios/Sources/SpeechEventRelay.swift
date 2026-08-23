import Tauri

/// Forwards lifecycle events to Rust, which re-emits them as `tts://<eventType>` so JS sees
/// the same shape as on desktop.
final class SpeechEventRelay {
    private var channel: Channel?

    func connect(_ channel: Channel) {
        self.channel = channel
    }

    func emit(
        _ eventType: String,
        id: String? = nil,
        error: String? = nil,
        interrupted: Bool? = nil,
        reason: String? = nil
    ) {
        let payload = Payload(
            eventType: eventType,
            id: id,
            error: error,
            interrupted: interrupted,
            reason: reason
        )
        try? channel?.send(payload)
    }

    /// `Channel.send` takes an `Encodable`; the shape matches `TtsEventPayload` in Rust.
    private struct Payload: Encodable {
        let eventType: String
        let id: String?
        let error: String?
        let interrupted: Bool?
        let reason: String?
    }
}
