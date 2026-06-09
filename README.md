# Tauri Plugin TTS (Text-to-Speech)

Native Text-to-Speech plugin for Tauri 2.x applications. Provides cross-platform TTS functionality for desktop (Windows, macOS, Linux) and mobile (iOS, Android).

## Features

- 🗣️ **Speak text** with customizable rate, pitch, and volume
- 🌍 **Multi-language support** - Set language/locale for speech
- 🎙️ **Voice selection** - Get available voices and filter by language
- ⏹️ **Control playback** - Stop speech and check speaking status
- 🎬 **Preview voices** - Test voices before using them
- 📝 **Queue mode** - Interrupt or queue speech requests
- ⏸️ **Pause/Resume** - Control playback on iOS (platform-specific)
- 📱 **Cross-platform** - Works on desktop and mobile

## Installation

### Rust

Add the plugin to your `Cargo.toml`:

```toml
[dependencies]
tauri-plugin-tts = "0.1"
```

### TypeScript

Install the JavaScript guest bindings:

```bash
npm install tauri-plugin-tts-api
# or
yarn add tauri-plugin-tts-api
# or
pnpm add tauri-plugin-tts-api
```

## Setup

### Register Plugin

In your Tauri app setup:

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_tts::init())
        .run(tauri::generate_context!())
        .expect("error while running application");
}
```

### Permissions

Add permissions to your `capabilities/default.json`:

```json
{
  "permissions": ["tts:default"]
}
```

For granular permissions, you can specify individual commands:

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

### Basic Examples

```typescript
import { speak, stop, getVoices, isSpeaking } from "tauri-plugin-tts-api";

// Simple speech
await speak({ text: "Hello, world!" });

// With options
await speak({
  text: "Olá, mundo!",
  language: "pt-BR",
  rate: 0.8, // 0.1 to 4.0 (1.0 = normal)
  pitch: 1.2, // 0.5 to 2.0 (1.0 = normal)
  volume: 1.0, // 0.0 to 1.0 (1.0 = full)
});

// Stop speaking
await stop();

// Get all voices
const voices = await getVoices();

// Get voices for a specific language
const englishVoices = await getVoices("en");

// Check if speaking
const speaking = await isSpeaking();
```

### Advanced Features

#### Queue Mode

Control how new speech requests interact with ongoing speech:

```typescript
// Default: interrupt current speech (flush)
await speak({ text: "First sentence" });
await speak({ text: "Second sentence" }); // Interrupts first

// Queue mode: add to queue
await speak({ text: "First sentence" });
await speak({ text: "Second sentence", queueMode: "add" }); // Waits for first
```

#### Voice Preview

Preview voices before selecting them:

```typescript
import { getVoices, previewVoice } from "tauri-plugin-tts-api";

// Get available voices
const voices = await getVoices("en");

// Preview a voice with default text
await previewVoice({ voiceId: voices[0].id });

// Preview with custom text
await previewVoice({
  voiceId: voices[0].id,
  text: "This is how I sound!",
});
```

#### Pause and Resume (iOS only)

```typescript
import { speak, pauseSpeaking, resumeSpeaking } from "tauri-plugin-tts-api";

await speak({ text: "Long text to speak..." });

