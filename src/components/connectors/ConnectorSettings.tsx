import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import ModuleSettings from "../../pages/ModuleSettings";
import { ModuleInfo, ModuleLogEntry } from "../../lib/modules";
import {
  AUTH_LABELS,
  ConnectorInfo,
  connectorStatusLabel,
  connectorsApi,
  healthCheckLabel,
} from "../../lib/connectors";

type Tab = "settings" | "permissions" | "logs";

interface Props {
  connector: ConnectorInfo;
  onChange: (c: ConnectorInfo) => void;
}

export default function ConnectorSettings({ connector, onChange }: Props) {
  const m = connector.manifest;
  const [tab, setTab] = useState<Tab>("settings");
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [logs, setLogs] = useState<ModuleLogEntry[]>([]);

  const refreshLogs = useCallback(() => connectorsApi.logs(m.id).then(setLogs), [m.id]);
  useEffect(() => {
    if (tab === "logs") refreshLogs();
  }, [tab, refreshLogs, connector]);

  async function run(action: () => Promise<ConnectorInfo>) {
    setError(null);
    try {
      onChange(await action());
    } catch (e) {
      setError(String(e));
    }
  }

  async function test() {
    setTesting(true);
    await run(() => connectorsApi.test(m.id));
    setTesting(false);
  }

  // Reuse the schema-driven settings form from the module pages.
  const asModule: ModuleInfo = {
    id: m.id,
    name: m.name,
    description: m.description,
    category: "integrations",
    enabled: connector.enabled,
    requires_restart: false,
    permissions: m.permissions,
    settings_schema: m.settings_schema,
    settings: connector.settings,
    status: { state: "stopped" },
  };

  return (
    <section className="details">
      <header className="details-head">
        <div>
          <h2>{m.name}</h2>
          <p className="muted">{m.description}</p>
        </div>
        <button className="secondary" onClick={test} disabled={testing}>
          {testing ? "Testing…" : "Test connection"}
        </button>
      </header>
      <p className="small">
        <span className={`status status-${connector.status.state === "ok" ? "running" : connector.status.state === "error" ? "error" : "stopped"}`}>
          {connectorStatusLabel(connector)}
        </span>{" "}
        · {AUTH_LABELS[m.auth_type]} · check: {healthCheckLabel(m.health_check)} ·{" "}
        <a
          href={m.docs_url}
          onClick={(e) => {
            e.preventDefault();
            openUrl(m.docs_url);
          }}
        >
          docs
        </a>
      </p>
      {error && <p className="error">{error}</p>}

      <nav className="tabs">
        {(["settings", "permissions", "logs"] as Tab[]).map((t) => (
          <button key={t} className={t === tab ? "tab active" : "tab"} onClick={() => setTab(t)}>
            {{ settings: "Settings", permissions: "Permissions & actions", logs: "Logs" }[t]}
          </button>
        ))}
      </nav>

      {tab === "settings" && (
        <div className="connector-settings">
          {(m.auth_type === "api_key" || m.auth_type === "oauth") && (
            <SecretForm connector={connector} onChange={onChange} onError={setError} />
          )}
          <ModuleSettings
            module={asModule}
            onChange={() => {}}
            save={(settings) => connectorsApi.updateSettings(m.id, settings).then(onChange)}
            reset={() => connectorsApi.resetSettings(m.id).then(onChange)}
          />
        </div>
      )}

      {tab === "permissions" && (
        <div className="skill-overview">
          <h3>Permissions</h3>
          <ul className="permissions">
            {m.permissions.map((p) => (
              <li key={p}>
                <code>{p}</code>
              </li>
            ))}
          </ul>
          <h3>Actions</h3>
          <p className="muted small">Declared only — actions are not implemented yet.</p>
          <ul className="permissions">
            {m.actions.map((a) => (
              <li key={a.id}>
                <code>{a.id}</code> — {a.description}{" "}
                {a.mutating && <span className="log-warn small">(needs confirmation)</span>}
              </li>
            ))}
          </ul>
          <h3>Events</h3>
          {m.events.length ? (
            <ul className="permissions">
              {m.events.map((e) => (
                <li key={e.id}>
                  <code>{e.id}</code> — {e.description}
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">None.</p>
          )}
        </div>
      )}

      {tab === "logs" &&
        (logs.length ? (
          <ol className="logs">
            {logs
              .slice()
              .reverse()
              .map((l, i) => (
                <li key={i} className={`log-${l.level}`}>
                  <time>{new Date(l.at).toLocaleTimeString()}</time> {l.message}
                </li>
              ))}
          </ol>
        ) : (
          <p className="muted">No log entries yet.</p>
        ))}
    </section>
  );
}

function SecretForm({
  connector,
  onChange,
  onError,
}: {
  connector: ConnectorInfo;
  onChange: (c: ConnectorInfo) => void;
  onError: (e: string | null) => void;
}) {
  const m = connector.manifest;
  const [value, setValue] = useState("");
  const label = m.auth_type === "oauth" ? "Access token" : "API key";

  async function save() {
    onError(null);
    try {
      onChange(await connectorsApi.setSecret(m.id, value));
      setValue("");
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <div className="secret-form">
      <label className="field">
        {label}{" "}
        {connector.has_secret ? (
          <span className="status-running small">(stored in Windows Credential Manager)</span>
        ) : (
          <span className="muted small">(not set)</span>
        )}
        <input
          type="password"
          autoComplete="off"
          value={value}
          placeholder={connector.has_secret ? "Enter a new value to replace" : `Paste ${label.toLowerCase()}`}
          onChange={(e) => setValue(e.target.value)}
        />
      </label>
      {m.auth_type === "oauth" && (
        <p className="muted small">OAuth sign-in is not implemented yet; you can paste an access token for now.</p>
      )}
      {m.secret_ref && <p className="muted small">Shared with the AI Assistant's provider settings.</p>}
      <div className="row-actions">
        <button onClick={save} disabled={!value.trim()}>
          Save {label.toLowerCase()}
        </button>
        {connector.has_secret && (
          <button className="secondary" onClick={() => connectorsApi.deleteSecret(m.id).then(onChange)}>
            Remove
          </button>
        )}
      </div>
    </div>
  );
}
