import { FormEvent, KeyboardEvent, ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import {
  AppWindow, ArrowDown, ArrowUp, ClipboardCopy, ClipboardPaste, Code2, EyeOff, File, FileText, FolderOpen, Globe, Image as ImageIcon,
  Link2, LucideIcon, Pencil, Pin, PinOff, Play, Plus, Search, ShieldAlert, Terminal, Trash2, Type, X,
} from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import HotkeyInput from "../components/HotkeyInput";
import SettingsForm from "../components/SettingsForm";
import { formatBytes } from "../components/widgets/common";
import { Badge, Empty, ErrorBanner, Field, PageHeader } from "../components/ui";
import { formatDateTime, locale, t } from "../i18n";
import { api, ApiError, ClipDetails, ClipItem, ClipKind, CommandOutput, errorText, EVENTS, HubStatus, QuickAction, QuickActionInput, QuickActionKind } from "../lib/api";
import { useModules } from "../lib/modules-context";

const C = t.clipboard;
const KINDS: ClipKind[] = ["text", "link", "files", "image", "html", "rtf"];
const KIND_ICON: Record<ClipKind, LucideIcon> = { text: Type, link: Link2, files: File, image: ImageIcon, html: Code2, rtf: FileText };
const ACTION_KINDS: QuickActionKind[] = ["open_app", "open_url", "open_folder", "open_file", "paste_text", "copy_text", "command"];
const ACTION_ICON: Record<QuickActionKind, LucideIcon> = {
  open_app: AppWindow, open_url: Globe, open_folder: FolderOpen, open_file: File, paste_text: ClipboardPaste, copy_text: ClipboardCopy, command: Terminal,
};
const PAGE = 100;

export default function ClipboardHub() {
  const [tab, setTab] = useState<"history" | "actions" | "settings">("history");
  const [error, setError] = useState<string | null>(null);
  return (
    <>
      <PageHeader title={C.title} subtitle={C.subtitle} />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="mb-4 flex gap-1" role="tablist">
        {(["history", "actions", "settings"] as const).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "btn-secondary" : "btn-ghost"} onClick={() => setTab(k)}>
            {C.tabs[k]}
          </button>
        ))}
      </div>
      {tab === "history" && <History onError={setError} />}
      {tab === "actions" && <Actions onError={setError} />}
      {tab === "settings" && <HubSettings onError={setError} />}
    </>
  );
}

// --- history ---

