/**
 * Native text-to-speech for Tauri 2.
 *
 * @module
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { PauseResumeResponse } from "./bindings/PauseResumeResponse";
import type { PreviewVoiceOptions } from "./bindings/PreviewVoiceOptions";
import type { SpeakOptions } from "./bindings/SpeakOptions";
import type { SpeakResponse } from "./bindings/SpeakResponse";
import type { Voice } from "./bindings/Voice";
import {
  DEFAULT_QUEUE_MODE,
  type SpeechEvent,
  type SpeechEventType,
} from "./types";

export * from "./types";

/**
 * `JSON.stringify` turns `NaN` and `Infinity` into `null`, which would read as "unset". A
 * slider that momentarily reads `NaN` should fall back to the documented default here,
 * where the intent is still visible.
 */
function finiteOr(value: number | null | undefined, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

let relayRegistration: Promise<void> | null = null;

/**
 * Registers the native event relay exactly once per page load. On mobile this hands a
 * channel to the native plugin; on desktop it resolves immediately.
 */
function ensureRelayRegistered(): Promise<void> {
  relayRegistration ??= invoke<void>("plugin:tts|register_listener").catch(
    (error) => {
      relayRegistration = null;
      throw error;
    },
  );
  return relayRegistration;
}

/**
 * Speaks `text`, resolving once the engine accepts the utterance.
 *
 * Check `warning` on the result: an uninstalled voice still speaks, using the system
 * default. Use `utteranceId` to match this call against its lifecycle events.
 *
 * @throws A {@link TtsError} if validation fails or the engine refuses the request.
 *
 * @example
 * ```typescript
 * await speak({ text: "Hello, world!" });
 * await speak({ text: "Olá!", language: "pt-BR", rate: 1.2 });
 *
 * const { warning } = await speak({ text: "Hi", voiceId: savedId });
 * if (warning) console.warn(warning);
 * ```
 */
export async function speak(options: SpeakOptions): Promise<SpeakResponse> {
  return invoke<SpeakResponse>("plugin:tts|speak", {
    payload: {
      text: options.text,
      language: options.language ?? null,
      voiceId: options.voiceId ?? null,
      rate: finiteOr(options.rate, 1.0),
      pitch: finiteOr(options.pitch, 1.0),
      volume: finiteOr(options.volume, 1.0),
      queueMode: options.queueMode ?? DEFAULT_QUEUE_MODE,
    },
  });
}

/** Stops the current utterance and clears anything queued behind it. */
export async function stop(): Promise<void> {
  await invoke("plugin:tts|stop");
}

/**
 * Lists installed voices, optionally filtered by locale prefix: `"pt"` matches both `pt-BR`
 * and `pt-PT`.
 */
export async function getVoices(language?: string): Promise<Voice[]> {
  const { voices } = await invoke<{ voices: Voice[] }>("plugin:tts|get_voices", {
    payload: { language: language ?? null },
  });
  return voices;
}

/**
 * Speaks a short sample in `voiceId`, at default rate, pitch and volume.
 *
 * Resolves with `success: false` when the voice is not installed. Unlike {@link speak} it
 * does not fall back to another voice, since hearing this one is the point of the call.
 */
export async function previewVoice(
  options: PreviewVoiceOptions,
): Promise<SpeakResponse> {
  return invoke<SpeakResponse>("plugin:tts|preview_voice", {
    payload: {
      voiceId: options.voiceId,
      text: options.text ?? null,
    },
  });
}

export async function isSpeaking(): Promise<boolean> {
  const { speaking } = await invoke<{ speaking: boolean }>(
    "plugin:tts|is_speaking",
  );
  return speaking;
}

/**
 * Reports whether the engine is ready. Mobile initializes asynchronously, so poll this
 * before calling {@link getVoices} on a cold start.
 *
 * @example
 * ```typescript
 * async function waitForTts(attempts = 10): Promise<boolean> {
 *   for (let i = 0; i < attempts; i++) {
 *     const { initialized, voiceCount } = await isInitialized();
 *     if (initialized && voiceCount > 0) return true;
 *     await new Promise((resolve) => setTimeout(resolve, 500));
 *   }
 *   return false;
 * }
 * ```
 */
export async function isInitialized(): Promise<{
  initialized: boolean;
  voiceCount: number;
}> {
  return invoke("plugin:tts|is_initialized");
}

/**
 * Pauses the current utterance. iOS only; desktop and Android resolve with
 * `success: false` and a `reason`, since neither engine can pause.
 */
export async function pauseSpeaking(): Promise<PauseResumeResponse> {
  return invoke<PauseResumeResponse>("plugin:tts|pause_speaking");
}

/** Resumes a paused utterance. iOS only — see {@link pauseSpeaking}. */
export async function resumeSpeaking(): Promise<PauseResumeResponse> {
  return invoke<PauseResumeResponse>("plugin:tts|resume_speaking");
}

/**
 * Controls what happens when the app backgrounds or the screen locks.
 *
 * With `false`, speech stops and `speech:backgroundPause` is emitted. Mobile only; desktop
 * resolves without doing anything.
 */
export async function setBackgroundBehavior(options: {
  continueInBackground: boolean;
}): Promise<void> {
  await invoke("plugin:tts|set_background_behavior", {
    payload: { continueInBackground: options.continueInBackground },
  });
}

/**
 * Subscribes to a lifecycle event. Call the returned function to stop listening.
 *
 * @example
 * ```typescript
 * const unlisten = await onSpeechEvent("speech:finish", (event) => {
 *   console.log("finished", event.id);
 * });
 * ```
 */
export async function onSpeechEvent(
  eventType: SpeechEventType,
  handler: (event: SpeechEvent) => void,
): Promise<UnlistenFn> {
  await ensureRelayRegistered();
  return listen<SpeechEvent>(`tts://${eventType}`, (event) =>
    handler(event.payload),
  );
}
