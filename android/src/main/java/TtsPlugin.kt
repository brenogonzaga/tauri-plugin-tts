package com.tts

import android.app.Activity
import android.content.Context
import android.media.AudioManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.Locale

private const val TAG = "TtsPlugin"

@TauriPlugin
class TtsPlugin(private val activity: Activity) : Plugin(activity), TextToSpeech.OnInitListener {

    private var engine: TextToSpeech? = null
    private val events = SpeechEventRelay()
    private val poller = ProgressPoller()
    private val pending = PendingRequests { delay, action ->
        Handler(Looper.getMainLooper()).postDelayed(action, delay)
    }

    private val focus = AudioFocusController(
        activity.getSystemService(Context.AUDIO_SERVICE) as? AudioManager,
    ) { _, reason ->
        // Android TTS cannot pause, so stopping is the only way to yield the audio.
        if (engine?.isSpeaking == true) {
            engine?.stop()
            events.emit(SpeechEvent.INTERRUPTED, reason = reason)
        }
    }

    @Volatile private var ready = false

    /** `onInit` fires once per engine; after a failure there is no second chance. */
    @Volatile private var initFailed = false

    private var continueInBackground = true
    private var currentUtteranceId: String? = null
    private var cachedVoices: List<VoiceInfo> = emptyList()

    init {
        engine = TextToSpeech(activity, this)
    }

    override fun onInit(status: Int) {
        if (status != TextToSpeech.SUCCESS) {
            Log.e(TAG, "TTS initialization failed with status $status")
            initFailed = true
            pending.rejectAll(INIT_FAILED_MESSAGE)
            return
        }

        ready = true
        initFailed = false
        engine?.setOnUtteranceProgressListener(progressListener)
        refreshVoiceCache()
        pending.drain { startSpeaking(it.invoke, it.args) }
    }

    private val progressListener = object : UtteranceProgressListener() {
        override fun onStart(utteranceId: String?) {
            events.start(utteranceId)
        }

        override fun onDone(utteranceId: String?) {
            finished(utteranceId, SpeechEvent.FINISH)
        }

        @Deprecated("Superseded by onError(String, Int)")
        override fun onError(utteranceId: String?) {
            finished(utteranceId, SpeechEvent.ERROR, error = "Speech synthesis error")
        }

        override fun onError(utteranceId: String?, errorCode: Int) {
            finished(utteranceId, SpeechEvent.ERROR, error = describe(errorCode))
        }

        override fun onStop(utteranceId: String?, interrupted: Boolean) {
            finished(utteranceId, SpeechEvent.CANCEL, interrupted = interrupted)
        }
    }

    private fun finished(
        utteranceId: String?,
        eventType: String,
        error: String? = null,
        interrupted: Boolean? = null,
    ) {
        events.finish(utteranceId, eventType, error, interrupted)
        if (utteranceId == currentUtteranceId) focus.release()
    }

    override fun onPause() {
        super.onPause()
        if (continueInBackground || engine?.isSpeaking != true) return

        // Android cannot pause, so stop; the listener reports the resulting cancel.
        engine?.stop()
        events.emit(SpeechEvent.BACKGROUND_PAUSE, reason = "app_paused")
    }

    override fun onDestroy(activity: androidx.appcompat.app.AppCompatActivity) {
        super.onDestroy(activity)
        focus.release()
        engine?.stop()
        engine?.shutdown()
        engine = null
        ready = false
    }

    /** Recreates the engine after it reports a state it cannot recover from. */
    private fun restart() {
        Log.w(TAG, "restarting the TTS engine")
        focus.release()
        engine?.stop()
        engine?.shutdown()
        ready = false
        initFailed = false
        cachedVoices = emptyList()
        events.reset()
        engine = TextToSpeech(activity, this)
    }

    @Command
    fun speak(invoke: Invoke) {
        val args = invoke.parseArgs(SpeakArgs::class.java)
        args.validate()?.let { return invoke.reject(it) }

        if (initFailed) return invoke.reject(INIT_FAILED_MESSAGE)

        if (!ready) {
            if (!pending.add(invoke, args)) {
                invoke.reject("Too many pending requests - TTS may have failed to initialize")
            }
            return
        }

        startSpeaking(invoke, args)
    }

    @Command
    fun stop(invoke: Invoke) {
        engine?.stop()
        focus.release()
        invoke.resolve(JSObject().put("success", true))
    }

    @Command
    fun getVoices(invoke: Invoke) {
        val args = invoke.parseArgs(GetVoicesArgs::class.java)

        // An empty list rather than a rejection: the UI can show a loading state and poll
        // isInitialized() instead of having to treat "not ready yet" as an error.
        val voices = if (ready) refreshVoiceCache() else emptyList()

        val array = JSArray()
        for (voice in VoiceCatalog.listable(voices, args.language)) {
            array.put(
                JSObject()
                    .put("id", voice.id)
                    .put("name", voice.displayName())
                    .put("language", voice.languageTag)
            )
        }

        invoke.resolve(JSObject().put("voices", array))
    }