function History({ onError }: { onError: (m: string | null) => void }) {
  const [items, setItems] = useState<ClipItem[]>([]);
  const [total, setTotal] = useState(0);
  const [status, setStatus] = useState<HubStatus | null>(null);
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState<ClipKind | null>(null);
  const [pinnedOnly, setPinnedOnly] = useState(false);
  const [limit, setLimit] = useState(PAGE);
  const [selected, setSelected] = useState(0);
  const [open, setOpen] = useState<number | null>(null);
  const [flash, setFlash] = useState<number | null>(null);
  const [clearing, setClearing] = useState(false);
  const privateMinutes = Number(useModules().settingsOf("clipboard_history").private_minutes ?? 15);
  const list = useRef<HTMLUListElement>(null);

  const load = useCallback(async () => {
    try {
      const [page, s] = await Promise.all([api.clipboardList({ search: search || undefined, kind: kind ?? undefined, pinned_only: pinnedOnly, limit }), api.clipboardStatus()]);
      setItems(page.items);
      setTotal(page.total);
      setStatus(s);
    } catch (e) {
      onError(errorText(e));
    }
  }, [search, kind, pinnedOnly, limit, onError]);

  useEffect(() => {
    const id = setTimeout(load, 120);
    return () => clearTimeout(id);
  }, [load]);

  useEffect(() => {
    const subs = [listen(EVENTS.clipboardChanged, () => load()), listen(EVENTS.clipboardPrivate, () => load())];
    return () => subs.forEach((u) => u.then((f) => f()));
  }, [load]);

  async function run(fn: () => Promise<unknown>, reload = true) {
    try {
      await fn();
      onError(null);
      if (reload) load();
      return true;
    } catch (e) {
      onError(errorText(e));
      return false;
    }
  }

  async function copy(c: ClipItem) {
    if (await run(() => api.clipboardCopy(c.id), false)) {
      setFlash(c.id);
      setTimeout(() => setFlash(null), 1200);
    }
  }

  function onKey(e: KeyboardEvent) {
    const c = items[selected];
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.max(0, Math.min(items.length - 1, selected + (e.key === "ArrowDown" ? 1 : -1)));
      setSelected(next);
      list.current?.children[next]?.scrollIntoView({ block: "nearest" });
    } else if (c && e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) run(() => api.clipboardPaste(c.id), false);
      else copy(c);
    } else if (c && e.key === "Delete" && (e.target as HTMLElement).tagName !== "INPUT") {
      run(() => api.clipboardDelete(c.id));
    }
  }

  const pm = status?.private_mode;
  return (
    <div onKeyDown={onKey}>
      {status && (
        <div className="card mb-3 flex flex-wrap items-center gap-3 py-2.5">
          {pm?.active ? (
            <Badge tone="warn">
              <EyeOff className="mr-1 size-3.5" />
              {C.privateOn} {pm.until && C.privateUntil(new Date(pm.until).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" }))}
            </Badge>
          ) : (
            <Badge tone={status.recording ? "ok" : "neutral"}>{status.recording ? C.recording : C.notRecording}</Badge>
          )}
          {!status.permission && (
            <span className="flex items-center gap-1 text-xs text-amber-600 dark:text-amber-400">
              <ShieldAlert className="size-3.5" /> {C.noPermission}
              <Link to="/permissions" className="text-accent-600 hover:underline dark:text-accent-400">
                {t.widgets.openPermissions}
              </Link>
            </span>
          )}
          <span className="text-xs text-zinc-500">{C.total(status.stats.total, formatBytes(status.stats.bytes))}</span>
          <span className="ml-auto flex gap-1">
            {pm?.active ? (
              <button className="btn-secondary" onClick={() => run(() => api.clipboardSetPrivate(false))}>
                {C.privateStop}
              </button>
            ) : (
              <button className="btn-secondary" onClick={() => run(() => api.clipboardSetPrivate(true, privateMinutes))} title={C.privateFor(privateMinutes)}>
                <EyeOff className="size-4" /> {C.privateStart} · {C.privateFor(privateMinutes)}
              </button>
            )}
            <button className="btn-danger" onClick={() => setClearing(true)} disabled={status.stats.total === 0}>
              <Trash2 className="size-4" /> {C.clear}
            </button>
          </span>
        </div>
      )}

      <div className="mb-3 flex flex-wrap items-center gap-2">
        <label className="relative min-w-52 flex-1 sm:max-w-sm">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400" />
          <input className="input pl-8" placeholder={C.search} value={search} autoFocus onChange={(e) => setSearch(e.target.value)} aria-label={C.search} />
        </label>
        <div className="flex flex-wrap gap-1" role="tablist">
          {[null, ...KINDS].map((k) => {
            const count = k ? status?.stats.by_kind.find(([x]) => x === k)?.[1] : undefined;
            return (
              <button
                key={k ?? "all"}
                role="tab"
                aria-selected={kind === k}
                className={`rounded-full px-3 py-1 text-xs font-medium ${kind === k ? "bg-accent-600 text-white" : "bg-zinc-200 hover:bg-zinc-300 dark:bg-zinc-800 dark:hover:bg-zinc-700"}`}
                onClick={() => setKind(k)}
              >
                {k ? C.kinds[k] : C.all}
                {count ? <span className="ml-1 opacity-60">{count}</span> : null}
              </button>
            );
          })}
          <button
            className={`rounded-full px-3 py-1 text-xs font-medium ${pinnedOnly ? "bg-accent-600 text-white" : "bg-zinc-200 hover:bg-zinc-300 dark:bg-zinc-800 dark:hover:bg-zinc-700"}`}
            onClick={() => setPinnedOnly(!pinnedOnly)}
            aria-pressed={pinnedOnly}
          >
            <Pin className="mr-1 inline size-3" />
            {C.pinnedOnly}
          </button>
        </div>
      </div>
      <p className="mb-2 text-xs text-zinc-500">{C.keyboardHint}</p>

      {items.length === 0 && <Empty>{search || kind || pinnedOnly ? C.noMatches : C.empty}</Empty>}
      <ul ref={list} className="flex flex-col gap-1.5">
        {items.map((c, i) => {
          const Icon = KIND_ICON[c.kind] ?? Type;
          return (
            <li
              key={c.id}
              onClick={() => setSelected(i)}
              className={`card group flex flex-col gap-1 p-3 ${i === selected ? "border-accent-500/60 dark:border-accent-500/60" : ""} ${c.pinned ? "bg-accent-500/5" : ""}`}
            >
              <div className="flex items-start gap-2">
                <Icon className="mt-0.5 size-4 shrink-0 text-accent-500" aria-label={C.kinds[c.kind]} />
                <div className="min-w-0 flex-1">
                  <ClipPreview c={c} />
                  <div className="mt-0.5 flex flex-wrap gap-x-2 text-[11px] text-zinc-500">
                    <span>{formatDateTime(c.created_at)}</span>
                    {c.source_app && <span>{C.from(c.source_app)}</span>}
                    <span>{formatBytes(c.size)}</span>
                    {(c.has_html || c.has_rtf) && <span>{[c.has_html && "HTML", c.has_rtf && "RTF"].filter(Boolean).join(" + ")}</span>}
                    {flash === c.id && <span className="text-accent-600 dark:text-accent-400">{C.copied}</span>}
                  </div>
                </div>
                <div className={`flex shrink-0 ${c.pinned || i === selected ? "" : "opacity-60 group-hover:opacity-100"}`}>
                  <IconBtn label={C.copy} onClick={() => copy(c)}>
                    <ClipboardCopy className="size-4" />
                  </IconBtn>
                  <IconBtn label={C.pasteHint} onClick={() => run(() => api.clipboardPaste(c.id), false)}>
                    <ClipboardPaste className="size-4" />
                  </IconBtn>
                  <IconBtn label={c.pinned ? C.unpin : C.pin} onClick={() => run(() => api.clipboardPin(c.id, !c.pinned))}>
                    {c.pinned ? <PinOff className="size-4 text-accent-500" /> : <Pin className="size-4" />}
                  </IconBtn>
                  <IconBtn label={C.details} onClick={() => setOpen(open === c.id ? null : c.id)}>
                    <Pencil className="size-4" />
                  </IconBtn>
                  <IconBtn label={t.common.delete} danger onClick={() => run(() => api.clipboardDelete(c.id))}>
                    <X className="size-4" />
                  </IconBtn>
                </div>
              </div>
              {open === c.id && <Details c={c} onError={onError} />}
            </li>
          );
        })}
      </ul>
      {total > items.length && (
        <button className="btn-ghost mt-3 w-full" onClick={() => setLimit(limit + PAGE)}>
          {C.loadMore} ({total - items.length})
        </button>
      )}

      <ConfirmDialog
        open={clearing}
        title={C.clear}
        confirmLabel={t.common.delete}
        danger
        onCancel={() => setClearing(false)}
        onConfirm={() => {
          setClearing(false);
          run(() => api.clipboardClear(false));
        }}
      >
        {C.clearConfirm}
      </ConfirmDialog>
    </div>
  );
}

function IconBtn({ label, onClick, children, danger }: { label: string; onClick: () => void; children: ReactNode; danger?: boolean }) {
  return (
    <button
      className={`rounded p-1.5 text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-800 ${danger ? "hover:text-red-500" : "hover:text-zinc-700 dark:hover:text-zinc-200"}`}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      title={label}
      aria-label={label}
    >
      {children}
    </button>
  );
}

function ClipPreview({ c }: { c: ClipItem }) {
  if (c.kind === "files") {
    const files: string[] = c.files_json ? JSON.parse(c.files_json) : c.content.split("\n");
    return (
      <div className="text-sm">
        <span className="text-xs text-zinc-500">{C.files(files.length)}</span>
        {files.slice(0, 3).map((f) => (
          <div key={f} className="truncate font-mono text-xs" title={f}>
            {f}
          </div>
        ))}
        {files.length > 3 && <div className="text-xs text-zinc-500">…</div>}
      </div>
    );
  }
  if (c.kind === "image") return <div className="text-sm">{c.image_width && c.image_height ? `${c.content}` : c.content}</div>;
  if (c.kind === "link") return <div className="truncate text-sm text-accent-600 dark:text-accent-400">{c.content}</div>;
  return <div className="line-clamp-3 text-sm break-words whitespace-pre-wrap">{c.content}</div>;
}

/** Full content; images are loaded only here. HTML/RTF are shown as text, never rendered. */
function Details({ c, onError }: { c: ClipItem; onError: (m: string | null) => void }) {
  const [d, setD] = useState<ClipDetails | null>(null);
  const [image, setImage] = useState<string | null>(null);
  useEffect(() => {
    api.clipboardDetails(c.id).then(setD, (e) => onError(errorText(e)));
    if (c.kind === "image") api.clipboardImage(c.id).then(setImage, (e) => onError(errorText(e)));
  }, [c.id, c.kind, onError]);
  if (!d) return null;
  const block = "max-h-60 overflow-auto rounded-md bg-zinc-200/50 p-2 font-mono text-xs whitespace-pre-wrap break-words dark:bg-zinc-800/50";
  return (
    <div className="mt-2 flex flex-col gap-2 border-t border-zinc-200 pt-2 dark:border-zinc-800">
      {image && <img src={image} alt={c.content} className="max-h-80 max-w-full self-start rounded border border-zinc-200 object-contain dark:border-zinc-800" />}
      {d.files && <pre className={block}>{d.files.join("\n")}</pre>}
      {d.text && c.kind !== "files" && <pre className={block}>{d.text}</pre>}
      {d.html_text && (
        <div>
          <Badge>HTML</Badge>
          <pre className={`${block} mt-1`}>{d.html_text}</pre>
        </div>
      )}
      {d.rtf && (
        <div>
          <Badge>RTF</Badge>
          <pre className={`${block} mt-1`}>{d.rtf}</pre>
        </div>
      )}
    </div>
  );
}

// --- quick actions ---

const EMPTY_ACTION: QuickActionInput = { name: "", kind: "open_url", target: "https://", hotkey: null };

function Actions({ onError }: { onError: (m: string | null) => void }) {
  const [actions, setActions] = useState<QuickAction[]>([]);
  const [form, setForm] = useState<{ id?: string; input: QuickActionInput } | null>(null);
  const [confirming, setConfirming] = useState<QuickAction | null>(null);
  const [output, setOutput] = useState<{ name: string; out: CommandOutput } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    api.quickActionsList().then(setActions, (e) => onError(errorText(e)));
  }, [onError]);

  async function update(fn: () => Promise<QuickAction[]>) {
    try {
      setActions(await fn());
      onError(null);
      return true;
    } catch (e) {
      onError(errorText(e));
      return false;
    }
  }

  async function save(e: FormEvent) {
    e.preventDefault();
    if (form && (await update(() => api.quickActionsSave(form.input, form.id)))) setForm(null);
  }

  async function run(a: QuickAction, confirm = false) {
    if (a.kind === "command" && !confirm) return setConfirming(a);
    try {
      const r = await api.quickActionsRun(a.id, confirm);
      onError(null);
      if (r.output) setOutput({ name: a.name, out: r.output });
      else {
        setNotice(C.ran(a.name));
        setTimeout(() => setNotice(null), 2500);
      }
      setActions(await api.quickActionsList());
    } catch (e) {
      if (e instanceof ApiError && e.kind === "confirmation_required") setConfirming(a);
      else onError(errorText(e));
    }
  }

  async function pick() {
    if (!form) return;
    const kind = form.input.kind === "open_app" ? "app" : form.input.kind === "open_folder" ? "folder" : "file";
    try {
      const path = await api.quickActionsPick(kind);
      if (path) setForm({ ...form, input: { ...form.input, target: path, name: form.input.name || path.split("\\").pop()!.replace(/\.(exe|lnk)$/i, "") } });
    } catch (e) {
      onError(errorText(e));
    }
  }

  const k = form?.input.kind;
  const multiline = k === "paste_text" || k === "copy_text";
  return (
    <>
      {notice && <div className="mb-3 rounded-lg bg-accent-500/10 px-3 py-2 text-accent-700 dark:text-accent-300">{notice}</div>}
      {!form && (
        <button className="btn-primary mb-3" onClick={() => setForm({ input: { ...EMPTY_ACTION } })}>
          <Plus className="size-4" /> {C.newAction}
        </button>
      )}

      {form && (
        <form onSubmit={save} className="card mb-4 grid gap-3 md:grid-cols-2">
          <Field label={C.actionName}>
            <input className="input" autoFocus value={form.input.name} maxLength={60} onChange={(e) => setForm({ ...form, input: { ...form.input, name: e.target.value } })} />
          </Field>
          <Field label={C.actionKind}>
            <select
              className="input"
              value={form.input.kind}
              onChange={(e) => {
                const kind = e.target.value as QuickActionKind;
                setForm({ ...form, input: { ...form.input, kind, target: kind === "open_url" ? "https://" : "", hotkey: kind === "command" ? null : form.input.hotkey } });
              }}
            >
              {ACTION_KINDS.map((x) => (
                <option key={x} value={x}>
                  {C.actionKinds[x]}
                </option>
              ))}
            </select>
          </Field>
          <div className="md:col-span-2">
            <Field label={C.actionTarget}>
              <span className="flex gap-2">
                {multiline ? (
                  <textarea className="input min-h-20 flex-1" placeholder={C.targetHints[k!]} value={form.input.target} onChange={(e) => setForm({ ...form, input: { ...form.input, target: e.target.value } })} />
                ) : (
                  <input
                    className={`input flex-1 ${k === "command" ? "font-mono" : ""}`}
                    placeholder={C.targetHints[k!]}
                    value={form.input.target}
                    onChange={(e) => setForm({ ...form, input: { ...form.input, target: e.target.value } })}
                  />
                )}
                {(k === "open_app" || k === "open_folder" || k === "open_file") && (
                  <button type="button" className="btn-secondary" onClick={pick}>
                    <FolderOpen className="size-4" /> {C.pick}
                  </button>
                )}
              </span>
            </Field>
          </div>
          {k === "command" ? (
            <p className="flex gap-2 text-xs text-amber-600 md:col-span-2 dark:text-amber-400">
              <ShieldAlert className="size-4 shrink-0" /> {C.commandWarning}
            </p>
          ) : (
            <Field label={C.actionHotkey}>
              <HotkeyInput label={C.actionHotkey} value={form.input.hotkey ?? null} onChange={(h) => setForm({ ...form, input: { ...form.input, hotkey: h } })} />
            </Field>
          )}
          <div className="flex items-end gap-2 md:col-span-2">
            <button className="btn-primary" disabled={!form.input.name.trim() || !form.input.target.trim()}>
              {t.common.save}
            </button>
            <button type="button" className="btn-secondary" onClick={() => setForm(null)}>
              {t.common.cancel}
            </button>
          </div>
        </form>
      )}

      {actions.length === 0 && !form && <Empty>{C.noActions}</Empty>}
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
        {actions.map((a, i) => {
          const Icon = ACTION_ICON[a.kind];
          return (
            <article key={a.id} className="card flex flex-col gap-2">
              <header className="flex items-start gap-3">
                <div className={`rounded-lg p-2 ${a.kind === "command" ? "bg-amber-500/15 text-amber-600" : "bg-accent-500/15 text-accent-600 dark:text-accent-400"}`}>
                  <Icon className="size-5" />
                </div>
                <div className="min-w-0 flex-1">
                  <h2 className="truncate font-semibold">{a.name}</h2>
                  <div className="text-xs text-zinc-500">{C.actionKinds[a.kind]}</div>
                </div>
                <div className="flex">
                  <IconBtn label={t.common.moveUp} onClick={() => i > 0 && update(() => api.quickActionsReorder(a.id, -1))}>
                    <ArrowUp className="size-3.5" />
                  </IconBtn>
                  <IconBtn label={t.common.moveDown} onClick={() => i < actions.length - 1 && update(() => api.quickActionsReorder(a.id, 1))}>
                    <ArrowDown className="size-3.5" />
                  </IconBtn>
                  <IconBtn label={t.common.edit} onClick={() => setForm({ id: a.id, input: { name: a.name, kind: a.kind, target: a.target, hotkey: a.hotkey } })}>
                    <Pencil className="size-3.5" />
                  </IconBtn>
                  <IconBtn label={t.common.delete} danger onClick={() => update(() => api.quickActionsDelete(a.id))}>
                    <Trash2 className="size-3.5" />
                  </IconBtn>
                </div>
              </header>
              <code className="truncate rounded bg-zinc-200/60 px-1.5 py-0.5 text-xs dark:bg-zinc-800/60" title={a.target}>
                {a.target}
              </code>
              <div className="mt-auto flex items-center gap-2">
                <button className="btn-primary" onClick={() => run(a)}>
                  <Play className="size-4" /> {C.run}
                </button>
                {a.hotkey && <Badge>{a.hotkey}</Badge>}
                {a.last_run_at && <span className="ml-auto text-[11px] text-zinc-500">{formatDateTime(a.last_run_at)}</span>}
              </div>
            </article>
          );
        })}
      </div>

      <ConfirmDialog
        open={confirming !== null}
        title={C.confirmCommand}
        confirmLabel={C.run}
        danger
        onCancel={() => setConfirming(null)}
        onConfirm={() => {
          const a = confirming!;
          setConfirming(null);
          run(a, true);
        }}
      >
        <p>{C.confirmCommandBody}</p>
        <pre className="mt-2 rounded bg-zinc-200 p-2 font-mono text-xs whitespace-pre-wrap break-all dark:bg-zinc-800">{confirming?.target}</pre>
      </ConfirmDialog>

      {output && (
        <section className="card mt-4">
          <header className="mb-2 flex items-center justify-between">
            <h2 className="font-semibold">
              {C.output}: {output.name}
            </h2>
            <span className="flex items-center gap-2 text-xs text-zinc-500">
              {output.out.timed_out ? <Badge tone="warn">{C.timedOut}</Badge> : <Badge tone={output.out.code === 0 ? "ok" : "error"}>{C.exitCode(output.out.code)}</Badge>}
              <button onClick={() => setOutput(null)} aria-label={t.widgets.close}>
                <X className="size-4" />
              </button>
            </span>
          </header>
          {output.out.stdout && <pre className="max-h-80 overflow-auto rounded bg-zinc-200/60 p-2 font-mono text-xs whitespace-pre-wrap dark:bg-zinc-800/60">{output.out.stdout}</pre>}
          {output.out.stderr && <pre className="mt-2 max-h-40 overflow-auto rounded bg-red-500/10 p-2 font-mono text-xs whitespace-pre-wrap text-red-700 dark:text-red-300">{output.out.stderr}</pre>}
        </section>
      )}
    </>
  );
}

