import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ShieldAlert } from "lucide-react";
import { t } from "../i18n";
import { api, errorText, EVENTS, PendingRun } from "../lib/api";

/**
 * Asks the user to confirm dangerous actions of automations that started by
 * themselves (time, process, window…). Without an answer the run expires
 * after 5 minutes and nothing happens.
 */
export default function AutomationConfirm() {
  const [queue, setQueue] = useState<PendingRun[]>([]);
  const [error, setError] = useState<string | null>(null);
  const A = t.automations;

  useEffect(() => {
    api.automationsPending().then(setQueue, () => undefined);
    const un = listen<PendingRun>(EVENTS.automationConfirm, (e) => setQueue((q) => (q.some((x) => x.token === e.payload.token) ? q : [...q, e.payload])));
    return () => {
      un.then((f) => f());
    };
  }, []);

  const current = queue[0];
  if (!current) return null;

  async function answer(approve: boolean) {
    try {
      await api.automationsConfirm(current.token, approve);
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
    setQueue((q) => q.slice(1));
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4" role="alertdialog" aria-modal="true" aria-labelledby="ac-title">
      <div className="w-full max-w-md rounded-xl border border-zinc-200 bg-white p-5 shadow-2xl dark:border-zinc-800 dark:bg-zinc-900">
        <div className="flex gap-3">
          <ShieldAlert className="mt-0.5 size-5 shrink-0 text-red-500" />
          <div className="min-w-0">
            <h2 id="ac-title" className="text-base font-semibold">
              {A.pendingTitle(current.name)}
            </h2>
            <p className="mt-1 text-sm text-zinc-600 dark:text-zinc-300">
              {A.pendingBody(A.sources[current.source] ?? current.source, current.dangers.join(", "))}
            </p>
            <ol className="mt-2 list-decimal pl-5 text-sm">
              {current.plan.map((p, i) => (
                <li key={i} className="break-words">
                  {p}
                </li>
              ))}
            </ol>
            {error && <p className="mt-2 text-sm text-red-500">{error}</p>}
          </div>
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <button className="btn-secondary" autoFocus onClick={() => answer(false)}>
            {A.reject}
          </button>
          <button className="btn bg-red-600 text-white hover:bg-red-500" onClick={() => answer(true)}>
            {A.runAnyway}
          </button>
        </div>
      </div>
    </div>
  );
}
