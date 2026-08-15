# Tauri Plugin TTS (Text-to-Speech)

Native Text-to-Speech for Tauri 2.x. Delegates to the OS synthesiser on each platform: SAPI (Windows), AVSpeechSynthesizer (macOS/iOS), speech-dispatcher (Linux), TextToSpeech (Android).

## Features

- Speak text with per-utterance rate, pitch and volume, normalised across platforms
- Voice enumeration and filtering by locale, plus a one-call voice preview
- Queue mode: interrupt the current utterance or append to it
- Speech lifecycle events (`start`, `finish`, `cancel`, `error`, …) on every platform
- Pause and resume (iOS)
- Background behaviour control for when the screen locks (mobile)
- Typed errors with stable machine-readable codes

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-tts = "0.1"
```

### TypeScript

```bash
npm install tauri-plugin-tts-api
```

## Setup

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_tts::init())
        .run(tauri::generate_context!())
        .unwrap();
}
```

### Permissions

```json
{ "permissions": ["tts:default"] }
```

Granular — note that `allow-register-listener` is required for `onSpeechEvent()` to work on mobile:

```json
{
  "permissions": [
    "tts:allow-speak",
    "tts:allow-stop",
    "tts:allow-get-voices",
    "tts:allow-is-speaking",
    "tts:allow-is-initialized",
    "tts:allow-preview-voice",
    "tts:allow-pause-speaking",
    "tts:allow-resume-speaking",
    "tts:allow-register-listener",
    "tts:allow-set-background-behavior"
  ]
}
```

## Usage

```typescript
import { speak, stop, getVoices, isSpeaking, previewVoice } from "tauri-plugin-tts-api";

// Basic speech
await speak({ text: "Hello, world!" });

// With options
await speak({
  text: "Olá, mundo!",
  language: "pt-BR",
  rate: 0.8,   // 0.1–4.0, 1.0 = normal
  pitch: 1.2,  // 0.5–2.0, 1.0 = normal
  volume: 1.0, // 0.0–1.0
});

await stop();
const speaking = await isSpeaking();

// Voices
const voices = await getVoices();
const ptVoices = await getVoices("pt"); // filter by locale prefix
await previewVoice({ voiceId: voices[0].id, text: "Sample" });
```

`voiceId` takes priority over `language`. When only `language` is given, the first voice
matching that locale prefix is used.

### Waiting for the engine

Mobile engines initialise asynchronously, so `getVoices()` can return an empty list right
after startup. Poll `isInitialized()` first:

```typescript
import { isInitialized, getVoices } from "tauri-plugin-tts-api";

for (let i = 0; i < 10; i++) {
  const { initialized, voiceCount } = await isInitialized();
  if (initialized && voiceCount > 0) break;
  await new Promise((r) => setTimeout(r, 500));
}
const voices = await getVoices();
```

### Speech events

```typescript
import { onSpeechEvent } from "tauri-plugin-tts-api";

const unlisten = await onSpeechEvent("speech:finish", (event) => {
  console.log("finished:", event.id);
});
unlisten();
```

Event types: `speech:start`, `speech:finish`, `speech:cancel`, `speech:pause`,
`speech:resume`, `speech:error`, `speech:interrupted`, `speech:backgroundPause`.
Desktop emits `start`, `finish` and `cancel`; the rest are mobile-only.

### Queue mode

By default each `speak()` call interrupts any ongoing speech (`queueMode: "flush"`). Pass `"add"` to queue instead:

```typescript
await speak({ text: "First sentence" });
await speak({ text: "Second sentence", queueMode: "add" }); // waits for first to finish
```

### Pause and Resume (iOS only)

`pauseSpeaking()` and `resumeSpeaking()` are only supported on iOS. On all other platforms they return `{ success: false, reason: "..." }`.

```typescript
import { pauseSpeaking, resumeSpeaking } from "tauri-plugin-tts-api";

const { success } = await pauseSpeaking();
if (success) await resumeSpeaking();
```

### Background behaviour (mobile)

