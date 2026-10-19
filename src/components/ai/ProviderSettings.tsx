import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ENDPOINT_PRESETS,
  OpenAiEndpoint,
  PROVIDER_LABELS,
  ProviderKind,
  RouterConfig,
  aiApi,
} from "../../lib/ai";

/** Endpoints, default provider, API keys (stored in the OS keyring) and connection tests. */
export default function ProviderSettings({ onConfigChanged }: { onConfigChanged?: () => void }) {
  const [config, setConfig] = useState<RouterConfig | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    aiApi.getConfig().then(setConfig);
  }, []);

  async function save(next: RouterConfig) {
    setConfig(next);
    setError(null);
    try {
      await aiApi.setConfig(next);
      onConfigChanged?.();
    } catch (e) {
      setError(String(e));
      aiApi.getConfig().then(setConfig);
    }
  }

  if (!config) return null;

  const updateEndpoint = (id: string, patch: Partial<OpenAiEndpoint>) =>
    setConfig({
      ...config,
      openai_endpoints: config.openai_endpoints.map((e) => (e.id === id ? { ...e, ...patch } : e)),
    });

  function addEndpoint(presetId: string) {
    const preset = ENDPOINT_PRESETS.find((p) => p.id === presetId);
    const base = preset?.id ?? "custom";
    let id = base;
    for (let n = 2; config!.openai_endpoints.some((e) => e.id === id); n++) id = `${base}-${n}`;
    const endpoint: OpenAiEndpoint = {
      id,
      name: preset?.name ?? "Custom endpoint",
      base_url: preset?.base_url ?? "https://",
      default_model: "",
    };
    save({
      ...config!,
      openai_endpoints: [...config!.openai_endpoints, endpoint],
      active_openai_endpoint: config!.active_openai_endpoint || id,
    });
  }

  function removeEndpoint(id: string) {
    const rest = config!.openai_endpoints.filter((e) => e.id !== id);
    save({
      ...config!,
      openai_endpoints: rest,
      active_openai_endpoint: config!.active_openai_endpoint === id ? rest[0]?.id ?? "" : config!.active_openai_endpoint,
      default_provider:
        rest.length === 0 && config!.default_provider === "openai_compatible" ? null : config!.default_provider,
    });
  }

  const active = config.openai_endpoints.find((e) => e.id === config.active_openai_endpoint);

  return (
    <details className="provider-settings">
      <summary>
        Providers &amp; API keys
        <span className="muted small">
          {" "}
          · default:{" "}
          {config.default_provider
            ? config.default_provider === "openai_compatible"
              ? active?.name ?? PROVIDER_LABELS.openai_compatible
              : PROVIDER_LABELS[config.default_provider]
            : "each profile's own"}
        </span>
      </summary>
      {error && <p className="error">{error}</p>}

      <section>
        <h3>Default provider</h3>
        <label className="field">
          Used by all profiles except Local Private Assistant (always Ollama)
          <select
            value={config.default_provider ?? ""}
            onChange={(e) => save({ ...config, default_provider: (e.target.value || null) as ProviderKind | null })}
          >
            <option value="">Each profile's own (Anthropic)</option>
            <option value="anthropic">{PROVIDER_LABELS.anthropic}</option>
            <option value="openai_compatible" disabled={config.openai_endpoints.length === 0}>
              OpenAI-compatible: {active?.name ?? "no endpoint"}
            </option>
            <option value="ollama">{PROVIDER_LABELS.ollama}</option>
          </select>
        </label>
      </section>

      <section>
        <h3>{PROVIDER_LABELS.anthropic}</h3>
        <KeyRow provider="anthropic" />
      </section>

      <section>
        <h3>OpenAI-compatible endpoints</h3>
        <p className="muted small">
          The selected endpoint is used whenever the provider is “OpenAI-compatible”. Free tiers have rate limits and
          may use your requests for training — check each service's terms before sending work data.
        </p>
        {config.openai_endpoints.map((e) => {
          const preset = ENDPOINT_PRESETS.find((p) => p.base_url === e.base_url);
          return (
            <div key={e.id} className={e.id === config.active_openai_endpoint ? "endpoint active" : "endpoint"}>
              <label className="checkbox">
                <input
                  type="radio"
                  name="active-endpoint"
                  checked={e.id === config.active_openai_endpoint}
                  onChange={() => save({ ...config, active_openai_endpoint: e.id })}
                />
                <strong>{e.name}</strong>
                {e.id === config.active_openai_endpoint && <span className="badge">active</span>}
              </label>
              <div className="endpoint-fields">
                <label className="field">
                  Name
                  <input value={e.name} onChange={(ev) => updateEndpoint(e.id, { name: ev.target.value })} onBlur={() => save(config)} />
                </label>
                <label className="field">
                  Base URL
                  <input value={e.base_url} spellCheck={false} onChange={(ev) => updateEndpoint(e.id, { base_url: ev.target.value })} onBlur={() => save(config)} />
                </label>
                <ModelField
                  endpointId={e.id}
                  value={e.default_model}
                  onChange={(v) => updateEndpoint(e.id, { default_model: v })}
                  onCommit={(v) => save({ ...config, openai_endpoints: config.openai_endpoints.map((x) => (x.id === e.id ? { ...x, default_model: v } : x)) })}
                />
              </div>
              <KeyRow provider="openai_compatible" endpointId={e.id} keysUrl={preset?.keys_url} />
              <button className="secondary small-button" onClick={() => removeEndpoint(e.id)}>
                Remove endpoint
              </button>
            </div>
          );
        })}
        <label className="field">
          Add endpoint
          <select value="" onChange={(e) => e.target.value && addEndpoint(e.target.value)}>
            <option value="">Choose a service…</option>
            {ENDPOINT_PRESETS.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
            <option value="custom">Custom URL</option>
          </select>
        </label>
      </section>

      <section>
        <h3>{PROVIDER_LABELS.ollama}</h3>
        <label className="field">
          URL
          <input
            value={config.ollama_url}
            onChange={(e) => setConfig({ ...config, ollama_url: e.target.value })}
            onBlur={() => save(config)}
          />
        </label>
        <label className="field">
          Default model
          <input
            value={config.ollama_default_model}
            placeholder="e.g. llama3.2"
            onChange={(e) => setConfig({ ...config, ollama_default_model: e.target.value })}
            onBlur={() => save(config)}
          />
        </label>
        <KeyRow provider="ollama" />
      </section>
    </details>
  );
}

