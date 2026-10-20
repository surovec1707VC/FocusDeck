import { KeyboardEvent, useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ExternalLink, FolderOpen, History, Play, Plus, Square, Star, Trash2, X, Eraser, CornerDownLeft } from "lucide-react";
import ConfirmDialog from "../../components/ConfirmDialog";
import { ErrorBanner } from "../../components/ui";
import { formatDateTime, t } from "../../i18n";
import { api, ApiError, errorText, EVENTS, ShellId, ShellInfo, TerminalFavorite, TerminalHistoryEntry, TerminalOutput } from "../../lib/api";
import { useGuarded } from "../../lib/use-guarded";
import { newId } from "./common";
import { SHELL_NAMES } from "./ProjectsTool";

interface Line {
  kind: "cmd" | "stdout" | "stderr" | "info";
  text: string;
}

interface Tab {
  id: string;
  shell: ShellId;
  cwd: string;
  lines: Line[];
  running: string | null;
  input: string;
}

const STORAGE_KEY = "focusdeck.terminal.tabs";
const MAX_LINES_CHARS = 2_000_000;

function loadTabs(): Tab[] {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]") as { shell: ShellId; cwd: string }[];
    return saved.map((s) => ({ id: newId(), shell: s.shell, cwd: s.cwd, lines: [], running: null, input: "" }));
  } catch {
    return [];
  }
}

