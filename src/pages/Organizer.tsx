import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ArrowDown, ArrowUp, ChevronDown, ChevronUp, EyeOff, FolderOpen, Info, Pencil, Plus, RefreshCw, RotateCcw, Undo2, Wand2, X } from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import SettingsForm from "../components/SettingsForm";
import { formatBytes } from "../components/widgets/common";
import { Badge, Empty, ErrorBanner, Field, PageHeader, Toggle } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import {
  api, errorText, EVENTS, FileCategory, OrganizerMove, OrganizerOperation, OrganizerPlan, OrganizerRule, OrganizerRuleInput,
} from "../lib/api";
import { useModules } from "../lib/modules-context";

const O = t.organizer;
const CATS: FileCategory[] = ["screenshots", "images", "documents", "archives", "installers", "video", "music", "code", "other"];

export default function Organizer() {
  const [tab, setTab] = useState<"organize" | "journal" | "rules" | "settings">("organize");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [paths, setPaths] = useState<{ desktop: string; root: string } | null>(null);
  const [confirmRun, setConfirmRun] = useState(false);

  const flash = (m: string) => {
    setNotice(m);
    setTimeout(() => setNotice(null), 4000);
  };

  useEffect(() => {
    api.organizerPaths().then(setPaths, () => undefined);
  }, [tab]);

  async function runNow() {
    try {
      const r = await api.organizerRunNow();
      flash(O.moved(r.moved, r.failed));
      if (r.errors.length) setError(r.errors.join("; "));
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function undoLast() {
    try {
      const r = await api.organizerUndoLast();
      flash(O.restored(r.restored, r.renamed, r.failed));
      if (r.errors.length) setError(r.errors.join("; "));
    } catch (e) {
      setError(errorText(e));
    }
  }

  return (
    <>
      <PageHeader
        title={O.title}
        subtitle={O.subtitle}
        actions={
          <>
            <button className="btn-secondary" onClick={undoLast}>
              <Undo2 className="size-4" /> {O.undoLast}
            </button>
            <button className="btn-secondary" onClick={() => setConfirmRun(true)} title={O.runNowHint}>
              <Wand2 className="size-4" /> {O.runNow}
            </button>
          </>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      {notice && <div className="mb-3 rounded-lg bg-accent-500/10 px-3 py-2 text-accent-700 dark:text-accent-300">{notice}</div>}
      {paths && <p className="mb-3 truncate text-xs text-zinc-500">{O.paths(paths.desktop, paths.root)}</p>}

      <div className="mb-4 flex gap-1" role="tablist">
        {(["organize", "journal", "rules", "settings"] as const).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "btn-secondary" : "btn-ghost"} onClick={() => setTab(k)}>
            {O.tabs[k]}
          </button>
        ))}
      </div>

      {tab === "organize" && <Preview onError={setError} onDone={flash} />}
      {tab === "journal" && <Journal onError={setError} onDone={flash} />}
      {tab === "rules" && <Rules onError={setError} />}
      {tab === "settings" && <OrganizerSettings onError={setError} />}
      <ConfirmDialog
        open={confirmRun}
        title={O.runNow}
        confirmLabel={O.runNow}
        onCancel={() => setConfirmRun(false)}
        onConfirm={() => {
          setConfirmRun(false);
          runNow();
        }}
      >
        {O.runNowHint}
      </ConfirmDialog>
    </>
  );
}

// --- preview ---

