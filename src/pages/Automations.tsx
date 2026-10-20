import { DragEvent, useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle, CheckCircle2, ChevronDown, ChevronUp, Crosshair, FlaskConical, FolderOpen, GripVertical, LayoutTemplate, Play, Plus,
  Save, ShieldAlert, Trash2, X, XCircle,
} from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import HotkeyInput from "../components/HotkeyInput";
import { Badge, Empty, ErrorBanner, Field, PageHeader, Toggle } from "../components/ui";
import { formatDateTime, locale, t } from "../i18n";
import {
  Action, api, ApiError, Automation, AutomationInput, AutomationLog, AutomationTemplate, Condition, errorText, EVENTS, FocusState, Layout,
  RunReport, Trigger,
} from "../lib/api";
import { useModules } from "../lib/modules-context";
import { ACTIONS, CONDITIONS, describeTrigger, Field as FieldSpec, isDangerous, TRIGGERS, WEEKDAYS } from "./automation-fields";

const A = t.automations;

const EMPTY: AutomationInput = {
  name: "",
  description: "",
  enabled: true,
  trigger: { type: "manual" },
  conditions: [],
  actions: [{ type: "notify", title: "Привет", body: "" }],
  cooldown_secs: 0,
};

const toInput = (a: Automation): AutomationInput => ({
  name: a.name,
  description: a.description,
  enabled: a.enabled,
  module_id: a.module_id,
  trigger: a.trigger,
  conditions: a.conditions,
  actions: a.actions,
  cooldown_secs: a.cooldown_secs,
});

