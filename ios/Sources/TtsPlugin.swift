import AVFoundation
import SwiftRs
import Tauri
import UIKit
import WebKit

/// Maximum text length in bytes (10KB)
private let maxTextLength = 10_000
/// Maximum voice ID length
private let maxVoiceIdLength = 256
/// Maximum language code length
private let maxLanguageLength = 35
/// Allowed characters in voice ID (alphanumeric, dots, underscores, hyphens)
private let voiceIdAllowedCharacters = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "._-"))

enum TtsValidationError: Error, LocalizedError {
    case emptyText
    case textTooLong(length: Int, max: Int)
    case voiceIdTooLong(length: Int, max: Int)
    case invalidVoiceIdFormat
    case languageTooLong(length: Int, max: Int)
    
    var errorDescription: String? {
        switch self {
        case .emptyText:
            return "Text cannot be empty"
        case .textTooLong(let length, let max):
            return "Text too long: \(length) characters (max: \(max))"
        case .voiceIdTooLong(let length, let max):
            return "Voice ID too long: \(length) characters (max: \(max))"
        case .invalidVoiceIdFormat:
            return "Invalid voice ID format - only alphanumeric, dots, underscores, and hyphens allowed"
        case .languageTooLong(let length, let max):
            return "Language code too long: \(length) characters (max: \(max))"
        }
    }
    
    var errorCode: String {
        switch self {
        case .emptyText: return "EMPTY_TEXT"
        case .textTooLong: return "TEXT_TOO_LONG"
        case .voiceIdTooLong: return "VOICE_ID_TOO_LONG"
        case .invalidVoiceIdFormat: return "INVALID_VOICE_ID"
        case .languageTooLong: return "LANGUAGE_TOO_LONG"
        }
    }
}

private struct InputValidator {
    static func validateText(_ text: String) throws {
        if text.isEmpty {
            throw TtsValidationError.emptyText
        }
        if text.count > maxTextLength {
            throw TtsValidationError.textTooLong(length: text.count, max: maxTextLength)
        }
    }
    
    static func validateVoiceId(_ voiceId: String) throws {
        if voiceId.count > maxVoiceIdLength {
            throw TtsValidationError.voiceIdTooLong(length: voiceId.count, max: maxVoiceIdLength)
        }
        // Check if voice ID contains only allowed characters
        if voiceId.unicodeScalars.contains(where: { !voiceIdAllowedCharacters.contains($0) }) {
            throw TtsValidationError.invalidVoiceIdFormat
        }
    }
    
    static func validateLanguage(_ language: String) throws {
        if language.count > maxLanguageLength {
            throw TtsValidationError.languageTooLong(length: language.count, max: maxLanguageLength)
        }
    }
}

class SpeakArgs: Decodable {
    let text: String
    let language: String?
    let voiceId: String?
    let rate: Float?
    let pitch: Float?
    let volume: Float?
    let queueMode: String?
    
    func validate() throws {
        try InputValidator.validateText(text)
        if let voiceId = voiceId {
            try InputValidator.validateVoiceId(voiceId)
        }
        if let language = language {
            try InputValidator.validateLanguage(language)
        }
    }
    
    var clampedRate: Float {
        guard let rate = rate else { return 1.0 }
        return min(max(rate, 0.1), 4.0)
    }
    
    var clampedPitch: Float {
        guard let pitch = pitch else { return 1.0 }
        return min(max(pitch, 0.5), 2.0)
    }
    
    var clampedVolume: Float {
        guard let volume = volume else { return 1.0 }
        return min(max(volume, 0.0), 1.0)
    }
}

class GetVoicesArgs: Decodable {
    let language: String?
}

class PreviewVoiceArgs: Decodable {
    let voiceId: String
    let text: String?
    
    static let defaultSampleText = "Hello! This is a sample of how this voice sounds."
    
    var sampleText: String {
        return text ?? Self.defaultSampleText
    }
    
    func validate() throws {
        try InputValidator.validateVoiceId(voiceId)
        if let text = text {
            try InputValidator.validateText(text)
        }
    }
}

class SetBackgroundBehaviorArgs: Decodable {
    let continueInBackground: Bool
}

class SetupEventRelayArgs: Decodable {
    let channel: Channel
}

