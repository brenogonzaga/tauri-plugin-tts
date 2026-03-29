# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.5] - 2026-03-30

### Fixed

- **Android/iOS** (issue #6): `onSpeechEvent()` now correctly receives speech events on mobile.
  The previous implementation used Tauri's global event system (`listen()`), which only works
  for desktop events emitted via Rust `app.emit()`. Android's `trigger()` uses the plugin
  Channel system, which requires `addPluginListener()`. Both are now called in parallel:
  `addPluginListener` handles mobile events; `listen` handles desktop events. No duplicate
  events occur since Android never calls `app.emit()` and desktop never calls `trigger()`.

## [0.1.4] - 2026-03-29

### Fixed

- **Android** (issue #5): `getVoices()` now returns voices when using third-party TTS engines
  (e.g. sherpa-onnx) that report `quality=300` with empty features — previously these were
  filtered out by an overly strict quality threshold.
- **Android**: `speak()` callbacks (`speech:start`, `speech:finish`) no longer fail silently on
  Google TTS. The deprecated `HashMap`-based `speak()` API was replaced with the modern
  `engine.speak(text, mode, null, utteranceId)` Bundle API (API 21+).
- **Android**: Added polling fallback for `speech:start`/`speech:finish` events when
  `UtteranceProgressListener` does not fire (known issue with Google TTS on emulators).
  On real devices the listener fires normally; duplicate events are prevented by shared
  `@Volatile` flags.
- **Android**: Corrected feature flag constants — `TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED`
  instead of the non-existent `Voice.FEATURE_NOT_INSTALLED`.
- **Android**: Removed `-language` routing stub voices (e.g. `en-US-language`) from `getVoices()`
  — these appear as local high-quality voices but produce no audio with no error callbacks.
- **Android**: Network voices are now always included in `getVoices()` (not filtered).

## [0.1.0] - 2025-12

### Added

- Initial release
- Cross-platform TTS support (macOS, Windows, Linux, iOS, Android)
- `speak()` - Text-to-speech with customizable options
- `stop()` - Stop current speech
- `getVoices()` - List available voices with language info
- `isSpeaking()` - Check if speech is in progress
- Voice selection by ID (`voiceId` parameter)
- Rate normalization (1.0 = normal speed across all platforms)
- Pitch control (0.5 - 2.0)
- Volume control (0.0 - 1.0)
- Language selection (`language` parameter)
- TypeScript bindings with full type definitions
- Comprehensive documentation and examples

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