// Pause (iOS only)
const pauseResult = await pauseSpeaking();
if (pauseResult.success) {
  console.log("Speech paused");

  // Resume later
  const resumeResult = await resumeSpeaking();
  if (resumeResult.success) {
    console.log("Speech resumed");
  }
} else {
  console.log("Pause not supported:", pauseResult.reason);
}
```

## Platform Support

| Platform | Status                         | Engine              |
| -------- | ------------------------------ | ------------------- |
| Windows  | ✅ Full support                | SAPI                |
| macOS    | ✅ Full support                | AVSpeechSynthesizer |
| Linux    | ✅ Full support                | speech-dispatcher   |
| iOS      | ✅ Full support + pause/resume | AVSpeechSynthesizer |
| Android  | ✅ Full support                | TextToSpeech        |

### Feature Support Matrix

| Feature            | Windows | macOS | Linux | iOS | Android |
| ------------------ | ------- | ----- | ----- | --- | ------- |
| `speak()`          | ✅      | ✅    | ✅    | ✅  | ✅      |
| `stop()`           | ✅      | ✅    | ✅    | ✅  | ✅      |
| `getVoices()`      | ✅      | ✅    | ✅    | ✅  | ✅      |
| `isSpeaking()`     | ✅      | ✅    | ✅    | ✅  | ✅      |
| `previewVoice()`   | ✅      | ✅    | ✅    | ✅  | ✅      |
| `queueMode`        | ✅      | ✅    | ✅    | ✅  | ✅      |
| `pauseSpeaking()`  | ❌      | ❌    | ❌    | ✅  | ❌      |
| `resumeSpeaking()` | ❌      | ❌    | ❌    | ✅  | ❌      |

## API Reference

### `speak(options: SpeakOptions): Promise<void>`

Speak the given text.

**Options:**

- `text` (required): The text to speak
- `language`: Language/locale code (e.g., "en-US", "pt-BR")
- `voiceId`: Specific voice ID from `getVoices()` (takes priority over `language`)
- `rate`: Speech rate (0.1 to 4.0, where 1.0 = normal speed, 2.0 = double, 0.5 = half)
- `pitch`: Voice pitch (0.5 to 2.0, where 1.0 = normal, 2.0 = high, 0.5 = low)
- `volume`: Volume level (0.0 to 1.0, where 0.0 = silent, 1.0 = full)
- `queueMode`: "flush" (default, interrupts current speech) or "add" (queues after current)

### `stop(): Promise<void>`

Stop any ongoing speech immediately.

### `getVoices(language?: string): Promise<Voice[]>`

Get available voices, optionally filtered by language.

**Returns:** Array of `Voice` objects with:

- `id`: Unique voice identifier
- `name`: Display name
- `language`: Language code (e.g., "en-US")

### `isSpeaking(): Promise<boolean>`

Check if TTS is currently speaking.

### `previewVoice(options: PreviewVoiceOptions): Promise<void>`

Preview a voice with sample text.

**Options:**

- `voiceId` (required): Voice ID to preview
- `text`: Optional custom preview text (uses default if not provided)

### `pauseSpeaking(): Promise<PauseResumeResponse>` (iOS only)

Pause the current speech.

**Returns:**

- `success`: Whether pause succeeded
- `reason`: Optional failure reason (e.g., "Not supported on this platform")

### `resumeSpeaking(): Promise<PauseResumeResponse>` (iOS only)

Resume paused speech.

**Returns:** Same as `pauseSpeaking()`

## Troubleshooting

### Linux: "No TTS backend available"

**Solution:** Install `speech-dispatcher`:

```bash
# Debian/Ubuntu
sudo apt-get install speech-dispatcher

# Fedora
sudo dnf install speech-dispatcher

# Arch
sudo pacman -S speech-dispatcher
```

### Android: No voices available

**Solution:** Ensure a TTS engine is installed:

1. Open Android Settings → Accessibility → Text-to-Speech
2. Install "Google Text-to-Speech" from Play Store if missing
3. Download language data for your desired languages

### Xiaomi HyperOS and OPPO ColorOS
**Solution:** TTS not initialized, queuing request
1. Add the following to AndroidManifest.xml:
``` xml
    <queries>
        <intent>
            <action android:name="android.intent.action.TTS_SERVICE" />
        </intent>
    </queries>
```

### iOS: Voices sound robotic

**Solution:** Download enhanced voices:

1. Open iOS Settings → Accessibility → Spoken Content → Voices
2. Select your language and download "Enhanced Quality" voices

### Rate/Pitch not working as expected

**Note:** Platform engines may have different interpretations:

- **Windows SAPI**: Limited pitch control
- **Linux**: Depends on speech-dispatcher backend
- **Mobile**: Full support for rate and pitch

### Pause/Resume not working

**Note:** `pauseSpeaking()` and `resumeSpeaking()` are only supported on **iOS**. Other platforms will return `{ success: false, reason: "Not supported" }`.

## Examples

See the [examples/tts-example](./examples/tts-example) directory for a complete working demo with React + Material UI.

## License

MIT
