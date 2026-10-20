import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { ChevronDown, ChevronUp, RotateCcw, ShieldCheck, ShieldOff, Undo2 } from "lucide-react";
import { Badge, Empty, ErrorBanner, PageHeader, Toggle } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { AccessEntry, ActionLogEntry, api, errorText, EVENTS, PermissionCategory, PermissionInfo, SafeMode, SafetyAction } from "../lib/api";
import { useSettings } from "../lib/settings-context";

const C = t.permissionsCenter;
const ORDER: PermissionCategory[] = ["files", "clipboard", "windows", "keyboard", "mouse", "screen", "processes", "network", "notifications", "system"];

/** Permissions Center: permissions, Safe Mode and the Action Log. */
export default function Permissions() {
  const [tab, setTab] = useState<"permissions" | "safeMode" | "actions">("permissions");
  const [error, setError] = useState<string | null>(null);
  return (
    <>
      <PageHeader title={C.title} subtitle={C.subtitle} />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="mb-4 flex gap-1" role="tablist">
        {(["permissions", "safeMode", "actions"] as const).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "btn-secondary" : "btn-ghost"} onClick={() => setTab(k)}>
            {C.tabs[k]}
          </button>
        ))}
      </div>
      {tab === "permissions" && <PermissionList onError={setError} />}
      {tab === "safeMode" && <SafeModePanel onError={setError} />}
      {tab === "actions" && <ActionLog onError={setError} />}
    </>
  );
}

function PermissionList({ onError }: { onError: (m: string | null) => void }) {
  const [list, setList] = useState<PermissionInfo[]>([]);
  const [open, setOpen] = useState<string | null>(null);
  const [log, setLog] = useState<AccessEntry[]>([]);
  const { reload: reloadSettings } = useSettings();
  const P = t.permissions;

  const load = useCallback(async () => {
    try {
      setList(await api.permissionsCenter());
    } catch (e) {
      onError(errorText(e));
    }
  }, [onError]);

  useEffect(() => {
    load();
    const un = listen(EVENTS.actionLog, () => load());
    return () => {
      un.then((f) => f());
    };
  }, [load]);

  useEffect(() => {
    if (open) api.permissionsAccessLog(open, 50).then(setLog, (e) => onError(errorText(e)));
  }, [open, list, onError]);

  async function toggle(p: PermissionInfo) {
    try {
      await api.permissionsSet(p.id, !p.granted);
      onError(null);
      // Revoking can switch off autostart / hotkeys in settings.
      reloadSettings();
      load();
    } catch (e) {
      onError(errorText(e));
    }
  }

  return (
    <div className="flex flex-col gap-4">
      {ORDER.map((cat) => {
        const items = list.filter((p) => p.category === cat);
        if (items.length === 0) return null;
        return (
          <section key={cat}>
            <h2 className="mb-2 text-xs font-semibold tracking-wide text-zinc-500 uppercase">{C.categories[cat]}</h2>
            <ul className="card divide-y divide-zinc-200 py-0 dark:divide-zinc-800">
              {items.map((p) => (
                <li key={p.id} className="py-3">
                  <div className="flex items-start gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-medium">{p.name}</span>
                        <code className="text-xs text-zinc-500">{p.id}</code>
                        <Badge tone={p.granted ? "ok" : "neutral"}>{p.granted ? P.granted : P.denied}</Badge>
                      </div>
                      <div className="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">{p.description}</div>
                      <div className="mt-1.5 flex flex-wrap items-center gap-1 text-xs">
                        <span className="text-zinc-500">{C.usedBy}:</span>
                        {p.used_by.length === 0 && <span className="text-zinc-500">{C.unused}</span>}
                        {p.used_by.map((m) => (
                          <Link key={m.id} to="/control-center" className={`rounded-full px-2 py-0.5 ${m.enabled ? "bg-accent-500/15 text-accent-700 dark:text-accent-300" : "bg-zinc-200 text-zinc-500 dark:bg-zinc-800"}`}>
                            {m.name}
                          </Link>
                        ))}
                      </div>
                      <div className="mt-1 flex flex-wrap gap-3 text-xs text-zinc-500">
                        <span>{C.uses(p.uses)}</span>
                        {p.denials > 0 && <span className="text-amber-600">{C.denials(p.denials)}</span>}
                        {p.last_used_at && (
                          <span>
                            {C.lastUsed}: {formatDateTime(p.last_used_at)}
                          </span>
                        )}
                        <button className="flex items-center gap-1 text-accent-600 dark:text-accent-400" onClick={() => setOpen(open === p.id ? null : p.id)} aria-expanded={open === p.id}>
                          {open === p.id ? <ChevronUp className="size-3" /> : <ChevronDown className="size-3" />} {C.accessLog}
                        </button>
                      </div>
                    </div>
                    <Toggle checked={p.granted} onChange={() => toggle(p)} label={p.name} />
                  </div>
                  {open === p.id && (
                    <ul className="mt-2 rounded-md bg-zinc-100 p-2 text-xs dark:bg-zinc-900">
                      {log.length === 0 && <li className="text-zinc-500">{C.noAccess}</li>}
                      {log.map((l, i) => (
                        <li key={i} className="flex gap-2 py-0.5">
                          <span className="w-36 shrink-0 text-zinc-500">{formatDateTime(l.created_at)}</span>
                          <Badge tone={l.kind === "used" || l.kind === "granted" ? "ok" : l.kind === "denied" || l.kind === "blocked" || l.kind === "error" ? "error" : "warn"}>
                            {C.kinds[l.kind] ?? l.kind}
                          </Badge>
                          <span className="text-zinc-500">{P.sources[l.source] ?? l.source}</span>
                          <span className="truncate">{l.detail}</span>
                        </li>
                      ))}
                    </ul>
                  )}
                </li>
              ))}
            </ul>
          </section>
        );
      })}
    </div>
  );
}

