import { useState, useEffect, useRef, useCallback } from "react";
import {
  speak,
  stop,
  getVoices,
  isInitialized,
  onSpeechEvent,
  previewVoice,
  isTtsError,
  type Voice,
} from "tauri-plugin-tts-api";
import type { UnlistenFn } from "@tauri-apps/api/event";
import "./App.css";

const SAMPLES = [
  { lang: "en", label: "EN", text: "Hello! How are you today?" },
  { lang: "pt", label: "PT", text: "Olá! Como você está hoje?" },
  { lang: "es", label: "ES", text: "¡Hola! ¿Cómo estás hoy?" },
  { lang: "fr", label: "FR", text: "Bonjour! Comment allez-vous?" },
  { lang: "de", label: "DE", text: "Hallo! Wie geht es Ihnen?" },
  { lang: "ja", label: "JA", text: "こんにちは！お元気ですか？" },
];

export default function App() {
  const [text, setText] = useState(
    "Hello! This is a test of the text-to-speech plugin. This sentence is longer to ensure we can hear the audio.",
  );
  const [selectedVoiceId, setSelectedVoiceId] = useState("");
  const [rate, setRate] = useState(1.0);
  const [pitch, setPitch] = useState(1.0);
  const [volume, setVolume] = useState(1.0);
  const [voices, setVoices] = useState<Voice[]>([]);
  const [isSpeaking, setIsSpeaking] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const speechUnlistenRef = useRef<UnlistenFn | null>(null);

  const waitForTtsInit = useCallback(async () => {
    for (let i = 0; i < 20; i++) {
      try {
        const s = await isInitialized();
        if (s.initialized && s.voiceCount > 0) return;
      } catch {
        // engine not ready yet
      }
      await new Promise((r) => setTimeout(r, 300));
    }
  }, []);

  useEffect(() => {
    const init = async () => {
      setLoading(true);
      await waitForTtsInit();
      await loadVoices();
    };
    init();
    return () => {
      speechUnlistenRef.current?.();
    };
  }, [waitForTtsInit]);

  // Register speech event listeners.
  // Using onSpeechEvent (instead of polling isSpeaking()) is reliable for network voices:
  // engine.isSpeaking returns false when synthesis hands off to the hardware audio buffer,
  // but audio may still be playing. The plugin's speech:finish uses a 1.5s debounce to
  // correctly detect when playback is truly done.
  const startSpeechListeners = async () => {
    speechUnlistenRef.current?.();
    const done = () => {
      setIsSpeaking(false);
      speechUnlistenRef.current = null;
    };
    const [a, b, c] = await Promise.all([
      onSpeechEvent("speech:finish", done),
      onSpeechEvent("speech:error", done),
      onSpeechEvent("speech:cancel", done),
    ]);
    speechUnlistenRef.current = () => {
      a();
      b();
      c();
    };
  };

  const stopSpeechListeners = () => {
    speechUnlistenRef.current?.();
    speechUnlistenRef.current = null;
    setIsSpeaking(false);
  };

  const loadVoices = async () => {
    setLoading(true);
    try {
      const available = await getVoices();
      if (available.length === 0) {
        setTimeout(loadVoices, 1000);
        return;
      }
      setVoices(available);
    } catch (err) {
      setError(isTtsError(err) ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleSpeak = async () => {
    setError(null);
    try {
      await startSpeechListeners();
      await speak({
        text,
        voiceId: selectedVoiceId || null,
        rate,
        pitch,
        volume,
        language: null,
        queueMode: null,
      });
      setIsSpeaking(true);
    } catch (err) {
      stopSpeechListeners();
      setError(isTtsError(err) ? err.message : String(err));
    }
  };

  const handleStop = async () => {
    setError(null);
    try {
      await stop();
      stopSpeechListeners();
    } catch (err) {
      setError(isTtsError(err) ? err.message : String(err));
    }
  };

  const handlePreview = async (voice: Voice) => {
    setError(null);
    try {
      setSelectedVoiceId(voice.id);
      await previewVoice({ voiceId: voice.id, text: text.trim() || null });
    } catch (err) {
      setError(isTtsError(err) ? err.message : String(err));
    }
  };

  const voicesByLanguage = voices.reduce(
    (acc, voice) => {
      const lang = voice.language.split("-")[0];
      if (!acc[lang]) acc[lang] = [];
      acc[lang].push(voice);
      return acc;
    },
    {} as Record<string, Voice[]>,
  );

  return (
    <div className="page">
      <header className="header">
        <div className="header-inner">
          <h1 className="header-title">Text-to-Speech</h1>
          <p className="header-subtitle">Tauri Plugin TTS — native synthesis on all platforms</p>
        </div>
      </header>

      <main className="main">
        {error && (
          <div className="alert alert-error">
            <span>{error}</span>
            <button className="alert-close" onClick={() => setError(null)}>
              ×
            </button>
          </div>
        )}

        <div className="card">
          <div className="field">
            <label className="field-label" htmlFor="text-input">
              Text
            </label>
            <textarea
              id="text-input"
              className="textarea"
              rows={4}
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="Enter text to speak..."
            />
          </div>

          <div className="field">
            <label className="field-label" htmlFor="voice-select">
              Voice
            </label>
            <div className="select-wrap">
              <select
                id="voice-select"
                className="select"
                value={selectedVoiceId}
                onChange={(e) => setSelectedVoiceId(e.target.value)}
              >
                <option value="">System Default</option>
                {Object.entries(voicesByLanguage).map(([lang, langVoices]) => (
                  <optgroup key={lang} label={`${lang.toUpperCase()} (${langVoices.length})`}>
                    {langVoices.map((v) => (
                      <option key={v.id} value={v.id}>
                        {v.name}
                      </option>
                    ))}
                  </optgroup>
                ))}
              </select>
              <svg
                className="select-chevron"
                width="12"
                height="12"
                viewBox="0 0 12 12"
                aria-hidden
              >
                <path
                  d="M2 4l4 4 4-4"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  fill="none"
                />
              </svg>
            </div>
          </div>

          <div className="sliders">
            <SliderField
              label="Rate"
              value={rate}
              display={`${rate.toFixed(2)}×`}
              min={0.25}
              max={2.0}
              step={0.05}
              onChange={setRate}
              ticks={["0.25×", "1×", "2×"]}
            />
            <SliderField
              label="Pitch"
              value={pitch}
              display={pitch.toFixed(1)}
              min={0.5}
              max={2.0}
              step={0.1}
              onChange={setPitch}
              ticks={["Low", "Normal", "High"]}
            />
            <SliderField
              label="Volume"
              value={volume}
              display={`${Math.round(volume * 100)}%`}
              min={0}
              max={1.0}
              step={0.1}
              onChange={setVolume}
              ticks={["Mute", "50%", "100%"]}
            />
          </div>

          <div className="actions">
            <button
              className="btn btn-primary"
              onClick={handleSpeak}
              disabled={!text.trim() || isSpeaking}
            >
              Speak
            </button>
            <button className="btn btn-secondary" onClick={handleStop} disabled={!isSpeaking}>
              Stop
            </button>
            <span className={`status-pill ${isSpeaking ? "status-pill--speaking" : ""}`}>
              {isSpeaking ? "Speaking" : "Idle"}
            </span>
          </div>
        </div>

        <div className="card">
          <div className="card-row">
            <h2 className="card-title">
              Voices {voices.length > 0 && `(${voices.length})`}
            </h2>
            <button className="btn btn-ghost" onClick={loadVoices} disabled={loading}>
              {loading ? "Loading…" : "Refresh"}
            </button>
          </div>

          {voices.length === 0 && !loading && (
            <p className="empty">No voices found. Click Refresh.</p>
          )}

          <div className="voices-scroll">
            {Object.entries(voicesByLanguage).map(([lang, langVoices]) => (
              <div key={lang} className="voice-group">
                <div className="voice-group-head">
                  <span className="badge">{lang.toUpperCase()}</span>
                  <span className="voice-group-count">{langVoices.length}</span>
                </div>
                {langVoices.slice(0, 5).map((voice) => (
                  <div
                    key={voice.id}
                    className={`voice-row ${selectedVoiceId === voice.id ? "voice-row--selected" : ""}`}
                  >
                    <button
                      className="voice-select-btn"
                      onClick={() => setSelectedVoiceId(voice.id)}
                    >
                      <span className="voice-name">{voice.name}</span>
                      <span className="voice-lang-tag">{voice.language}</span>
                    </button>
                    <button className="btn btn-ghost btn-sm" onClick={() => handlePreview(voice)}>
                      Preview
                    </button>
                  </div>
                ))}
                {langVoices.length > 5 && (
                  <span className="voice-overflow">+{langVoices.length - 5} more</span>
                )}
              </div>
            ))}
          </div>
        </div>

        <div className="card">
          <h2 className="card-title" style={{ marginBottom: 4 }}>
            Sample Phrases
          </h2>
          <div className="samples">
            {SAMPLES.map((s) => {
              const firstVoice = (voicesByLanguage[s.lang] || [])[0];
              return (
                <button
                  key={s.lang}
                  className="sample-btn"
                  onClick={() => {
                    setText(s.text);
                    if (firstVoice) setSelectedVoiceId(firstVoice.id);
                  }}
                >
                  <span className="badge badge--sm">{s.label}</span>
                  <span className="sample-text">{s.text}</span>
                </button>
              );
            })}
          </div>
        </div>
      </main>
    </div>
  );
}

interface SliderFieldProps {
  label: string;
  value: number;
  display: string;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
  ticks: [string, string, string];
}

function SliderField({ label, value, display, min, max, step, onChange, ticks }: SliderFieldProps) {
  return (
    <div className="slider-field">
      <div className="slider-head">
        <label className="field-label">{label}</label>
        <span className="slider-val">{display}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      <div className="slider-ticks">
        <span>{ticks[0]}</span>
        <span>{ticks[1]}</span>
        <span>{ticks[2]}</span>
      </div>
    </div>
  );
}
