import { useCallback, useEffect, useMemo, useState } from "react";
import ConnectorCard from "../components/connectors/ConnectorCard";
import ConnectorSettings from "../components/connectors/ConnectorSettings";
import { ConnectorInfo, LoadIssue, connectorsApi } from "../lib/connectors";

export default function Connectors() {
  const [connectors, setConnectors] = useState<ConnectorInfo[]>([]);
  const [issues, setIssues] = useState<LoadIssue[]>([]);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (reload = false) => {
    try {
      setConnectors(await (reload ? connectorsApi.reload() : connectorsApi.list()));
      setIssues(await connectorsApi.issues());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const replace = (updated: ConnectorInfo) =>
    setConnectors((list) => list.map((c) => (c.manifest.id === updated.manifest.id ? updated : c)));

  async function toggle(c: ConnectorInfo) {
    try {
      replace(await connectorsApi.setEnabled(c.manifest.id, !c.enabled));
    } catch (e) {
      setError(String(e));
    }
  }

  const categories = useMemo(
    () => Array.from(new Set(connectors.map((c) => c.manifest.category))).sort(),
    [connectors],
  );
  const visible = connectors.filter((c) => {
    if (category !== "all" && c.manifest.category !== category) return false;
    const q = query.trim().toLowerCase();
    return !q || c.manifest.name.toLowerCase().includes(q) || c.manifest.description.toLowerCase().includes(q);
  });
  const selected = connectors.find((c) => c.manifest.id === selectedId) ?? null;

  return (
    <div className="page">
      <header className="page-header">
        <h1>Connectors</h1>
        <div className="filters">
          <input
            type="search"
            placeholder="Search connectors…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <select value={category} onChange={(e) => setCategory(e.target.value)}>
            <option value="all">All categories</option>
            {categories.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
          <button className="secondary" onClick={() => load(true)} title="Re-read connectors/*.json">
            Reload
          </button>
        </div>
      </header>

      {error && <p className="error">{error}</p>}
      {issues.length > 0 && (
        <div className="issues">
          <strong>Manifests not loaded</strong>
          {issues.map((i) => (
            <p key={i.file} className="small error">
              {i.file}: {i.errors.join("; ")}
            </p>
          ))}
        </div>
      )}

      <div className="split">
        <ul className="module-list">
          {visible.map((c) => (
            <ConnectorCard
              key={c.manifest.id}
              connector={c}
              selected={c.manifest.id === selectedId}
              onSelect={() => setSelectedId(c.manifest.id)}
              onToggle={() => toggle(c)}
            />
          ))}
          {visible.length === 0 && <li className="muted">No connectors match.</li>}
        </ul>
        {selected ? (
          <ConnectorSettings key={selected.manifest.id} connector={selected} onChange={replace} />
        ) : (
          <div className="details muted">Select a connector to configure it, test the connection and see its logs.</div>
        )}
      </div>
    </div>
  );
}
