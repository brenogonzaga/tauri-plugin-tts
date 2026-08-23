import AVFoundation

/// Maps each in-flight utterance to the ID its lifecycle events carry.
///
/// `AVSpeechUtterance` has no identity of its own, so entries are keyed by object identity
/// and removed when the utterance reaches a terminal state. An utterance that is never
/// spoken must never be registered, or its entry stays forever.
final class UtteranceRegistry {
    private var ids = [ObjectIdentifier: String]()
    private let lock = NSLock()

    /// Assigns a fresh ID to `utterance` and remembers it for the delegate callbacks.
    func register(_ utterance: AVSpeechUtterance) -> String {
        let id = UUID().uuidString

        lock.lock()
        defer { lock.unlock() }
        ids[ObjectIdentifier(utterance)] = id
        return id
    }

    func id(of utterance: AVSpeechUtterance) -> String? {
        lock.lock()
        defer { lock.unlock() }
        return ids[ObjectIdentifier(utterance)]
    }

    /// The ID of an utterance that has ended, forgetting it in the process.
    func release(_ utterance: AVSpeechUtterance) -> String? {
        lock.lock()
        defer { lock.unlock() }
        return ids.removeValue(forKey: ObjectIdentifier(utterance))
    }
}
