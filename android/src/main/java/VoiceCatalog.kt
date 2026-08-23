package com.tts

import android.speech.tts.TextToSpeech
import android.speech.tts.Voice

/**
 * One installed voice, decoupled from [android.speech.tts.Voice] so the selection and
 * filtering rules can be exercised without the Android framework.
 */
data class VoiceInfo(
    val id: String,
    val languageTag: String,
    val displayLanguage: String,
    val displayCountry: String,
    val requiresNetwork: Boolean,
    val languageAvailable: Boolean,
    val features: Set<String> = emptySet(),
) {
    /**
     * Whether this voice can actually speak on this device.
     *
     * Network voices always can. Local ones are excluded when the language data is missing,
     * when the engine flags them as not installed, or when they are Google TTS
     * "-language" routing stubs — those report as local and high quality, but `speak()`
     * fails silently with no callbacks when the pack is not downloaded.
     */
    fun isUsable(): Boolean = when {
        requiresNetwork -> true
        !languageAvailable -> false
        features.contains(TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED) -> false
        id.endsWith("-language") -> false
        else -> true
    }

    /**
     * A readable label, since Android exposes no human name for a voice.
     */
    fun displayName(): String =
        listOfNotNull(localeLabel(), variantLabel(), connectionLabel()).joinToString(" · ")

    private fun localeLabel(): String =
        if (displayCountry.isEmpty()) displayLanguage else "$displayLanguage ($displayCountry)"

    private fun connectionLabel(): String = if (requiresNetwork) "Network" else "Local"

    /**
     * Whatever is left of the ID once the locale and the connection suffix are removed —
     * the variant and gender that actually tell two voices of the same locale apart.
     */
    private fun variantLabel(): String? =
        id.removeSuffixIgnoringCase("-local")
            .removeSuffixIgnoringCase("-network")
            .removePrefixIgnoringCase(languageTag)
            .removePrefix("-")
            .removePrefixIgnoringCase("x-")
            .replace('#', ' ')
            .replace('_', ' ')
            .split(" ")
            .filter { it.isNotBlank() }
            .joinToString(" ") { word ->
                // Only all-lowercase words are capitalized, so engine acronyms such as
                // "SMTf00" survive intact.
                if (word == word.lowercase()) word.replaceFirstChar(Char::uppercaseChar) else word
            }
            .ifBlank { null }
}

private fun String.removePrefixIgnoringCase(prefix: String): String =
    if (startsWith(prefix, ignoreCase = true)) substring(prefix.length) else this

private fun String.removeSuffixIgnoringCase(suffix: String): String =
    if (endsWith(suffix, ignoreCase = true)) dropLast(suffix.length) else this

/** Selection, filtering and ordering of installed voices. */
object VoiceCatalog {

    fun from(voice: Voice, engine: TextToSpeech): VoiceInfo = VoiceInfo(
        id = voice.name,
        languageTag = voice.locale.toLanguageTag(),
        displayLanguage = voice.locale.displayLanguage,
        displayCountry = voice.locale.country.takeIf { it.isNotEmpty() }
            ?.let { voice.locale.displayCountry }
            .orEmpty(),
        requiresNetwork = voice.isNetworkConnectionRequired,
        languageAvailable = engine.isLanguageAvailable(voice.locale) >= TextToSpeech.LANG_AVAILABLE,
        features = voice.features.orEmpty(),
    )

    /**
     * Whether [filter] is a locale prefix of [languageTag]: both "pt" and "pt-BR" match a
     * "pt-BR" voice, while "US" does not match "en-US".
     */
    fun languageMatches(languageTag: String, filter: String): Boolean =
        languageTag.lowercase().startsWith(filter.lowercase())

    /**
     * The voices to show for [filter]: usable ones only, local before network, deduplicated
     * by ID and ordered so the list is stable between calls.
     */
    fun listable(voices: Collection<VoiceInfo>, filter: String?): List<VoiceInfo> =
        voices
            .filter { it.isUsable() }
            .filter { filter == null || languageMatches(it.languageTag, filter) }
            .sortedWith(
                compareBy(
                    { if (it.requiresNetwork) 1 else 0 },
                    { it.languageTag },
                    { it.id },
                )
            )
            .distinctBy { it.id }

    /**
     * The best local voice for a voice ID the engine no longer knows, matched on the ID's
     * language prefix. Returns null when nothing comparable is installed.
     */
    fun fallbackFor(voices: Collection<VoiceInfo>, missingId: String): VoiceInfo? {
        val parts = missingId.split("-")
        val prefix = if (parts.size >= 2) "${parts[0]}-${parts[1]}" else parts[0]

        return voices.firstOrNull {
            !it.requiresNetwork && languageMatches(it.languageTag, prefix)
        }
    }

    /** The voice to use when none was requested and the engine has none set. */
    fun defaultVoice(voices: Collection<VoiceInfo>): VoiceInfo? =
        voices.filter { !it.requiresNetwork }.minByOrNull { it.languageTag }
}
