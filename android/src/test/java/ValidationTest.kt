package com.tts

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ValidationTest {

    @Test
    fun `empty text is rejected`() {
        assertEquals("Text cannot be empty", Validation.text(""))
    }

    @Test
    fun `text is measured in utf8 bytes not characters`() {
        assertNull(Validation.text("a".repeat(MAX_TEXT_LENGTH)))
        assertTrue(Validation.text("a".repeat(MAX_TEXT_LENGTH + 1))!!.contains("too long"))

        // A two-byte character reaches the byte limit at half the character count.
        assertTrue(Validation.text("\u00e9".repeat(MAX_TEXT_LENGTH / 2 + 1))!!.contains("too long"))
    }

    @Test
    fun `voice ids allow the punctuation real engines use`() {
        val ids = listOf(
            "pt-br-x-afs#female_1-local",
            "en-US-SMTf00",
            "com.apple.voice.enhanced.pt-BR",
        )
        for (id in ids) assertNull(id, Validation.voiceId(id))
    }

    @Test
    fun `voice ids reject control characters`() {
        assertTrue(Validation.voiceId("voice\u0000id")!!.contains("control characters"))
    }

    @Test
    fun `voice ids reject overlong input`() {
        assertNull(Validation.voiceId("v".repeat(MAX_VOICE_ID_LENGTH)))
        assertTrue(Validation.voiceId("v".repeat(MAX_VOICE_ID_LENGTH + 1))!!.contains("too long"))
    }

    @Test
    fun `language is measured in characters`() {
        assertNull(Validation.language("x".repeat(MAX_LANGUAGE_LENGTH)))
        assertTrue(Validation.language("x".repeat(MAX_LANGUAGE_LENGTH + 1))!!.contains("too long"))
    }
}

class SpeakArgsTest {

    private fun args(build: SpeakArgs.() -> Unit = {}) = SpeakArgs().apply {
        text = "Hello"
        build()
    }

    @Test
    fun `validation reports the first problem it finds`() {
        assertNull(args().validate())
        assertEquals("Text cannot be empty", args { text = "" }.validate())
        assertTrue(args { voiceId = "bad\u0000id" }.validate()!!.contains("control characters"))
        assertTrue(
            args { language = "x".repeat(MAX_LANGUAGE_LENGTH + 1) }.validate()!!
                .contains("too long")
        )
    }

    @Test
    fun `default and blank mean no selection`() {
        assertNull(args { voiceId = "default" }.requestedVoiceId())
        assertNull(args { voiceId = "   " }.requestedVoiceId())
        assertNull(args { voiceId = null }.requestedVoiceId())
        assertEquals("en-US-x-1", args { voiceId = "en-US-x-1" }.requestedVoiceId())

        assertNull(args { language = "default" }.requestedLanguage())
        assertEquals("pt-BR", args { language = "pt-BR" }.requestedLanguage())
    }

    @Test
    fun `queue mode defaults to flush and is case insensitive`() {
        assertTrue(args().flushes())
        assertTrue(args { queueMode = "flush" }.flushes())
        assertTrue(args { queueMode = "unrecognised" }.flushes())
        assertFalse(args { queueMode = "add" }.flushes())
        assertFalse(args { queueMode = "ADD" }.flushes())
    }
}

class PreviewVoiceArgsTest {

    @Test
    fun `preview falls back to the built-in sample`() {
        val preview = PreviewVoiceArgs().apply { voiceId = "voice" }
        assertTrue(preview.sampleText().isNotEmpty())
        assertNull(preview.validate())

        preview.text = "Custom"
        assertEquals("Custom", preview.sampleText())
    }

    @Test
    fun `preview rejects an empty custom sample`() {
        val preview = PreviewVoiceArgs().apply {
            voiceId = "voice"
            text = ""
        }
        assertEquals("Text cannot be empty", preview.validate())
    }
}
