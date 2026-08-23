package com.tts

import android.speech.tts.TextToSpeech
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

private fun voice(
    id: String,
    languageTag: String = "en-US",
    requiresNetwork: Boolean = false,
    languageAvailable: Boolean = true,
    features: Set<String> = emptySet(),
) = VoiceInfo(
    id = id,
    languageTag = languageTag,
    displayLanguage = "English",
    displayCountry = "United States",
    requiresNetwork = requiresNetwork,
    languageAvailable = languageAvailable,
    features = features,
)

private fun ptBr(id: String, requiresNetwork: Boolean = false) = VoiceInfo(
    id = id,
    languageTag = "pt-BR",
    displayLanguage = "Portuguese",
    displayCountry = "Brazil",
    requiresNetwork = requiresNetwork,
    languageAvailable = true,
)

class VoiceUsabilityTest {

    @Test
    fun `network voices are always usable`() {
        assertTrue(voice("v", requiresNetwork = true, languageAvailable = false).isUsable())
    }

    @Test
    fun `local voices need their language data`() {
        assertTrue(voice("v").isUsable())
        assertFalse(voice("v", languageAvailable = false).isUsable())
    }

    @Test
    fun `voices flagged as not installed are excluded`() {
        val flagged = voice("v", features = setOf(TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED))
        assertFalse(flagged.isUsable())
    }

    /** Google TTS routing stubs look local and high quality but speak silently. */
    @Test
    fun `language routing stubs are excluded`() {
        assertFalse(voice("en-US-language").isUsable())
        assertTrue(voice("en-US-language-local").isUsable())
    }

    @Test
    fun `display names carry the variant that tells voices apart`() {
        assertEquals(
            "Portuguese (Brazil) · Afs Female 1 · Local",
            ptBr("pt-br-x-afs#female_1-local").displayName(),
        )
        assertEquals(
            "Portuguese (Brazil) · Pte · Network",
            ptBr("pt-br-x-pte-network", requiresNetwork = true).displayName(),
        )
    }

    /** Engine acronyms are not lowercase words and must survive verbatim. */
    @Test
    fun `display names preserve engine acronyms`() {
        assertEquals(
            "English (United States) · SMTf00 · Local",
            voice("en-US-SMTf00").displayName(),
        )
    }

    @Test
    fun `display names fall back to the locale when the id adds nothing`() {
        assertEquals("English (United States) · Local", voice("en-US").displayName())
        assertEquals("English (United States) · Local", voice("en-us-local").displayName())
    }

    @Test
    fun `display names omit an empty country`() {
        val info = VoiceInfo(
            id = "eo-x-vfl",
            languageTag = "eo",
            displayLanguage = "Esperanto",
            displayCountry = "",
            requiresNetwork = false,
            languageAvailable = true,
        )

        assertEquals("Esperanto · Vfl · Local", info.displayName())
    }

    /**
     * The reason this matters: a device carries several voices per locale, and building the
     * label from the locale alone rendered every one of them as the same string.
     */
    @Test
    fun `voices of the same locale get distinct names`() {
        val installed = listOf(
            ptBr("pt-br-x-afs#female_1-local"),
            ptBr("pt-br-x-afs#female_2-local"),
            ptBr("pt-br-x-afs#male_1-local"),
            ptBr("pt-br-x-pte#female_3-local"),
            ptBr("pt-br-x-pte-network", requiresNetwork = true),
        )

        val names = VoiceCatalog.listable(installed, null).map { it.displayName() }

        assertEquals(installed.size, names.size)
        assertEquals("labels collide: $names", names.size, names.distinct().size)
    }
}

class VoiceFilteringTest {

    @Test
    fun `a filter matches bare and full locale tags`() {
        assertTrue(VoiceCatalog.languageMatches("pt-BR", "pt"))
        assertTrue(VoiceCatalog.languageMatches("pt-BR", "pt-BR"))
        assertTrue(VoiceCatalog.languageMatches("pt-BR", "PT-br"))
        assertFalse(VoiceCatalog.languageMatches("en-US", "pt"))
        assertFalse(VoiceCatalog.languageMatches("pt-BR", "pt-PT"))
    }

    /** A substring match made a "US" filter return the en-US voices. */
    @Test
    fun `filtering matches by prefix not substring`() {
        val voices = listOf(
            voice("a", "en-US"),
            voice("b", "pt-BR"),
            voice("c", "pt-PT"),
        )

        assertEquals(3, VoiceCatalog.listable(voices, null).size)
        assertEquals(listOf("b", "c"), VoiceCatalog.listable(voices, "pt").map { it.id })
        assertEquals(listOf("b"), VoiceCatalog.listable(voices, "pt-BR").map { it.id })
        assertTrue(VoiceCatalog.listable(voices, "US").isEmpty())
    }

    @Test
    fun `unusable voices never reach the list`() {
        val voices = listOf(voice("ok"), voice("missing", languageAvailable = false))

        assertEquals(listOf("ok"), VoiceCatalog.listable(voices, null).map { it.id })
    }

    @Test
    fun `local voices come before network voices`() {
        val voices = listOf(
            voice("net", "en-US", requiresNetwork = true),
            voice("local", "en-US"),
        )

        assertEquals(listOf("local", "net"), VoiceCatalog.listable(voices, null).map { it.id })
    }

    @Test
    fun `duplicate ids are collapsed`() {
        val voices = listOf(voice("same"), voice("same"), voice("other"))

        assertEquals(listOf("other", "same"), VoiceCatalog.listable(voices, null).map { it.id })
    }
}

class VoiceFallbackTest {

    @Test
    fun `an unknown id falls back to a local voice in the same locale`() {
        val voices = listOf(voice("pt-br-x-afs", "pt-BR"), voice("en-us-x-sfg", "en-US"))

        val fallback = VoiceCatalog.fallbackFor(voices, "pt-br-x-removed")
        assertEquals("pt-br-x-afs", fallback?.id)
    }

    @Test
    fun `network voices are not used as a fallback`() {
        val voices = listOf(voice("pt-br-net", "pt-BR", requiresNetwork = true))

        assertNull(VoiceCatalog.fallbackFor(voices, "pt-br-x-removed"))
    }

    @Test
    fun `no comparable voice means no fallback`() {
        assertNull(VoiceCatalog.fallbackFor(listOf(voice("en-us-x", "en-US")), "ja-jp-x"))
        assertNull(VoiceCatalog.fallbackFor(emptyList(), "pt-br-x"))
    }

    @Test
    fun `the default voice is the first local one by locale`() {
        val voices = listOf(
            voice("pt", "pt-BR"),
            voice("en", "en-US"),
            voice("net", "ar-AE", requiresNetwork = true),
        )

        assertEquals("en", VoiceCatalog.defaultVoice(voices)?.id)
        assertNull(VoiceCatalog.defaultVoice(emptyList()))
    }
}
