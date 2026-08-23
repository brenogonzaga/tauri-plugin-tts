package com.tts

import android.speech.tts.TextToSpeech
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Covers the one piece that cannot run on the JVM: turning the platform's own
 * [android.speech.tts.Voice] into the [VoiceInfo] the rest of the plugin works with.
 *
 * Everything downstream of the mapping — filtering, fallbacks, ordering — is exercised by
 * the unit tests in `src/test`.
 */
@RunWith(AndroidJUnit4::class)
class VoiceMappingTest {

    private lateinit var engine: TextToSpeech

    @Before
    fun initializeEngine() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val ready = CountDownLatch(1)

        engine = TextToSpeech(context) { status ->
            if (status == TextToSpeech.SUCCESS) ready.countDown()
        }

        assertTrue(
            "no usable TTS engine on this device",
            ready.await(10, TimeUnit.SECONDS),
        )
    }

    @After
    fun shutdownEngine() {
        engine.shutdown()
    }

    @Test
    fun mappedVoicesKeepTheFieldsSelectionDependsOn() {
        val voices = engine.voices.orEmpty().map { VoiceCatalog.from(it, engine) }
        assertTrue("engine reported no voices", voices.isNotEmpty())

        for (voice in voices) {
            assertTrue("empty id", voice.id.isNotEmpty())
            assertTrue("empty language tag for ${voice.id}", voice.languageTag.isNotEmpty())
            assertTrue("no display name for ${voice.id}", voice.displayName().isNotEmpty())
        }
    }

    @Test
    fun idsMatchWhatTheEngineAcceptsBack() {
        val voices = engine.voices.orEmpty().map { VoiceCatalog.from(it, engine) }
        val listable = VoiceCatalog.listable(voices, null)

        for (voice in listable) {
            val native = engine.voices.orEmpty().firstOrNull { it.name == voice.id }
            assertTrue("id ${voice.id} does not resolve back to a voice", native != null)
        }
    }

    @Test
    fun listableIsASubsetOrderedLocalFirst() {
        val voices = engine.voices.orEmpty().map { VoiceCatalog.from(it, engine) }
        val listable = VoiceCatalog.listable(voices, null)

        assertTrue(listable.size <= voices.size)
        assertEquals(listable.map { it.id }.distinct().size, listable.size)

        val firstNetwork = listable.indexOfFirst { it.requiresNetwork }
        if (firstNetwork >= 0) {
            assertTrue(
                "a local voice was sorted after a network one",
                listable.drop(firstNetwork).all { it.requiresNetwork },
            )
        }
    }

    @Test
    fun filteringByTheEnginesOwnLocaleReturnsThatVoice() {
        val voices = engine.voices.orEmpty().map { VoiceCatalog.from(it, engine) }
        val sample = VoiceCatalog.listable(voices, null).firstOrNull() ?: return

        val language = sample.languageTag.substringBefore("-")
        val matches = VoiceCatalog.listable(voices, language)

        assertTrue("filtering by $language dropped its own voice", matches.contains(sample))
    }
}
