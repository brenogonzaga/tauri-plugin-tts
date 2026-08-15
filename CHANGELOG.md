# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
