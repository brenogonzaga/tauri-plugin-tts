import AVFoundation
import SwiftRs
import Tauri
import TtsCore

class TtsPlugin: Plugin, AVSpeechSynthesizerDelegate {

    private let synthesizer = AVSpeechSynthesizer()
    private let utterances = UtteranceRegistry()
    private let events = SpeechEventRelay()
    private let voices = VoiceProvider()
    private lazy var audio = AudioSessionController(events: audioEvents())

    private var continueInBackground = true
    private var wasInterrupted = false

    /// AVFoundation's own rate bounds, which 1.0 is mapped onto.
    private var rateRange: SpeechScale.Range {
        SpeechScale.Range(
            min: AVSpeechUtteranceMinimumSpeechRate,
            normal: AVSpeechUtteranceDefaultSpeechRate,
            max: AVSpeechUtteranceMaximumSpeechRate
        )
    }

    override init() {
        super.init()
        synthesizer.delegate = self
        audio.activate()
    }


    @objc public func speak(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(SpeakArgs.self)
        guard validated(args.validate, rejecting: invoke) else { return }
        guard activatedSession(for: invoke) else { return }

        if args.flushes && synthesizer.isSpeaking {
            synthesizer.stopSpeaking(at: .immediate)
        }

        let installed = voices.info()
        let selected = VoiceCatalog.select(
            installed,
            voiceId: args.voiceId,
            language: args.language
        )

        let utterance = AVSpeechUtterance(string: args.text)
        // Falling back to the default voice is fine, but it has to be reported: a stale
        // voiceId otherwise speaks in a different voice and still claims success.
        let warning: String?
        if let selected, let voice = voices.voice(withId: selected.id) {
            utterance.voice = voice
            warning = nil
        } else {
            warning = VoiceCatalog.unavailableWarning(
                voiceId: args.voiceId,
                language: args.language
            )
        }

        utterance.rate = SpeechScale.anchoredAtNormal(
            args.clampedRate,
            userRange: SpeechLimits.rateRange,
            range: rateRange
        )
        utterance.pitchMultiplier = args.clampedPitch
        utterance.volume = args.clampedVolume

        let id = utterances.register(utterance)
        synthesizer.speak(utterance)

        var response: [String: Any] = ["success": true, "utteranceId": id]
        response["warning"] = warning
        invoke.resolve(response)
    }

    @objc public func stop(_ invoke: Invoke) throws {
        synthesizer.stopSpeaking(at: .immediate)
        invoke.resolve(["success": true])
    }

    @objc public func getVoices(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(GetVoicesArgs.self)
        let matching = VoiceCatalog.filter(voices.info(), language: args.language)

        invoke.resolve([
            "voices": matching.map {
                ["id": $0.id, "name": $0.name, "language": $0.language]
            }
        ])
    }

    @objc public func isSpeaking(_ invoke: Invoke) throws {
        invoke.resolve(["speaking": synthesizer.isSpeaking])
    }

    /// `AVSpeechSynthesizer` needs no asynchronous setup, so it is ready as soon as it exists.
    @objc public func isInitialized(_ invoke: Invoke) throws {
        invoke.resolve(["initialized": true, "voiceCount": voices.info().count])
    }

    @objc public func previewVoice(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(PreviewVoiceArgs.self)
        guard validated(args.validate, rejecting: invoke) else { return }
        guard activatedSession(for: invoke) else { return }

        // Unlike speak(), no fallback: hearing this exact voice is the point of the call.
        guard let voice = voices.voice(withId: args.voiceId) else {
            invoke.resolve([
                "success": false,
                "warning": "Voice '\(args.voiceId)' not found",
            ])
            return
        }

        if synthesizer.isSpeaking {
            synthesizer.stopSpeaking(at: .immediate)
        }

        let utterance = AVSpeechUtterance(string: args.sampleText)
        utterance.voice = voice
        utterance.rate = AVSpeechUtteranceDefaultSpeechRate
        utterance.pitchMultiplier = 1.0
        utterance.volume = 1.0

        let id = utterances.register(utterance)
        synthesizer.speak(utterance)

        invoke.resolve(["success": true, "utteranceId": id])
    }

    @objc public func pauseSpeaking(_ invoke: Invoke) throws {
        guard synthesizer.isSpeaking, !synthesizer.isPaused else {
            let reason = synthesizer.isPaused ? "Already paused" : "Not speaking"
            invoke.resolve(["success": false, "reason": reason])
            return
        }

        synthesizer.pauseSpeaking(at: .word)
        invoke.resolve(["success": true])
    }