/** Default-model input with suggestions loaded from the endpoint on demand. */
function ModelField({
  endpointId,
  value,
  onChange,
  onCommit,
}: {
  endpointId: string;
  value: string;
  onChange: (v: string) => void;
  onCommit: (v: string) => void;
}) {
  const [models, setModels] = useState<string[]>([]);
  const [status, setStatus] = useState<string | null>(null);
  const listId = `endpoint-models-${endpointId}`;

  async function load() {
    setStatus("Loading…");
    try {
      const list = await aiApi.listModels("openai_compatible", endpointId);
      setModels(list.map((m) => m.id));
      setStatus(`${list.length} models`);
    } catch (e) {
      setStatus(`Could not load models: ${e}`);
    }
  }

  return (
    <label className="field">
      Default model
      <div className="row-actions">
        <input
          className="grow"
          list={listId}
          value={value}
          placeholder="pick after saving the key"
          spellCheck={false}
          onChange={(e) => onChange(e.target.value)}
          onBlur={(e) => onCommit(e.target.value)}
        />
        <button type="button" className="secondary" onClick={load}>
          Load models
        </button>
      </div>
      <datalist id={listId}>
        {models.map((m) => (
          <option key={m} value={m} />
        ))}
      </datalist>
      {status && <span className="muted small">{status}</span>}
    </label>
  );
}

function KeyRow({ provider, endpointId, keysUrl }: { provider: ProviderKind; endpointId?: string; keysUrl?: string }) {
  const needsKey = provider !== "ollama";
  const [hasKey, setHasKey] = useState(false);
  const [draft, setDraft] = useState("");
  const [result, setResult] = useState<string | null>(null);

  useEffect(() => {
    if (needsKey) aiApi.hasApiKey(provider, endpointId).then(setHasKey).catch(() => setHasKey(false));
  }, [provider, endpointId, needsKey]);

  async function act(fn: () => Promise<unknown>, ok: string) {
    setResult(null);
    try {
      await fn();
      setResult(ok);
    } catch (e) {
      setResult(`✗ ${e}`);
    }
    if (needsKey) setHasKey(await aiApi.hasApiKey(provider, endpointId).catch(() => false));
  }

  return (
    <>
      {needsKey && (
        <label className="field">
          <span>
            API key{" "}
            {hasKey ? (
              <span className="status-running small">(stored in Windows Credential Manager)</span>
            ) : (
              <span className="muted small">(not set)</span>
            )}
            {keysUrl && (
              <>
                {" "}
                ·{" "}
                <a
                  href={keysUrl}
                  className="small"
                  onClick={(e) => {
                    e.preventDefault();
                    openUrl(keysUrl);
                  }}
                >
                  get a key
                </a>
              </>
            )}
          </span>
          <input
            type="password"
            autoComplete="off"
            placeholder={hasKey ? "Enter a new key to replace" : "Paste API key"}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
        </label>
      )}
      <div className="row-actions">
        {needsKey && (
          <button
            onClick={() => act(() => aiApi.setApiKey(provider, draft, endpointId).then(() => setDraft("")), "Key saved")}
            disabled={!draft.trim()}
          >
            Save key
          </button>
        )}
        {needsKey && hasKey && (
          <button className="secondary" onClick={() => act(() => aiApi.deleteApiKey(provider, endpointId), "Key removed")}>
            Remove key
          </button>
        )}
        <button className="secondary" onClick={() => act(() => aiApi.testConnection(provider, endpointId), "✓ Connected")}>
          Test connection
        </button>
        {result && <span className="small">{result}</span>}
      </div>
    </>
  );
}
