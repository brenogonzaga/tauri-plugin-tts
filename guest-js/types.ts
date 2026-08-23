import type { QueueMode } from "./bindings/QueueMode";

export type { QueueMode } from "./bindings/QueueMode";
export type { Voice } from "./bindings/Voice";
export type { SpeakOptions } from "./bindings/SpeakOptions";
export type { SpeakResponse } from "./bindings/SpeakResponse";
export type { PreviewVoiceOptions } from "./bindings/PreviewVoiceOptions";
export type { PauseResumeResponse } from "./bindings/PauseResumeResponse";

/** Maximum `text` length in UTF-8 bytes. Longer input is rejected with `TEXT_TOO_LONG`. */
export const MAX_TEXT_LENGTH = 10_000;

/** Accepted `rate`, where 1.0 is the platform's normal speed. */
export const RATE_RANGE = [0.1, 4.0] as const;
/** Accepted `pitch`, where 1.0 is the platform's normal pitch. */
export const PITCH_RANGE = [0.5, 2.0] as const;
/** Accepted `volume`. */
export const VOLUME_RANGE = [0.0, 1.0] as const;

export const DEFAULT_QUEUE_MODE: QueueMode = "flush";

export type TtsErrorCode =
  | "IO_ERROR"
  | "PLUGIN_INVOKE_ERROR"
  | "TTS_ENGINE_ERROR"
  | "MUTEX_POISONED"
  | "OPERATION_FAILED"
  | "EMPTY_TEXT"
  | "TEXT_TOO_LONG"
  | "VOICE_ID_TOO_LONG"
  | "INVALID_VOICE_ID"
  | "LANGUAGE_TOO_LONG";

export interface TtsError {
  code: TtsErrorCode;
  message: string;
}

export function isTtsError(error: unknown): error is TtsError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error
  );
}

export type SpeechEventType =
  | "speech:start"
  | "speech:finish"
  | "speech:cancel"
  | "speech:error"
  | "speech:pause"
  | "speech:resume"
  | "speech:interrupted"
  | "speech:backgroundPause";

export interface SpeechEvent {
  eventType?: SpeechEventType;
  /** Matches the `utteranceId` returned by `speak()`, when the platform can attribute it. */
  id?: string;
  /** Set on `speech:error`. */
  error?: string;
  /** Set on `speech:cancel` on Android. */
  interrupted?: boolean;
  /** Why an automatic event fired, e.g. `audio_focus_lost`, `route_change`. */
  reason?: string;
}