// --- settings ---

function HubSettings({ onError }: { onError: (m: string | null) => void }) {
  const { modules } = useModules();
  const m = modules?.find((x) => x.id === "clipboard_history");
  const [actions, setActions] = useState<QuickAction[]>([]);
  useEffect(() => {
    api.quickActionsList().then(setActions, () => undefined);
  }, []);
  if (!m) return null;
  const save = async (hotkey: string | null) => {
    try {
      await api.updateModuleSettings(m.id, { ...m.settings, open_hotkey: hotkey ?? "" });
      onError(null);
    } catch (e) {
      onError(errorText(e));
    }
  };
  return (
    <div className="grid gap-4 lg:grid-cols-[1fr_20rem]">
      <section className="card">
        <h2 className="mb-3 font-semibold">{C.moduleSettings}</h2>
        <SettingsForm
          schema={m.settings_schema}
          value={m.settings}
          onSave={(s) => api.updateModuleSettings(m.id, s).catch((e) => onError(errorText(e)))}
          onReset={() => api.resetModuleSettings(m.id).catch((e) => onError(errorText(e)))}
        />
      </section>
      <section className="card flex flex-col gap-3 self-start">
        <h2 className="font-semibold">{C.hotkeys}</h2>
        <p className="text-xs text-zinc-500">{C.hotkeysHint}</p>
        <Field label={C.openHotkey}>
          <HotkeyInput label={C.openHotkey} value={String(m.settings.open_hotkey || "") || null} onChange={save} />
        </Field>
        <ul className="flex flex-col gap-1 text-sm">
          {actions
            .filter((a) => a.hotkey)
            .map((a) => (
              <li key={a.id} className="flex justify-between gap-2">
                <span className="truncate">{a.name}</span>
                <Badge>{a.hotkey}</Badge>
              </li>
            ))}
        </ul>
      </section>
    </div>
  );
}
