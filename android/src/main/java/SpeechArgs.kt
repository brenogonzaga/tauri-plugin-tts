package com.tts

import app.tauri.annotation.InvokeArg
import app.tauri.plugin.Channel

@InvokeArg
class SpeakArgs {
    var text: String = ""
    var language: String? = null
    var voiceId: String? = null
    var rate: Float = 1.0f
    var pitch: Float = 1.0f
    var volume: Float = 1.0f
    var queueMode: String = "flush"

    /** Returns the reason this request cannot be spoken, or null. */
    fun validate(): String? =
        Validation.text(text)
            ?: voiceId?.let(Validation::voiceId)
            ?: language?.let(Validation::language)

    /** The engine treats "default" and blank as "no selection". */
    fun requestedVoiceId(): String? = voiceId?.takeIf { it != "default" && it.isNotBlank() }

    fun requestedLanguage(): String? = language?.takeIf { it != "default" && it.isNotBlank() }

    fun flushes(): Boolean = !queueMode.equals("add", ignoreCase = true)
}

@InvokeArg
class GetVoicesArgs {
    var language: String? = null
}

@InvokeArg
class PreviewVoiceArgs {
    var voiceId: String = ""
    var text: String? = null

    fun sampleText(): String = text ?: DEFAULT_SAMPLE_TEXT

    fun validate(): String? = Validation.voiceId(voiceId) ?: text?.let(Validation::text)

    private companion object {
        const val DEFAULT_SAMPLE_TEXT = "Hello! This is a sample of how this voice sounds."
    }
}

@InvokeArg
class SetBackgroundBehaviorArgs {
    var continueInBackground: Boolean = true
}

@InvokeArg
class SetupEventRelayArgs {
    lateinit var channel: Channel
}
