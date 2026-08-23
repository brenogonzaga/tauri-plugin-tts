package com.tts

import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build

/**
 * Holds audio focus for the duration of an utterance.
 *
 * Focus is requested as `AUDIOFOCUS_GAIN_TRANSIENT`, not `AUDIOFOCUS_GAIN`: the Google TTS
 * engine runs as a separate service and requests its own focus to play the synthesized
 * audio. With a permanent request the system answers that with `AUDIOFOCUS_LOSS` to this
 * listener, which stops the very utterance it was granted for.
 */
class AudioFocusController(
    private val audioManager: AudioManager?,
    private val onLoss: (permanent: Boolean, reason: String) -> Unit,
) {
    private var request: AudioFocusRequest? = null

    private val listener = AudioManager.OnAudioFocusChangeListener { change ->
        when (change) {
            AudioManager.AUDIOFOCUS_LOSS ->
                onLoss(true, "audio_focus_lost")
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT ->
                onLoss(false, "audio_focus_transient_loss")
            // Ducking is an option, but speech at a lower volume is not worth hearing.
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK ->
                onLoss(false, "audio_focus_duck")
        }
    }

    fun request(): Boolean {
        val granted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT)
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_ASSISTANT)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build()
                )
                .setOnAudioFocusChangeListener(listener)
                .build()
                .also { request = it }
                .let { audioManager?.requestAudioFocus(it) }
        } else {
            @Suppress("DEPRECATION")
            audioManager?.requestAudioFocus(
                listener,
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN_TRANSIENT,
            )
        }

        return granted == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
    }

    fun release() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            request?.let { audioManager?.abandonAudioFocusRequest(it) }
            request = null
        } else {
            @Suppress("DEPRECATION")
            audioManager?.abandonAudioFocus(listener)
        }
    }
}