```typescript
import { setBackgroundBehavior } from "tauri-plugin-tts-api";

// Pause speech when the app backgrounds or the screen locks
await setBackgroundBehavior({ continueInBackground: false });
```

Speech continues in the background by default. With `false`, a `speech:backgroundPause`
event is emitted when the app is backgrounded. Desktop: no-op.

### Error handling

```typescript
import { speak, isTtsError } from "tauri-plugin-tts-api";

try {
  await speak({ text: "" });
} catch (e) {
  if (isTtsError(e) && e.code === "EMPTY_TEXT") { /* … */ }
}
```

## API Reference

- `speak(options)` — `text` required; `language`, `voiceId`, `rate`, `pitch`, `volume`, `queueMode` optional
- `stop()` — interrupts speech immediately
- `getVoices(language?)` → `Voice[]` — pass a locale prefix like `"en"` or `"pt"` to filter
- `isSpeaking()` → `boolean`
- `isInitialized()` → `{ initialized, voiceCount }` — poll this before `getVoices()` on mobile
- `previewVoice({ voiceId, text? })` — plays a short sample with the given voice
- `pauseSpeaking()` → `{ success, reason? }` — iOS only
- `resumeSpeaking()` → `{ success, reason? }` — iOS only
- `setBackgroundBehavior({ continueInBackground })` — mobile only, no-op on desktop
- `onSpeechEvent(type, callback)` → `Promise<UnlistenFn>` — subscribe to speech lifecycle events
- `isTtsError(e)` → type guard narrowing `unknown` to `TtsError`

### Voice

```typescript
interface Voice {
  id: string;
  name: string;
  language: string; // e.g. "en-US"
}
```

### TtsError

```typescript
interface TtsError {
  code: TtsErrorCode;
  message: string;
}
```

`TtsErrorCode` is one of `IO_ERROR`, `PLUGIN_INVOKE_ERROR`, `TTS_ENGINE_ERROR`,
`MUTEX_POISONED`, `OPERATION_FAILED`, `EMPTY_TEXT`, `TEXT_TOO_LONG`,
`VOICE_ID_TOO_LONG`, `INVALID_VOICE_ID`, `LANGUAGE_TOO_LONG`.

### Limits

`text` is capped at 10,000 **bytes**, not characters — non-ASCII text (CJK, emoji) fits
proportionally fewer characters. `voiceId` is capped at 256 characters.

## Feature Support Matrix

| Feature               | Windows | macOS | Linux | iOS | Android |
| --------------------- | ------- | ----- | ----- | --- | ------- |
| Engine                | SAPI    | AVSpeechSynthesizer | speech-dispatcher | AVSpeechSynthesizer | TextToSpeech |
| speak / stop          | ✅      | ✅    | ✅    | ✅  | ✅      |
| Voice selection       | ✅      | ✅    | ✅    | ✅  | ✅      |
| Rate / pitch / volume | ✅      | ✅    | ✅    | ✅  | ✅      |
| Queue mode            | ✅      | ✅    | ✅    | ✅  | ✅      |
| Pause / resume        | —       | —     | —     | ✅  | —       |
| Background behaviour  | —       | —     | —     | ✅  | ✅      |
| Speech events         | ✅      | ✅    | ✅    | ✅  | ✅      |

## Troubleshooting

**Linux: "No TTS backend available"** — install speech-dispatcher:

```bash
sudo apt-get install speech-dispatcher   # Debian/Ubuntu
sudo dnf install speech-dispatcher       # Fedora
sudo pacman -S speech-dispatcher         # Arch
```

**Android: no voices** — open Settings → Accessibility → Text-to-Speech, install Google TTS from Play Store, then download language data for your locale.

**iOS: voices sound robotic** — Settings → Accessibility → Spoken Content → Voices → select your language and download Enhanced Quality.

**`onSpeechEvent` never fires on mobile** — the capability is missing `tts:allow-register-listener`. Use `tts:default` or add it explicitly.

**Rate/Pitch behavior differs across platforms** — values are normalised onto each backend's own scale, but the underlying engines still differ: Windows SAPI has limited pitch control, and Linux results depend on the speech-dispatcher output module.

## License

MIT
