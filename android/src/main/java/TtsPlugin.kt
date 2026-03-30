package io.affex.tts

import android.app.Activity
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.os.Bundle
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.speech.tts.Voice
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke
import java.util.Locale
import java.util.concurrent.ConcurrentLinkedQueue

@InvokeArg
class SpeakArgs {
    var text: String = ""
    var language: String? = null
    var voiceId: String? = null
    var rate: Float = 1.0f
    var pitch: Float = 1.0f
    var volume: Float = 1.0f
    var queueMode: String = "flush"
}

@InvokeArg
class GetVoicesArgs {
    var language: String? = null
}

@InvokeArg
class PreviewVoiceArgs {
    var voiceId: String = ""
    var text: String? = null
    
    fun sampleText(): String = text ?: "Hello! This is a sample of how this voice sounds."
}

@InvokeArg
class SetBackgroundBehaviorArgs {
    var continueInBackground: Boolean = true
}

@InvokeArg
class SetupEventRelayArgs {
    lateinit var channel: Channel
}

/** Maximum text length allowed (10KB) */
private const val MAX_TEXT_LENGTH = 10_000

/** Maximum voice ID length */
private const val MAX_VOICE_ID_LENGTH = 256

/** Maximum language code length */
private const val MAX_LANGUAGE_LENGTH = 35

/** Maximum pending requests in queue */
private const val MAX_PENDING_REQUESTS = 50

/** Timeout for pending requests in milliseconds */
private const val PENDING_TIMEOUT_MS = 30_000L

/** Allowed pattern for voice ID (alphanumeric, dots, underscores, hyphens) */
private val VOICE_ID_PATTERN = Regex("^[a-zA-Z0-9._-]+$")


private object InputValidator {
    fun validateText(text: String): String? {
        if (text.isEmpty()) return "Text cannot be empty"
        if (text.length > MAX_TEXT_LENGTH) return "Text too long: ${text.length} bytes (max: $MAX_TEXT_LENGTH)"
        return null
    }
    
    fun validateVoiceId(voiceId: String): String? {
        if (voiceId.length > MAX_VOICE_ID_LENGTH) return "Voice ID too long: ${voiceId.length} chars (max: $MAX_VOICE_ID_LENGTH)"
        if (!VOICE_ID_PATTERN.matches(voiceId)) return "Invalid voice ID format - only alphanumeric, dots, underscores, and hyphens allowed"
        return null
    }
    
    fun validateLanguage(language: String): String? {
        if (language.length > MAX_LANGUAGE_LENGTH) return "Language code too long: ${language.length} chars (max: $MAX_LANGUAGE_LENGTH)"
        return null
    }
}

data class PendingSpeak(
    val invoke: Invoke, 
    val args: SpeakArgs,
    val timestamp: Long = System.currentTimeMillis()
)

@TauriPlugin
class TtsPlugin(private val activity: Activity) : Plugin(activity), TextToSpeech.OnInitListener {
    private var tts: TextToSpeech? = null
    private var isInitialized = false
    private var isForeground = true
    private var continueInBackground = true
    private var isPaused = false
    // Relay channel: forwards events to Rust app.emit() so JS listen() works on mobile.
    private var eventChannel: Channel? = null
    private val pendingRequests = ConcurrentLinkedQueue<PendingSpeak>()
    private var audioManager: AudioManager? = null
    private var audioFocusRequest: AudioFocusRequest? = null
    private var cachedVoices: Set<Voice>? = null
    private var lastVoiceId: String? = null
    private var wasPlayingBeforeInterruption = false
    @Volatile private var lastUtteranceId: String? = null
    // Shared flags between UtteranceProgressListener (background thread) and polling (main thread).
    // @Volatile ensures cross-thread visibility; compareAndSet semantics via the check-then-set
    // pattern prevent duplicate speech:start / speech:finish events on real devices.
    @Volatile private var startEmitted = false
    @Volatile private var finishEmitted = false


