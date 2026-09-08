# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-09-08

### Fixed

- **Android**: Fixed no audio output over Android Auto ([#13](https://github.com/brenogonzaga/tauri-plugin-tts/issues/13)). Thanks [@patarapolw](https://github.com/patarapolw).

## [0.2.0] - 2026-09-07

### Fixed

- **Android**: Fixed a failing TTS engine crashing the host app instead of reporting an init failure ([#14](https://github.com/brenogonzaga/tauri-plugin-tts/issues/14)).
- **TypeScript**: Fixed `SpeakOptions` and `PreviewVoiceOptions` requiring every field, breaking `strict` compilation of `speak({ text })` and `previewVoice({ voiceId })`.
- **Android**: Fixed `speak()` hanging forever instead of rejecting when the TTS engine failed to initialize.
- **Desktop**: Fixed lifecycle events carrying the wrong utterance ID under `queueMode: "add"`.
- **iOS**: Fixed `rate` saturating at `2.0`, making `2.0`–`4.0` sound identical.
- **Desktop**: Fixed `rate` values between `0.1` and `0.25` collapsing onto the platform minimum.
- **All platforms**: `speak()` and `previewVoice()` now resolve with `{ success, warning, utteranceId }` instead of discarding the result, so an unavailable voice can be detected.
- **Desktop**: Fixed a missing `voiceId`/`language` falling back to the default voice without a `warning`.
- **Desktop / iOS**: Fixed `speech:error` never being emitted; it was Android-only.
- **Android**: Fixed audio focus being retained after `speak()`/`previewVoice()` failed before speech started.
- **Android**: Fixed `previewVoice()` emitting no events on engines where `UtteranceProgressListener` never fires.
- **iOS**: Fixed a leaked `utteranceIds` entry per `previewVoice()` call on an uninstalled voice.
- **All platforms**: Fixed `null` `rate`/`pitch`/`volume`/`text` (from `NaN`/`Infinity`) failing with an unclassified error.
- **Desktop**: Fixed an empty voice list being cached for 60s while speech-dispatcher was still starting.
- **iOS**: Fixed speech staying "paused" with no event after an interruption ended without `shouldResume`.
- **Android**: Fixed every voice of a locale reporting the same name.

### Changed

- **macOS**: Synthesizer calls are dispatched to the main thread instead of Tauri's worker threads.
- **Desktop**: `speech:start` now fires from the engine's `on_utterance_begin` callback, not on queueing.
- **TypeScript** (Breaking): `speak()` and `previewVoice()` return `Promise<SpeakResponse>` instead of `Promise<void>`; existing `await` calls that ignore the result keep working.
- Exported `MAX_TEXT_LENGTH`, `RATE_RANGE`, `PITCH_RANGE` and `VOLUME_RANGE` from the JavaScript API.
- **Android**: Removed two full voice enumerations that ran per `speak()` purely for debug logs.

### Internal

- Split each backend into focused modules (Rust: `validation`, `desktop/`; Android: `Validation`, `SpeechArgs`, `VoiceCatalog`, `AudioFocusController`, `SpeechEvents`, `PendingRequests`, `ProgressPoller`; iOS: a `TtsCore` package plus `SpeechArguments`, `AudioSessionController`, `VoiceProvider`, `UtteranceRegistry`, `SpeechEventRelay`).
- Replaced the Android and iOS test suites with tests that exercise the extracted logic instead of asserting literals against literals.

## [0.1.13] - 2026-08-15

### Fixed

- **Android**: Fixed `speech:finish` never being emitted when a `speak()` interrupted another.
- **Android**: Fixed `pauseSpeaking()` returning `success: true` without pausing; speech kept playing through phone calls and through `setBackgroundBehavior({ continueInBackground: false })`.
- **Android**: Fixed `queueMode: "add"` emitting `speech:start` before the queued utterance began.
- **Android**: Fixed a flushed utterance releasing the audio focus of the one replacing it.
- **Android**: Fixed `previewVoice()` inheriting rate and pitch from the previous `speak()` instead of using 1.0.
- **Android**: Fixed `pitch` being clamped to a `0.1` floor instead of the `0.5` used elsewhere.
- **Linux**: Fixed a missing speech-dispatcher aborting application startup; the plugin now loads degraded and `isInitialized()` reports `false`.
- **iOS**: Fixed `speech:cancel` carrying the replacing utterance's ID, and the `speech:finish` that followed carrying none.
- **Desktop**: Fixed `speech:cancel` carrying the ID of the utterance that was just starting.
- **Android / iOS**: Fixed validation errors always arriving as `PLUGIN_INVOKE_ERROR`; typed codes such as `EMPTY_TEXT` now reach JavaScript.
- **All platforms**: Fixed the 10,000 `text` limit counting bytes on desktop, UTF-16 units on Android and graphemes on iOS; it is UTF-8 bytes everywhere.
- **All platforms**: Fixed `getVoices(language)` matching substrings, so `"BR"` returned `pt-BR` voices; it now matches by locale prefix, as `speak({ language })` already did.

### Changed

- **Android** (Breaking): `pauseSpeaking()` / `resumeSpeaking()` now return `{ success: false, reason }`, and `speech:pause` / `speech:resume` are no longer emitted. Interruptions stop the utterance and emit `speech:interrupted` or `speech:backgroundPause`.
- **Android / iOS**: `rate`, `pitch` and `volume` are clamped once by the shared Rust validation instead of separately per platform.
- **iOS**: Warns when `continueInBackground` is enabled but `UIBackgroundModes: audio` is missing from the Info.plist.
- **Desktop**: `voiceId` and `language` limits count characters, not bytes.
- Rewrote the README and corrected the documented Windows backend from SAPI to WinRT.

## [0.1.12] - 2026-08-15

### Fixed

- **Permissions**: Added `allow-set-background-behavior` permission to `tts:default` (previously `setBackgroundBehavior()` was missing in ACL generation and calls were denied).
- **Desktop**: Fixed `language` parameter being ignored when `voiceId` was omitted; the first matching voice for that locale prefix is now selected.
- **Android / Desktop**: Fixed `rate` and `pitch` leaking across utterances when subsequent calls used the default `1.0`.
- **Android / iOS**: Relaxed `voiceId` character validation to accept Google TTS IDs containing `#` and system identifiers across platforms.
- **Desktop**: Fixed pitch and volume normalization on Linux (`speech-dispatcher`).
- Fixed failing unit test for voice ID charset validation.

### Changed

- **Validation errors** (Breaking): Now emit specific error codes (`EMPTY_TEXT`, `TEXT_TOO_LONG`, `VOICE_ID_TOO_LONG`, `INVALID_VOICE_ID`, `LANGUAGE_TOO_LONG`) instead of a generic `VALIDATION_ERROR`.
- Removed unused `LOCK_ERROR` and `NOT_INITIALIZED` codes from `TtsErrorCode`.
- Mobile: Relay-channel lock failures now return `MUTEX_POISONED` instead of panicking.
- Documented `MAX_TEXT_LENGTH` as a byte budget.
- Updated documentation and granular permissions list for `isInitialized()`, `onSpeechEvent()`, and `setBackgroundBehavior()`.

## [0.1.11] - 2026-06-09

### Fixed

- **Android**: Fixed TTS initialization failure on Xiaomi HyperOS and OPPO ColorOS.

## [0.1.10] - 2026-06-06

### Fixed

- **Desktop (Windows)**: Allowed registry path characters in SAPI `voiceId` validation (thanks @viktorkrp-dev in #11).

## [0.1.9] - 2026-04-18

### Fixed

- **Android / iOS** (#8): Automatically register native event relay on first `speak()`, allowing Rust-side event listeners (`app.listen`) to receive `tts://speech:*` events without manual setup.

## [0.1.8] - 2026-03-30

### Fixed

- **Android / iOS** (#6): Fixed speech events (`speech:start`, `speech:finish`, etc.) not reaching JavaScript by properly retaining the native relay channel lifecycle.

## [0.1.7] - 2026-03-29

### Fixed

- **Android**: Automatically restart and recover the TTS engine when voices return `null` via `reinitializeTts()`.
- **Android / iOS**: Surfaced TTS errors in the example app UI instead of swallowing them silently.
- **Desktop**: Improved error handling in `desktop.rs`.

## [0.1.6] - 2026-03-29

### Changed

- **Android / iOS / Desktop**: Unified internal event payload type into `TtsEventPayload` with an optional `reason` field.
- **Android**: Renamed speech events to `speech:pause` and `speech:resume` to match iOS and TypeScript definitions.
- **TypeScript**: Added optional `reason` property to the `SpeechEvent` interface.

### Fixed

- **Android / iOS** (#7): Speech now continues uninterrupted in the background when the screen locks or the app is backgrounded.
- **Android**: Fixed premature `speech:finish` event on long texts on Android 14+ (API 34+).
- **Android**: Fixed `volume` parameter being silently ignored in `speak()`.
- **Desktop**: Emitted `speech:start` only after synthesis starts successfully, and prevented duplicate `speech:cancel` events on `stop()`.
- **All platforms**: Added validation for `voiceId` parameters.

## [0.1.5] - 2026-03-29

### Fixed

- **Android / iOS** (#6): Fixed `onSpeechEvent()` to receive mobile events via plugin channel listener in addition to desktop event listeners.

## [0.1.4] - 2026-03-29

### Fixed

- **Android** (#5): Included voices from third-party TTS engines (e.g. sherpa-onnx) and network voices in `getVoices()`.
- **Android**: Updated `speak()` to the modern Android Bundle API and added fallback for emulator utterance progress callbacks.
- **Android**: Corrected feature flag constants and filtered out non-functional stub voices.

## [0.1.0] - 2025-12

### Added

- Initial release.
- Cross-platform TTS support (macOS, Windows, Linux, iOS, Android).
- `speak()` - Text-to-speech with customizable options.
- `stop()` - Stop current speech.
- `getVoices()` - List available voices with language info.
- `isSpeaking()` - Check if speech is in progress.
- Voice selection by ID (`voiceId` parameter).
- Rate normalization (1.0 = normal speed across all platforms).
- Pitch control (0.5 - 2.0).
- Volume control (0.0 - 1.0).
- Language selection (`language` parameter).
- TypeScript bindings with full type definitions.
- Comprehensive documentation and examples.

### Platform Support

| Platform | Engine                            |
| -------- | --------------------------------- |
| macOS    | AVFoundation (via tts crate)      |
| Windows  | SAPI (via tts crate)              |
| Linux    | speech-dispatcher (via tts crate) |
| iOS      | AVSpeechSynthesizer               |
| Android  | TextToSpeech API                  |

### Requirements

- Tauri: 2.9+
- Rust: 1.77+
- Android SDK: 24+ (Android 7.0+)
- iOS: 14.0+
