import { invoke, addPluginListener } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Voice } from "./bindings/Voice";
import type { PauseResumeResponse } from "./bindings/PauseResumeResponse";
import type { SpeakOptions } from "./bindings/SpeakOptions";
import type { PreviewVoiceOptions } from "./bindings/PreviewVoiceOptions";

export type { QueueMode } from "./bindings/QueueMode";
export type { Voice } from "./bindings/Voice";
export type { PauseResumeResponse } from "./bindings/PauseResumeResponse";
export type { SpeakOptions } from "./bindings/SpeakOptions";
export type { PreviewVoiceOptions } from "./bindings/PreviewVoiceOptions";

export type TtsErrorCode =
  | "IO_ERROR"
  | "PLUGIN_INVOKE_ERROR"
  | "TTS_ENGINE_ERROR"
  | "LOCK_ERROR"
  | "MUTEX_POISONED"
  | "NOT_INITIALIZED"
  | "VALIDATION_ERROR"
  | "OPERATION_FAILED"
  | "EMPTY_TEXT"
  | "TEXT_TOO_LONG"
  | "VOICE_ID_TOO_LONG"
  | "INVALID_VOICE_ID"
  | "LANGUAGE_TOO_LONG";

export interface TtsError {
  /** Error code for programmatic handling */
  code: TtsErrorCode;
  /** Human-readable error message */
  message: string;
}

export function isTtsError(error: unknown): error is TtsError {
  return typeof error === "object" && error !== null && "code" in error && "message" in error;
}

export interface SpeechEvent {
  /** Unique identifier for the utterance (if available) */
  id?: string;
  /** Event type description */
  eventType?: string;
  /** Error message (for error events) */
  error?: string;
  /** Whether speech was interrupted */
  interrupted?: boolean;
}

export type SpeechEventType =
  | "speech:start"
  | "speech:finish"
  | "speech:cancel"
  | "speech:pause"
  | "speech:resume"
  | "speech:error"
  | "speech:interrupted"
  | "speech:backgroundPause";

/**
 * Listen for TTS speech events
 *
 * @param eventType - The type of speech event to listen for
 * @param callback - Function called when the event occurs
 * @returns Promise that resolves to an unlisten function
 *
 * @example
 * ```typescript
 * import { onSpeechEvent } from "tauri-plugin-tts-api";
 *
 * const unlisten = await onSpeechEvent("speech:finish", (event) => {
 *   console.log("Speech finished:", event.id);
 * });
 *
 * // Later, stop listening
 * unlisten();
 * ```
 */
export async function onSpeechEvent(
  eventType: SpeechEventType,
  callback: (event: SpeechEvent) => void,
): Promise<UnlistenFn> {
  // Desktop: events emitted via Rust app.emit("tts://...")
  const desktopUnlisten = listen<SpeechEvent>(`tts://${eventType}`, (e) => callback(e.payload));

  // Mobile (Android/iOS): events sent via plugin trigger()
  // addPluginListener registers through the plugin Channel system
  const mobileUnlisten = addPluginListener<SpeechEvent>("tts", eventType, callback)
    .then((listener) => (() => listener.unregister()) as UnlistenFn)
    .catch(() => (() => Promise.resolve()) as UnlistenFn);

  const [desktop, mobile] = await Promise.all([desktopUnlisten, mobileUnlisten]);

  return () => {
    desktop();
    mobile();
  };
}

/**
 * Speak the given text using text-to-speech
 *
 * @param options - The speak options including text and optional parameters
 * @returns Promise that resolves when speech has started
 * @throws TtsError if validation fails or TTS operation fails
 *
 * @example
 * ```typescript
 * import { speak } from "tauri-plugin-tts-api";
 *
 * // Simple usage
 * await speak({ text: "Hello, world!" });
 *
 * // With options
 * await speak({
 *   text: "Olá, mundo!",
 *   language: "pt-BR",
 *   rate: 0.8,
 *   pitch: 1.2,
 *   volume: 1.0
 * });
 *
 * // Queue mode - add to queue instead of interrupting
 * await speak({ text: "First sentence" });
 * await speak({ text: "Second sentence", queueMode: "add" });
 * ```
 */
export async function speak(options: SpeakOptions): Promise<void> {
  await invoke("plugin:tts|speak", {
    payload: {
      text: options.text,
      language: options.language ?? null,
      voiceId: options.voiceId ?? null,
      rate: options.rate ?? 1.0,
      pitch: options.pitch ?? 1.0,
      volume: options.volume ?? 1.0,
      queueMode: options.queueMode ?? "flush",
    },
  });
}

/**
 * Stop any ongoing speech
 *
 * @example
 * ```typescript
 * import { stop } from "tauri-plugin-tts-api";
 *
 * await stop();
 * ```
 */
export async function stop(): Promise<void> {
  await invoke("plugin:tts|stop");
}

