import AVFoundation
import UIKit

/// Owns the audio session and reports the system events that interrupt speech.
///
/// The controller never decides what happens to the synthesizer; it only says what the
/// system did, so the pause-versus-stop policy lives in one place in the plugin.
final class AudioSessionController {

    struct Events {
        /// A call or another app took the session.
        let interruptionBegan: () -> Void
        /// The interruption ended. `shouldResume` is the system's advice.
        let interruptionEnded: (_ shouldResume: Bool) -> Void
        /// Headphones were unplugged or the output otherwise disappeared.
        let outputLost: () -> Void
        let enteredBackground: () -> Void
        let willTerminate: () -> Void
    }

    private let events: Events

    init(events: Events) {
        self.events = events
        observe()
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }

    /// Configures and activates the session.
    ///
    /// - Returns: nil on success, or a reason the session is unusable. Speaking into a
    ///   session that never activated produces silence with no error, so the caller reports
    ///   this rather than continuing.
    @discardableResult
    func activate() -> String? {
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playback, mode: .spokenAudio, options: [.duckOthers])
            try session.setActive(true)
            return nil
        } catch {
            return "Audio session unavailable: \(error.localizedDescription)"
        }
    }

    func reactivate() throws {
        try AVAudioSession.sharedInstance().setActive(true)
    }

    func deactivate() {
        try? AVAudioSession.sharedInstance()
            .setActive(false, options: .notifyOthersOnDeactivation)
    }

    /// Whether the app is allowed to keep playing once suspended. Without this entitlement
    /// speech stops on lock with no event, which looks like a plugin bug.
    var hasBackgroundAudioEntitlement: Bool {
        let modes = Bundle.main.object(forInfoDictionaryKey: "UIBackgroundModes") as? [String]
        return modes?.contains("audio") == true
    }

    private func observe() {
        let center = NotificationCenter.default

        center.addObserver(
            self,
            selector: #selector(handleInterruption),
            name: AVAudioSession.interruptionNotification,
            object: AVAudioSession.sharedInstance()
        )
        center.addObserver(
            self,
            selector: #selector(handleRouteChange),
            name: AVAudioSession.routeChangeNotification,
            object: AVAudioSession.sharedInstance()
        )
        center.addObserver(
            self,
            selector: #selector(handleDidEnterBackground),
            name: UIApplication.didEnterBackgroundNotification,
            object: nil
        )
        center.addObserver(
            self,
            selector: #selector(handleWillTerminate),
            name: UIApplication.willTerminateNotification,
            object: nil
        )
    }

    @objc private func handleInterruption(_ notification: Notification) {
        guard
            let raw = notification.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
            let type = AVAudioSession.InterruptionType(rawValue: raw)
        else { return }

        switch type {
        case .began:
            events.interruptionBegan()
        case .ended:
            let raw = notification.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0
            let options = AVAudioSession.InterruptionOptions(rawValue: raw)
            events.interruptionEnded(options.contains(.shouldResume))
        @unknown default:
            break
        }
    }

    @objc private func handleRouteChange(_ notification: Notification) {
        guard
            let raw = notification.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
            let reason = AVAudioSession.RouteChangeReason(rawValue: raw),
            reason == .oldDeviceUnavailable
        else { return }

        events.outputLost()
    }

    @objc private func handleDidEnterBackground() {
        events.enteredBackground()
    }

    @objc private func handleWillTerminate() {
        events.willTerminate()
    }
}
