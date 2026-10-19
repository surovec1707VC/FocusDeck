import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Intent,
  PTT_EVENT,
  ProviderStatus,
  SAMPLE_RATES,
  STT_LABELS,
  SttProvider,
  TTS_LABELS,
  TtsProvider,
  VoiceSettings,
  voiceApi,
} from "../lib/voice";

export default function VoiceControl() {
  const [saved, setSaved] = useState<VoiceSettings | null>(null);
  const [draft, setDraft] = useState<VoiceSettings | null>(null);
  const [statuses, setStatuses] = useState<ProviderStatus[]>([]);
  const [mics, setMics] = useState<MediaDeviceInfo[]>([]);
  const [pttDown, setPttDown] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [transcript, setTranscript] = useState("");
  const [intent, setIntent] = useState<Intent | null>(null);

  const refreshStatus = useCallback(() => voiceApi.providerStatus().then(setStatuses), []);

  const loadMics = useCallback(async () => {
    try {
      const devices = await navigator.mediaDevices.enumerateDevices();
      setMics(devices.filter((d) => d.kind === "audioinput"));
    } catch {
      setMics([]);
    }
  }, []);

  useEffect(() => {
    voiceApi.getSettings().then((s) => {
      setSaved(s);
      setDraft(s);
    });
    refreshStatus();
    loadMics();
    const unlisten = listen<string>(PTT_EVENT, (e) => setPttDown(e.payload === "pressed"));
    return () => {
      unlisten.then((f) => f());
    };
  }, [refreshStatus, loadMics]);

  async function askMicPermission() {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      stream.getTracks().forEach((t) => t.stop());
      await loadMics();
    } catch (e) {
      setError(`Microphone access was not granted: ${e}`);
    }
  }

  async function run(action: () => Promise<VoiceSettings>, message: string) {
    setError(null);
    setNotice(null);
    try {
      const s = await action();
      setSaved(s);
      setDraft(s);
      setNotice(message);
      refreshStatus();
    } catch (e) {
      setError(String(e));
    }
  }

  if (!draft || !saved) return null;
  const set = <K extends keyof VoiceSettings>(key: K, value: VoiceSettings[K]) => setDraft({ ...draft, [key]: value });
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const micsHaveLabels = mics.some((m) => m.label);

  return (
    <div className="page voice">
      <header className="page-header">
        <h1>Voice Control</h1>
        <span className={`ptt ${pttDown ? "on" : ""}`} aria-live="polite">
          {saved.voice_enabled
            ? pttDown
              ? "● Listening (push-to-talk held)"
              : `Push-to-talk: ${saved.push_to_talk || "off"}`
            : "Voice is off"}
        </span>
      </header>

      <p className="muted small">
        Speech recognition and synthesis engines are not bundled. Install them manually (see
        <code> docs/voice-setup.md</code>), then point the paths below at them. Nothing is downloaded automatically.
      </p>

      <form
        className="voice-form"
        onSubmit={(e) => {
          e.preventDefault();
          run(() => voiceApi.updateSettings(draft), "Settings saved.");
        }}
      >
        <section>
          <h2>General</h2>
          <label className="checkbox">
            <input type="checkbox" checked={draft.voice_enabled} onChange={(e) => set("voice_enabled", e.target.checked)} />
            Voice enabled (registers the push-to-talk hotkey)
          </label>
          <label className="checkbox">
            <input type="checkbox" checked={draft.auto_send_to_ai} onChange={(e) => set("auto_send_to_ai", e.target.checked)} />
            Auto-send unrecognised commands to the AI
          </label>
          <label className="field">
            Push-to-talk hotkey
            <input value={draft.push_to_talk} placeholder="e.g. Ctrl+Alt+M" onChange={(e) => set("push_to_talk", e.target.value)} />
          </label>
          <label className="field">
            Wake word <span className="muted small">(leave empty to use push-to-talk only)</span>
            <input value={draft.wake_word} onChange={(e) => set("wake_word", e.target.value)} />
          </label>
          <label className="field">
            Language
            <input value={draft.language} placeholder="ru, en-US or auto" onChange={(e) => set("language", e.target.value)} />
          </label>
        </section>

        <section>
          <h2>Microphone</h2>
          <label className="field">
            Input device
            <select value={draft.microphone} onChange={(e) => set("microphone", e.target.value)}>
              <option value="default">System default</option>
              {mics
                .filter((m) => m.deviceId && m.deviceId !== "default")
                .map((m, i) => (
                  <option key={m.deviceId} value={m.deviceId}>
                    {m.label || `Microphone ${i + 1}`}
                  </option>
                ))}
            </select>
          </label>
          {!micsHaveLabels && (
            <button type="button" className="secondary" onClick={askMicPermission}>
              Show microphone names
            </button>
          )}
          <label className="field">
            Sample rate
            <select value={draft.sample_rate} onChange={(e) => set("sample_rate", Number(e.target.value))}>
              {SAMPLE_RATES.map((r) => (
                <option key={r} value={r}>
                  {r} Hz{r === 16000 ? " (recommended for Whisper)" : ""}
                </option>
              ))}
            </select>
          </label>
        </section>

        <section>
          <h2>Speech-to-text</h2>
          <label className="field">
            Provider
            <select value={draft.stt_provider} onChange={(e) => set("stt_provider", e.target.value as SttProvider)}>
              {(Object.keys(STT_LABELS) as SttProvider[]).map((p) => (
                <option key={p} value={p}>
                  {STT_LABELS[p]}
                </option>
              ))}
            </select>
          </label>
          {draft.stt_provider === "whisper_cpp" && (
            <>
              <PathField label="whisper-cli.exe path" value={draft.whisper_cpp_binary} onChange={(v) => set("whisper_cpp_binary", v)} />
              <PathField label="Model (.bin) path" value={draft.whisper_cpp_model} onChange={(v) => set("whisper_cpp_model", v)} />
            </>
          )}
          {draft.stt_provider === "faster_whisper" && (
            <PathField label="Server URL" value={draft.faster_whisper_url} onChange={(v) => set("faster_whisper_url", v)} />
          )}
        </section>

        <section>
          <h2>Text-to-speech</h2>
          <label className="field">
            Provider
            <select value={draft.tts_provider} onChange={(e) => set("tts_provider", e.target.value as TtsProvider)}>
              {(Object.keys(TTS_LABELS) as TtsProvider[]).map((p) => (
                <option key={p} value={p}>
                  {TTS_LABELS[p]}
                </option>
              ))}
            </select>
          </label>
          {draft.tts_provider === "piper" && (
            <>
              <PathField label="piper.exe path" value={draft.piper_binary} onChange={(v) => set("piper_binary", v)} />
              <PathField label="Voice (.onnx) path" value={draft.piper_voice} onChange={(v) => set("piper_voice", v)} />
            </>
          )}
        </section>

        {error && <p className="error">{error}</p>}
        {notice && <p className="status-running small">{notice}</p>}
        <div className="row-actions">
          <button type="submit" disabled={!dirty}>
            Save
          </button>
          <button type="button" className="secondary" onClick={() => run(voiceApi.resetSettings, "Settings reset to defaults.")}>
            Reset to defaults
          </button>
        </div>
      </form>

      <section className="details voice-status">
        <h2>Provider status</h2>
        <ul className="provider-list">
          {statuses.map((s) => (
            <li key={`${s.kind}-${s.id}`}>
              <span className={s.ready ? "status-running" : "muted"}>{s.ready ? "●" : "○"}</span>{" "}
              <strong>{s.name}</strong> <span className="badge">{s.kind.toUpperCase()}</span>
              <div className="small muted">{s.detail}</div>
            </li>
          ))}
        </ul>
        <p className="muted small">Status reflects saved settings; save first after changing paths.</p>
      </section>

      <section className="details voice-status">
        <h2>Intent test</h2>
        <p className="muted small">Type what you would say. The command is only classified, never executed.</p>
        <form
          className="row-actions"
          onSubmit={async (e) => {
            e.preventDefault();
            setIntent(await voiceApi.routeIntent(transcript));
          }}
        >
          <input
            className="grow"
            value={transcript}
            placeholder="e.g. «добавь задачу написать тесты»"
            onChange={(e) => setTranscript(e.target.value)}
          />
          <button type="submit" disabled={!transcript.trim()}>
            Route
          </button>
        </form>
        {intent && <pre className="code">{JSON.stringify(intent, null, 2)}</pre>}
        {intent?.intent === "unknown" && (
          <p className="muted small">Not matched locally — this would go to the voice-intent-router skill.</p>
        )}
      </section>
    </div>
  );
}

function PathField({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <label className="field">
      {label}
      <input value={value} spellCheck={false} onChange={(e) => onChange(e.target.value)} />
    </label>
  );
}
