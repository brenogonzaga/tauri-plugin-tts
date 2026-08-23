package com.tts

/** Maximum `text` length, in UTF-8 bytes. Mirrors `MAX_TEXT_LENGTH` in the Rust plugin. */
const val MAX_TEXT_LENGTH = 10_000

/** Maximum `voiceId` length, in characters. */
const val MAX_VOICE_ID_LENGTH = 256

/** Maximum `language` length, in characters. */
const val MAX_LANGUAGE_LENGTH = 35

/**
 * Second line of defence behind the shared Rust validation, which every call already passes
 * through. Returns the reason the input is unusable, or null when it is fine.
 */
object Validation {

    fun text(text: String): String? {
        if (text.isEmpty()) return "Text cannot be empty"

        val bytes = text.toByteArray(Charsets.UTF_8).size
        if (bytes > MAX_TEXT_LENGTH) {
            return "Text too long: $bytes bytes (max: $MAX_TEXT_LENGTH)"
        }
        return null
    }

    fun voiceId(voiceId: String): String? {
        if (voiceId.length > MAX_VOICE_ID_LENGTH) {
            return "Voice ID too long: ${voiceId.length} chars (max: $MAX_VOICE_ID_LENGTH)"
        }
        if (voiceId.any { it.isISOControl() }) {
            return "Invalid voice ID - control characters are not allowed"
        }
        return null
    }

    fun language(language: String): String? {
        if (language.length > MAX_LANGUAGE_LENGTH) {
            return "Language code too long: ${language.length} chars (max: $MAX_LANGUAGE_LENGTH)"
        }
        return null
    }
}
