# Tauri Plugin TTS (Text-to-Speech)

Native Text-to-Speech for Tauri 2.x. Delegates to the OS synthesiser on each platform: SAPI (Windows), AVSpeechSynthesizer (macOS/iOS), speech-dispatcher (Linux), TextToSpeech (Android).

## Platform Matrix

| Platform | Engine              | Pause/Resume |
| -------- | ------------------- | ------------ |
| Windows  | SAPI                | —            |
| macOS    | AVSpeechSynthesizer | —            |
| Linux    | speech-dispatcher   | —            |
| iOS      | AVSpeechSynthesizer | ✅           |
| Android  | TextToSpeech        | —            |

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

Granular:

```json
{
  "permissions": [
    "tts:allow-speak",
    "tts:allow-stop",
    "tts:allow-get-voices",
    "tts:allow-is-speaking",
    "tts:allow-preview-voice",
    "tts:allow-pause-speaking",
    "tts:allow-resume-speaking"
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

### Queue mode

By default each `speak()` call interrupts any ongoing speech (`queueMode: "flush"`). Pass `"add"` to queue instead:

```typescript
await speak({ text: "First sentence" });
await speak({ text: "Second sentence", queueMode: "add" }); // waits for first to finish
```

### Pause and Resume (iOS only)

`pauseSpeaking()` and `resumeSpeaking()` are only supported on iOS. On all other platforms they return `{ success: false, reason: "Not supported on this platform" }`.

```typescript
import { pauseSpeaking, resumeSpeaking } from "tauri-plugin-tts-api";

const { success } = await pauseSpeaking();
if (success) await resumeSpeaking();
```

## API Reference

- `speak(options)` — `text` required; `language`, `voiceId`, `rate`, `pitch`, `volume`, `queueMode` optional
- `stop()` — interrupts speech immediately
- `getVoices(language?)` → `Voice[]` — pass a locale prefix like `"en"` or `"pt"` to filter
- `isSpeaking()` → `boolean`
- `previewVoice({ voiceId, text? })` — plays a short sample with the given voice
- `pauseSpeaking()` → `{ success, reason? }` — iOS only
- `resumeSpeaking()` → `{ success, reason? }` — iOS only

### Voice

```typescript
interface Voice {
  id: string;
  name: string;
  language: string; // e.g. "en-US"
}
```

## Troubleshooting

**Linux: "No TTS backend available"** — install speech-dispatcher:

```bash
sudo apt-get install speech-dispatcher   # Debian/Ubuntu
sudo dnf install speech-dispatcher       # Fedora
sudo pacman -S speech-dispatcher         # Arch
```

**Android: no voices** — open Settings → Accessibility → Text-to-Speech, install Google TTS from Play Store, then download language data for your locale.

**iOS: voices sound robotic** — Settings → Accessibility → Spoken Content → Voices → select your language and download Enhanced Quality.

**Rate/Pitch behavior differs across platforms** — Windows SAPI has limited pitch control; Linux results depend on the speech-dispatcher backend. Mobile engines honour the full specified range.

## License

MIT