    @Command
    fun isSpeaking(invoke: Invoke) {
        invoke.resolve(JSObject().put("speaking", engine?.isSpeaking ?: false))
    }

    @Command
    fun isInitialized(invoke: Invoke) {
        invoke.resolve(
            JSObject()
                .put("initialized", ready)
                .put("voiceCount", if (ready) cachedVoices.size else 0)
        )
    }

    @Command
    fun previewVoice(invoke: Invoke) {
        val args = invoke.parseArgs(PreviewVoiceArgs::class.java)
        args.validate()?.let { return invoke.reject(it) }

        val engine = this.engine
        if (!ready || engine == null) return invoke.reject("TTS not initialized")

        // Unlike speak(), no fallback: hearing this exact voice is the point of the call.
        val voice = engine.voices.orEmpty().firstOrNull { it.name == args.voiceId }
            ?: return invoke.resolve(
                JSObject()
                    .put("success", false)
                    .put("warning", "Voice '${args.voiceId}' not found")
            )

        focus.request()
        engine.stop()
        engine.voice = voice
        engine.setSpeechRate(1.0f)
        engine.setPitch(1.0f)

        val utteranceId = nextUtteranceId("preview")
        if (engine.speak(args.sampleText(), TextToSpeech.QUEUE_FLUSH, null, utteranceId)
            != TextToSpeech.SUCCESS
        ) {
            focus.release()
            return invoke.reject("Failed to preview voice - the TTS engine rejected the request.")
        }

        watchProgress(engine, utteranceId)
        invoke.resolve(
            JSObject().put("success", true).put("utteranceId", utteranceId)
        )
    }

    @Command
    fun pauseSpeaking(invoke: Invoke) {
        invoke.resolve(unsupported("Pause is not supported on Android"))
    }

    @Command
    fun resumeSpeaking(invoke: Invoke) {
        invoke.resolve(unsupported("Resume is not supported on Android"))
    }

    @Command
    fun setBackgroundBehavior(invoke: Invoke) {
        continueInBackground = invoke.parseArgs(SetBackgroundBehaviorArgs::class.java)
            .continueInBackground
        invoke.resolve(JSObject().put("success", true))
    }

    @Command
    fun setupEventRelay(invoke: Invoke) {
        events.connect(invoke.parseArgs(SetupEventRelayArgs::class.java).channel)
        invoke.resolve()
    }
    
    private fun startSpeaking(invoke: Invoke, args: SpeakArgs) {
        val engine = this.engine ?: return failSpeak(invoke, "TTS not initialized")

        try {
            focus.request()

            val voices = refreshVoiceCache()
            if (voices.isEmpty()) {
                // No voices and no cache means the engine is in a state it will not report
                // as an error. Requeue and rebuild it rather than speaking into the void.
                focus.release()
                if (!pending.add(invoke, args)) {
                    invoke.reject("TTS engine is temporarily unavailable. Please try again.")
                }
                restart()
                return
            }

            val warning = when (val selection = selectVoice(engine, args, voices)) {
                is VoiceSelection.Rejected -> return failSpeak(invoke, selection.message)
                is VoiceSelection.Ok -> selection.warning
            }
            applyParameters(engine, args)

            val utteranceId = nextUtteranceId("tts")
            val queueMode =
                if (args.flushes()) TextToSpeech.QUEUE_FLUSH else TextToSpeech.QUEUE_ADD
            val params = volumeParams(args.volume)

            if (engine.speak(args.text, queueMode, params, utteranceId) != TextToSpeech.SUCCESS) {
                return failSpeak(
                    invoke,
                    "The TTS engine refused to speak. It may have lost its voice " +
                        "configuration - try again, or select a different voice.",
                )
            }

            if (args.flushes()) watchProgress(engine, utteranceId)

            invoke.resolve(
                JSObject()
                    .put("success", true)
                    .put("utteranceId", utteranceId)
                    .apply { warning?.let { put("warning", it) } }
            )
        } catch (e: Exception) {
            Log.e(TAG, "speak failed", e)
            failSpeak(invoke, "Failed to speak: ${e.message}")
        }
    }

    /** Points the engine at the requested voice or language. */
    private fun selectVoice(
        engine: TextToSpeech,
        args: SpeakArgs,
        voices: List<VoiceInfo>,
    ): VoiceSelection {
        val requestedId = args.requestedVoiceId()
        if (requestedId != null) return selectById(engine, requestedId, voices)

        val requestedLanguage = args.requestedLanguage()
        if (requestedLanguage != null) return selectByLanguage(engine, requestedLanguage)

        if (engine.voice == null) {
            VoiceCatalog.defaultVoice(voices)?.let { setVoice(engine, it.id) }
        }
        return VoiceSelection.Ok()
    }