/** Moves item `from` to index `to`. */
function move<T>(list: T[], from: number, to: number): T[] {
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

export default function Automations() {
  const [list, setList] = useState<Automation[]>([]);
  const [selected, setSelected] = useState<string | "new" | null>(null);
  const [draft, setDraft] = useState<AutomationInput>(EMPTY);
  const [tab, setTab] = useState<"editor" | "log">("editor");
  const [report, setReport] = useState<RunReport | null>(null);
  const [confirmRun, setConfirmRun] = useState<{ message: string; run: () => void } | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [templates, setTemplates] = useState<AutomationTemplate[] | null>(null);
  const [focus, setFocus] = useState<FocusState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragIndex, setDragIndex] = useState<number | null>(null);

  const load = useCallback(async () => {
    try {
      setList(await api.automationsList());
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  useEffect(() => {
    load();
    api.focusModeGet().then(setFocus, () => undefined);
    const subs = [listen(EVENTS.automationRun, () => load()), listen<FocusState>(EVENTS.focusMode, (e) => setFocus(e.payload))];
    return () => subs.forEach((u) => u.then((f) => f()));
  }, [load]);

  const current = list.find((a) => a.id === selected);
  function open(a: Automation | null) {
    setSelected(a ? a.id : "new");
    setDraft(a ? toInput(a) : { ...EMPTY });
    setReport(null);
    setTab("editor");
  }

  async function save() {
    try {
      const saved = selected && selected !== "new" ? await api.automationsUpdate(selected, draft) : await api.automationsCreate(draft);
      setError(null);
      await load();
      setSelected(saved.id);
      setDraft(toInput(saved));
    } catch (e) {
      setError(errorText(e));
    }
  }

  /** Runs; dangerous automations ask first (backend answers `confirmation_required`). */
  async function run(fn: (confirm: boolean) => Promise<RunReport>) {
    try {
      setReport(await fn(false));
      setError(null);
      load();
    } catch (e) {
      if (e instanceof ApiError && e.kind === "confirmation_required") {
        setConfirmRun({
          message: e.message,
          run: async () => {
            try {
              setReport(await fn(true));
              load();
            } catch (err) {
              setError(errorText(err));
            }
          },
        });
      } else setError(errorText(e));
    }
  }

  async function reorder(from: number, to: number) {
    if (from === to) return;
    const next = move(list, from, to);
    setList(next);
    try {
      setList(await api.automationsReorder(next.map((a) => a.id)));
    } catch (e) {
      setError(errorText(e));
    }
  }

  const buttons = list.filter((a) => a.enabled && a.trigger.type === "ui_button");

  return (
    <>
      <PageHeader
        title={A.title}
        subtitle={A.subtitle}
        actions={
          <>
            <button
              className={focus?.active ? "btn-primary" : "btn-secondary"}
              onClick={async () => setFocus(await api.focusModeSet(!focus?.active, focus?.active ? undefined : 25))}
              title={A.focusHint}
            >
              <Crosshair className="size-4" />
              {focus?.active ? A.focusOn(focus.until ? new Date(focus.until).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" }) : null) : A.focusOff}
            </button>
            <button className="btn-secondary" onClick={async () => setTemplates(await api.automationsTemplates())}>
              <LayoutTemplate className="size-4" /> {A.templates}
            </button>
            <button className="btn-primary" onClick={() => open(null)}>
              <Plus className="size-4" /> {A.create}
            </button>
          </>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />

      {buttons.length > 0 && (
        <div className="card mb-4 flex flex-wrap items-center gap-2 py-3">
          <span className="text-xs text-zinc-500">{A.buttons}</span>
          {buttons.map((a) => (
            <button key={a.id} className="btn-secondary" onClick={() => run((c) => api.automationsRun(a.id, c, "ui_button"))}>
              <Play className="size-4" /> {(a.trigger.type === "ui_button" && a.trigger.label) || a.name}
              {a.dangerous.length > 0 && <AlertTriangle className="size-3.5 text-amber-500" />}
            </button>
          ))}
        </div>
      )}

      <div className="grid gap-4 lg:grid-cols-[19rem_1fr]">
        <aside className="flex flex-col gap-1.5">
          {list.length === 0 && <Empty>{A.empty}</Empty>}
          {list.map((a, i) => (
            <div
              key={a.id}
              draggable
              onDragStart={() => setDragIndex(i)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={() => dragIndex !== null && reorder(dragIndex, i)}
              onDragEnd={() => setDragIndex(null)}
              className={`card flex cursor-pointer items-center gap-2 p-2.5 ${a.id === selected ? "border-accent-500/60 dark:border-accent-500/60" : ""} ${dragIndex === i ? "opacity-50" : ""}`}
              onClick={() => open(a)}
            >
              <GripVertical className="size-4 shrink-0 cursor-grab text-zinc-400" aria-label={A.drag} />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1">
                  <span className="truncate font-medium">{a.name}</span>
                  {a.dangerous.length > 0 && <AlertTriangle className="size-3.5 shrink-0 text-amber-500" aria-label={A.dangerous} />}
                </div>
                <div className="truncate text-xs text-zinc-500">{describeTrigger(a.trigger)}</div>
              </div>
              <span onClick={(e) => e.stopPropagation()}>
                <Toggle
                  checked={a.enabled}
                  label={a.name}
                  onChange={async (v) => {
                    try {
                      await api.automationsSetEnabled(a.id, v);
                      load();
                    } catch (err) {
                      setError(errorText(err));
                    }
                  }}
                />
              </span>
            </div>
          ))}
        </aside>

        {selected === null ? (
          <Empty>{A.select}</Empty>
        ) : (
          <section className="flex min-w-0 flex-col gap-3">
            <div className="flex gap-1" role="tablist">
              <button role="tab" aria-selected={tab === "editor"} className={tab === "editor" ? "btn-secondary" : "btn-ghost"} onClick={() => setTab("editor")}>
                {A.editor}
              </button>
              {current && (
                <button role="tab" aria-selected={tab === "log"} className={tab === "log" ? "btn-secondary" : "btn-ghost"} onClick={() => setTab("log")}>
                  {A.logs}
                </button>
              )}
            </div>
            {tab === "editor" ? (
              <Editor
                draft={draft}
                setDraft={setDraft}
                current={current}
                report={report}
                onSave={save}
                onTest={() => run(() => api.automationsTest(draft, current?.id))}
                onRun={() => current && run((c) => api.automationsRun(current.id, c))}
                onDelete={() => setDeleting(true)}
                onError={setError}
              />
            ) : (
              current && <Logs id={current.id} />
            )}
          </section>
        )}
      </div>

      {templates && (
        <Templates
          templates={templates}
          onClose={() => setTemplates(null)}
          onInstall={async (key) => {
            try {
              const a = await api.automationsInstallTemplate(key);
              setTemplates(null);
              await load();
              open(a);
            } catch (e) {
              setError(errorText(e));
            }
          }}
        />
      )}

      <ConfirmDialog
        open={confirmRun !== null}
        title={A.confirmTitle}
        confirmLabel={A.runAnyway}
        danger
        onCancel={() => setConfirmRun(null)}
        onConfirm={() => {
          const c = confirmRun!;
          setConfirmRun(null);
          c.run();
        }}
      >
        {confirmRun?.message}
      </ConfirmDialog>
      <ConfirmDialog
        open={deleting}
        title={t.common.delete}
        confirmLabel={t.common.delete}
        danger
        onCancel={() => setDeleting(false)}
        onConfirm={async () => {
          setDeleting(false);
          if (current) {
            try {
              await api.automationsDelete(current.id);
              setSelected(null);
              load();
            } catch (e) {
              setError(errorText(e));
            }
          }
        }}
      >
        {current && A.deleteConfirm(current.name)}
      </ConfirmDialog>
    </>
  );
}

// --- editor ---

interface EditorProps {
  draft: AutomationInput;
  setDraft: (d: AutomationInput) => void;
  current: Automation | undefined;
  report: RunReport | null;
  onSave: () => void;
  onTest: () => void;
  onRun: () => void;
  onDelete: () => void;
  onError: (m: string | null) => void;
}

function Editor({ draft, setDraft, current, report, onSave, onTest, onRun, onDelete, onError }: EditorProps) {
  const { modules } = useModules();
  const [layouts, setLayouts] = useState<Layout[]>([]);
  const [drag, setDrag] = useState<number | null>(null);
  const [collapsed, setCollapsed] = useState<Set<number>>(new Set());
  useEffect(() => {
    api.layoutsList().then(setLayouts, () => undefined);
  }, []);

  const conditions = draft.conditions ?? [];
  const set = (patch: Partial<AutomationInput>) => setDraft({ ...draft, ...patch });
  const dangerous = draft.actions.filter(isDangerous);
  const ctx = { layouts, modules: modules ?? [], onError };

  function onDrop(e: DragEvent, to: number) {
    e.preventDefault();
    if (drag !== null && drag !== to) set({ actions: move(draft.actions, drag, to) });
    setDrag(null);
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="card grid gap-3 md:grid-cols-2">
        <Field label={A.name}>
          <input className="input" value={draft.name} maxLength={60} onChange={(e) => set({ name: e.target.value })} />
        </Field>
        <Field label={A.owner}>
          <select className="input" value={draft.module_id ?? "automations"} onChange={(e) => set({ module_id: e.target.value })}>
            {(modules ?? []).map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
          </select>
        </Field>
        <div className="md:col-span-2">
          <Field label={A.description}>
            <input className="input" value={draft.description ?? ""} maxLength={500} onChange={(e) => set({ description: e.target.value })} />
          </Field>
        </div>
        <Field label={A.cooldown}>
          <input className="input" type="number" min={0} max={604800} value={draft.cooldown_secs ?? 0} onChange={(e) => set({ cooldown_secs: Math.max(0, Number(e.target.value) || 0) })} />
        </Field>
        <label className="flex items-center gap-2 self-end pb-2">
          <Toggle checked={draft.enabled} onChange={(v) => set({ enabled: v })} label={A.enabled} />
          <span>{A.enabled}</span>
        </label>
      </div>

      {/* Trigger */}
      <div className="card flex flex-col gap-3">
        <h3 className="font-semibold">1. {A.trigger}</h3>
        <select className="input" value={draft.trigger.type} onChange={(e) => set({ trigger: { ...TRIGGERS[e.target.value as Trigger["type"]].defaults } })}>
          {Object.entries(TRIGGERS).map(([k, v]) => (
            <option key={k} value={k}>
              {v.label}
            </option>
          ))}
        </select>
        <Fields fields={TRIGGERS[draft.trigger.type].fields} value={draft.trigger} onChange={(v) => set({ trigger: v as Trigger })} ctx={ctx} />
      </div>

      {/* Conditions */}
      <div className="card flex flex-col gap-3">
        <h3 className="font-semibold">2. {A.conditions}</h3>
        {conditions.length === 0 && <p className="text-xs text-zinc-500">{A.noConditions}</p>}
        {conditions.map((c, i) => (
          <div key={i} className="flex flex-col gap-2 rounded-lg border border-zinc-200 p-3 dark:border-zinc-800">
            <div className="flex items-center gap-2">
              <span className="flex-1 font-medium">{CONDITIONS[c.type].label}</span>
              <button className="btn-ghost px-1.5 hover:text-red-500" onClick={() => set({ conditions: conditions.filter((_, j) => j !== i) })} aria-label={t.common.delete}>
                <X className="size-4" />
              </button>
            </div>
            <Fields fields={CONDITIONS[c.type].fields} value={c} onChange={(v) => set({ conditions: conditions.map((x, j) => (j === i ? (v as Condition) : x)) })} ctx={ctx} />
          </div>
        ))}
        <AddSelect label={A.addCondition} options={Object.entries(CONDITIONS).map(([k, v]) => [k, v.label])} onAdd={(k) => set({ conditions: [...conditions, { ...CONDITIONS[k as Condition["type"]].defaults }] })} />
      </div>

      {/* Actions */}
      <div className="card flex flex-col gap-3">
        <h3 className="font-semibold">3. {A.actions}</h3>
        <p className="text-xs text-zinc-500">{A.dragHint}</p>
        {draft.actions.map((a, i) => {
          const spec = ACTIONS[a.type];
          const danger = isDangerous(a);
          const isCollapsed = collapsed.has(i);
          return (
            <div
              key={i}
              draggable
              onDragStart={() => setDrag(i)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => onDrop(e, i)}
              onDragEnd={() => setDrag(null)}
              className={`flex flex-col gap-2 rounded-lg border p-3 ${danger ? "border-amber-500/50 bg-amber-500/5" : "border-zinc-200 dark:border-zinc-800"} ${drag === i ? "opacity-50" : ""}`}
            >
              <div className="flex items-center gap-2">
                <GripVertical className="size-4 shrink-0 cursor-grab text-zinc-400" aria-label={A.drag} />
                <span className="w-5 text-xs text-zinc-500 tabular-nums">{i + 1}.</span>
                <span className="flex-1 font-medium">{spec.label}</span>
                {danger && (
                  <Badge tone="warn">
                    <ShieldAlert className="mr-1 size-3" />
                    {A.needsConfirm}
                  </Badge>
                )}
                <button className="btn-ghost px-1.5" disabled={i === 0} onClick={() => set({ actions: move(draft.actions, i, i - 1) })} aria-label={t.common.moveUp}>
                  <ChevronUp className="size-4" />
                </button>
                <button className="btn-ghost px-1.5" disabled={i === draft.actions.length - 1} onClick={() => set({ actions: move(draft.actions, i, i + 1) })} aria-label={t.common.moveDown}>
                  <ChevronDown className="size-4" />
                </button>
                <button
                  className="btn-ghost px-1.5 text-xs"
                  onClick={() => {
                    const next = new Set(collapsed);
                    if (isCollapsed) next.delete(i);
                    else next.add(i);
                    setCollapsed(next);
                  }}
                >
                  {isCollapsed ? A.expand : A.collapse}
                </button>
                <button className="btn-ghost px-1.5 hover:text-red-500" onClick={() => set({ actions: draft.actions.filter((_, j) => j !== i) })} aria-label={t.common.delete}>
                  <X className="size-4" />
                </button>
              </div>
              {!isCollapsed && <Fields fields={spec.fields} value={a} onChange={(v) => set({ actions: draft.actions.map((x, j) => (j === i ? (v as Action) : x)) })} ctx={ctx} />}
            </div>
          );
        })}
        <AddSelect label={A.addAction} options={Object.entries(ACTIONS).map(([k, v]) => [k, v.label + (v.dangerous ? " ⚠" : "")])} onAdd={(k) => set({ actions: [...draft.actions, { ...ACTIONS[k as Action["type"]].defaults }] })} />
      </div>

      {dangerous.length > 0 && (
        <p className="flex items-start gap-2 rounded-lg bg-amber-500/10 px-3 py-2 text-sm text-amber-700 dark:text-amber-300">
          <ShieldAlert className="mt-0.5 size-4 shrink-0" /> {A.dangerNote}
        </p>
      )}
      {current && current.permissions.length > 0 && (
        <p className="text-xs text-zinc-500">
          {A.permissions}: {current.permissions.join(", ")}
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        <button className="btn-primary" onClick={onSave} disabled={!draft.name.trim() || draft.actions.length === 0}>
          <Save className="size-4" /> {t.common.save}
        </button>
        <button className="btn-secondary" onClick={onTest} title={A.testHint}>
          <FlaskConical className="size-4" /> {A.test}
        </button>
        {current && (
          <button className="btn-secondary" onClick={onRun}>
            <Play className="size-4" /> {A.runNow}
          </button>
        )}
        {current && (
          <button className="btn-danger ml-auto" onClick={onDelete}>
            <Trash2 className="size-4" /> {t.common.delete}
          </button>
        )}
      </div>

      {report && <Report report={report} />}
    </div>
  );
}

interface Ctx {
  layouts: Layout[];
  modules: { id: string; name: string; critical: boolean }[];
  onError: (m: string | null) => void;
}

/** Generic form for one trigger / condition / action object. */
function Fields({ fields, value, onChange, ctx }: { fields: FieldSpec[]; value: object; onChange: (v: object) => void; ctx: Ctx }) {
  const v = value as Record<string, unknown>;
  const set = (key: string, x: unknown) => onChange({ ...v, [key]: x });
  if (fields.length === 0) return null;
  return (
    <div className="grid gap-2 sm:grid-cols-2">
      {fields.map((f) => {
        const val = v[f.key];
        const wide = f.kind === "textarea" || f.kind === "path" || f.kind === "days";
        let control;
        switch (f.kind) {
          case "bool":
            control = (
              <label className="flex h-9 items-center gap-2">
                <Toggle checked={val !== false} onChange={(x) => set(f.key, x)} label={f.label} />
              </label>
            );
            break;
          case "number":
            control = <input className="input" type="number" min={f.min} max={f.max} value={Number(val ?? 0)} onChange={(e) => set(f.key, Number(e.target.value) || 0)} />;
            break;
          case "time":
            control = <input className="input" type="time" value={String(val ?? "")} onChange={(e) => set(f.key, e.target.value)} />;
            break;
          case "textarea":
            control = <textarea className="input min-h-16 font-mono text-xs" value={String(val ?? "")} placeholder={f.placeholder} onChange={(e) => set(f.key, e.target.value)} />;
            break;
          case "select":
            control = (
              <select className="input" value={String(val ?? "")} onChange={(e) => set(f.key, e.target.value)}>
                {f.options!.map(([k, l]) => (
                  <option key={k} value={k}>
                    {l}
                  </option>
                ))}
              </select>
            );
            break;
          case "hotkey":
            control = <HotkeyInput label={f.label} value={(val as string) || null} onChange={(h) => set(f.key, h ?? "")} />;
            break;
          case "layout":
            control = (
              <select className="input" value={String(val ?? "")} onChange={(e) => set(f.key, e.target.value)}>
                <option value="">—</option>
                {ctx.layouts.map((l) => (
                  <option key={l.id} value={l.id}>
                    {l.name}
                  </option>
                ))}
              </select>
            );
            break;
          case "module":
            control = (
              <select className="input" value={String(val ?? "")} onChange={(e) => set(f.key, e.target.value)}>
                {ctx.modules.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
              </select>
            );
            break;
          case "days": {
            const days = (val as number[]) ?? [];
            control = (
              <div className="flex flex-wrap gap-1">
                {WEEKDAYS.map((d, i) => {
                  const on = days.includes(i + 1);
                  return (
                    <button
                      type="button"
                      key={d}
                      aria-pressed={on}
                      className={`rounded-md px-2.5 py-1 text-xs font-medium ${on ? "bg-accent-600 text-white" : "bg-zinc-200 dark:bg-zinc-800"}`}
                      onClick={() => set(f.key, on ? days.filter((x) => x !== i + 1) : [...days, i + 1].sort())}
                    >
                      {d}
                    </button>
                  );
                })}
              </div>
            );
            break;
          }
          case "path":
            control = (
              <span className="flex gap-2">
                <input className="input flex-1 font-mono text-xs" value={String(val ?? "")} placeholder={f.placeholder} onChange={(e) => set(f.key, e.target.value)} />
                <button
                  type="button"
                  className="btn-secondary"
                  onClick={async () => {
                    try {
                      const p = await api.automationsPick(f.pick ?? "file");
                      if (p) set(f.key, p);
                    } catch (e) {
                      ctx.onError(errorText(e));
                    }
                  }}
                >
                  <FolderOpen className="size-4" />
                </button>
              </span>
            );
            break;
          default:
            control = <input className="input" value={String(val ?? "")} placeholder={f.placeholder} onChange={(e) => set(f.key, e.target.value)} />;
        }
        return (
          <div key={f.key} className={wide ? "sm:col-span-2" : ""}>
            <Field label={f.label}>{control}</Field>
          </div>
        );
      })}
    </div>
  );
}

function AddSelect({ label, options, onAdd }: { label: string; options: [string, string][]; onAdd: (key: string) => void }) {
  return (
    <select
      className="input self-start"
      value=""
      onChange={(e) => {
        if (e.target.value) onAdd(e.target.value);
      }}
      aria-label={label}
    >
      <option value="">+ {label}</option>
      {options.map(([k, l]) => (
        <option key={k} value={k}>
          {l}
        </option>
      ))}
    </select>
  );
}

function Report({ report }: { report: RunReport }) {
  const tone = report.status === "ok" || report.status === "dry_run" ? "ok" : report.status === "error" ? "error" : "warn";
  return (
    <div className="card flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Badge tone={tone}>{A.statuses[report.status] ?? report.status}</Badge>
        <span className="text-sm">{report.message}</span>
      </div>
      {report.conditions.length > 0 && (
        <ul className="text-sm">
          {report.conditions.map((c, i) => (
            <li key={i} className="flex items-center gap-1.5">
              {c.ok ? <CheckCircle2 className="size-4 text-emerald-500" /> : <XCircle className="size-4 text-red-500" />}
              {c.text}
            </li>
          ))}
        </ul>
      )}
      {report.status === "dry_run" && (
        <ol className="list-decimal pl-5 text-sm text-zinc-600 dark:text-zinc-300">
          {report.plan.map((p, i) => (
            <li key={i}>{p}</li>
          ))}
        </ol>
      )}
      {report.actions.length > 0 && (
        <ul className="text-sm">
          {report.actions.map((r, i) => (
            <li key={i} className="flex items-start gap-1.5">
              {r.ok ? <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-emerald-500" /> : <XCircle className="mt-0.5 size-4 shrink-0 text-red-500" />}
              <span>
                <b>{ACTIONS[r.action as Action["type"]]?.label ?? r.action}</b>: {r.message}
              </span>
            </li>
          ))}
        </ul>
      )}
      {report.missing_permissions.length > 0 && (
        <p className="text-xs text-amber-600 dark:text-amber-400">
          {A.missingPermissions}: {report.missing_permissions.join(", ")}
        </p>
      )}
    </div>
  );
}

function Logs({ id }: { id: string }) {
  const [logs, setLogs] = useState<AutomationLog[]>([]);
  const [open, setOpen] = useState<number | null>(null);
  useEffect(() => {
    const load = () => api.automationsLogs(id, 100).then(setLogs, () => undefined);
    load();
    const un = listen(EVENTS.automationRun, load);
    return () => {
      un.then((f) => f());
    };
  }, [id]);
  if (logs.length === 0) return <Empty>{A.noLogs}</Empty>;
  return (
    <ul className="card divide-y divide-zinc-200 p-0 dark:divide-zinc-800">
      {logs.map((l) => {
        const details = JSON.parse(l.details_json || "{}") as { actions?: RunReport["actions"]; conditions?: RunReport["conditions"] };
        return (
          <li key={l.id} className="px-3 py-2">
            <button className="flex w-full flex-wrap items-center gap-2 text-left" onClick={() => setOpen(open === l.id ? null : l.id)}>
              <Badge tone={l.status === "ok" ? "ok" : l.status === "error" ? "error" : "warn"}>{A.statuses[l.status]}</Badge>
              <span className="min-w-0 flex-1 truncate text-sm">{l.message}</span>
              <span className="text-xs text-zinc-500">
                {A.sources[l.source] ?? l.source} · {l.duration_ms} мс · {formatDateTime(l.created_at)}
              </span>
            </button>
            {open === l.id && (details.actions?.length || details.conditions?.length) ? (
              <ul className="mt-2 pl-2 text-xs">
                {details.conditions?.map((c, i) => (
                  <li key={`c${i}`}>
                    {c.ok ? "✓" : "✗"} {c.text}
                  </li>
                ))}
                {details.actions?.map((a, i) => (
                  <li key={`a${i}`}>
                    {a.ok ? "✓" : "✗"} {ACTIONS[a.action as Action["type"]]?.label ?? a.action}: {a.message}
                  </li>
                ))}
              </ul>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}

function Templates({ templates, onClose, onInstall }: { templates: AutomationTemplate[]; onClose: () => void; onInstall: (key: string) => void }) {
  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-label={A.templates}>
      <div className="card flex max-h-[85vh] w-full max-w-3xl flex-col gap-3 overflow-y-auto bg-white dark:bg-zinc-900">
        <header className="flex items-center justify-between">
          <h2 className="text-base font-semibold">{A.templates}</h2>
          <button onClick={onClose} aria-label={t.widgets.close}>
            <X className="size-4" />
          </button>
        </header>
        <p className="text-xs text-zinc-500">{A.templatesHint}</p>
        <div className="grid gap-3 sm:grid-cols-2">
          {templates.map((tpl) => {
            const dangerous = tpl.input.actions.some(isDangerous);
            return (
              <article key={tpl.key} className="flex flex-col gap-2 rounded-lg border border-zinc-200 p-3 dark:border-zinc-800">
                <div className="flex items-center gap-2">
                  <h3 className="flex-1 font-semibold">{tpl.name}</h3>
                  {dangerous && <Badge tone="warn">{A.needsConfirm}</Badge>}
                </div>
                <p className="text-xs text-zinc-500">{tpl.description}</p>
                <p className="text-xs">
                  <b>{A.trigger}:</b> {describeTrigger(tpl.input.trigger)}
                </p>
                <ol className="list-decimal pl-5 text-xs text-zinc-600 dark:text-zinc-300">
                  {tpl.input.actions.map((a, i) => (
                    <li key={i}>{ACTIONS[a.type].label}</li>
                  ))}
                </ol>
                <button className="btn-primary mt-auto self-start" onClick={() => onInstall(tpl.key)}>
                  <Plus className="size-4" /> {A.install}
                </button>
              </article>
            );
          })}
        </div>
      </div>
    </div>
  );
}
