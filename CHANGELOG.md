# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.8] - 2026-03-30

### Fixed

- **Android/iOS** (issue #6): Speech events (`speech:start`, `speech:finish`, etc.) now
  correctly reach JavaScript on mobile. Two root causes were fixed:
  1. `setupEventRelay` was called during the Rust `setup()` hook before the Android/iOS
     plugin was ready, causing a silent failure with no channel ever registered.
  2. The `Channel<TtsEventPayload>` Rust object was a local variable in
     `setup_event_relay()` and was immediately dropped at the end of the function,
     destroying the callback — so `eventChannel.send()` in Kotlin/Swift was a silent no-op.
     Fixed by adding a `register_listener` Tauri command invoked by JS after app load (deferring
     channel setup to when the native plugin is ready) and by storing the channel in the `Tts<R>`
     struct (`relay_channel: Mutex<Option<Channel<TtsEventPayload>>>`) to keep it alive for the
     plugin lifetime.

## [0.1.7] - 2026-03-29

### Fixed

- **Android**: TTS engine bad state (voices returning `null`) is now recovered automatically
  via `reinitializeTts()`. Previously, if the Google TTS engine entered a broken state, all
  subsequent `speak()` calls would fail silently. The engine is now restarted from scratch
  and the pending request is re-queued.
- **Android/iOS**: Improved error display in the example app — errors are now surfaced to
  the UI instead of being swallowed silently.
- **Desktop**: `speak()` error handling improved in `desktop.rs`.

## [0.1.6] - 2026-03-29

### Changed

- **Android/iOS/Desktop**: Unified the internal event payload type — `SpeechEvent` (desktop)
  and the mobile `TtsEventPayload` are now a single `TtsEventPayload` struct in `models.rs` with
  an optional `reason` field for `speech:backgroundPause` and audio-focus events.
- **Android**: Speech event names corrected to match iOS and the TypeScript `SpeechEventType`
  definition: `speech:paused` → `speech:pause`, `speech:resumed` → `speech:resume`.
- **TypeScript**: `SpeechEvent` interface now includes a `reason?: string` field populated for
  background-pause and audio-focus events.

### Fixed

- **Android** (issue #7): TTS no longer stops when the screen is locked or the app goes to
  background. Previously `onPause()` explicitly called `tts.stop()` without emitting any
  event, leaving the JS side with no way to detect the interruption and no audio. Now speech
  continues uninterrupted in the background (TTS engine runs as a system service). When the
  app goes to background while speaking, a `speech:backgroundPause` event is emitted so the
  UI can update its state. Audio focus changes (phone calls, notifications) continue to be
  handled separately via `AudioManager.OnAudioFocusChangeListener`.
- **iOS** (issue #7): Same fix applied — `AVSpeechSynthesizer` no longer pauses when the
  screen locks. The `AVAudioSession` category `.playback` already enables background audio;
  the explicit `pauseSpeaking()` call on background transition was removed. A
  `speech:backgroundPause` event is still emitted so the UI can update its state.
- **Android**: `speak()` with large texts no longer emits `speech:finish` prematurely. On
  Android 14+ (API 34+), `isSpeaking()` returns `false` as soon as synthesis is handed to the
  hardware audio buffer — before playback actually completes. The previous single-poll check
  fired `speech:finish` ~2 seconds into a long utterance. Fixed with a debounce: 15
  consecutive `isSpeaking() == false` readings (1.5s of confirmed silence) are now required
  before emitting `speech:finish`.
- **Android**: `volume` parameter in `speak()` was accepted but silently ignored. It is now
  correctly passed via `TextToSpeech.Engine.KEY_PARAM_VOLUME` in the Bundle.
- **Desktop**: `speech:start` is now emitted only after `engine.speak()` succeeds (previously
  fired before the speak call, so a failed speak still emitted a start event).
- **Desktop**: Duplicate `speech:cancel` events on `stop()` are no longer emitted for engines
  that already provide utterance callbacks.
- **All platforms**: `voiceId` in `speak()` options is now validated — must contain only
  alphanumeric characters, `.`, `_`, or `-`. Invalid IDs are rejected with a clear error
  instead of being silently ignored.

## [0.1.5] - 2026-03-29

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
