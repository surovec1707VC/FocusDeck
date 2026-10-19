import { useEffect, useState } from "react";
import { CATEGORY_LABELS, ModuleInfo, modulesApi, statusLabel } from "../lib/modules";

/** Compact overview table of all modules. */
export default function Modules({ onOpen }: { onOpen: (id: string) => void }) {
  const [modules, setModules] = useState<ModuleInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    modulesApi.list().then(setModules).catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="page">
      <header className="page-header">
        <h1>Modules</h1>
      </header>
      {error && <p className="error">{error}</p>}
      <table className="table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Category</th>
            <th>Enabled</th>
            <th>Status</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {modules.map((m) => (
            <tr key={m.id}>
              <td>{m.name}</td>
              <td>{CATEGORY_LABELS[m.category]}</td>
              <td>{m.enabled ? "Yes" : "No"}</td>
              <td>{statusLabel(m.status)}</td>
              <td>
                <button className="secondary" onClick={() => onOpen(m.id)}>
                  Open
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
