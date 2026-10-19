import { useEffect, useState } from "react";
import { PROVIDER_LABELS, ProviderKind, RouterConfig, aiApi } from "../../lib/ai";

const KEYED: ProviderKind[] = ["anthropic", "openai_compatible"];

/** Endpoints, API keys (stored in the OS keyring) and connection tests. */
export default function ProviderSettings() {
  const [config, setConfig] = useState<RouterConfig | null>(null);
  const [hasKey, setHasKey] = useState<Partial<Record<ProviderKind, boolean>>>({});
  const [keyDraft, setKeyDraft] = useState<Partial<Record<ProviderKind, string>>>({});
  const [results, setResults] = useState<Partial<Record<ProviderKind, string>>>({});

  async function refreshKeys() {
    const entries = await Promise.all(KEYED.map(async (p) => [p, await aiApi.hasApiKey(p)] as const));
    setHasKey(Object.fromEntries(entries));
  }

  useEffect(() => {
    aiApi.getConfig().then(setConfig);
    refreshKeys();
  }, []);

  async function test(provider: ProviderKind) {
    setResults((r) => ({ ...r, [provider]: "Testing…" }));
    try {
      await aiApi.testConnection(provider);
      setResults((r) => ({ ...r, [provider]: "✓ Connected" }));
    } catch (e) {
      setResults((r) => ({ ...r, [provider]: `✗ ${e}` }));
    }
  }

  async function saveKey(provider: ProviderKind) {
    try {
      await aiApi.setApiKey(provider, keyDraft[provider] ?? "");
      setKeyDraft((d) => ({ ...d, [provider]: "" }));
      setResults((r) => ({ ...r, [provider]: "Key saved" }));
      await refreshKeys();
    } catch (e) {
      setResults((r) => ({ ...r, [provider]: String(e) }));
    }
  }

  async function removeKey(provider: ProviderKind) {
    await aiApi.deleteApiKey(provider);
    setResults((r) => ({ ...r, [provider]: "Key removed" }));
    await refreshKeys();
  }

  if (!config) return null;

  return (
    <details className="provider-settings">
      <summary>Providers &amp; API keys</summary>

      <section>
        <h3>{PROVIDER_LABELS.ollama}</h3>
        <label className="field">
          URL
          <input
            value={config.ollama_url}
            onChange={(e) => setConfig({ ...config, ollama_url: e.target.value })}
            onBlur={() => aiApi.setConfig(config)}
          />
        </label>
        <div className="row-actions">
          <button className="secondary" onClick={() => test("ollama")}>
            Test connection
          </button>
          <span className="small">{results.ollama}</span>
        </div>
      </section>

      <section>
        <h3>{PROVIDER_LABELS.openai_compatible}</h3>
        <label className="field">
          Base URL
          <input
            value={config.openai_base_url}
            onChange={(e) => setConfig({ ...config, openai_base_url: e.target.value })}
            onBlur={() => aiApi.setConfig(config)}
          />
        </label>
        <KeyRow
          provider="openai_compatible"
          hasKey={hasKey.openai_compatible}
          draft={keyDraft.openai_compatible ?? ""}
          onDraft={(v) => setKeyDraft((d) => ({ ...d, openai_compatible: v }))}
          onSave={() => saveKey("openai_compatible")}
          onRemove={() => removeKey("openai_compatible")}
          onTest={() => test("openai_compatible")}
          result={results.openai_compatible}
        />
      </section>

      <section>
        <h3>{PROVIDER_LABELS.anthropic}</h3>
        <KeyRow
          provider="anthropic"
          hasKey={hasKey.anthropic}
          draft={keyDraft.anthropic ?? ""}
          onDraft={(v) => setKeyDraft((d) => ({ ...d, anthropic: v }))}
          onSave={() => saveKey("anthropic")}
          onRemove={() => removeKey("anthropic")}
          onTest={() => test("anthropic")}
          result={results.anthropic}
        />
      </section>
    </details>
  );
}

function KeyRow(props: {
  provider: ProviderKind;
  hasKey?: boolean;
  draft: string;
  onDraft: (v: string) => void;
  onSave: () => void;
  onRemove: () => void;
  onTest: () => void;
  result?: string;
}) {
  return (
    <>
      <label className="field">
        API key {props.hasKey ? <span className="status-running small">(stored in Windows Credential Manager)</span> : <span className="muted small">(not set)</span>}
        <input
          type="password"
          autoComplete="off"
          placeholder={props.hasKey ? "Enter a new key to replace" : "Paste API key"}
          value={props.draft}
          onChange={(e) => props.onDraft(e.target.value)}
        />
      </label>
      <div className="row-actions">
        <button onClick={props.onSave} disabled={!props.draft.trim()}>
          Save key
        </button>
        {props.hasKey && (
          <button className="secondary" onClick={props.onRemove}>
            Remove key
          </button>
        )}
        <button className="secondary" onClick={props.onTest}>
          Test connection
        </button>
        <span className="small">{props.result}</span>
      </div>
    </>
  );
}