function SafeModePanel({ onError }: { onError: (m: string | null) => void }) {
  const [mode, setMode] = useState<SafeMode | null>(null);
  const [actions, setActions] = useState<SafetyAction[]>([]);

  useEffect(() => {
    Promise.all([api.safeModeGet(), api.safetyActions()]).then(([m, a]) => {
      setMode(m);
      setActions(a);
    }, (e) => onError(errorText(e)));
  }, [onError]);

  async function save(next: SafeMode) {
    try {
      setMode(await api.safeModeSet(next));
      onError(null);
    } catch (e) {
      onError(errorText(e));
    }
  }

  if (!mode) return null;
  const modules = [...new Set(actions.map((a) => a.module))];
  const tone = { read: "neutral", change: "warn", dangerous: "error" } as const;
  return (
    <div className="flex flex-col gap-4">
      <div className={`card flex items-center gap-3 ${mode.enabled ? "border-accent-500/60 dark:border-accent-500/60" : ""}`}>
        {mode.enabled ? <ShieldCheck className="size-6 text-accent-500" /> : <ShieldOff className="size-6 text-zinc-400" />}
        <div className="min-w-0 flex-1">
          <div className="font-semibold">{mode.enabled ? C.safeModeOn : C.safeModeOff}</div>
          <p className="text-xs text-zinc-500">{C.safeModeHint}</p>
        </div>
        <Toggle checked={mode.enabled} onChange={(v) => save({ ...mode, enabled: v })} label={C.safeModeTitle} />
      </div>
      <section className="card">
        <header className="mb-2 flex items-center justify-between">
          <h2 className="font-semibold">{C.whitelist}</h2>
          <button className="btn-ghost text-xs" onClick={() => save({ ...mode, whitelist: actions.filter((a) => a.risk === "read").map((a) => a.id) })}>
            <RotateCcw className="size-3.5" /> {C.whitelistReset}
          </button>
        </header>
        {modules.map((m) => (
          <div key={m} className="mb-3">
            <h3 className="mb-1 text-xs font-semibold text-zinc-500">{m}</h3>
            <ul className="flex flex-col gap-0.5">
              {actions
                .filter((a) => a.module === m)
                .map((a) => {
                  const on = mode.whitelist.includes(a.id);
                  return (
                    <li key={a.id}>
                      <label className="flex cursor-pointer items-center gap-2 rounded-md px-1 py-1 hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60">
                        <input
                          type="checkbox"
                          className="size-4 accent-accent-600"
                          checked={on}
                          onChange={() => save({ ...mode, whitelist: on ? mode.whitelist.filter((x) => x !== a.id) : [...mode.whitelist, a.id] })}
                        />
                        <span className="flex-1 text-sm">{a.name}</span>
                        <code className="hidden text-xs text-zinc-500 sm:inline">{a.id}</code>
                        <Badge tone={tone[a.risk]}>{C.risks[a.risk]}</Badge>
                        {a.undoable && <Badge>{C.undoable}</Badge>}
                      </label>
                    </li>
                  );
                })}
            </ul>
          </div>
        ))}
      </section>
    </div>
  );
}

