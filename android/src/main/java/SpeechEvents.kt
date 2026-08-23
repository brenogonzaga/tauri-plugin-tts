package com.tts

import android.util.Log
import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject

object SpeechEvent {
    const val START = "speech:start"
    const val FINISH = "speech:finish"
    const val CANCEL = "speech:cancel"
    const val ERROR = "speech:error"
    const val INTERRUPTED = "speech:interrupted"
    const val BACKGROUND_PAUSE = "speech:backgroundPause"
}

/**
 * Forwards lifecycle events to Rust, which re-emits them as `tts://<eventType>` so JS sees
 * the same shape as on desktop.
 *
 * Also deduplicates: the [UtteranceProgressListener] and [ProgressPoller] both report the
 * same utterance, and whichever arrives first wins.
 *
 * Deduplication is keyed by utterance ID rather than a flag, because under `QUEUE_FLUSH` the
 * outgoing utterance's stop callback fires *after* its replacement was queued; a shared flag
 * was consumed by the cancelled utterance and swallowed the replacement's finish event.
 */
class SpeechEventRelay {
    @Volatile private var channel: Channel? = null
    @Volatile private var startedId: String? = null
    @Volatile private var finishedId: String? = null

    fun connect(channel: Channel) {
        this.channel = channel
    }

    /** Emits [SpeechEvent.START] at most once for [utteranceId]. */
    fun start(utteranceId: String?) {
        val id = utteranceId ?: return
        if (id == startedId) return

        startedId = id
        emit(SpeechEvent.START, id = id)
    }

    fun hasStarted(utteranceId: String): Boolean = utteranceId == startedId

    fun hasFinished(utteranceId: String): Boolean = utteranceId == finishedId

    /**
     * Emits a terminal event (finish, cancel or error) at most once for [utteranceId].
     *
     * @return true when this call was the one that emitted it.
     */
    fun finish(
        utteranceId: String?,
        eventType: String,
        error: String? = null,
        interrupted: Boolean? = null,
    ): Boolean {
        val id = utteranceId ?: return false
        if (id == finishedId) return false

        finishedId = id
        emit(eventType, id = id, error = error, interrupted = interrupted)
        return true
    }

    /** Reports something that is not tied to one utterance, such as an audio focus change. */
    fun emit(
        eventType: String,
        id: String? = null,
        error: String? = null,
        interrupted: Boolean? = null,
        reason: String? = null,
    ) {
        val target = channel
        if (target == null) {
            Log.w(TAG, "dropped $eventType: register_listener has not run yet")
            return
        }

        val payload = JSObject().apply {
            put("eventType", eventType)
            id?.let { put("id", it) }
            error?.let { put("error", it) }
            interrupted?.let { put("interrupted", it) }
            reason?.let { put("reason", it) }
        }
        target.send(payload)
    }

    /** Forgets the deduplication state; a replacement engine starts over. */
    fun reset() {
        startedId = null
        finishedId = null
    }

    private companion object {
        const val TAG = "TtsPlugin"
    }
}
