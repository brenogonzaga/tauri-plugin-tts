package com.tts

import android.speech.tts.TextToSpeech
import android.os.Handler
import android.os.Looper

private const val POLL_INTERVAL_MS = 100L
private const val FINISH_DEBOUNCE_POLLS = 15
private const val START_TIMEOUT_MS = 10_000L

/**
 * Reports start and finish by watching [TextToSpeech.isSpeaking], for engines where
 * `UtteranceProgressListener` never fires — a known problem with Google TTS on emulators.
 *
 * Both paths go through [SpeechEventRelay], so exactly one wins each event.
 *
 * Finish detection is debounced: on Android 14+ `isSpeaking()` returns false as soon as
 * synthesis reaches the audio buffer, before playback ends. A plain `!isSpeaking` check
 * reported long utterances as finished while they were still audible, so the caller stopped
 * or replaced audio that was still playing.
 *
 * Only usable under `QUEUE_FLUSH`: `isSpeaking` cannot say *which* utterance is speaking, so
 * under `QUEUE_ADD` it reports the predecessor and would announce the start of an utterance
 * that has not begun. Queued utterances rely on the listener alone.
 */
class ProgressPoller(
    private val handler: Handler = Handler(Looper.getMainLooper()),
) {
    fun watch(
        engine: TextToSpeech,
        utteranceId: String,
        events: SpeechEventRelay,
        isCurrent: () -> Boolean,
    ) {
        val startedAt = System.currentTimeMillis()
        var quietPolls = 0

        val poll = object : Runnable {
            override fun run() {
                if (!isCurrent() || events.hasFinished(utteranceId)) return

                val speaking = engine.isSpeaking

                if (!events.hasStarted(utteranceId)) {
                    if (speaking) {
                        events.start(utteranceId)
                    } else if (System.currentTimeMillis() - startedAt > START_TIMEOUT_MS) {
                        events.finish(
                            utteranceId,
                            SpeechEvent.ERROR,
                            error = "TTS engine did not start speaking after " +
                                "${START_TIMEOUT_MS / 1000} seconds",
                        )
                        return
                    }
                }

                if (events.hasStarted(utteranceId)) {
                    if (speaking) {
                        quietPolls = 0
                    } else if (++quietPolls >= FINISH_DEBOUNCE_POLLS) {
                        events.finish(utteranceId, SpeechEvent.FINISH)
                        return
                    }
                }

                handler.postDelayed(this, POLL_INTERVAL_MS)
            }
        }

        handler.postDelayed(poll, POLL_INTERVAL_MS)
    }
}
