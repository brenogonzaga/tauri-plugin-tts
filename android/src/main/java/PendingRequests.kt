package com.tts

import app.tauri.plugin.Invoke

/** How long a queued request waits for the engine before it is rejected. */
const val PENDING_TIMEOUT_MS = 30_000L

/** Maximum number of requests held while the engine initializes. */
const val MAX_PENDING_REQUESTS = 50

data class PendingSpeak(val invoke: Invoke, val args: SpeakArgs)

/**
 * Requests received before the engine finished initializing.
 *
 * Every entry is answered exactly once: either it is dispatched from `onInit`, or it is
 * rejected — by the timeout armed when it was queued, or because initialization failed. A
 * queue drained only from `onInit` left the JS promise pending forever whenever `onInit`
 * reported failure or never arrived.
 */
class PendingRequests(private val schedule: (Long, () -> Unit) -> Unit) {
    private val queue = java.util.concurrent.ConcurrentLinkedQueue<PendingSpeak>()

    val size: Int get() = queue.size

    /** @return false when the queue is full, in which case nothing was enqueued. */
    fun add(invoke: Invoke, args: SpeakArgs): Boolean {
        if (queue.size >= MAX_PENDING_REQUESTS) return false

        val pending = PendingSpeak(invoke, args)
        queue.add(pending)
        schedule(PENDING_TIMEOUT_MS) {
            // remove() only succeeds while it is still waiting; a dispatched or already
            // rejected request is gone from the queue and this does nothing.
            if (queue.remove(pending)) {
                pending.invoke.reject("Timed out waiting for TTS initialization")
            }
        }
        return true
    }

    fun drain(consume: (PendingSpeak) -> Unit) {
        while (true) {
            consume(queue.poll() ?: return)
        }
    }

    fun rejectAll(reason: String) {
        drain { it.invoke.reject(reason) }
    }
}
