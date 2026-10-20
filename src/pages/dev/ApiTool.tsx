import { useCallback, useEffect, useState } from "react";
import { ChevronDown, ChevronRight, FolderPlus, History, Pencil, Plus, Save, Send, Terminal, Trash2, X } from "lucide-react";
import ConfirmDialog from "../../components/ConfirmDialog";
import { Badge, ErrorBanner, Field } from "../../components/ui";
import { formatDateTime, t } from "../../i18n";
import { api, ApiEnvironment, ApiHistoryEntry, ApiPair, ApiRequestSpec, ApiResponse, ApiSavedRequest, ApiWorkspace, BodyType, errorText, HttpMethod } from "../../lib/api";
import { useGuarded } from "../../lib/use-guarded";
import { formatBytes } from "../../components/widgets/common";
import { CopyButton, PairsEditor } from "./common";

const METHODS: HttpMethod[] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
const METHOD_TONE: Record<string, string> = { GET: "text-emerald-600", POST: "text-amber-600", PUT: "text-sky-600", PATCH: "text-violet-600", DELETE: "text-red-600" };
const EMPTY: ApiRequestSpec = { method: "GET", url: "", headers: [], query: [], body: "", body_type: "none" };

interface Current {
  id?: string;
  name: string;
  collection_id: string | null;
  spec: ApiRequestSpec;
}

const spec = (r: ApiRequestSpec): ApiRequestSpec => ({ method: r.method, url: r.url, headers: r.headers, query: r.query, body: r.body, body_type: r.body_type });

