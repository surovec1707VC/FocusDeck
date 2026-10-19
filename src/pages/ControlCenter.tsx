import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CATEGORY_LABELS,
  ModuleCategory,
  ModuleInfo,
  ModuleLogEntry,
  modulesApi,
  statusLabel,
} from "../lib/modules";
import ModuleSettings from "./ModuleSettings";

type Tab = "settings" | "permissions" | "logs";

export default function ControlCenter({ initialSelectedId = null }: { initialSelectedId?: string | null }) {
  const [modules, setModules] = useState<ModuleInfo[]>([]);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<ModuleCategory | "all">("all");
  const [selectedId, setSelectedId] = useState<string | null>(initialSelectedId);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setModules(await modulesApi.list());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const replace = (updated: ModuleInfo) =>
    setModules((list) => list.map((m) => (m.id === updated.id ? updated : m)));

  async function toggle(module: ModuleInfo) {
    try {
      replace(await (module.enabled ? modulesApi.disable(module.id) : modulesApi.enable(module.id)));
    } catch (e) {
      setError(String(e));
    }
  }

  const categories = useMemo(
    () => Array.from(new Set(modules.map((m) => m.category))),
    [modules],
  );

  const visible = modules.filter((m) => {
    if (category !== "all" && m.category !== category) return false;
    const q = query.trim().toLowerCase();
    return !q || m.name.toLowerCase().includes(q) || m.description.toLowerCase().includes(q);
  });

  const selected = modules.find((m) => m.id === selectedId) ?? null;

  return (
    <div className="page control-center">
      <header className="page-header">
        <h1>Control Center</h1>
        <div className="filters">
          <input
            type="search"
            placeholder="Search modules…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <select value={category} onChange={(e) => setCategory(e.target.value as ModuleCategory | "all")}>
            <option value="all">All categories</option>
            {categories.map((c) => (
              <option key={c} value={c}>
                {CATEGORY_LABELS[c]}
              </option>
            ))}
          </select>
        </div>
      </header>

      {error && <p className="error">{error}</p>}

      <div className="split">
        <ul className="module-list">
          {visible.map((m) => (
            <li
              key={m.id}
              className={m.id === selectedId ? "module-card selected" : "module-card"}
              onClick={() => setSelectedId(m.id)}
            >
              <div className="module-card-head">
                <div>
                  <strong>{m.name}</strong>
                  <span className="badge">{CATEGORY_LABELS[m.category]}</span>
                </div>
                <label className="switch" onClick={(e) => e.stopPropagation()}>
                  <input
                    type="checkbox"
                    checked={m.enabled}
                    onChange={() => toggle(m)}
                    aria-label={`${m.enabled ? "Disable" : "Enable"} ${m.name}`}
                  />
                  <span />
                </label>
              </div>
              <p className="muted">{m.description}</p>
              <span className={`status status-${m.status.state}`}>{statusLabel(m.status)}</span>
            </li>
          ))}
          {visible.length === 0 && <li className="muted">No modules match.</li>}
        </ul>

        {selected ? (
          <ModuleDetails key={selected.id} module={selected} onChange={replace} />
        ) : (
          <div className="details muted">Select a module to see its settings, permissions and logs.</div>
        )}
      </div>
    </div>
  );
}

function ModuleDetails({ module, onChange }: { module: ModuleInfo; onChange: (m: ModuleInfo) => void }) {
  const [tab, setTab] = useState<Tab>("settings");
  const [logs, setLogs] = useState<ModuleLogEntry[]>([]);

  const refreshLogs = useCallback(async () => {
    setLogs(await modulesApi.logs(module.id));
  }, [module.id]);

  async function refreshStatus() {
    const status = await modulesApi.status(module.id);
    onChange({ ...module, status });
    await refreshLogs();
  }

  useEffect(() => {
    if (tab === "logs") refreshLogs();
  }, [tab, refreshLogs, module]);

  return (
    <section className="details">
      <header className="details-head">
        <div>
          <h2>{module.name}</h2>
          <p className="muted">{module.description}</p>
        </div>
        <button className="secondary" onClick={refreshStatus}>
          Check status
        </button>
      </header>
      <p>
        Status: <span className={`status status-${module.status.state}`}>{statusLabel(module.status)}</span>
      </p>

      <nav className="tabs">
        {(["settings", "permissions", "logs"] as Tab[]).map((t) => (
          <button key={t} className={t === tab ? "tab active" : "tab"} onClick={() => setTab(t)}>
            {t[0].toUpperCase() + t.slice(1)}
          </button>
        ))}
      </nav>

      {tab === "settings" && <ModuleSettings module={module} onChange={onChange} />}

      {tab === "permissions" &&
        (module.permissions.length ? (
          <ul className="permissions">
            {module.permissions.map((p) => (
              <li key={p}>
                <code>{p}</code>
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted">No special permissions.</p>
        ))}

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
