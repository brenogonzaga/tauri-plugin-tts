/// Lifecycle event names, emitted to JavaScript as `tts://<name>`.
public enum SpeechEventName {
    public static let start = "speech:start"
    public static let finish = "speech:finish"
    public static let cancel = "speech:cancel"
    public static let error = "speech:error"
    public static let pause = "speech:pause"
    public static let resume = "speech:resume"
    public static let interrupted = "speech:interrupted"
    public static let backgroundPause = "speech:backgroundPause"
}

/// Why an event fired, when it was not the caller's doing.
public enum SpeechEventReason {
    public static let routeChange = "route_change"
    public static let interruptionEnded = "interruption_ended"
    public static let appPaused = "app_paused"
}