/**
 * Get available voices, optionally filtered by language
 *
 * @param language - Optional language code to filter voices
 * @returns Array of available voices
 *
 * @example
 * ```typescript
 * import { getVoices } from "tauri-plugin-tts-api";
 *
 * // Get all voices
 * const allVoices = await getVoices();
 *
 * // Get only English voices
 * const englishVoices = await getVoices("en");
 *
 * // Get only Brazilian Portuguese voices
 * const ptBrVoices = await getVoices("pt-BR");
 * ```
 */
export async function getVoices(language?: string): Promise<Voice[]> {
  const response = await invoke<{ voices: Voice[] }>("plugin:tts|get_voices", {
    payload: { language: language ?? null },
  });
  return response.voices;
}

/**
 * Check if TTS is currently speaking
 *
 * @returns True if speech is in progress
 *
 * @example
 * ```typescript
 * import { isSpeaking, stop } from "tauri-plugin-tts-api";
 *
 * if (await isSpeaking()) {
 *   await stop();
 * }
 * ```
 */
export async function isSpeaking(): Promise<boolean> {
  const response = await invoke<{ speaking: boolean }>("plugin:tts|is_speaking");
  return response.speaking;
}

/**
 * Check if TTS engine is initialized and ready
 *
 * On mobile platforms, TTS initialization is asynchronous. Use this
 * to wait for the engine to be ready before calling getVoices().
 *
 * @returns Object with initialized status and voice count
 *
 * @example
 * ```typescript
 * import { isInitialized, getVoices } from "tauri-plugin-tts-api";
 *
 * // Wait for TTS to be ready
 * const waitForTts = async () => {
 *   for (let i = 0; i < 10; i++) {
 *     const status = await isInitialized();
 *     if (status.initialized && status.voiceCount > 0) {
 *       return true;
 *     }
 *     await new Promise(r => setTimeout(r, 500));
 *   }
 *   return false;
 * };
 *
 * if (await waitForTts()) {
 *   const voices = await getVoices();
 * }
 * ```
 */
export async function isInitialized(): Promise<{
  initialized: boolean;
  voiceCount: number;
}> {
  return invoke<{ initialized: boolean; voiceCount: number }>("plugin:tts|is_initialized");
}

/**
 * Pause the current speech (iOS only - Android/Desktop not supported)
 *
 * @returns Promise with success status and optional reason
 *
 * @example
 * ```typescript
 * import { pauseSpeaking, resumeSpeaking } from "tauri-plugin-tts-api";
 *
 * const result = await pauseSpeaking();
 * if (result.success) {
 *   // Speech is paused
 *   await resumeSpeaking();
 * } else {
 *   console.log("Cannot pause:", result.reason);
 * }
 * ```
 */
export async function pauseSpeaking(): Promise<PauseResumeResponse> {
  return await invoke<PauseResumeResponse>("plugin:tts|pause_speaking");
}

/**
 * Resume paused speech (iOS only - Android/Desktop not supported)
 *
 * @returns Promise with success status and optional reason
 */
export async function resumeSpeaking(): Promise<PauseResumeResponse> {
  return await invoke<PauseResumeResponse>("plugin:tts|resume_speaking");
}

/**
 * Preview a voice with sample text
 *
 * Useful for letting users hear what a voice sounds like before selecting it.
 * Uses default rate, pitch, and volume settings.
 *
 * @param options - The preview options including voiceId and optional text
 * @returns Promise that resolves when preview has started
 *
 * @example
 * ```typescript
 * import { getVoices, previewVoice } from "tauri-plugin-tts-api";
 *
 * // Get available voices
 * const voices = await getVoices();
 *
 * // Preview a specific voice
 * await previewVoice({ voiceId: voices[0].id });
 *
 * // Preview with custom text
 * await previewVoice({
 *   voiceId: voices[0].id,
 *   text: "Testing this voice!"
 * });
 * ```
 */
export async function previewVoice(options: PreviewVoiceOptions): Promise<void> {
  await invoke("plugin:tts|preview_voice", {
    payload: {
      voiceId: options.voiceId,
      text: options.text ?? null,
    },
  });
}

/**
 * Set whether TTS should continue speaking when the app goes to background or the screen locks.
 *
 * When `continueInBackground` is `true` (default), speech continues uninterrupted.
 * When `false`, speech is paused when the app goes to background and a `speech:backgroundPause`
 * event is emitted.
 *
 * Desktop: no-op (always resolves successfully).
 *
 * @example
 * ```typescript
 * import { setBackgroundBehavior } from "tauri-plugin-tts-api";
 *
 * // Pause speech when screen locks (e.g., app only makes sense in foreground)
 * await setBackgroundBehavior({ continueInBackground: false });
 *
 * // Allow speech to continue in background (default)
 * await setBackgroundBehavior({ continueInBackground: true });
 * ```
 */
export async function setBackgroundBehavior(options: {
  continueInBackground: boolean;
}): Promise<void> {
  await invoke("plugin:tts|set_background_behavior", {
    payload: {
      continueInBackground: options.continueInBackground,
    },
  });
}