function prettyBody(r: ApiResponse): string {
  const ct = r.headers.find(([k]) => k.toLowerCase() === "content-type")?.[1] ?? "";
  if (r.binary || !(ct.includes("json") || /^\s*[[{]/.test(r.body))) return r.body;
  try {
    return JSON.stringify(JSON.parse(r.body), null, 2);
  } catch {
    return r.body;
  }
}

export default function ApiTool() {
  const A = t.dev.api;
  const [ws, setWs] = useState<ApiWorkspace>({ collections: [], loose: [] });
  const [envs, setEnvs] = useState<ApiEnvironment[]>([]);
  const [history, setHistory] = useState<ApiHistoryEntry[]>([]);
  const [side, setSide] = useState<"collections" | "history">("collections");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [cur, setCur] = useState<Current>({ name: A.newRequest, collection_id: null, spec: EMPTY });
  const [part, setPart] = useState<"query" | "headers" | "body">("query");
  const [resp, setResp] = useState<ApiResponse | null>(null);
  const [view, setView] = useState<"pretty" | "raw" | "headers">("pretty");
  const [sending, setSending] = useState(false);
  const [curl, setCurl] = useState<{ style: "bash" | "cmd" | "powershell"; text: string } | null>(null);
  const [saveAs, setSaveAs] = useState<{ name: string; collection_id: string | null } | null>(null);
  const [envEdit, setEnvEdit] = useState<{ id?: string; name: string; vars: ApiPair[] } | null>(null);
  const [newCol, setNewCol] = useState<string | null>(null);
  const [delCol, setDelCol] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { run, dialog } = useGuarded(setError);

  const reload = useCallback(async () => {
    try {
      const [w, e, h] = await Promise.all([api.apiWorkspace(), api.apiEnvironments(), api.apiHistory()]);
      setWs(w);
      setEnvs(e);
      setHistory(h);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);
  useEffect(() => {
    reload();
  }, [reload]);

  const setSpec = (patch: Partial<ApiRequestSpec>) => setCur((c) => ({ ...c, spec: { ...c.spec, ...patch } }));
  const openSaved = (r: ApiSavedRequest) => {
    setCur({ id: r.id, name: r.name, collection_id: r.collection_id, spec: spec(r) });
    setResp(null);
  };

  function send() {
    setSending(true);
    run(
      (c) => api.apiSend(cur.spec, c),
      (r) => {
        setResp(r);
        setView("pretty");
      },
    ).finally(() => {
      setSending(false);
      api.apiHistory().then(setHistory);
    });
  }

  async function save(asNew?: { name: string; collection_id: string | null }) {
    try {
      const input = { ...cur.spec, name: asNew?.name ?? cur.name, collection_id: asNew ? asNew.collection_id : cur.collection_id };
      const r = await api.apiRequestSave(input, asNew ? undefined : cur.id);
      setCur({ id: r.id, name: r.name, collection_id: r.collection_id, spec: spec(r) });
      setSaveAs(null);
      setWs(await api.apiWorkspace());
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function exportCurl(style: "bash" | "cmd" | "powershell") {
    try {
      setCurl({ style, text: await api.apiToCurl(cur.spec, style) });
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function newCollection() {
    if (!newCol?.trim()) return;
    await api.apiCollectionSave(newCol).catch((e) => setError(errorText(e)));
    setNewCol(null);
    setWs(await api.apiWorkspace());
  }

  async function saveEnv() {
    if (!envEdit) return;
    try {
      await api.apiEnvironmentSave(envEdit.name, envEdit.vars, envEdit.id);
      setEnvs(await api.apiEnvironments());
      setEnvEdit(null);
    } catch (e) {
      setError(errorText(e));
    }
  }

  const activeEnv = envs.find((e) => e.active);
  const requestItem = (r: ApiSavedRequest) => (
    <li key={r.id} className="group flex items-center">
      <button className={`flex min-w-0 flex-1 items-center gap-2 rounded px-2 py-1 text-left ${cur.id === r.id ? "bg-accent-500/10" : "hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"}`} onClick={() => openSaved(r)}>
        <span className={`w-12 shrink-0 font-mono text-[10px] font-bold ${METHOD_TONE[r.method] ?? "text-zinc-500"}`}>{r.method}</span>
        <span className="truncate">{r.name}</span>
      </button>
      <button
        className="px-1 text-red-500 opacity-0 group-hover:opacity-60 hover:opacity-100"
        title={t.common.delete}
        onClick={async () => {
          await api.apiRequestDelete(r.id);
          if (cur.id === r.id) setCur((c) => ({ ...c, id: undefined }));
          setWs(await api.apiWorkspace());
        }}
      >
        <Trash2 className="size-3.5" />
      </button>
    </li>
  );

  return (
    <div className="flex flex-col gap-3">
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="grid gap-3 lg:grid-cols-[16rem_minmax(0,1fr)]">
        <aside className="card flex flex-col gap-3 p-3">
          <Field label={A.environment}>
            <div className="flex gap-1">
              <select
                className="input min-w-0 flex-1"
                value={activeEnv?.id ?? ""}
                onChange={async (e) => {
                  await api.apiEnvironmentActivate(e.target.value || null);
                  setEnvs(await api.apiEnvironments());
                }}
              >
                <option value="">{A.noEnvironment}</option>
                {envs.map((e) => (
                  <option key={e.id} value={e.id}>
                    {e.name}
                  </option>
                ))}
              </select>
              <button className="btn-ghost px-1.5" title={A.editEnvironment} onClick={() => setEnvEdit(activeEnv ? { id: activeEnv.id, name: activeEnv.name, vars: activeEnv.vars } : { name: A.newEnvironment, vars: [] })}>
                <Pencil className="size-4" />
              </button>
              <button className="btn-ghost px-1.5" title={A.newEnvironment} onClick={() => setEnvEdit({ name: A.newEnvironment, vars: [{ name: "base", value: "", enabled: true }] })}>
                <Plus className="size-4" />
              </button>
            </div>
          </Field>
          <div className="flex gap-1 text-xs">
            <button className={`flex-1 rounded px-2 py-1 ${side === "collections" ? "bg-zinc-200 font-medium dark:bg-zinc-800" : "text-zinc-500"}`} onClick={() => setSide("collections")}>
              {A.collections}
            </button>
            <button className={`flex-1 rounded px-2 py-1 ${side === "history" ? "bg-zinc-200 font-medium dark:bg-zinc-800" : "text-zinc-500"}`} onClick={() => setSide("history")}>
              {A.history}
            </button>
          </div>
          {side === "collections" ? (
            <div className="flex flex-col gap-2">
              <div className="flex gap-1">
                <button className="btn-secondary flex-1 py-1 text-xs" onClick={() => (setCur({ name: A.newRequest, collection_id: null, spec: EMPTY }), setResp(null))}>
                  <Plus className="size-3.5" />
                  {A.newRequest}
                </button>
                <button className="btn-ghost px-1.5" title={A.newCollection} onClick={() => setNewCol("")}>
                  <FolderPlus className="size-4" />
                </button>
              </div>
              {newCol !== null && (
                <input
                  className="input text-xs"
                  autoFocus
                  placeholder={A.collectionName}
                  value={newCol}
                  onChange={(e) => setNewCol(e.target.value)}
                  onKeyDown={(e) => (e.key === "Enter" ? newCollection() : e.key === "Escape" && setNewCol(null))}
                  onBlur={() => !newCol.trim() && setNewCol(null)}
                />
              )}
              {ws.collections.map((c) => (
                <div key={c.id}>
                  <div className="group flex items-center">
                    <button
                      className="flex min-w-0 flex-1 items-center gap-1 text-sm font-medium"
                      onClick={() => setCollapsed((s) => (s.has(c.id) ? new Set([...s].filter((x) => x !== c.id)) : new Set([...s, c.id])))}
                    >
                      {collapsed.has(c.id) ? <ChevronRight className="size-4" /> : <ChevronDown className="size-4" />}
                      <span className="truncate">{c.name}</span>
                      <span className="text-xs text-zinc-500">({c.requests.length})</span>
                    </button>
                    <button className="px-1 text-red-500 opacity-0 group-hover:opacity-60 hover:opacity-100" title={t.common.delete} onClick={() => setDelCol(c.id)}>
                      <Trash2 className="size-3.5" />
                    </button>
                  </div>
                  {!collapsed.has(c.id) && <ul className="ml-3">{c.requests.map(requestItem)}</ul>}
                </div>
              ))}
              {ws.loose.length > 0 && (
                <div>
                  <div className="text-xs text-zinc-500">{A.noCollection}</div>
                  <ul>{ws.loose.map(requestItem)}</ul>
                </div>
              )}
            </div>
          ) : (
            <div className="flex min-h-0 flex-col gap-1">
              {history.length === 0 && <p className="text-xs text-zinc-500">{A.historyEmpty}</p>}
              <ul className="flex max-h-[28rem] flex-col overflow-y-auto">
                {history.map((h) => (
                  <li key={h.id}>
                    <button
                      className="w-full rounded px-2 py-1 text-left hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"
                      title={formatDateTime(h.created_at)}
                      onClick={() => (setCur({ name: A.newRequest, collection_id: null, spec: h.request }), setResp(null))}
                    >
                      <div className="flex items-center gap-1.5">
                        <span className={`font-mono text-[10px] font-bold ${METHOD_TONE[h.method] ?? "text-zinc-500"}`}>{h.method}</span>
                        {h.status !== null ? <Badge tone={h.status < 400 ? "ok" : "error"}>{h.status}</Badge> : <Badge tone="error">ERR</Badge>}
                        <span className="text-xs text-zinc-500">{h.duration_ms} мс</span>
                      </div>
                      <div className="truncate font-mono text-xs">{h.url}</div>
                    </button>
                  </li>
                ))}
              </ul>
              {history.length > 0 && (
                <button className="self-start text-xs text-red-500 hover:underline" onClick={() => api.apiHistoryClear().then(() => setHistory([]))}>
                  <History className="mr-1 inline size-3" />
                  {A.clearHistory}
                </button>
              )}
            </div>
          )}
        </aside>

        <div className="flex min-w-0 flex-col gap-3">
          <div className="card flex flex-col gap-3 p-3">
            <div className="flex items-center gap-2">
              <input className="input min-w-0 flex-1 font-medium" value={cur.name} onChange={(e) => setCur({ ...cur, name: e.target.value })} aria-label={A.requestName} />
              <button className="btn-secondary" onClick={() => save()} title={cur.id ? t.dev.save : t.dev.saveAs}>
                <Save className="size-4" />
                {t.dev.save}
              </button>
              <button className="btn-ghost" onClick={() => setSaveAs({ name: cur.name, collection_id: cur.collection_id })}>
                {t.dev.saveAs}
              </button>
            </div>
            <div className="flex gap-2">
              <select className={`input w-28 font-mono font-bold ${METHOD_TONE[cur.spec.method] ?? ""}`} value={cur.spec.method} onChange={(e) => setSpec({ method: e.target.value as HttpMethod })}>
                {METHODS.map((m) => (
                  <option key={m}>{m}</option>
                ))}
              </select>
              <input className="input min-w-0 flex-1 font-mono text-xs" placeholder={A.url} value={cur.spec.url} onChange={(e) => setSpec({ url: e.target.value })} onKeyDown={(e) => e.key === "Enter" && send()} spellCheck={false} />
              <button className="btn-primary" onClick={send} disabled={sending || !cur.spec.url.trim()}>
                <Send className="size-4" />
                {sending ? A.sending : A.send}
              </button>
            </div>
            <div className="flex gap-1 border-b border-zinc-200 text-xs dark:border-zinc-800">
              {(["query", "headers", "body"] as const).map((p) => (
                <button key={p} className={`-mb-px border-b-2 px-3 py-1.5 ${part === p ? "border-accent-500 font-medium" : "border-transparent text-zinc-500"}`} onClick={() => setPart(p)}>
                  {p === "query" ? A.params : p === "headers" ? A.headers : A.body}
                  {p === "query" && cur.spec.query.length > 0 && ` (${cur.spec.query.length})`}
                  {p === "headers" && cur.spec.headers.length > 0 && ` (${cur.spec.headers.length})`}
                </button>
              ))}
            </div>
            {part === "query" && <PairsEditor rows={cur.spec.query} onChange={(query) => setSpec({ query })} />}
            {part === "headers" && <PairsEditor rows={cur.spec.headers} onChange={(headers) => setSpec({ headers })} />}
            {part === "body" && (
              <div className="flex flex-col gap-2">
                <div className="flex flex-wrap gap-3 text-xs">
                  {(["none", "json", "text", "form"] as BodyType[]).map((b) => (
                    <label key={b} className="flex items-center gap-1">
                      <input type="radio" checked={cur.spec.body_type === b} onChange={() => setSpec({ body_type: b })} />
                      {A.bodyTypes[b]}
                    </label>
                  ))}
                </div>
                {cur.spec.body_type !== "none" && (
                  <textarea
                    className="input min-h-36 font-mono text-xs"
                    value={cur.spec.body}
                    onChange={(e) => setSpec({ body: e.target.value })}
                    placeholder={cur.spec.body_type === "json" ? '{\n  "name": "{{user}}"\n}' : cur.spec.body_type === "form" ? "a=1&b={{value}}" : ""}
                    spellCheck={false}
                  />
                )}
              </div>
            )}
            <div className="flex flex-wrap items-center gap-1.5 border-t border-zinc-200 pt-2 text-xs dark:border-zinc-800">
              <Terminal className="size-3.5 text-zinc-500" />
              <span className="text-zinc-500">{A.curl}:</span>
              {(["bash", "cmd", "powershell"] as const).map((s) => (
                <button key={s} className={`rounded px-2 py-0.5 ${curl?.style === s ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"}`} onClick={() => exportCurl(s)}>
                  {A.curlStyles[s]}
                </button>
              ))}
            </div>
            {curl && (
              <div className="relative">
                <pre className="max-h-48 overflow-auto rounded-md bg-zinc-950 p-3 pr-20 font-mono text-xs text-zinc-200 select-text">{curl.text}</pre>
                <div className="absolute top-1 right-1 flex">
                  <CopyButton text={curl.text} />
                  <button className="btn-ghost px-1.5 text-zinc-400" onClick={() => setCurl(null)} aria-label={t.dev.close}>
                    <X className="size-4" />
                  </button>
                </div>
              </div>
            )}
            <p className="text-xs text-zinc-500">{A.permission}</p>
          </div>

          <div className="card flex min-h-64 flex-col overflow-hidden">
            {resp ? (
              <>
                <div className="flex flex-wrap items-center gap-2 border-b border-zinc-200 px-3 py-2 dark:border-zinc-800">
                  <Badge tone={resp.status < 300 ? "ok" : resp.status < 400 ? "accent" : "error"}>
                    {resp.status} {resp.status_text}
                  </Badge>
                  <span className="text-xs text-zinc-500">{resp.duration_ms} мс</span>
                  <span className="text-xs text-zinc-500">{formatBytes(resp.size)}</span>
                  <span className="flex-1" />
                  {(["pretty", "raw", "headers"] as const).map((v) => (
                    <button key={v} className={`rounded px-2 py-0.5 text-xs ${view === v ? "bg-zinc-200 dark:bg-zinc-800" : "text-zinc-500"}`} onClick={() => setView(v)}>
                      {v === "pretty" ? A.pretty : v === "raw" ? A.raw : `${A.responseHeaders} (${resp.headers.length})`}
                    </button>
                  ))}
                  <CopyButton text={view === "headers" ? resp.headers.map(([k, v]) => `${k}: ${v}`).join("\n") : view === "pretty" ? prettyBody(resp) : resp.body} />
                </div>
                {resp.missing_vars.length > 0 && <p className="px-3 pt-2 text-xs text-amber-600">{A.missingVars(resp.missing_vars.join(", "))}</p>}
                {resp.truncated && <p className="px-3 pt-2 text-xs text-amber-600">{A.truncated}</p>}
                {view === "headers" ? (
                  <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-0.5 overflow-auto p-3 font-mono text-xs select-text">
                    {resp.headers.map(([k, v], i) => (
                      <div key={i} className="contents">
                        <dt className="text-zinc-500">{k}</dt>
                        <dd className="break-all">{v}</dd>
                      </div>
                    ))}
                  </dl>
                ) : (
                  <pre className="max-h-[36rem] overflow-auto p-3 font-mono text-xs whitespace-pre-wrap select-text">{view === "pretty" ? prettyBody(resp) : resp.body}</pre>
                )}
              </>
            ) : (
              <p className="p-3 text-zinc-500">{A.responseEmpty}</p>
            )}
          </div>
        </div>
      </div>

      {saveAs && (
        <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-label={t.dev.saveAs}>
          <div className="card flex w-full max-w-md flex-col gap-3 p-4">
            <h2 className="text-lg font-semibold">{t.dev.saveAs}</h2>
            <Field label={A.requestName}>
              <input className="input" autoFocus value={saveAs.name} onChange={(e) => setSaveAs({ ...saveAs, name: e.target.value })} />
            </Field>
            <Field label={A.collections}>
              <select className="input" value={saveAs.collection_id ?? ""} onChange={(e) => setSaveAs({ ...saveAs, collection_id: e.target.value || null })}>
                <option value="">{A.noCollection}</option>
                {ws.collections.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </Field>
            <div className="flex justify-end gap-2">
              <button className="btn-secondary" onClick={() => setSaveAs(null)}>
                {t.common.cancel}
              </button>
              <button className="btn-primary" onClick={() => save(saveAs)}>
                {t.dev.save}
              </button>
            </div>
          </div>
        </div>
      )}

      {envEdit && (
        <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-label={A.editEnvironment}>
          <div className="card flex max-h-[85vh] w-full max-w-2xl flex-col gap-3 overflow-y-auto p-4">
            <h2 className="text-lg font-semibold">{A.editEnvironment}</h2>
            <p className="text-xs text-zinc-500">{A.envHint}</p>
            <Field label={A.name}>
              <input className="input" value={envEdit.name} onChange={(e) => setEnvEdit({ ...envEdit, name: e.target.value })} />
            </Field>
            <PairsEditor rows={envEdit.vars} onChange={(vars) => setEnvEdit({ ...envEdit, vars })} />
            <div className="flex justify-end gap-2">
              {envEdit.id && (
                <button
                  className="btn-danger mr-auto"
                  onClick={async () => {
                    await api.apiEnvironmentDelete(envEdit.id!);
                    setEnvs(await api.apiEnvironments());
                    setEnvEdit(null);
                  }}
                >
                  <Trash2 className="size-4" />
                  {t.common.delete}
                </button>
              )}
              <button className="btn-secondary" onClick={() => setEnvEdit(null)}>
                {t.common.cancel}
              </button>
              <button className="btn-primary" onClick={saveEnv}>
                {t.dev.save}
              </button>
            </div>
          </div>
        </div>
      )}

      <ConfirmDialog
        open={delCol !== null}
        title={t.common.delete}
        confirmLabel={t.common.delete}
        danger
        onCancel={() => setDelCol(null)}
        onConfirm={async () => {
          const id = delCol!;
          setDelCol(null);
          await api.apiCollectionDelete(id).catch((e) => setError(errorText(e)));
          setWs(await api.apiWorkspace());
        }}
      >
        {A.deleteCollection}
      </ConfirmDialog>
      {dialog}
    </div>
  );
}