export default function TerminalTool() {
  const T = t.dev.terminal;
  const [shells, setShells] = useState<ShellInfo[]>([]);
  const [tabs, setTabs] = useState<Tab[]>(loadTabs);
  const [active, setActive] = useState<string | null>(null);
  const [history, setHistory] = useState<TerminalHistoryEntry[]>([]);
  const [histQuery, setHistQuery] = useState("");
  const [favorites, setFavorites] = useState<TerminalFavorite[]>([]);
  const [favName, setFavName] = useState<string | null>(null);
  const [histPos, setHistPos] = useState(-1);
  const [pending, setPending] = useState<{ message: string; command: string; target: Tab } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { run, dialog } = useGuarded(setError);
  const outRef = useRef<HTMLPreElement>(null);
  const runs = useRef(new Map<string, string>()); // run id → tab id

  const tab = tabs.find((x) => x.id === active) ?? tabs[0];
  const patch = (id: string, f: (t: Tab) => Partial<Tab>) => setTabs((list) => list.map((x) => (x.id === id ? { ...x, ...f(x) } : x)));
  const append = (id: string, line: Line) =>
    patch(id, (x) => {
      const lines = [...x.lines];
      const last = lines[lines.length - 1];
      // Chunks of the same stream are merged into one block.
      if (last && last.kind === line.kind && line.kind !== "cmd" && line.kind !== "info") lines[lines.length - 1] = { ...last, text: last.text + line.text };
      else lines.push(line);
      let total = lines.reduce((n, l) => n + l.text.length, 0);
      while (total > MAX_LINES_CHARS && lines.length > 1) total -= lines.shift()!.text.length;
      return { lines };
    });

  const loadHistory = useCallback(() => api.terminalHistory(histQuery || undefined).then(setHistory, (e) => setError(errorText(e))), [histQuery]);

  useEffect(() => {
    api.terminalShells().then(setShells, (e) => setError(errorText(e)));
    api.terminalFavorites().then(setFavorites, (e) => setError(errorText(e)));
    api.terminalHome().then((home) => setTabs((list) => (list.length ? list : [{ id: newId(), shell: "powershell", cwd: home, lines: [], running: null, input: "" }])));
    const un = listen<TerminalOutput>(EVENTS.terminalOutput, (e) => {
      const tabId = runs.current.get(e.payload.run_id);
      if (tabId) append(tabId, { kind: e.payload.stream, text: e.payload.text });
    });
    return () => {
      un.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const id = setTimeout(loadHistory, 150);
    return () => clearTimeout(id);
  }, [loadHistory]);

  // Tabs (shell and folder) survive restarts; output doesn't.
  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(tabs.map((x) => ({ shell: x.shell, cwd: x.cwd }))));
    } catch {
      /* storage unavailable */
    }
  }, [tabs]);

  useEffect(() => {
    outRef.current?.scrollTo(0, outRef.current.scrollHeight);
  }, [tab?.lines]);

  function newTab(shell: ShellId) {
    const id = newId();
    setTabs((list) => [...list, { id, shell, cwd: tab?.cwd ?? "C:\\", lines: [], running: null, input: "" }]);
    setActive(id);
  }

  function closeTab(id: string) {
    const x = tabs.find((y) => y.id === id);
    if (x?.running) api.terminalCancel(x.running);
    setTabs((list) => list.filter((y) => y.id !== id));
    if (active === id) setActive(null);
  }

  /** Runs a command in a tab; Safe Mode may ask to confirm first (`confirm` = the retry). */
  async function execute(command: string, target = tab, confirm = false) {
    if (!target || !command.trim() || target.running) return;
    const runId = newId();
    runs.current.set(runId, target.id);
    if (!confirm) append(target.id, { kind: "cmd", text: command });
    patch(target.id, () => ({ running: runId, input: "" }));
    setHistPos(-1);
    try {
      const r = await api.terminalRun(runId, target.shell, target.cwd, command, confirm);
      const status = r.cancelled ? T.cancelled : r.timed_out ? T.timedOut : T.exit(r.code, r.duration_ms);
      append(target.id, { kind: "info", text: status });
      patch(target.id, () => ({ cwd: r.cwd }));
    } catch (e) {
      if (e instanceof ApiError && e.kind === "confirmation_required" && !confirm) setPending({ message: e.message, command, target });
      else append(target.id, { kind: "stderr", text: errorText(e) });
    } finally {
      runs.current.delete(runId);
      patch(target.id, () => ({ running: null }));
      loadHistory();
    }
  }

  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (!tab) return;
    if (e.key === "Enter") {
      e.preventDefault();
      execute(tab.input);
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      const next = Math.max(-1, Math.min(history.length - 1, histPos + (e.key === "ArrowUp" ? 1 : -1)));
      setHistPos(next);
      patch(tab.id, () => ({ input: next < 0 ? "" : history[next].command }));
    } else if (e.key === "c" && e.ctrlKey && tab.running) {
      e.preventDefault();
      api.terminalCancel(tab.running);
    }
  }

  async function pickFolder() {
    const dir = await api.devPickFolder(T.folder);
    if (dir && tab) patch(tab.id, () => ({ cwd: dir }));
  }

  async function saveFavorite() {
    if (!tab || !favName?.trim() || !tab.input.trim()) return;
    try {
      await api.terminalFavoriteSave({ name: favName, shell: tab.shell, command: tab.input, cwd: tab.cwd });
      setFavorites(await api.terminalFavorites());
      setFavName(null);
    } catch (e) {
      setError(errorText(e));
    }
  }

  const available = (id: ShellId) => shells.find((s) => s.id === id)?.available ?? false;

  return (
    <div className="flex flex-col gap-3">
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="flex flex-wrap items-center gap-1.5">
        {shells.map((s) => (
          <div key={s.id} className="flex">
            <button className="btn-secondary rounded-r-none py-1 text-xs" disabled={!s.available} title={s.available ? (s.path ?? "") : s.id === "custom" ? T.customHint : T.notInstalled} onClick={() => newTab(s.id)}>
              <Plus className="size-3.5" />
              {s.name}
            </button>
            <button
              className="btn-secondary rounded-l-none border-l-0 px-1.5 py-1"
              disabled={!s.available || !tab}
              title={T.openWindow}
              onClick={() => tab && run((c) => api.terminalOpen(s.id, tab.cwd, undefined, c))}
            >
              <ExternalLink className="size-3.5" />
            </button>
          </div>
        ))}
      </div>

      <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="card flex min-h-[28rem] flex-col overflow-hidden">
          <div className="flex items-center gap-1 overflow-x-auto border-b border-zinc-200 px-1 pt-1 dark:border-zinc-800">
            {tabs.map((x) => (
              <div
                key={x.id}
                className={`group flex shrink-0 items-center gap-1 rounded-t-md px-2.5 py-1.5 text-xs ${x.id === tab?.id ? "bg-zinc-200 font-medium dark:bg-zinc-800" : "text-zinc-500 hover:bg-zinc-200/50 dark:hover:bg-zinc-800/50"}`}
              >
                <button onClick={() => setActive(x.id)} title={x.cwd}>
                  {x.running && <span className="mr-1 inline-block size-1.5 animate-pulse rounded-full bg-emerald-500" />}
                  {SHELL_NAMES[x.shell]} · {x.cwd.split(/[\\/]/).filter(Boolean).pop() ?? x.cwd}
                </button>
                <button className="opacity-50 hover:opacity-100" onClick={() => closeTab(x.id)} aria-label={t.dev.close}>
                  <X className="size-3" />
                </button>
              </div>
            ))}
          </div>
          {tab ? (
            <>
              <div className="flex items-center gap-2 border-b border-zinc-200 px-3 py-1.5 text-xs dark:border-zinc-800">
                <button className="flex min-w-0 items-center gap-1 text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200" onClick={pickFolder} title={T.folder}>
                  <FolderOpen className="size-3.5 shrink-0" />
                  <span className="truncate font-mono">{tab.cwd}</span>
                </button>
                {!available(tab.shell) && shells.length > 0 && <span className="text-red-500">{T.notInstalled}</span>}
                <span className="flex-1" />
                <button className="btn-ghost px-1.5 py-0.5" title={T.clear} onClick={() => patch(tab.id, () => ({ lines: [] }))}>
                  <Eraser className="size-3.5" />
                </button>
              </div>
              <pre ref={outRef} className="min-h-0 flex-1 overflow-auto bg-zinc-950 p-3 font-mono text-xs leading-5 whitespace-pre-wrap text-zinc-200 select-text">
                {tab.lines.length === 0 && <span className="text-zinc-500">{T.noninteractive}</span>}
                {tab.lines.map((l, i) => (
                  <div key={i} className={l.kind === "cmd" ? "mt-1 text-sky-400" : l.kind === "stderr" ? "text-red-400" : l.kind === "info" ? "text-zinc-500" : ""}>
                    {l.kind === "cmd" ? `> ${l.text}` : l.kind === "info" ? `[${l.text}]` : l.text}
                  </div>
                ))}
              </pre>
              <div className="flex items-center gap-1.5 border-t border-zinc-200 p-2 dark:border-zinc-800">
                <input
                  className="input flex-1 font-mono text-xs"
                  placeholder={T.placeholder}
                  value={tab.input}
                  onChange={(e) => patch(tab.id, () => ({ input: e.target.value }))}
                  onKeyDown={onKey}
                  autoFocus
                  spellCheck={false}
                />
                {tab.running ? (
                  <button className="btn-danger" onClick={() => api.terminalCancel(tab.running!)}>
                    <Square className="size-4" />
                    {T.stop}
                  </button>
                ) : (
                  <button className="btn-primary" disabled={!tab.input.trim()} onClick={() => execute(tab.input)}>
                    <Play className="size-4" />
                    {T.run}
                  </button>
                )}
                <button className="btn-ghost px-1.5" title={T.addFavorite} disabled={!tab.input.trim()} onClick={() => setFavName("")}>
                  <Star className="size-4" />
                </button>
              </div>
              {favName !== null && (
                <div className="flex items-center gap-1.5 border-t border-zinc-200 p-2 dark:border-zinc-800">
                  <input className="input flex-1" autoFocus placeholder={T.favoriteName} value={favName} onChange={(e) => setFavName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && saveFavorite()} />
                  <button className="btn-primary" onClick={saveFavorite}>
                    {t.dev.save}
                  </button>
                  <button className="btn-ghost" onClick={() => setFavName(null)}>
                    {t.common.cancel}
                  </button>
                </div>
              )}
            </>
          ) : (
            <p className="p-4 text-zinc-500">{T.newTab}</p>
          )}
        </div>

        <aside className="flex flex-col gap-3">
          <div className="card p-3">
            <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold tracking-wide text-zinc-500 uppercase">
              <Star className="size-3.5" />
              {T.favorites}
            </h3>
            {favorites.length === 0 && <p className="text-xs text-zinc-500">{T.favoritesEmpty}</p>}
            <ul className="flex flex-col gap-1">
              {favorites.map((f) => (
                <li key={f.id} className="group rounded px-1.5 py-1 hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60">
                  <div className="flex items-center gap-1">
                    <span className="min-w-0 flex-1 truncate text-sm font-medium">{f.name}</span>
                    <button
                      className="opacity-60 hover:opacity-100"
                      title={T.runHere}
                      onClick={() => {
                        if (!tab) return;
                        const target = { ...tab, shell: f.shell, cwd: f.cwd ?? tab.cwd };
                        patch(tab.id, () => ({ shell: target.shell, cwd: target.cwd }));
                        execute(f.command, target);
                      }}
                    >
                      <Play className="size-3.5" />
                    </button>
                    <button className="opacity-60 hover:opacity-100" title={T.insert} onClick={() => tab && patch(tab.id, () => ({ input: f.command }))}>
                      <CornerDownLeft className="size-3.5" />
                    </button>
                    <button className="opacity-60 hover:opacity-100" title={T.openWindow} onClick={() => run((c) => api.terminalOpen(f.shell, f.cwd ?? tab?.cwd ?? "C:\\", f.command, c))}>
                      <ExternalLink className="size-3.5" />
                    </button>
                    <button
                      className="text-red-500 opacity-0 group-hover:opacity-60 hover:opacity-100"
                      title={t.common.delete}
                      onClick={async () => {
                        await api.terminalFavoriteDelete(f.id);
                        setFavorites(await api.terminalFavorites());
                      }}
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                  </div>
                  <code className="block truncate text-xs text-zinc-500">
                    {SHELL_NAMES[f.shell]}: {f.command}
                  </code>
                </li>
              ))}
            </ul>
          </div>
          <div className="card flex min-h-0 flex-col p-3">
            <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold tracking-wide text-zinc-500 uppercase">
              <History className="size-3.5" />
              {T.history}
              <span className="flex-1" />
              {history.length > 0 && (
                <button className="font-normal normal-case text-red-500 hover:underline" onClick={() => api.terminalHistoryClear().then(loadHistory)}>
                  {T.clearHistory}
                </button>
              )}
            </h3>
            <input className="input mb-2 text-xs" placeholder={T.historySearch} value={histQuery} onChange={(e) => setHistQuery(e.target.value)} />
            {history.length === 0 && <p className="text-xs text-zinc-500">{T.historyEmpty}</p>}
            <ul className="flex max-h-80 flex-col overflow-y-auto">
              {history.map((h) => (
                <li key={h.id}>
                  <button
                    className="w-full rounded px-1.5 py-1 text-left hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"
                    title={`${SHELL_NAMES[h.shell]} · ${h.cwd} · ${formatDateTime(h.created_at)}`}
                    onClick={() => tab && patch(tab.id, () => ({ input: h.command }))}
                  >
                    <code className={`block truncate text-xs ${h.exit_code ? "text-red-500" : ""}`}>{h.command}</code>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </aside>
      </div>
      <ConfirmDialog
        open={pending !== null}
        title={t.dev.confirmTitle}
        confirmLabel={t.dev.confirm}
        danger
        onCancel={() => {
          if (pending) append(pending.target.id, { kind: "info", text: T.cancelled });
          setPending(null);
        }}
        onConfirm={() => {
          const p = pending!;
          setPending(null);
          execute(p.command, p.target, true);
        }}
      >
        {pending?.message}
      </ConfirmDialog>
      {dialog}
    </div>
  );
}
