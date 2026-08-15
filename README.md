# Tauri Plugin TTS (Text-to-Speech)

Native Text-to-Speech for Tauri 2.x, delegating to the OS synthesiser: WinRT (Windows),
AVSpeechSynthesizer (macOS/iOS), speech-dispatcher (Linux), TextToSpeech (Android).

## Features

- `speak()` with rate, pitch and volume normalised onto each backend's own scale
- Voice enumeration, locale filtering and one-call preview
- Queue mode: interrupt the current utterance or append to it
- Speech lifecycle events, delivered identically on every platform
- Typed errors with stable codes, raised from shared Rust validation

## Installation

```toml
# src-tauri/Cargo.toml
[dependencies]
tauri-plugin-tts = "0.1"
```

```bash
npm install tauri-plugin-tts-api
```

## Setup

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_tts::init())
```

```json
// src-tauri/capabilities/default.json
{ "permissions": ["tts:default"] }
```

Listing permissions individually instead? `tts:allow-register-listener` is the one that is
easy to miss — without it `onSpeechEvent()` silently never fires on mobile.

## Usage

```typescript
import { speak, stop, getVoices, previewVoice, onSpeechEvent } from "tauri-plugin-tts-api";

await speak({ text: "Hello, world!" });

await speak({
  text: "Olá, mundo!",
  language: "pt-BR",
  rate: 0.8,   // 0.1–4.0, 1.0 = normal
  pitch: 1.2,  // 0.5–2.0, 1.0 = normal
  volume: 1.0, // 0.0–1.0
});

await speak({ text: "Queued after the current one", queueMode: "add" });
await stop();

const voices = await getVoices("pt");
await previewVoice({ voiceId: voices[0].id });

const unlisten = await onSpeechEvent("speech:finish", (e) => console.log(e.id));
```

Four rules that are not guessable from the signatures:

- **`voiceId` beats `language`.** With only `language`, the first voice whose tag starts with
  it is used.
- **Filtering is by locale prefix, not substring.** `"pt"` and `"pt-BR"` both match a `pt-BR`
  voice; `"BR"` matches nothing.
- **`speak()` resolves when speech *starts*, not when it ends.** Listen for `speech:finish`.
- **Utterance IDs only ever arrive on events**, never as a return value.

### Waiting for the engine

Mobile engines initialise asynchronously and `getVoices()` returns `[]` until they are ready,
so poll `isInitialized()` until `voiceCount > 0` before populating a voice picker. On Android
a `speak()` issued during that window is queued and flushed on init (rejected after 30 s), so
speaking early works — only enumeration needs the wait.

### Speech events

| Platform | Emits |
| -------- | ----- |
| Desktop  | `speech:start`, `speech:finish`, `speech:cancel` |
| Android  | the above plus `speech:error`, `speech:interrupted`, `speech:backgroundPause` |
| iOS      | all of them, plus `speech:pause` / `speech:resume` |

### Background behaviour (mobile)

```typescript
await setBackgroundBehavior({ continueInBackground: false });
```

Speech continues in the background by default; with `false` it stops when the app is
backgrounded and emits `speech:backgroundPause`. iOS pauses and can resume; Android has no
pause in its TTS API, so it stops and also emits `speech:cancel`. Desktop: no-op.

**iOS needs the background mode declared, or the default silently fails** — without it the app
is suspended and speech dies on leaving the foreground. The plugin logs a warning when it is
missing. In `src-tauri/Info.plist`:

```xml
<key>UIBackgroundModes</key>
<array><string>audio</string></array>
```

### Error handling

Rejections are plain `{ code, message }` objects, not `Error` instances — `instanceof` will
not work, use the guard:

```typescript
try {
  await speak({ text: "" });
} catch (e) {
  if (isTtsError(e) && e.code === "EMPTY_TEXT") { /* … */ }
}
```

Codes: `EMPTY_TEXT`, `TEXT_TOO_LONG`, `VOICE_ID_TOO_LONG`, `INVALID_VOICE_ID`,
`LANGUAGE_TOO_LONG`, `TTS_ENGINE_ERROR`, `PLUGIN_INVOKE_ERROR`, `OPERATION_FAILED`,
`MUTEX_POISONED`, `IO_ERROR`.

## API Reference

| Function | Returns | Notes |
| -------- | ------- | ----- |
| `speak({ text, language?, voiceId?, rate?, pitch?, volume?, queueMode? })` | `void` | resolves when speech starts |
| `stop()` | `void` | interrupts immediately |
| `getVoices(language?)` | `Voice[]` | `Voice` is `{ id, name, language }`; filter is a locale prefix |
| `isSpeaking()` | `boolean` | |
| `isInitialized()` | `{ initialized, voiceCount }` | poll before `getVoices()` on mobile |
| `previewVoice({ voiceId, text? })` | `void` | plays a sample at rate/pitch/volume 1.0 |
| `pauseSpeaking()` / `resumeSpeaking()` | `{ success, reason? }` | iOS only; `success: false` elsewhere |
| `setBackgroundBehavior({ continueInBackground })` | `void` | mobile only |
| `onSpeechEvent(type, cb)` | `UnlistenFn` | |
| `isTtsError(e)` | type guard | narrows `unknown` to `{ code, message }` |

`text` is capped at 10,000 **bytes**, not characters — CJK and emoji fit proportionally fewer.
`voiceId` is capped at 256 characters. Out-of-range `rate` / `pitch` / `volume` are clamped,
not rejected.

## Feature Support Matrix

Rows omitted where every platform behaves the same: `speak`, `stop`, voice selection, queue
mode, and rate/pitch/volume work everywhere.

| | Windows | macOS | Linux | iOS | Android |
| --- | --- | --- | --- | --- | --- |
| Engine | WinRT | AVSpeechSynthesizer | speech-dispatcher | AVSpeechSynthesizer | TextToSpeech |
| Pause / resume | — | — | — | ✅ | — |
| Background behaviour | — | — | — | ✅ | ✅ |
| `speech:error` event | — | — | — | ✅ | ✅ |

## Troubleshooting

**Linux: every call fails with "Speech Dispatcher not available"** — the daemon is missing.
The plugin loads in a degraded state rather than aborting app startup, so `isInitialized()`
reports `false` and each command returns the install hint:

```bash
sudo apt-get install speech-dispatcher   # Debian/Ubuntu
sudo dnf install speech-dispatcher       # Fedora
sudo pacman -S speech-dispatcher         # Arch
```

**`onSpeechEvent` never fires on mobile** — the capability is missing
`tts:allow-register-listener`. Use `tts:default` or add it explicitly.

**Android: no voices** — Settings → Accessibility → Text-to-Speech, install Google TTS, then
download language data for your locale. Voices needing an uninstalled language pack are
filtered out of `getVoices()`, so an empty list usually means nothing is downloaded.

**iOS: voices sound robotic** — Settings → Accessibility → Spoken Content → Voices, pick your
language and download Enhanced Quality.

**Same rate/pitch sounds different per platform** — values are mapped onto each backend's own
scale, but the engines themselves differ. Windows WinRT has coarse pitch control, and on Linux
the result depends on the speech-dispatcher output module (espeak, festival, …).

## License

MIT