function Preview({ onError, onDone }: { onError: (m: string | null) => void; onDone: (m: string) => void }) {
  const [plan, setPlan] = useState<OrganizerPlan | null>(null);
  const [excluded, setExcluded] = useState<Set<string>>(new Set());
  const [pending, setPending] = useState<{ source: string; files: number } | null>(null);
  const [showSkipped, setShowSkipped] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setPlan(await api.organizerPreview());
      setPending(await api.organizerPending());
      setExcluded(new Set());
      onError(null);
    } catch (e) {
      onError(errorText(e));
    }
  }, [onError]);

  useEffect(() => {
    load();
    const un = listen(EVENTS.organizerChanged, () => load());
    return () => {
      un.then((f) => f());
    };
  }, [load]);

  if (!plan) return null;
  const chosen = plan.items.filter((i) => !excluded.has(i.path));
  const toggle = (path: string) => {
    const next = new Set(excluded);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    setExcluded(next);
  };

  async function apply() {
    setBusy(true);
    try {
      const r = await api.organizerApply(chosen);
      onDone(O.moved(r.moved, r.failed));
      if (r.errors.length) onError(r.errors.join("; "));
      await load();
    } catch (e) {
      onError(errorText(e));
    }
    setBusy(false);
  }

  return (
    <div className="flex flex-col gap-3">
      <p className="flex gap-2 text-xs text-zinc-500">
        <Info className="size-4 shrink-0" /> {O.neverDelete}
      </p>
      {pending && (
        <div className="flex items-center gap-2 rounded-lg bg-amber-500/10 px-3 py-2 text-sm text-amber-700 dark:text-amber-300">
          <span className="flex-1">{O.pending(pending.files, O.sources[pending.source] ?? pending.source)}</span>
          <button className="text-xs underline" onClick={() => api.organizerDismissPending().then(() => setPending(null))}>
            {O.dismiss}
          </button>
        </div>
      )}

      <div className="card flex flex-wrap items-center gap-2 py-3">
        <span className="flex-1 text-sm">{O.summary(chosen.length, plan.desktop_files, plan.targets.length)}</span>
        <button className="btn-ghost" onClick={load}>
          <RefreshCw className="size-4" /> {O.refresh}
        </button>
        <button className="btn-ghost" onClick={() => setExcluded(new Set())}>
          {O.selectAll}
        </button>
        <button className="btn-ghost" onClick={() => setExcluded(new Set(plan.items.map((i) => i.path)))}>
          {O.selectNone}
        </button>
        <button className="btn-secondary" onClick={() => setExcluded(new Set(plan.items.map((i) => i.path)))}>
          {O.cancel}
        </button>
        <button className="btn-primary" disabled={chosen.length === 0 || busy} onClick={apply}>
          {O.move(chosen.length)}
        </button>
      </div>

      {plan.items.length === 0 && <Empty>{O.nothing}</Empty>}
      {plan.targets.map(([dir, count]) => (
        <section key={dir} className="card flex flex-col gap-1 p-3">
          <header className="flex items-center gap-2">
            <FolderOpen className="size-4 shrink-0 text-accent-500" />
            <span className="min-w-0 flex-1 truncate font-mono text-xs" title={dir}>
              {dir}
            </span>
            <Badge>{count}</Badge>
          </header>
          <ul className="divide-y divide-zinc-200 dark:divide-zinc-800">
            {plan.items
              .filter((i) => i.target_dir === dir)
              .map((i) => {
                const on = !excluded.has(i.path);
                return (
                  <li key={i.path} className={`group flex items-center gap-2 py-1.5 ${on ? "" : "opacity-50"}`}>
                    <input type="checkbox" className="size-4 accent-accent-600" checked={on} onChange={() => toggle(i.path)} aria-label={i.name} />
                    <span className={`min-w-0 flex-1 truncate text-sm ${on ? "" : "line-through"}`} title={i.path}>
                      {i.name}
                    </span>
                    <Badge>{O.fileCategories[i.category]}</Badge>
                    <span className="hidden text-xs text-zinc-500 sm:inline">{i.rule_name}</span>
                    <span className="w-16 text-right text-xs text-zinc-500 tabular-nums">{formatBytes(i.size)}</span>
                    <span className="hidden w-32 text-right text-xs text-zinc-500 md:inline">{formatDateTime(i.modified)}</span>
                    <button
                      className="rounded p-1 text-zinc-400 opacity-0 group-hover:opacity-100 hover:text-amber-600 focus:opacity-100"
                      title={O.excludeAlways}
                      aria-label={`${O.excludeAlways}: ${i.name}`}
                      onClick={async () => {
                        try {
                          await api.organizerExclude(i.name);
                          load();
                        } catch (e) {
                          onError(errorText(e));
                        }
                      }}
                    >
                      <EyeOff className="size-4" />
                    </button>
                  </li>
                );
              })}
          </ul>
        </section>
      ))}

      {plan.skipped.length > 0 && (
        <section className="card p-3">
          <button className="flex w-full items-center gap-2 text-sm text-zinc-500" onClick={() => setShowSkipped(!showSkipped)}>
            {showSkipped ? <ChevronUp className="size-4" /> : <ChevronDown className="size-4" />} {O.skipped(plan.skipped.length)}
          </button>
          {showSkipped && (
            <ul className="mt-2 text-xs">
              {plan.skipped.map((s) => (
                <li key={s.name} className="flex justify-between gap-2 py-0.5">
                  <span className="truncate">{s.name}</span>
                  <span className="shrink-0 text-zinc-500">{s.reason}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}

// --- journal ---

function Journal({ onError, onDone }: { onError: (m: string | null) => void; onDone: (m: string) => void }) {
  const [ops, setOps] = useState<OrganizerOperation[]>([]);
  const [open, setOpen] = useState<string | null>(null);
  const [moves, setMoves] = useState<OrganizerMove[]>([]);
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [confirm, setConfirm] = useState<OrganizerOperation | null>(null);

  const load = useCallback(async () => {
    try {
      setOps(await api.organizerOperations());
      if (open) setMoves(await api.organizerMoves(open));
    } catch (e) {
      onError(errorText(e));
    }
  }, [open, onError]);

  useEffect(() => {
    load();
    const un = listen(EVENTS.organizerChanged, () => load());
    return () => {
      un.then((f) => f());
    };
  }, [load]);

  async function expand(id: string) {
    setPicked(new Set());
    if (open === id) return setOpen(null);
    setOpen(id);
  }

  async function report(fn: () => Promise<{ restored: number; renamed: number; failed: number; errors: string[] }>) {
    try {
      const r = await fn();
      onDone(O.restored(r.restored, r.renamed, r.failed));
      if (r.errors.length) onError(r.errors.join("; "));
      setPicked(new Set());
      load();
    } catch (e) {
      onError(errorText(e));
    }
  }

  if (ops.length === 0) return <Empty>{O.noOperations}</Empty>;
  return (
    <ul className="flex flex-col gap-2">
      {ops.map((op) => {
        const undoable = op.status === "done" || op.status === "partial" || op.status === "partially_undone";
        return (
          <li key={op.id} className="card p-3">
            <div className="flex flex-wrap items-center gap-2">
              <button className="btn-ghost px-1" onClick={() => expand(op.id)} aria-expanded={open === op.id}>
                {open === op.id ? <ChevronUp className="size-4" /> : <ChevronDown className="size-4" />}
              </button>
              <Badge tone={op.status === "done" ? "ok" : op.status === "undone" ? "neutral" : "warn"}>{O.statuses[op.status]}</Badge>
              <span className="text-sm">{op.summary}</span>
              <span className="text-xs text-zinc-500">
                {O.sources[op.source] ?? op.source} · {formatDateTime(op.created_at)}
              </span>
              {undoable && (
                <button className="btn-secondary ml-auto" onClick={() => setConfirm(op)}>
                  <RotateCcw className="size-4" /> {O.undo}
                </button>
              )}
            </div>
            {open === op.id && (
              <div className="mt-2">
                {picked.size > 0 && (
                  <button className="btn-primary mb-2" onClick={() => report(() => api.organizerRestore([...picked]))}>
                    <Undo2 className="size-4" /> {O.restoreSelected(picked.size)}
                  </button>
                )}
                <ul className="divide-y divide-zinc-200 text-xs dark:divide-zinc-800">
                  {moves.map((m) => {
                    const canRestore = m.status === "moved" || m.status === "undo_failed";
                    return (
                      <li key={m.id} className="flex items-start gap-2 py-1.5">
                        <input
                          type="checkbox"
                          className="mt-0.5 size-4 accent-accent-600"
                          disabled={!canRestore}
                          checked={picked.has(m.id)}
                          onChange={() => {
                            const next = new Set(picked);
                            if (next.has(m.id)) next.delete(m.id);
                            else next.add(m.id);
                            setPicked(next);
                          }}
                          aria-label={m.original_path}
                        />
                        <div className="min-w-0 flex-1">
                          <div className="truncate" title={m.original_path}>
                            <span className="text-zinc-500">{O.from}:</span> {m.original_path}
                          </div>
                          {m.new_path && (
                            <div className="truncate" title={m.new_path}>
                              <span className="text-zinc-500">{O.to}:</span> {m.new_path}
                            </div>
                          )}
                          {m.restored_path && m.restored_path !== m.original_path && <div className="truncate text-amber-600">→ {m.restored_path}</div>}
                          {m.error && <div className="text-red-500">{m.error}</div>}
                        </div>
                        <Badge tone={m.status === "moved" ? "ok" : m.status === "undone" ? "neutral" : "error"}>{O.moveStatuses[m.status]}</Badge>
                        {m.new_path && m.status === "moved" && (
                          <button className="rounded p-0.5 text-zinc-400 hover:text-zinc-700" title={O.openFolder} onClick={() => api.organizerOpenFolder(m.new_path).catch((e) => onError(errorText(e)))}>
                            <FolderOpen className="size-3.5" />
                          </button>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </div>
            )}
          </li>
        );
      })}
      <ConfirmDialog
        open={confirm !== null}
        title={O.undo}
        confirmLabel={O.undo}
        onCancel={() => setConfirm(null)}
        onConfirm={() => {
          const op = confirm!;
          setConfirm(null);
          report(() => api.organizerUndoOperation(op.id));
        }}
      >
        {confirm && `${confirm.summary} · ${formatDateTime(confirm.created_at)}`}
      </ConfirmDialog>
    </ul>
  );
}

// --- rules ---

const EMPTY_RULE: OrganizerRuleInput = { name: "", enabled: true, match: { name_pattern: "" }, target: "{root}\\" };

function Rules({ onError }: { onError: (m: string | null) => void }) {
  const [rules, setRules] = useState<OrganizerRule[]>([]);
  const [edit, setEdit] = useState<{ id?: string; input: OrganizerRuleInput; ext: string } | null>(null);
  const [resetting, setResetting] = useState(false);

  useEffect(() => {
    api.organizerRules().then(setRules, (e) => onError(errorText(e)));
  }, [onError]);

  async function run(fn: () => Promise<OrganizerRule[]>) {
    try {
      setRules(await fn());
      onError(null);
      return true;
    } catch (e) {
      onError(errorText(e));
      return false;
    }
  }

  const describe = (r: OrganizerRule) =>
    [
      r.match.categories?.length ? r.match.categories.map((c) => O.fileCategories[c]).join(", ") : null,
      r.match.extensions?.length ? r.match.extensions.map((e) => `.${e}`).join(" ") : null,
      r.match.name_pattern ? `«${r.match.name_pattern}»` : null,
      r.match.older_than_days ? `старше ${r.match.older_than_days} дн.` : null,
      r.match.newer_than_days ? `новее ${r.match.newer_than_days} дн.` : null,
      r.match.min_size_kb ? `от ${r.match.min_size_kb} КБ` : null,
      r.match.max_size_kb ? `до ${r.match.max_size_kb} КБ` : null,
    ]
      .filter(Boolean)
      .join(" · ");

  const num = (v: string) => (v.trim() === "" ? null : Math.max(0, Number(v) || 0));
  const m = edit?.input.match;

  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-zinc-500">{O.rulesHint}</p>
      <div className="flex gap-2">
        <button className="btn-primary" onClick={() => setEdit({ input: { ...EMPTY_RULE, match: { ...EMPTY_RULE.match } }, ext: "" })}>
          <Plus className="size-4" /> {O.newRule}
        </button>
        <button className="btn-ghost" onClick={() => setResetting(true)}>
          <RotateCcw className="size-4" /> {O.resetRules}
        </button>
      </div>

      {edit && m && (
        <form
          className="card grid gap-3 md:grid-cols-2"
          onSubmit={async (e) => {
            e.preventDefault();
            const input = { ...edit.input, match: { ...m, extensions: edit.ext.split(/[,\s]+/).map((x) => x.replace(/^\./, "").toLowerCase()).filter(Boolean) } };
            if (await run(() => api.organizerSaveRule(input, edit.id))) setEdit(null);
          }}
        >
          <Field label={O.ruleName}>
            <input className="input" autoFocus value={edit.input.name} maxLength={60} onChange={(e) => setEdit({ ...edit, input: { ...edit.input, name: e.target.value } })} />
          </Field>
          <Field label={O.target}>
            <input className="input font-mono text-xs" value={edit.input.target} onChange={(e) => setEdit({ ...edit, input: { ...edit.input, target: e.target.value } })} />
          </Field>
          <p className="text-xs text-zinc-500 md:col-span-2">{O.targetHint}</p>
          <div className="md:col-span-2">
            <Field label={O.categories}>
              <div className="flex flex-wrap gap-1">
                {CATS.map((c) => {
                  const on = m.categories?.includes(c) ?? false;
                  return (
                    <button
                      type="button"
                      key={c}
                      aria-pressed={on}
                      className={`rounded-full px-2.5 py-1 text-xs ${on ? "bg-accent-600 text-white" : "bg-zinc-200 dark:bg-zinc-800"}`}
                      onClick={() => {
                        const cats = on ? (m.categories ?? []).filter((x) => x !== c) : [...(m.categories ?? []), c];
                        setEdit({ ...edit, input: { ...edit.input, match: { ...m, categories: cats } } });
                      }}
                    >
                      {O.fileCategories[c]}
                    </button>
                  );
                })}
              </div>
            </Field>
          </div>
          <Field label={O.extensions}>
            <input className="input" value={edit.ext} placeholder="pdf, docx" onChange={(e) => setEdit({ ...edit, ext: e.target.value })} />
          </Field>
          <Field label={O.namePattern}>
            <input className="input" value={m.name_pattern ?? ""} onChange={(e) => setEdit({ ...edit, input: { ...edit.input, match: { ...m, name_pattern: e.target.value } } })} />
          </Field>
          <div className="grid grid-cols-2 gap-2 md:col-span-2 lg:grid-cols-4">
            {(
              [
                ["older_than_days", O.olderThan],
                ["newer_than_days", O.newerThan],
                ["min_size_kb", O.minSize],
                ["max_size_kb", O.maxSize],
              ] as const
            ).map(([key, label]) => (
              <Field key={key} label={label}>
                <input className="input" type="number" min={0} value={m[key] ?? ""} onChange={(e) => setEdit({ ...edit, input: { ...edit.input, match: { ...m, [key]: num(e.target.value) } } })} />
              </Field>
            ))}
          </div>
          <div className="flex gap-2 md:col-span-2">
            <button className="btn-primary" disabled={!edit.input.name.trim()}>
              {t.common.save}
            </button>
            <button type="button" className="btn-secondary" onClick={() => setEdit(null)}>
              {t.common.cancel}
            </button>
          </div>
        </form>
      )}

      <ul className="flex flex-col gap-2">
        {rules.map((r, i) => (
          <li key={r.id} className={`card flex items-center gap-3 p-3 ${r.enabled ? "" : "opacity-60"}`}>
            <span className="w-5 text-xs text-zinc-500 tabular-nums">{i + 1}.</span>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="font-medium">{r.name}</span>
                {r.builtin && <Badge>стандартное</Badge>}
              </div>
              <div className="truncate text-xs text-zinc-500">{describe(r)}</div>
              <div className="truncate font-mono text-xs" title={r.target}>
                → {r.target}
              </div>
            </div>
            <Toggle checked={r.enabled} label={r.name} onChange={(v) => run(() => api.organizerSaveRule({ name: r.name, enabled: v, match: r.match, target: r.target }, r.id))} />
            <button className="btn-ghost px-1.5" disabled={i === 0} onClick={() => run(() => api.organizerReorderRules(move(rules, i, i - 1)))} aria-label={t.common.moveUp}>
              <ArrowUp className="size-4" />
            </button>
            <button className="btn-ghost px-1.5" disabled={i === rules.length - 1} onClick={() => run(() => api.organizerReorderRules(move(rules, i, i + 1)))} aria-label={t.common.moveDown}>
              <ArrowDown className="size-4" />
            </button>
            <button
              className="btn-ghost px-1.5"
              onClick={() => setEdit({ id: r.id, input: { name: r.name, enabled: r.enabled, match: { ...r.match }, target: r.target }, ext: (r.match.extensions ?? []).join(", ") })}
              aria-label={t.common.edit}
            >
              <Pencil className="size-4" />
            </button>
            <button className="btn-ghost px-1.5 hover:text-red-500" onClick={() => run(() => api.organizerDeleteRule(r.id))} aria-label={t.common.delete}>
              <X className="size-4" />
            </button>
          </li>
        ))}
      </ul>

      <ConfirmDialog
        open={resetting}
        title={O.resetRules}
        confirmLabel={O.resetRules}
        danger
        onCancel={() => setResetting(false)}
        onConfirm={() => {
          setResetting(false);
          run(() => api.organizerResetRules());
        }}
      >
        {O.resetConfirm}
      </ConfirmDialog>
    </div>
  );
}

function move(rules: OrganizerRule[], from: number, to: number): string[] {
  const ids = rules.map((r) => r.id);
  const [x] = ids.splice(from, 1);
  ids.splice(to, 0, x);
  return ids;
}

// --- settings ---

function OrganizerSettings({ onError }: { onError: (m: string | null) => void }) {
  const m = useModules().modules?.find((x) => x.id === "desktop_organizer");
  if (!m) return null;
  return (
    <section className="card">
      <SettingsForm
        schema={m.settings_schema}
        value={m.settings}
        onSave={(s) => api.updateModuleSettings(m.id, s).catch((e) => onError(errorText(e)))}
        onReset={() => api.resetModuleSettings(m.id).catch((e) => onError(errorText(e)))}
      />
      <p className="mt-3 flex gap-2 text-xs text-zinc-500">
        <Info className="size-4 shrink-0" /> {O.neverDelete}
      </p>
    </section>
  );
}