    private fun selectById(
        engine: TextToSpeech,
        requestedId: String,
        voices: List<VoiceInfo>,
    ): VoiceSelection {
        val selected = voices.firstOrNull { it.id == requestedId }

        if (selected != null && !selected.isUsable()) {
            // getVoices() never offered this voice; speaking with it produces silence.
            return VoiceSelection.Rejected(
                "Voice '$requestedId' is missing its language data and cannot speak."
            )
        }

        if (selected != null) {
            setVoice(engine, selected.id)
            return VoiceSelection.Ok()
        }

        val fallback = VoiceCatalog.fallbackFor(voices.filter { it.isUsable() }, requestedId)
            ?: return VoiceSelection.Ok("Voice '$requestedId' not found, using default voice")

        setVoice(engine, fallback.id)
        return VoiceSelection.Ok(
            "Voice '$requestedId' not available, using '${fallback.id}' instead"
        )
    }

    private fun selectByLanguage(engine: TextToSpeech, language: String): VoiceSelection {
        val result = engine.setLanguage(Locale.forLanguageTag(language))
        val supported = result != TextToSpeech.LANG_MISSING_DATA &&
            result != TextToSpeech.LANG_NOT_SUPPORTED

        return if (supported) {
            VoiceSelection.Ok()
        } else {
            VoiceSelection.Ok("Language '$language' not supported, using default language")
        }
    }

    /** The engine only accepts its own voice type, so the chosen ID is resolved once more. */
    private fun setVoice(engine: TextToSpeech, id: String) {
        engine.voices.orEmpty().firstOrNull { it.name == id }?.let { engine.voice = it }
    }

    private fun applyParameters(engine: TextToSpeech, args: SpeakArgs) {
        // Rate and pitch are engine-global and persist across utterances, so both are set
        // every time; skipping one leaks the previous call's value into this one. Android's
        // scale already matches the plugin's, so the values pass through unchanged.
        engine.setSpeechRate(args.rate)
        engine.setPitch(args.pitch)
    }

    /** Volume is per-utterance, unlike rate and pitch. */
    private fun volumeParams(volume: Float): Bundle? =
        if (volume == 1.0f) {
            null
        } else {
            Bundle().apply { putFloat(TextToSpeech.Engine.KEY_PARAM_VOLUME, volume) }
        }

    private fun watchProgress(engine: TextToSpeech, utteranceId: String) {
        poller.watch(engine, utteranceId, events) { utteranceId == currentUtteranceId }
    }

    private fun nextUtteranceId(prefix: String): String =
        "${prefix}_${System.currentTimeMillis()}".also { currentUtteranceId = it }

    /** Rejects a speak that already took audio focus. */
    private fun failSpeak(invoke: Invoke, message: String) {
        focus.release()
        invoke.reject(message)
    }

    private fun refreshVoiceCache(): List<VoiceInfo> {
        val engine = this.engine ?: return cachedVoices
        val native = engine.voices

        if (native.isNullOrEmpty()) return cachedVoices

        cachedVoices = native.map { VoiceCatalog.from(it, engine) }
        return cachedVoices
    }

    private fun describe(errorCode: Int): String = when (errorCode) {
        TextToSpeech.ERROR_INVALID_REQUEST -> "Invalid request"
        TextToSpeech.ERROR_NETWORK -> "Network error"
        TextToSpeech.ERROR_NETWORK_TIMEOUT -> "Network timeout"
        TextToSpeech.ERROR_NOT_INSTALLED_YET -> "TTS not installed"
        TextToSpeech.ERROR_OUTPUT -> "Output error"
        TextToSpeech.ERROR_SERVICE -> "Service error"
        TextToSpeech.ERROR_SYNTHESIS -> "Synthesis error"
        else -> "Unknown error ($errorCode)"
    }

    private fun unsupported(reason: String): JSObject =
        JSObject().put("success", false).put("reason", reason)

    private companion object {
        const val INIT_FAILED_MESSAGE =
            "TTS engine failed to initialize on this device. " +
                "Install a text-to-speech engine in Settings > Accessibility."
    }
}

/** The outcome of pointing the engine at a voice, before anything is spoken. */
sealed interface VoiceSelection {
    /** The engine is configured. [warning] is set when a substitute voice was used. */
    data class Ok(val warning: String? = null) : VoiceSelection

    /** Nothing can be spoken; [message] explains why. */
    data class Rejected(val message: String) : VoiceSelection
}
