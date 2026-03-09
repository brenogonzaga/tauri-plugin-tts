# TTS Plugin Example

Complete demonstration of the `tauri-plugin-tts` functionality using React + TypeScript + Material UI.

## Features Demonstrated

- ✅ Basic text-to-speech with customizable text
- ✅ Voice selection with language grouping
- ✅ Rate control (0.1x - 4.0x)
- ✅ Pitch control (0.5 - 2.0)
- ✅ Volume control (0% - 100%)
- ✅ Real-time speaking status polling
- ✅ Stop functionality
- ✅ Voice reloading
- ✅ Error handling and user feedback
- ✅ Responsive design (mobile-friendly)

## Running the Example

**Before running the example**, build the plugin from the root directory:

```bash
# From the tauri-plugin-tts root directory
npm run build
```

**For mobile platforms**, initialize before running:

```bash
# For Android
npm run android init

# For iOS
npm run ios init
```

**Then run the example**:

```bash
npm install
npm run tauri dev
```

## Project Structure

```
tts-example/
├── src/
│   ├── App.tsx          # Main demo component
│   └── main.tsx         # React entry point
├── src-tauri/
│   ├── src/
│   │   └── main.rs      # Tauri setup with TTS plugin
│   └── Cargo.toml       # Rust dependencies
└── package.json         # NPM dependencies
```

## Code Highlights

### Voice Selection

The example groups voices by language for better UX:

```typescript
const voicesByLanguage = voices.reduce(
  (acc, voice) => {
    const lang = voice.language.split("-")[0];
    if (!acc[lang]) acc[lang] = [];
    acc[lang].push(voice);
    return acc;
  },
  {} as Record<string, Voice[]>,
);
```

### Speaking Status Polling

Uses interval polling to update UI when speech finishes:

```typescript
pollingRef.current = setInterval(async () => {
  const speaking = await isSpeaking();
  setIsSpeaking(speaking);

  if (!speaking) {
    clearInterval(pollingRef.current);
    pollingRef.current = null;
  }
}, 500);
```

## Technologies Used

- **Tauri 2.x** - Desktop/Mobile application framework
- **React 18** - UI library
- **TypeScript** - Type safety
- **Material UI 6** - Component library
- **Vite** - Build tool

## Learn More

- [tauri-plugin-tts Documentation](../../README.md)
- [Tauri Documentation](https://tauri.app/)
- [Material UI Documentation](https://mui.com/)

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/)
- [Tauri Extension](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