class TtsPlugin: Plugin, AVSpeechSynthesizerDelegate {
    private let synthesizer = AVSpeechSynthesizer()
    private var currentUtteranceId: String?
    private var wasInterrupted: Bool = false
    private var isInForeground: Bool = true
    private var continueInBackground: Bool = true
    private var eventChannel: Channel? = nil
    private var voiceCache: [AVSpeechSynthesisVoice]?
    private var voiceCacheTimestamp: Date?
    private let voiceCacheTTL: TimeInterval = 60.0
        override init() {
        super.init()
        NSLog("[TtsPlugin] PLUGIN INIT")
        NSLog("[TtsPlugin]   iOS Version: \(UIDevice.current.systemVersion)")
        NSLog("[TtsPlugin]   Device: \(UIDevice.current.model)")
        synthesizer.delegate = self
        NSLog("[TtsPlugin]   Synthesizer delegate set")
        setupAudioSession()
        setupInterruptionHandling()
        setupLifecycleObservers()
    }
    
    private func setupAudioSession() {
        NSLog("[TtsPlugin] setupAudioSession() CALLED")
        do {
            let session = AVAudioSession.sharedInstance()
            // Use playback category with spoken audio mode for TTS
            try session.setCategory(.playback, mode: .spokenAudio, options: [.duckOthers])
            try session.setActive(true)
            NSLog("[TtsPlugin]   Audio session configured: category=playback, mode=spokenAudio")
        } catch {
            NSLog("[TtsPlugin]   ERROR: Failed to configure audio session: \(error.localizedDescription)")
        }
    }
    