function ActionLog({ onError }: { onError: (m: string | null) => void }) {
  const [entries, setEntries] = useState<ActionLogEntry[]>([]);
  const [module, setModule] = useState<string>("");
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setEntries(await api.actionLogList(module || undefined, 300));
    } catch (e) {
      onError(errorText(e));
    }
  }, [module, onError]);

  useEffect(() => {
    load();
    const un = listen(EVENTS.actionLog, () => load());
    return () => {
      un.then((f) => f());
    };
  }, [load]);

  const modules = [...new Set(entries.map((e) => e.module_id))];
  return (
    <div className="flex flex-col gap-3">
      {notice && <div className="rounded-lg bg-accent-500/10 px-3 py-2 text-accent-700 dark:text-accent-300">{notice}</div>}
      <select className="input w-60" value={module} onChange={(e) => setModule(e.target.value)} aria-label={C.module}>
        <option value="">{C.allModules}</option>
        {modules.map((m) => (
          <option key={m}>{m}</option>
        ))}
      </select>
      {entries.length === 0 ? (
        <Empty>{C.noActions}</Empty>
      ) : (
        <div className="card overflow-x-auto p-0">
          <table className="w-full text-left text-sm">
            <thead className="text-xs text-zinc-500">
              <tr className="border-b border-zinc-200 dark:border-zinc-800">
                <th className="px-3 py-2 font-medium">{C.time}</th>
                <th className="px-3 py-2 font-medium">{C.module}</th>
                <th className="px-3 py-2 font-medium">{C.action}</th>
                <th className="px-3 py-2 font-medium">{C.target}</th>
                <th className="px-3 py-2 font-medium">{C.result}</th>
                <th className="px-3 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-200 dark:divide-zinc-800">
              {entries.map((e) => (
                <tr key={e.id} className={e.undone_at ? "opacity-60" : ""}>
                  <td className="px-3 py-2 text-xs whitespace-nowrap text-zinc-500">{formatDateTime(e.created_at)}</td>
                  <td className="px-3 py-2 text-xs">{e.module_id}</td>
                  <td className="px-3 py-2">
                    <code className="text-xs">{e.action}</code>
                  </td>
                  <td className="max-w-56 truncate px-3 py-2 text-xs" title={e.target}>
                    {e.target}
                  </td>
                  <td className="px-3 py-2">
                    <Badge tone={e.result === "ok" ? "ok" : e.result === "undone" ? "neutral" : e.result === "error" ? "error" : "warn"}>{C.results[e.result]}</Badge>
                    <div className="max-w-64 truncate text-xs text-zinc-500" title={e.message}>
                      {e.message}
                    </div>
                  </td>
                  <td className="px-3 py-2">
                    {e.can_undo && (
                      <button
                        className="btn-ghost px-2 text-xs"
                        onClick={async () => {
                          try {
                            setNotice(C.undone(await api.actionLogUndo(e.id)));
                            onError(null);
                          } catch (err) {
                            onError(errorText(err));
                          }
                        }}
                      >
                        <Undo2 className="size-3.5" /> {C.undo}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