    private val audioFocusChangeListener = AudioManager.OnAudioFocusChangeListener { focusChange ->
        when (focusChange) {
            AudioManager.AUDIOFOCUS_LOSS -> {
                // Permanent loss - another app took focus
                Log.d(TAG, "Audio focus LOST permanently")
                wasPlayingBeforeInterruption = tts?.isSpeaking == true
                tts?.stop()
                emitEvent("speech:interrupted", reason = "audio_focus_lost")
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> {
                // Temporary loss - e.g., phone call
                Log.d(TAG, "Audio focus LOST transiently (phone call, notification, etc.)")
                wasPlayingBeforeInterruption = tts?.isSpeaking == true
                if (wasPlayingBeforeInterruption) {
                    pauseSpeakingInternal()
                    emitEvent("speech:pause", reason = "audio_focus_transient_loss")
                }
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                // We could lower volume, but for TTS it's better to pause
                Log.d(TAG, "Audio focus LOSS_TRANSIENT_CAN_DUCK - pausing speech")
                wasPlayingBeforeInterruption = tts?.isSpeaking == true
                if (wasPlayingBeforeInterruption) {
                    pauseSpeakingInternal()
                    emitEvent("speech:pause", reason = "audio_focus_duck")
                }
            }
            AudioManager.AUDIOFOCUS_GAIN -> {
                // Regained focus - resume if we were playing before
                Log.d(TAG, "Audio focus GAINED")
                if (wasPlayingBeforeInterruption && isPaused) {
                    resumeSpeakingInternal()
                    emitEvent("speech:resume", reason = "audio_focus_regained")
                }
                wasPlayingBeforeInterruption = false
            }
        }
    }

    companion object {
        private const val TAG = "TtsPlugin"
    }

    init {
        Log.d(TAG, "TtsPlugin INIT")
        Log.d(TAG, "  Package: ${activity.packageName}")
        Log.d(TAG, "  Android SDK: ${Build.VERSION.SDK_INT}")
        Log.d(TAG, "  Creating TextToSpeech engine...")
        tts = TextToSpeech(activity, this)
        audioManager = activity.getSystemService(android.content.Context.AUDIO_SERVICE) as? AudioManager
        Log.d(TAG, "  AudioManager initialized: ${audioManager != null}")
    }

    override fun onInit(status: Int) {
        Log.d(TAG, "TTS onInit() CALLED")
        Log.d(TAG, "  Status: $status (SUCCESS=${TextToSpeech.SUCCESS}, ERROR=${TextToSpeech.ERROR})")
        
        if (status == TextToSpeech.SUCCESS) {
            isInitialized = true
            Log.i(TAG, "  TTS initialized successfully")
            
            tts?.let { engine ->
                val defaultVoice = engine.defaultVoice
                Log.d(TAG, "  Default voice: ${defaultVoice?.name ?: "null"}")
                Log.d(TAG, "  Default language: ${engine.defaultVoice?.locale?.toLanguageTag() ?: "unknown"}")
                Log.d(TAG, "  Available voices: ${engine.voices?.size ?: 0}")
            }
            
            // Setup utterance progress listener for speech events
            setupUtteranceProgressListener()
            
            // Process all pending requests (with timeout check)
            val pendingCount = pendingRequests.size
            Log.d(TAG, "  Processing $pendingCount pending requests")
            processPendingRequests()
        } else {
            Log.e(TAG, "  TTS initialization FAILED with status: $status")
            // Reject all pending requests
            while (pendingRequests.isNotEmpty()) {
                val pending = pendingRequests.poll()
                pending?.invoke?.reject("TTS initialization failed")
            }
        }
    }
    
    private fun processPendingRequests() {
        val now = System.currentTimeMillis()
        while (pendingRequests.isNotEmpty()) {
            val pending = pendingRequests.poll() ?: break
            if (now - pending.timestamp > PENDING_TIMEOUT_MS) {
                Log.w(TAG, "  Pending request timed out after ${now - pending.timestamp}ms")
                pending.invoke.reject("Request timed out while waiting for TTS initialization")
            } else {
                executeSpeakInternal(pending.invoke, pending.args)
            }
        }
    }
    
    private fun setupUtteranceProgressListener() {
        Log.d(TAG, "setupUtteranceProgressListener() CALLED")
        tts?.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
            override fun onStart(utteranceId: String?) {
                Log.d(TAG, "✓ UtteranceProgressListener.onStart() CALLED: $utteranceId")
                if (!startEmitted) {
                    startEmitted = true
                    emitEvent("speech:start", id = utteranceId ?: "")
                }
            }
            
            override fun onDone(utteranceId: String?) {
                Log.d(TAG, "✓ UtteranceProgressListener.onDone() CALLED: $utteranceId")
                if (!finishEmitted) {
                    finishEmitted = true
                    emitEvent("speech:finish", id = utteranceId ?: "")
                    releaseAudioFocus()
                }
            }
            
            @Deprecated("Deprecated in API level 21")
            override fun onError(utteranceId: String?) {
                Log.e(TAG, "✗ UtteranceProgressListener.onError() CALLED: $utteranceId")
                if (!finishEmitted) {
                    finishEmitted = true
                    emitEvent("speech:error", id = utteranceId ?: "", error = "Speech synthesis error")
                    releaseAudioFocus()
                }
            }
            
            override fun onError(utteranceId: String?, errorCode: Int) {
                Log.e(TAG, "✗ UtteranceProgressListener.onError() CALLED: $utteranceId, code: $errorCode")
                if (!finishEmitted) {
                    finishEmitted = true
                    emitEvent("speech:error", id = utteranceId ?: "", error = getErrorMessage(errorCode))
                    releaseAudioFocus()
                }
            }
            
            override fun onStop(utteranceId: String?, interrupted: Boolean) {
                Log.d(TAG, "✓ UtteranceProgressListener.onStop() CALLED: $utteranceId, interrupted: $interrupted")
                if (!finishEmitted) {
                    finishEmitted = true
                    emitEvent("speech:cancel", id = utteranceId ?: "", interrupted = interrupted)
                    releaseAudioFocus()
                }
            }
        })
        Log.d(TAG, "  ✓ UtteranceProgressListener registered successfully")
    }
    
    private fun requestAudioFocus(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            // AUDIOFOCUS_GAIN_TRANSIENT: correct type for TTS/navigation speech.
            // The Google TTS engine runs as a separate service and also requests audio focus
            // internally to play back synthesized audio. Using AUDIOFOCUS_GAIN (permanent)
            // causes a conflict: when the TTS service requests its own focus, the system sends
            // AUDIOFOCUS_LOSS to our listener which then calls tts.stop() — producing silence.
            // AUDIOFOCUS_GAIN_TRANSIENT avoids this conflict.
            val focusRequest = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT)
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_ASSISTANT)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build()
                )
                .setOnAudioFocusChangeListener(audioFocusChangeListener)
                .build()
            audioFocusRequest = focusRequest
            audioManager?.requestAudioFocus(focusRequest) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        } else {
            @Suppress("DEPRECATION")
            audioManager?.requestAudioFocus(
                audioFocusChangeListener,
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN_TRANSIENT
            ) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        }
    }
    
    private fun releaseAudioFocus() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            audioFocusRequest?.let { audioManager?.abandonAudioFocusRequest(it) }
        } else {
            @Suppress("DEPRECATION")
            audioManager?.abandonAudioFocus(audioFocusChangeListener)
        }
    }

    /**
     * Reinitialize the TTS engine from scratch.
     */
    private fun reinitializeTts() {
        Log.w(TAG, "reinitializeTts() - engine in bad state, restarting")
        releaseAudioFocus()
        tts?.stop()
        tts?.shutdown()
        tts = null
        isInitialized = false
        cachedVoices = null
        lastVoiceId = null
        startEmitted = false
        finishEmitted = false
        Log.d(TAG, "reinitializeTts() - creating new TextToSpeech instance...")
        tts = TextToSpeech(activity, this)
    }
    
    private fun getErrorMessage(errorCode: Int): String {
        return when (errorCode) {
            TextToSpeech.ERROR -> "Generic error"
            TextToSpeech.ERROR_INVALID_REQUEST -> "Invalid request"
            TextToSpeech.ERROR_NETWORK -> "Network error"
            TextToSpeech.ERROR_NETWORK_TIMEOUT -> "Network timeout"
            TextToSpeech.ERROR_NOT_INSTALLED_YET -> "TTS not installed"
            TextToSpeech.ERROR_OUTPUT -> "Output error"
            TextToSpeech.ERROR_SERVICE -> "Service error"
            TextToSpeech.ERROR_SYNTHESIS -> "Synthesis error"
            else -> "Unknown error ($errorCode)"
        }
    }

    @Command
    fun speak(invoke: Invoke) {
        Log.i(TAG, "speak() CALLED")
        val args = invoke.parseArgs(SpeakArgs::class.java)
        
        InputValidator.validateText(args.text)?.let { error ->
            invoke.reject(error)
            return
        }
        args.voiceId?.let { voiceId ->
            InputValidator.validateVoiceId(voiceId)?.let { error ->
                invoke.reject(error)
                return
            }
        }
        args.language?.let { language ->
            InputValidator.validateLanguage(language)?.let { error ->
                invoke.reject(error)
                return
            }
        }
        
        Log.d(TAG, "  Text: \"${args.text.take(50)}${if (args.text.length > 50) "..." else ""}\"")
        Log.d(TAG, "  Language: ${args.language ?: "(null -> system default)"}")
        Log.d(TAG, "  VoiceId: ${args.voiceId ?: "(null -> system default)"}")
        Log.d(TAG, "  Rate: ${args.rate}, Pitch: ${args.pitch}, Volume: ${args.volume}")
        Log.d(TAG, "  QueueMode: ${args.queueMode}")
        
        audioManager?.let { am ->
            Log.d(TAG, "  Media volume: ${am.getStreamVolume(AudioManager.STREAM_MUSIC)}/${am.getStreamMaxVolume(AudioManager.STREAM_MUSIC)}")
        }
        Log.d(TAG, "  TTS initialized: $isInitialized, Foreground: $isForeground")
        
        if (!isInitialized) {
            if (pendingRequests.size >= MAX_PENDING_REQUESTS) {
                Log.e(TAG, "  Too many pending requests (${pendingRequests.size})")
                invoke.reject("Too many pending requests - TTS may have failed to initialize")
                return
            }
            Log.w(TAG, "  TTS not initialized, queuing request (queue size: ${pendingRequests.size})")
            pendingRequests.add(PendingSpeak(invoke, args))
            return
        }
        
        executeSpeakInternal(invoke, args)
    }
    
    private fun executeSpeakInternal(invoke: Invoke, args: SpeakArgs) {
        Log.d(TAG, "executeSpeakInternal() called")
        try {
            tts?.let { engine ->
                // Request audio focus before speaking
                val hasFocus = requestAudioFocus()
                Log.d(TAG, "  Audio focus requested: $hasFocus")
                
                var warning: String? = null
                
                // Treat "default" as no voice selection (use system default)
                val voiceId = args.voiceId?.takeIf { it != "default" && it.isNotBlank() }
                
                voiceId?.let { id ->
                    Log.d(TAG, "  Looking for voice: $id")
                    var voices = engine.voices
                    
                    // If voices are null, try aggressive refresh strategies
                    if (voices == null || voices.isEmpty()) {
                        Log.w(TAG, "  Initial voices query returned null/empty, attempting aggressive refresh...")
                        
                        // Strategy 1: Access current voice
                        val currentVoice = engine.voice
                        Log.d(TAG, "  Strategy 1 - Current voice: ${currentVoice?.name ?: "null"}")
                        voices = engine.voices
                        
                        // Strategy 2: Access default voice if still null
                        if (voices == null || voices.isEmpty()) {
                            try {
                                val defaultVoice = engine.defaultVoice
                                Log.d(TAG, "  Strategy 2 - Default voice: ${defaultVoice?.name ?: "null"}")
                                voices = engine.voices
                            } catch (e: Exception) {
                                Log.w(TAG, "  Strategy 2 failed: ${e.message}")
                            }
                        }
                        
                        // Strategy 3: Try to reset language to force engine refresh
                        if (voices == null || voices.isEmpty()) {
                            try {
                                val currentLocale = engine.language
                                Log.d(TAG, "  Strategy 3 - Resetting language: $currentLocale")
                                engine.setLanguage(currentLocale)
                                voices = engine.voices
                            } catch (e: Exception) {
                                Log.w(TAG, "  Strategy 3 failed: ${e.message}")
                            }
                        }
                        
                        // Strategy 4: fall back to the cache populated by getVoices()
                        if (voices == null || voices.isEmpty()) {
                            val cached = cachedVoices
                            if (cached != null && cached.isNotEmpty()) {
                                Log.i(TAG, "  ✓ Strategy 4 - Using cachedVoices: ${cached.size} voices")
                                voices = cached
                            } else {
                                // Engine truly broken AND no cache — reinitialize and retry.
                                Log.w(TAG, "  Engine in bad state (voices=null, no cache). Queuing and reinitializing...")
                                if (pendingRequests.size < MAX_PENDING_REQUESTS) {
                                    pendingRequests.add(PendingSpeak(invoke, args))
                                } else {
                                    invoke.reject("TTS engine is temporarily unavailable. Please try again in a moment.")
                                }
                                reinitializeTts()
                                return
                            }
                        } else {
                            Log.i(TAG, "  ✓ Voices refreshed successfully! Now have ${voices.size} voices")
                        }
                    }

                    when {
                        else -> {
                            // Voices available - can set new voice
                            cachedVoices = voices
                            Log.d(TAG, "  Available voices count: ${voices.size} (cache updated)")
                            
                            val selectedVoice = voices.find { it.name == id }
                            if (selectedVoice != null) {
                                // Check if voice is actually usable
                                val isNetworkRequired = selectedVoice.isNetworkConnectionRequired
                                val quality = selectedVoice.quality
                                val voiceLocale = selectedVoice.locale
                                
                                Log.d(TAG, "  Voice details: name=${selectedVoice.name}, network=$isNetworkRequired, quality=$quality")
                                Log.d(TAG, "  Voice locale: ${voiceLocale.toLanguageTag()}")
                                
                                // Check if the voice's language data is available on the device
                                val langAvailability = engine.isLanguageAvailable(voiceLocale)
                                Log.d(TAG, "  Language availability: $langAvailability (AVAILABLE=0, MISSING_DATA=-1, NOT_SUPPORTED=-2)")
                                
                                // For local voices, check if language data is actually present
                                if (!isNetworkRequired && langAvailability == TextToSpeech.LANG_MISSING_DATA) {
                                    Log.e(TAG, "  ✗ Local voice missing data: ${selectedVoice.name}")
                                    invoke.reject("Voice '${selectedVoice.name}' requires language data that is not installed. This voice should have been filtered from the list.")
                                    return
                                }
                                
                                // Try to set the voice
                                try {
                                    engine.voice = selectedVoice
                                    
                                    // Verify voice was actually set
                                    val verifyVoice = engine.voice
                                    if (verifyVoice?.name != selectedVoice.name) {
                                        Log.e(TAG, "  ✗ Failed to set voice - engine rejected it")
                                        Log.e(TAG, "  Requested: ${selectedVoice.name}, Got: ${verifyVoice?.name}")
                                        invoke.reject("Failed to set voice '${selectedVoice.name}' - TTS engine rejected the voice configuration.")
                                        return
                                    }
                                    
                                    lastVoiceId = id
                                    Log.d(TAG, "  ✓ Voice set successfully: ${selectedVoice.name}")
                                } catch (e: Exception) {
                                    Log.e(TAG, "  ✗ Exception setting voice: ${e.message}", e)
                                    invoke.reject("Failed to set voice: ${e.message}")
                                    return
                                }
                            } else {
                                // Try fallback
                                val voiceParts = id.split("-")
                                val languagePrefix = if (voiceParts.size >= 2) "${voiceParts[0]}-${voiceParts[1]}" else voiceParts[0]
                                
                                val fallbackVoice = voices
                                    .filter { it.locale.toLanguageTag().lowercase().startsWith(languagePrefix.lowercase()) }
                                    .filter { !it.isNetworkConnectionRequired }
                                    .firstOrNull()
                                
                                if (fallbackVoice != null) {
                                    engine.voice = fallbackVoice
                                    lastVoiceId = fallbackVoice.name
                                    Log.w(TAG, "  Voice not found: $id, using fallback: ${fallbackVoice.name}")
                                    warning = "Voice '$id' not available, using '${fallbackVoice.name}' instead"
                                } else {
                                    Log.w(TAG, "  Voice not found: $id, using default")
                                    warning = "Voice '$id' not found, using default voice"
                                }
                            }
                        }
                    }
                } ?: run {
                    // No specific voice requested - try to set language if provided
                    val language = args.language?.takeIf { it != "default" && it.isNotBlank() }
                    language?.let { lang ->
                        Log.d(TAG, "  Setting language: $lang")
                        val locale = parseLocale(lang)
                        val result = engine.setLanguage(locale)
                        Log.d(TAG, "  setLanguage result: $result")
                        if (result == TextToSpeech.LANG_MISSING_DATA || result == TextToSpeech.LANG_NOT_SUPPORTED) {
                            Log.w(TAG, "  Language not supported: $lang, using default")
                            warning = "Language '$lang' not supported, using default language"
                        }
                    } ?: run {
                        Log.d(TAG, "  Using system default voice")
                        val currentVoice = engine.voice
                        val currentLanguage = engine.language
                        Log.d(TAG, "  Current voice: ${currentVoice?.name ?: "null"}")
                        Log.d(TAG, "  Current language: ${currentLanguage?.toLanguageTag() ?: "unknown"}")
                        
                        // If no voice is set, try to set a default one
                        if (currentVoice == null) {
                            Log.w(TAG, "  No voice is currently set, attempting to set default")
                            val voices = (engine.voices?.takeIf { it.isNotEmpty() } ?: cachedVoices)
                            if (voices != null && voices.isNotEmpty()) {
                                // Find first local (non-network) voice
                                val defaultVoice = voices
                                    .filter { !it.isNetworkConnectionRequired }
                                    .minByOrNull { it.locale.toLanguageTag() }
                                
                                if (defaultVoice != null) {
                                    engine.voice = defaultVoice
                                    Log.d(TAG, "  Set default voice: ${defaultVoice.name}")
                                } else {
                                    Log.w(TAG, "  No local voices available, using engine default")
                                }
                            } else {
                                // Engine has no voice and no cache — reinitialize and retry
                                Log.w(TAG, "  No voices available from engine or cache. Queuing and reinitializing...")
                                if (pendingRequests.size < MAX_PENDING_REQUESTS) {
                                    pendingRequests.add(PendingSpeak(invoke, args))
                                } else {
                                    invoke.reject("TTS engine is temporarily unavailable. Please try again in a moment.")
                                }
                                reinitializeTts()
                                return
                            }
                        }
                    }
                }

                // Android TTS: 1.0 is normal speed, 0.5 is half, 2.0 is double
                // Match user API directly (no normalization needed)
                val rate = args.rate.coerceIn(0.1f, 4.0f)
                val pitch = args.pitch.coerceIn(0.1f, 2.0f)
                val volume = args.volume.coerceIn(0.0f, 1.0f)
                
                // CRITICAL WORKAROUND: If ALL values are default (1.0), configure NOTHING
                // Google TTS engine has a severe bug when any setter is called with default values
                // Solution: only configure if at least one value is not default
                val allDefaults = (rate == 1.0f && pitch == 1.0f && volume == 1.0f)
                
                if (allDefaults) {
                    Log.d(TAG, "  Using engine defaults (rate=1.0, pitch=1.0, volume=1.0) - not setting anything")
                } else {
                    if (rate != 1.0f) {
                        engine.setSpeechRate(rate)
                        Log.d(TAG, "  Rate set to: $rate")
                    } else {
                        Log.d(TAG, "  Rate: 1.0 (default, not set)")
                    }
                    
                    if (pitch != 1.0f) {
                        engine.setPitch(pitch)
                        Log.d(TAG, "  Pitch set to: $pitch")
                    } else {
                        Log.d(TAG, "  Pitch: 1.0 (default, not set)")
                    }
                    
                    Log.d(TAG, "  Volume: $volume")
                }

                val utteranceId = "tts_${System.currentTimeMillis()}"
                
                Log.d(TAG, "  Utterance ID: $utteranceId")
                lastUtteranceId = utteranceId
                startEmitted = false
                finishEmitted = false
                
                // Determine queue mode: QUEUE_FLUSH (default) or QUEUE_ADD
                val queueMode = if (args.queueMode.lowercase() == "add") {
                    Log.d(TAG, "  Queue mode: QUEUE_ADD")
                    TextToSpeech.QUEUE_ADD
                } else {
                    Log.d(TAG, "  Queue mode: QUEUE_FLUSH")
                    TextToSpeech.QUEUE_FLUSH
                }
                
                // Verify engine state before speak
                Log.d(TAG, "  About to call engine.speak()...")
                Log.d(TAG, "    Engine default voice: ${engine.defaultVoice?.name}")
                Log.d(TAG, "    Engine voices available: ${engine.voices?.size ?: 0}")
                
                // Use modern Bundle API (API 21+) — the deprecated HashMap API does not reliably
                // trigger UtteranceProgressListener callbacks on some voices/engines.
                // Pass volume in the Bundle (rate/pitch are set directly on the engine).
                val params = if (volume != 1.0f) {
                    Bundle().apply { putFloat(TextToSpeech.Engine.KEY_PARAM_VOLUME, volume) }
                } else null
                val speakResult = engine.speak(args.text, queueMode, params, utteranceId)
                Log.d(TAG, "  speak() result: $speakResult (SUCCESS=${TextToSpeech.SUCCESS}, ERROR=${TextToSpeech.ERROR})")
                
                // Log final engine state after speak attempt
                val voiceAfterSpeak = engine.voice
                Log.d(TAG, "  Engine voice after speak: ${voiceAfterSpeak?.name ?: "null"}")
                Log.d(TAG, "  Engine language: ${voiceAfterSpeak?.locale?.toLanguageTag() ?: engine.language?.toLanguageTag() ?: "unknown"}")
                Log.d(TAG, "  Is speaking (immediate): ${engine.isSpeaking}")
                
                // Check if speak() was successful
                if (speakResult != TextToSpeech.SUCCESS) {
                    Log.e(TAG, "  speak() returned ERROR!")
                    
                    // Provide context based on what we know
                    val errorMsg = when {
                        voiceAfterSpeak == null && warning?.contains("temporarily unavailable") == true -> {
                            // Voice was temporarily unavailable and speak() failed
                            Log.e(TAG, "  Engine couldn't speak - voice configuration was lost")
                            "TTS engine temporarily lost voice configuration. Please try again in a moment or select a different voice."
                        }
                        voiceAfterSpeak == null -> {
                            // Voice is null but we didn't expect it
                            Log.e(TAG, "  ENGINE STATE CORRUPTED: voice is null unexpectedly")
                            "TTS engine lost voice configuration. Please try again or restart the app."
                        }
                        warning?.contains("temporarily unavailable") == true -> {
                            // Voice was unavailable but speak still failed
                            Log.e(TAG, "  Engine has voice but failed to speak - may need reinitialization")
                            "TTS engine is temporarily unavailable. Please try again in a moment."
                        }
                        else -> {
                            // Unknown error
                            Log.e(TAG, "  Unexpected speak() failure with voice: ${voiceAfterSpeak.name}")
                            "Failed to start speaking. Please try again."
                        }
                    }
                    
                    invoke.reject(errorMsg)
                    return
                }
                
                Log.d(TAG, "  Text to speak: \"${args.text.take(50)}${if (args.text.length > 50) "..." else ""}\"")
                Log.d(TAG, "  Text length: ${args.text.length} characters")
                
                // Polling fallback: emit speech:start / speech:finish by watching isSpeaking when
                // UtteranceProgressListener doesn't fire (known issue with Google TTS on emulators).
                // Uses the same startEmitted/finishEmitted flags as the listener, so exactly
                // one path wins each event even if both fire around the same time.
                //
                // On Android 14+ (API 34+), isSpeaking() returns false as soon as synthesis is
                // handed to the hardware audio buffer — BEFORE playback actually completes. A naive
                // !speaking check would then fire speech:finish prematurely for long texts, causing
                // the caller to stop or replace audio that is still playing.
                // Fix: debounce finish detection by requiring FINISH_DEBOUNCE_POLLS consecutive
                // not-speaking readings before concluding that speech is truly over.
                val pollStartTime = System.currentTimeMillis()
                val FINISH_DEBOUNCE_POLLS = 15  // 15 × 100ms = 1.5 s of confirmed silence

                activity.runOnUiThread {
                    val handler = android.os.Handler(android.os.Looper.getMainLooper())
                    var notSpeakingStreak = 0
                    val poll = object : Runnable {
                        override fun run() {
                            if (utteranceId != lastUtteranceId) return  // superseded by newer speak()
                            if (finishEmitted) return                    // already done

                            val speaking = engine.isSpeaking
                            val elapsed = System.currentTimeMillis() - pollStartTime

                            if (!startEmitted && speaking) {
                                startEmitted = true
                                Log.d(TAG, "Polling: speech:start for $utteranceId (+${elapsed}ms)")
                                emitEvent("speech:start", id = utteranceId)
                            }

                            if (startEmitted && !finishEmitted) {
                                if (!speaking) {
                                    notSpeakingStreak++
                                    if (notSpeakingStreak >= FINISH_DEBOUNCE_POLLS) {
                                        finishEmitted = true
                                        Log.d(TAG, "Polling: speech:finish for $utteranceId (+${elapsed}ms, ${notSpeakingStreak} quiet polls)")
                                        emitEvent("speech:finish", id = utteranceId)
                                        releaseAudioFocus()
                                        return
                                    }
                                } else {
                                    notSpeakingStreak = 0  // transient false — reset streak
                                }
                            }

                            if (!startEmitted && elapsed > 10_000L) {
                                if (!finishEmitted) {
                                    finishEmitted = true
                                    Log.e(TAG, "⚠️ Polling timeout: speech never started for $utteranceId")
                                    emitEvent("speech:error", id = utteranceId, error = "TTS engine did not start speaking after 10 seconds")
                                    releaseAudioFocus()
                                }
                                return
                            }

                            handler.postDelayed(this, 100)
                        }
                    }
                    handler.postDelayed(poll, 100)
                }

                val ret = JSObject()
                ret.put("success", true)
                ret.put("utteranceId", utteranceId)
                warning?.let { ret.put("warning", it) }
                invoke.resolve(ret)
            } ?: run {
                invoke.reject("TTS not initialized")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error speaking: ${e.message}")
            invoke.reject("Failed to speak: ${e.message}")
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        Log.i(TAG, "stop() CALLED")
        try {
            tts?.stop()
            Log.d(TAG, "  TTS stopped")
            val ret = JSObject()
            ret.put("success", true)
            invoke.resolve(ret)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to stop: ${e.message}", e)
            invoke.reject("Failed to stop: ${e.message}")
        }
    }

    @Command
    fun getVoices(invoke: Invoke) {
        Log.i(TAG, "getVoices() CALLED")
        val args = invoke.parseArgs(GetVoicesArgs::class.java)
        Log.d(TAG, "  Language filter: ${args.language ?: "none"}")
        
        if (!isInitialized) {
            Log.w(TAG, "  TTS not initialized, returning empty list")
            // Return empty list instead of rejecting - allows UI to show loading state
            val ret = JSObject()
            ret.put("voices", JSArray())
            ret.put("initialized", false)
            invoke.resolve(ret)
            return
        }

        try {
            var voices = tts?.voices
            
            // If voices is null/empty, try cache
            if (voices == null || voices.isEmpty()) {
                Log.w(TAG, "  TTS voices unavailable, using cache")
                voices = cachedVoices ?: emptySet()
            } else {
                // Update cache
                cachedVoices = voices
            }
            
            Log.d(TAG, "  Total voices available: ${voices.size}")
            
            // Filter out voices with missing data (not installed)
            // Problem: isLanguageAvailable() only checks language, not voice-specific data
            // Solution: Use stricter quality threshold (400+) and check features
            val engine = tts // Local reference for filtering
            val installedVoices = if (engine != null) {
                voices.filter { voice ->
                    val features = voice.features

                    // Network voices: include all — they work with internet connection
                    if (voice.isNetworkConnectionRequired) {
                        return@filter true
                    }

                    // --- Local voice filtering ---

                    // 1. Language must be available on device
                    val langAvailability = engine.isLanguageAvailable(voice.locale)
                    if (langAvailability < TextToSpeech.LANG_AVAILABLE) {
                        Log.d(TAG, "  Filtering out local voice (language unavailable): ${voice.name}")
                        return@filter false
                    }

                    // 2. Must NOT be flagged as not installed (produces garbled/no audio)
                    //    Feature flag lives on TextToSpeech.Engine, not Voice
                    if (features?.contains(TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED) == true) {
                        Log.d(TAG, "  Filtering out local voice (not installed, features: $features): ${voice.name}")
                        return@filter false
                    }

                    // 3. Filter out Google TTS "-language" routing stubs (e.g. "en-US-language").
                    //    These appear as local + quality=400 but speak() silently fails with no
                    //    callbacks when the language pack is not downloaded.
                    if (voice.name.endsWith("-language")) {
                        Log.d(TAG, "  Filtering out language-routing stub: ${voice.name}")
                        return@filter false
                    }

                    true
                }
            } else {
                voices // If engine is null, return all voices (shouldn't happen)
            }
            
            Log.d(TAG, "  Installed/network voices: ${installedVoices.size}")
            
            val voicesArray = JSArray()
            
            // Track unique voice IDs to avoid duplicates
            val seenIds = mutableSetOf<String>()
            
            // Sort: local first, then by language, then by name
            installedVoices.sortedWith(
                compareBy(
                    { voice -> if (voice.isNetworkConnectionRequired) 1 else 0 },
                    { voice -> voice.locale.toLanguageTag() },
                    { voice -> voice.name }
                )
            ).forEach { voice ->
                val languageFilter = args.language?.lowercase()
                val voiceLanguage = voice.locale.toLanguageTag().lowercase()
                
                // Skip if already seen (avoid duplicates)
                if (voice.name in seenIds) {
                    return@forEach
                }
                
                if (languageFilter == null || voiceLanguage.contains(languageFilter)) {
                    seenIds.add(voice.name)
                    
                    val voiceObj = JSObject()
                    voiceObj.put("id", voice.name)
                    // Create friendly display name from voice identifier
                    voiceObj.put("name", formatVoiceDisplayName(voice))
                    voiceObj.put("language", voice.locale.toLanguageTag())
                    voicesArray.put(voiceObj)
                }
            }
            
            Log.d(TAG, "  Returning ${voicesArray.length()} voices")
            val ret = JSObject()
            ret.put("voices", voicesArray)
            invoke.resolve(ret)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to get voices: ${e.message}", e)
            invoke.reject("Failed to get voices: ${e.message}")
        }
    }
    
    private fun formatVoiceDisplayName(voice: Voice): String {
        val locale = voice.locale
        val language = locale.displayLanguage
        val country = if (locale.country.isNotEmpty()) locale.displayCountry else null
        val quality = if (voice.name.contains("-local")) "Local" 
                     else if (voice.name.contains("-network")) "Network" 
                     else ""
        
        return buildString {
            append(language)
            if (country != null && country.isNotEmpty()) {
                append(" ($country)")
            }
            if (quality.isNotEmpty()) {
                append(" - $quality")
            }
        }
    }

    @Command
    fun isSpeaking(invoke: Invoke) {
        Log.d(TAG, "isSpeaking() CALLED")
        try {
            val speaking = tts?.isSpeaking ?: false
            Log.d(TAG, "  Speaking: $speaking")
            val ret = JSObject()
            ret.put("speaking", speaking)
            invoke.resolve(ret)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to check speaking status: ${e.message}", e)
            invoke.reject("Failed to check speaking status: ${e.message}")
        }
    }
    
    @Command
    fun isInitialized(invoke: Invoke) {
        Log.d(TAG, "isInitialized() CALLED")
        val ret = JSObject()
        ret.put("initialized", isInitialized)
        ret.put("voiceCount", tts?.voices?.size ?: 0)
        invoke.resolve(ret)
    }
    
    @Command
    fun pauseSpeaking(invoke: Invoke) {
        try {
            if (!isInitialized || tts == null) {
                val ret = JSObject()
                ret.put("success", false)
                ret.put("reason", "TTS not initialized")
                invoke.resolve(ret)
                return
            }
            
            val success = pauseSpeakingInternal()
            
            if (success) {
                Log.d(TAG, "Speech paused successfully")
                
                // Emit pause event
                emitEvent("speech:pause")
                
                val ret = JSObject()
                ret.put("success", true)
                invoke.resolve(ret)
            } else {
                val ret = JSObject()
                ret.put("success", false)
                ret.put("reason", "Failed to pause speech")
                invoke.resolve(ret)
            }
        } catch (e: Exception) {
            invoke.reject("Failed to pause: ${e.message}")
        }
    }
    
    private fun pauseSpeakingInternal(): Boolean {
        if (tts == null) return false
        
        // Android pause workaround: use playSilentUtterance with QUEUE_ADD
        // This effectively pauses by queuing silence
        val result = tts!!.playSilentUtterance(0, TextToSpeech.QUEUE_ADD, null)
        if (result == TextToSpeech.SUCCESS) {
            isPaused = true
            return true
        }
        return false
    }
    
    @Command
    fun resumeSpeaking(invoke: Invoke) {
        try {
            if (!isInitialized || tts == null) {
                val ret = JSObject()
                ret.put("success", false)
                ret.put("reason", "TTS not initialized")
                invoke.resolve(ret)
                return
            }
            
            if (!isPaused) {
                val ret = JSObject()
                ret.put("success", false)
                ret.put("reason", "Speech is not paused")
                invoke.resolve(ret)
                return
            }
            
            resumeSpeakingInternal()
            Log.d(TAG, "Speech resumed successfully")
            
            // Emit resume event
            emitEvent("speech:resume")
            
            val ret = JSObject()
            ret.put("success", true)
            invoke.resolve(ret)
        } catch (e: Exception) {
            invoke.reject("Failed to resume: ${e.message}")
        }
    }
    

    private fun resumeSpeakingInternal() {
        // Resume is automatic - the queue continues after playSilentUtterance
        // We just need to clear the pause flag
        isPaused = false
    }
    
    @Command
    fun previewVoice(invoke: Invoke) {
        Log.i(TAG, "previewVoice() CALLED")
        val args = invoke.parseArgs(PreviewVoiceArgs::class.java)
        
        // Validate inputs
        InputValidator.validateVoiceId(args.voiceId)?.let { error ->
            invoke.reject(error)
            return
        }
        args.text?.let { text ->
            InputValidator.validateText(text)?.let { error ->
                invoke.reject(error)
                return
            }
        }
        
        Log.d(TAG, "  VoiceId: ${args.voiceId}")
        Log.d(TAG, "  Sample text: \"${args.sampleText().take(30)}...\"")
        
        if (!isInitialized) {
            Log.w(TAG, "  TTS not initialized")
            invoke.reject("TTS not initialized")
            return
        }
        
        try {
            tts?.let { engine ->
                requestAudioFocus()
                
                engine.stop()
                Log.d(TAG, "  Stopped current speech")
                
                val voices = engine.voices ?: emptySet()
                val selectedVoice = voices.find { it.name == args.voiceId }
                
                if (selectedVoice != null) {
                    engine.voice = selectedVoice
                    Log.d(TAG, "  Voice set: ${selectedVoice.name}")
                } else {
                    Log.w(TAG, "  Voice not found: ${args.voiceId}")
                    val ret = JSObject()
                    ret.put("success", false)
                    ret.put("warning", "Voice '${args.voiceId}' not found")
                    invoke.resolve(ret)
                    return
                }
                
                // WORKAROUND: Don't set rate/pitch to 1.0f (Google TTS bug)
                // Just use engine defaults instead of explicitly setting to 1.0
                
                val utteranceId = "preview_${System.currentTimeMillis()}"
                
                engine.speak(args.sampleText(), TextToSpeech.QUEUE_FLUSH, null, utteranceId)
                Log.d(TAG, "  Preview started with utterance: $utteranceId")
                
                val ret = JSObject()
                ret.put("success", true)
                invoke.resolve(ret)
            } ?: run {
                Log.e(TAG, "  TTS engine is null")
                invoke.reject("TTS not initialized")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error previewing voice: ${e.message}", e)
            invoke.reject("Failed to preview voice: ${e.message}")
        }
    }

    @Command
    fun setBackgroundBehavior(invoke: Invoke) {
        val args = invoke.parseArgs(SetBackgroundBehaviorArgs::class.java)
        continueInBackground = args.continueInBackground
        Log.d(TAG, "setBackgroundBehavior() continueInBackground=$continueInBackground")
        val ret = JSObject()
        ret.put("success", true)
        invoke.resolve(ret)
    }

    @Command
    fun setupEventRelay(invoke: Invoke) {
        val args = invoke.parseArgs(SetupEventRelayArgs::class.java)
        eventChannel = args.channel
        Log.d(TAG, "setupEventRelay() channel registered")
        invoke.resolve()
    }

    /**
     * Emit a TTS event via the Rust relay channel.
     * Rust receives it and re-emits via app.emit("tts://<eventType>") so that
     * JS listen("tts://speech:finish") works uniformly on every platform.
     */
    private fun emitEvent(
        eventType: String,
        id: String? = null,
        error: String? = null,
        interrupted: Boolean? = null,
        reason: String? = null
    ) {
        if (eventChannel == null) {
            Log.w(TAG, "emitEvent($eventType) — eventChannel is NULL, register_listener was not called yet")
            return
        }
        val data = JSObject()
        data.put("eventType", eventType)
        id?.let { data.put("id", it) }
        error?.let { data.put("error", it) }
        interrupted?.let { data.put("interrupted", it) }
        reason?.let { data.put("reason", it) }
        eventChannel?.send(data)
    }

    private fun parseLocale(languageTag: String): Locale {
        Log.d(TAG, "parseLocale($languageTag)")
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            Locale.forLanguageTag(languageTag)
        } else {
            val parts = languageTag.split("-", "_")
            when (parts.size) {
                1 -> Locale(parts[0])
                2 -> Locale(parts[0], parts[1])
                else -> Locale(parts[0], parts[1], parts[2])
            }
        }
    }

    fun cleanup() {
        Log.d(TAG, "cleanup() CALLED")
        releaseAudioFocus()
        tts?.stop()
        tts?.shutdown()
        tts = null
        isInitialized = false
        Log.d(TAG, "  TTS resources released")
    }
    
    override fun onPause() {
        super.onPause()
        Log.d(TAG, "onPause() CALLED (continueInBackground=$continueInBackground)")
        isForeground = false
        if (tts?.isSpeaking == true) {
            if (continueInBackground) {
                // Continue speaking — TTS engine runs as a system service in background.
                // No event emitted: speech is not paused, no state change to report.
                Log.d(TAG, "  App going to background while speaking — continuing in background")
            } else {
                // User opted out of background audio — pause and notify JS.
                Log.d(TAG, "  App going to background while speaking — pausing (continueInBackground=false)")
                pauseSpeakingInternal()
                emitEvent("speech:backgroundPause", reason = "app_paused")
            }
        }
    }
    
    override fun onResume() {
        super.onResume()
        Log.d(TAG, "onResume() CALLED")
        isForeground = true
    }
    
    override fun onDestroy() {
        Log.d(TAG, "onDestroy() CALLED")
        super.onDestroy()
        cleanup()
    }
}