    @objc public func resumeSpeaking(_ invoke: Invoke) throws {
        guard synthesizer.isPaused else {
            invoke.resolve(["success": false, "reason": "Not paused"])
            return
        }

        synthesizer.continueSpeaking()
        invoke.resolve(["success": true])
    }

    @objc public func setBackgroundBehavior(_ invoke: Invoke) throws {
        continueInBackground = try invoke.parseArgs(SetBackgroundBehaviorArgs.self)
            .continueInBackground
        invoke.resolve(["success": true])
    }

    @objc public func setupEventRelay(_ invoke: Invoke) throws {
        events.connect(try invoke.parseArgs(SetupEventRelayArgs.self).channel)
        invoke.resolve()
    }


    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didStart utterance: AVSpeechUtterance) {
        events.emit(SpeechEventName.start, id: utterances.id(of: utterance))
    }

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) {
        events.emit(SpeechEventName.finish, id: utterances.release(utterance))
    }

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didCancel utterance: AVSpeechUtterance) {
        events.emit(SpeechEventName.cancel, id: utterances.release(utterance))
    }

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didPause utterance: AVSpeechUtterance) {
        events.emit(SpeechEventName.pause, id: utterances.id(of: utterance))
    }

    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didContinue utterance: AVSpeechUtterance) {
        events.emit(SpeechEventName.resume, id: utterances.id(of: utterance))
    }


    private func audioEvents() -> AudioSessionController.Events {
        AudioSessionController.Events(
            interruptionBegan: { [weak self] in self?.handleInterruptionBegan() },
            interruptionEnded: { [weak self] in self?.handleInterruptionEnded(shouldResume: $0) },
            outputLost: { [weak self] in self?.handleOutputLost() },
            enteredBackground: { [weak self] in self?.handleEnteredBackground() },
            willTerminate: { [weak self] in self?.handleWillTerminate() }
        )
    }

    private func handleInterruptionBegan() {
        guard synthesizer.isSpeaking else { return }

        wasInterrupted = true
        synthesizer.pauseSpeaking(at: .word)
        events.emit(SpeechEventName.interrupted)
    }

    private func handleInterruptionEnded(shouldResume: Bool) {
        defer { wasInterrupted = false }
        guard wasInterrupted else { return }

        guard shouldResume else {
            // Speech stays paused indefinitely, so say so: otherwise the UI keeps reporting
            // that it is speaking and no event ever arrives to correct it.
            events.emit(SpeechEventName.pause, reason: SpeechEventReason.interruptionEnded)
            return
        }

        do {
            try audio.reactivate()
            synthesizer.continueSpeaking()
        } catch {
            events.emit(
                SpeechEventName.error,
                error: "Could not resume after interruption: \(error.localizedDescription)"
            )
        }
    }

    private func handleOutputLost() {
        guard synthesizer.isSpeaking else { return }

        // Nothing resumes automatically, so the reason has to reach JS for the UI to offer it.
        synthesizer.pauseSpeaking(at: .word)
        events.emit(SpeechEventName.pause, reason: SpeechEventReason.routeChange)
    }

    private func handleEnteredBackground() {
        guard synthesizer.isSpeaking, !synthesizer.isPaused else { return }

        guard continueInBackground else {
            synthesizer.pauseSpeaking(at: .word)
            events.emit(SpeechEventName.backgroundPause, reason: SpeechEventReason.appPaused)
            return
        }

        if !audio.hasBackgroundAudioEntitlement {
            Logger.error(
                "UIBackgroundModes has no 'audio' entry, so speech will stop once the app is "
                    + "suspended. Add it, or call setBackgroundBehavior with "
                    + "continueInBackground: false."
            )
        }
    }

    private func handleWillTerminate() {
        synthesizer.stopSpeaking(at: .immediate)
        audio.deactivate()
    }


    /// Runs a validator, rejecting `invoke` with the typed code when it throws.
    private func validated(_ validate: () throws -> Void, rejecting invoke: Invoke) -> Bool {
        do {
            try validate()
            return true
        } catch let error as TtsValidationError {
            invoke.reject(error.rejection)
            return false
        } catch {
            invoke.reject(error.localizedDescription)
            return false
        }
    }

    private func activatedSession(for invoke: Invoke) -> Bool {
        guard let reason = audio.activate() else { return true }

        events.emit(SpeechEventName.error, error: reason)
        invoke.reject("AUDIO_SESSION_ERROR: \(reason)")
        return false
    }
}

@_cdecl("init_plugin_tts")
func initPlugin() -> Plugin {
    TtsPlugin()
}