    private func setupLifecycleObservers() {
        NSLog("[TtsPlugin] setupLifecycleObservers() CALLED")
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleAppDidEnterBackground),
            name: UIApplication.didEnterBackgroundNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleAppWillEnterForeground),
            name: UIApplication.willEnterForegroundNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleAppWillTerminate),
            name: UIApplication.willTerminateNotification,
            object: nil
        )
        NSLog("[TtsPlugin]   Lifecycle observers registered")
    }
    
    @objc private func handleAppDidEnterBackground() {
        NSLog("[TtsPlugin] App entered background (continueInBackground=\(continueInBackground))")
        isInForeground = false
        
        if synthesizer.isSpeaking && !synthesizer.isPaused {
            if continueInBackground {
                // Continue speaking — AVAudioSession .playback category supports background audio.
                // Notify JS so it can update UI state if needed.
                NSLog("[TtsPlugin]   App going to background while speaking — continuing in background")
                emitEvent("speech:backgroundPause")
            } else {
                // User opted out of background audio — pause and notify JS.
                NSLog("[TtsPlugin]   App going to background while speaking — pausing (continueInBackground=false)")
                synthesizer.pauseSpeaking(at: .word)
                emitEvent("speech:backgroundPause")
            }
        }
    }
    
    @objc private func handleAppWillEnterForeground() {
        NSLog("[TtsPlugin] App will enter foreground")
        isInForeground = true
        
        // Optionally resume paused speech
        // Note: We don't auto-resume to avoid unexpected audio
    }
    
    @objc private func handleAppWillTerminate() {
        NSLog("[TtsPlugin] App will terminate - cleaning up")
        synthesizer.stopSpeaking(at: .immediate)
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
    
    private func setupInterruptionHandling() {
        NSLog("[TtsPlugin] setupInterruptionHandling() CALLED")
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleAudioSessionInterruption),
            name: AVAudioSession.interruptionNotification,
            object: AVAudioSession.sharedInstance()
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleAudioRouteChange),
            name: AVAudioSession.routeChangeNotification,
            object: AVAudioSession.sharedInstance()
        )
        NSLog("[TtsPlugin]   Observers registered")
    }
    
    @objc private func handleAudioRouteChange(notification: Notification) {
        guard let userInfo = notification.userInfo,
              let reasonValue = userInfo[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: reasonValue) else {
            return
        }
        
        switch reason {
        case .oldDeviceUnavailable:
            // Headphones disconnected - pause speech if playing
            if synthesizer.isSpeaking {
                synthesizer.pauseSpeaking(at: .word)
                NSLog("[TtsPlugin] Speech paused due to audio route change (device unavailable)")
                emitEvent("speech:pause")
            }
        case .newDeviceAvailable:
            NSLog("[TtsPlugin] New audio device available")
        default:
            break
        }
    }
    
    @objc private func handleAudioSessionInterruption(notification: Notification) {
        guard let userInfo = notification.userInfo,
              let typeValue = userInfo[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: typeValue) else {
            return
        }
        
        switch type {
        case .began:
            // Interruption began - TTS will be paused automatically
            if synthesizer.isSpeaking {
                wasInterrupted = true
                synthesizer.pauseSpeaking(at: .word)
                NSLog("[TtsPlugin] Speech paused due to interruption")
                
                emitEvent("speech:interrupted")
            }
            
        case .ended:
            // Interruption ended - check if we should resume
            guard let optionsValue = userInfo[AVAudioSessionInterruptionOptionKey] as? UInt else { return }
            let options = AVAudioSession.InterruptionOptions(rawValue: optionsValue)
            
            if options.contains(.shouldResume) && wasInterrupted {
                // Reactivate audio session and resume speech
                do {
                    try AVAudioSession.sharedInstance().setActive(true)
                    synthesizer.continueSpeaking()
                    NSLog("[TtsPlugin] Speech resumed after interruption")
                } catch {
                    NSLog("[TtsPlugin] Failed to resume after interruption: \(error.localizedDescription)")
                }
            }
            wasInterrupted = false
            
        @unknown default:
            break
        }
    }
    
    deinit {
        NotificationCenter.default.removeObserver(self)
    }
    
    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didStart utterance: AVSpeechUtterance) {
        emitEvent("speech:start", id: currentUtteranceId)
        NSLog("[TtsPlugin] Speech started")
    }
    
    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) {
        emitEvent("speech:finish", id: currentUtteranceId)
        currentUtteranceId = nil
        NSLog("[TtsPlugin] Speech finished")
        
        // Optionally deactivate audio session to allow other audio
        // try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
    
    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didCancel utterance: AVSpeechUtterance) {
        emitEvent("speech:cancel", id: currentUtteranceId)
        currentUtteranceId = nil
        NSLog("[TtsPlugin] Speech cancelled")
    }
    
    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didPause utterance: AVSpeechUtterance) {
        emitEvent("speech:pause")
    }
    
    func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didContinue utterance: AVSpeechUtterance) {
        emitEvent("speech:resume")
    }
    
    @objc public func speak(_ invoke: Invoke) throws {
        NSLog("[TtsPlugin] speak() CALLED")
        
        let args = try invoke.parseArgs(SpeakArgs.self)
        
        do {
            try args.validate()
        } catch let error as TtsValidationError {
            NSLog("[TtsPlugin]   Validation error: \(error.localizedDescription)")
            invoke.reject("\(error.errorCode): \(error.localizedDescription)")
            return
        }
        
        NSLog("[TtsPlugin]   Text length: \(args.text.count) chars")
        NSLog("[TtsPlugin]   Language: \(args.language ?? "default")")
        NSLog("[TtsPlugin]   VoiceId: \(args.voiceId ?? "default")")
        NSLog("[TtsPlugin]   Rate: \(args.clampedRate)")
        NSLog("[TtsPlugin]   Pitch: \(args.clampedPitch)")
        NSLog("[TtsPlugin]   Volume: \(args.clampedVolume)")
        NSLog("[TtsPlugin]   QueueMode: \(args.queueMode ?? "flush")")
        NSLog("[TtsPlugin]   Foreground: \(isInForeground)")
        
        setupAudioSession()
        
        // Handle queue mode: "flush" (default) stops current speech, "add" queues it
        let shouldFlush = (args.queueMode ?? "flush").lowercased() != "add"
        if shouldFlush && synthesizer.isSpeaking {
            NSLog("[TtsPlugin]   Flushing current speech")
            synthesizer.stopSpeaking(at: .immediate)
        }
        
        let utterance = AVSpeechUtterance(string: args.text)
        
        currentUtteranceId = UUID().uuidString
        
        var warning: String? = nil
        
        if let voiceId = args.voiceId {
            if let voice = AVSpeechSynthesisVoice.speechVoices().first(where: { $0.identifier == voiceId }) {
                utterance.voice = voice
            } else {
                NSLog("[TtsPlugin] Voice not found: \(voiceId), using default")
                warning = "Voice '\(voiceId)' not found, using default voice"
            }
        } else if let language = args.language {
            if let voice = AVSpeechSynthesisVoice(language: language) {
                utterance.voice = voice
            } else {
                NSLog("[TtsPlugin] Language not supported: \(language), using default")
                warning = "Language '\(language)' not supported, using default language"
            }
        }
        
        // WORKAROUND: If all values are default (1.0), skip configuration
        // Allow the engine to use the system's default values
        let allDefaults = (args.clampedRate == 1.0 && args.clampedPitch == 1.0 && args.clampedVolume == 1.0)
        
        if !allDefaults {
            if args.clampedRate != 1.0 {
                let normalizedRate = args.clampedRate * 0.5
                utterance.rate = min(max(normalizedRate, AVSpeechUtteranceMinimumSpeechRate), AVSpeechUtteranceMaximumSpeechRate)
            }
            
            if args.clampedPitch != 1.0 {
                utterance.pitchMultiplier = args.clampedPitch
            }
            
            if args.clampedVolume != 1.0 {
                utterance.volume = args.clampedVolume
            }
        }
        
        synthesizer.speak(utterance)
        
        var response: [String: Any] = [
            "success": true,
            "utteranceId": currentUtteranceId as Any
        ]
        if let w = warning {
            response["warning"] = w
        }
        invoke.resolve(response)
    }
    
    @objc public func stop(_ invoke: Invoke) throws {
        NSLog("[TtsPlugin] stop() CALLED")
        NSLog("[TtsPlugin]   isSpeaking: \(synthesizer.isSpeaking)")
        synthesizer.stopSpeaking(at: .immediate)
        NSLog("[TtsPlugin]   Speech stopped")
        invoke.resolve(["success": true])
    }
    
    @objc public func pauseSpeaking(_ invoke: Invoke) throws {
        NSLog("[TtsPlugin] pauseSpeaking() CALLED")
        NSLog("[TtsPlugin]   isSpeaking: \(synthesizer.isSpeaking), isPaused: \(synthesizer.isPaused)")
        
        if synthesizer.isSpeaking && !synthesizer.isPaused {
            synthesizer.pauseSpeaking(at: .word)
            NSLog("[TtsPlugin]   Speech paused")
            invoke.resolve(["success": true])
        } else {
            let reason = synthesizer.isPaused ? "Already paused" : "Not speaking"
            NSLog("[TtsPlugin]   Cannot pause: \(reason)")
            invoke.resolve(["success": false, "reason": reason])
        }
    }
    
    @objc public func resumeSpeaking(_ invoke: Invoke) throws {
        NSLog("[TtsPlugin] resumeSpeaking() CALLED")
        NSLog("[TtsPlugin]   isPaused: \(synthesizer.isPaused)")
        
        if synthesizer.isPaused {
            synthesizer.continueSpeaking()
            NSLog("[TtsPlugin]   Speech resumed")
            invoke.resolve(["success": true])
        } else {
            NSLog("[TtsPlugin]   Cannot resume: Not paused")
            invoke.resolve(["success": false, "reason": "Not paused"])
        }
    }
    
    @objc public func getVoices(_ invoke: Invoke) throws {
        NSLog("[TtsPlugin] getVoices() CALLED")
        
        let args = try invoke.parseArgs(GetVoicesArgs.self)
        
        // Check cache first
        let now = Date()
        let isCacheValid = voiceCache != nil && 
                          voiceCacheTimestamp != nil && 
                          now.timeIntervalSince(voiceCacheTimestamp!) < voiceCacheTTL
        
        let allVoices: [AVSpeechSynthesisVoice]
        if isCacheValid {
            allVoices = voiceCache!
            NSLog("[TtsPlugin]   Using cached voices (age: \(Int(now.timeIntervalSince(voiceCacheTimestamp!)))s)")
        } else {
            allVoices = AVSpeechSynthesisVoice.speechVoices()
            voiceCache = allVoices
            voiceCacheTimestamp = now
            NSLog("[TtsPlugin]   Refreshed voice cache")
        }
        
        NSLog("[TtsPlugin]   Total available voices: \(allVoices.count)")
        NSLog("[TtsPlugin]   Language filter: \(args.language ?? "none")")
        
        var voices: [[String: String]] = []
        
        for voice in allVoices {
            let languageFilter = args.language?.lowercased()
            let voiceLanguage = voice.language.lowercased()
            
            if languageFilter == nil || voiceLanguage.contains(languageFilter!) {
                voices.append([
                    "id": voice.identifier,
                    "name": voice.name,
                    "language": voice.language
                ])
            }
        }
        
        NSLog("[TtsPlugin]   Filtered voices: \(voices.count)")
        invoke.resolve(["voices": voices])
    }
    
    @objc public func isSpeaking(_ invoke: Invoke) throws {
        NSLog("[TtsPlugin] isSpeaking() CALLED")
        NSLog("[TtsPlugin]   Speaking: \(synthesizer.isSpeaking), Paused: \(synthesizer.isPaused)")
        // Only return "speaking" field for consistency with Desktop and Android
        // The paused state can be inferred from pauseSpeaking/resumeSpeaking API
        invoke.resolve([
            "speaking": synthesizer.isSpeaking
        ])
    }
    
    @objc public func isInitialized(_ invoke: Invoke) throws {
        NSLog("[TtsPlugin] isInitialized() CALLED")
        // iOS AVSpeechSynthesizer is always ready - no async init needed
        let voices = AVSpeechSynthesisVoice.speechVoices()
        invoke.resolve([
            "initialized": true,
            "voiceCount": voices.count
        ])
    }
    
    @objc public func previewVoice(_ invoke: Invoke) throws {
        NSLog("[TtsPlugin] previewVoice() CALLED")
        
        let args = try invoke.parseArgs(PreviewVoiceArgs.self)
        
        do {
            try args.validate()
        } catch let error as TtsValidationError {
            NSLog("[TtsPlugin]   Validation error: \(error.localizedDescription)")
            invoke.reject("\(error.errorCode): \(error.localizedDescription)")
            return
        }
        
        NSLog("[TtsPlugin]   VoiceId: \(args.voiceId)")
        NSLog("[TtsPlugin]   Sample text: \(args.sampleText)")
        
        setupAudioSession()
        
        if synthesizer.isSpeaking {
            NSLog("[TtsPlugin]   Stopping current speech")
            synthesizer.stopSpeaking(at: .immediate)
        }
        
        let utterance = AVSpeechUtterance(string: args.sampleText)
        currentUtteranceId = UUID().uuidString
        
        if let voice = AVSpeechSynthesisVoice.speechVoices().first(where: { $0.identifier == args.voiceId }) {
            utterance.voice = voice
            NSLog("[TtsPlugin]   Voice found: \(voice.name)")
        } else {
            NSLog("[TtsPlugin]   ERROR: Voice not found: \(args.voiceId)")
            invoke.resolve(["success": false, "warning": "Voice '\(args.voiceId)' not found"])
            return
        }
        
        utterance.rate = AVSpeechUtteranceDefaultSpeechRate
        utterance.pitchMultiplier = 1.0
        utterance.volume = 1.0
        
        synthesizer.speak(utterance)
        NSLog("[TtsPlugin]   Preview started")
        invoke.resolve(["success": true])
    }

    @objc public func setBackgroundBehavior(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(SetBackgroundBehaviorArgs.self)
        continueInBackground = args.continueInBackground
        NSLog("[TtsPlugin] setBackgroundBehavior() continueInBackground=\(continueInBackground)")
        invoke.resolve(["success": true])
    }

    @objc public func setupEventRelay(_ invoke: Invoke) throws {
        let args = try invoke.parseArgs(SetupEventRelayArgs.self)
        eventChannel = args.channel
        NSLog("[TtsPlugin] setupEventRelay() channel registered")
        invoke.resolve()
    }

    /// Payload forwarded to Rust via Channel so it can call app.emit().
    /// Must be Encodable to use Channel.send<T: Encodable>.
    private struct EventRelayPayload: Encodable {
        let eventType: String
        let id: String?
        let error: String?
        let interrupted: Bool?
        let reason: String?
    }

    /// Emit a TTS event via the relay channel.
    /// Rust receives it and re-emits via app.emit("tts://<eventType>") so that
    /// JS listen("tts://speech:finish") works uniformly on every platform.
    private func emitEvent(
        _ eventType: String,
        id: String? = nil,
        error: String? = nil,
        interrupted: Bool? = nil,
        reason: String? = nil
    ) {
        let payload = EventRelayPayload(
            eventType: eventType,
            id: id,
            error: error,
            interrupted: interrupted,
            reason: reason
        )
        try? eventChannel?.send(payload)
    }
}

@_cdecl("init_plugin_tts")
func initPlugin() -> Plugin {
    return TtsPlugin()
}

