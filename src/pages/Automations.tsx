import { FormEvent, useState } from "react";
import { Pencil, Play, Plus, ScrollText, Trash2, X } from "lucide-react";
import { Badge, Empty, ErrorBanner, Field, PageHeader, Toggle } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { Action, api, Automation, AutomationInput, errorText, Trigger } from "../lib/api";
import { attempt, useAsync } from "../lib/use-async";

const A = t.automations;
type TriggerType = Trigger["type"];
type ActionType = Action["type"];

const blankAction = (type: ActionType): Action => {
  switch (type) {
    case "notify":
      return { type, title: "", body: "" };
    case "log":
      return { type, message: "" };
    default:
      return { type };
  }
};

const blankTrigger = (type: TriggerType): Trigger =>
  type === "interval" ? { type, minutes: 30 } : type === "hotkey" ? { type, shortcut: "Ctrl+Alt+J" } : { type };

const describeTrigger = (tr: Trigger) =>
  tr.type === "interval" ? A.describeInterval(tr.minutes) : tr.type === "hotkey" ? A.describeHotkey(tr.shortcut) : A.triggers.app_start;

const EMPTY: AutomationInput = {
  name: "",
  enabled: true,
  trigger: { type: "app_start" },
  actions: [{ type: "notify", title: "", body: "" }],
};

function AutomationForm({
  initial,
  onSubmit,
  onCancel,
}: {
  initial: AutomationInput;
  onSubmit: (input: AutomationInput) => Promise<boolean>;
  onCancel: () => void;
}) {
  const [form, setForm] = useState<AutomationInput>(initial);
  const [busy, setBusy] = useState(false);

  const setAction = (i: number, a: Action) => setForm({ ...form, actions: form.actions.map((x, j) => (j === i ? a : x)) });

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    if (await onSubmit(form)) onCancel();
    setBusy(false);
  }

  return (
    <form onSubmit={submit} className="card mb-4 flex flex-col gap-4">
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        <Field label={A.name}>
          <input className="input" required maxLength={60} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
        </Field>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <Field label={A.trigger}>
            <select
              className="input"
              value={form.trigger.type}
              onChange={(e) => setForm({ ...form, trigger: blankTrigger(e.target.value as TriggerType) })}
            >
              {(Object.keys(A.triggers) as TriggerType[]).map((k) => (
                <option key={k} value={k}>
                  {A.triggers[k]}
                </option>
              ))}
            </select>
          </Field>
          {form.trigger.type === "interval" && (
            <Field label={A.everyMinutes}>
              <input
                className="input"
                type="number"
                min={1}
                max={1440}
                value={form.trigger.minutes}
                onChange={(e) => setForm({ ...form, trigger: { type: "interval", minutes: Number(e.target.value) } })}
              />
            </Field>
          )}
          {form.trigger.type === "hotkey" && (
            <Field label={A.shortcut}>
              <input
                className="input font-mono"
                value={form.trigger.shortcut}
                spellCheck={false}
                onChange={(e) => setForm({ ...form, trigger: { type: "hotkey", shortcut: e.target.value } })}
              />
            </Field>
          )}
        </div>
      </div>

      <fieldset className="flex flex-col gap-2">
        <legend className="mb-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">{A.actions}</legend>
        {form.actions.map((a, i) => (
          <div key={i} className="flex flex-wrap items-end gap-2 rounded-lg bg-zinc-100 p-2 dark:bg-zinc-950/60">
            <select className="input w-52" value={a.type} onChange={(e) => setAction(i, blankAction(e.target.value as ActionType))} aria-label={A.actions}>
              {(Object.keys(A.actionTypes) as ActionType[]).map((k) => (
                <option key={k} value={k}>
                  {A.actionTypes[k]}
                </option>
              ))}
            </select>
            {a.type === "notify" && (
              <>
                <input className="input w-48 flex-1" placeholder={A.notifyTitle} maxLength={80} value={a.title} onChange={(e) => setAction(i, { ...a, title: e.target.value })} />
                <input className="input w-48 flex-[2]" placeholder={A.notifyBody} maxLength={500} value={a.body} onChange={(e) => setAction(i, { ...a, body: e.target.value })} />
              </>
            )}
            {a.type === "log" && (
              <input className="input flex-1" placeholder={A.logMessage} maxLength={500} value={a.message} onChange={(e) => setAction(i, { ...a, message: e.target.value })} />
            )}
            <button
              type="button"
              className="btn-ghost px-2"
              disabled={form.actions.length === 1}
              onClick={() => setForm({ ...form, actions: form.actions.filter((_, j) => j !== i) })}
              aria-label={t.common.delete}
            >
              <X className="size-4" />
            </button>
          </div>
        ))}
        <button
          type="button"
          className="btn-ghost self-start"
          disabled={form.actions.length >= 10}
          onClick={() => setForm({ ...form, actions: [...form.actions, blankAction("log")] })}
        >
          <Plus className="size-4" />
          {A.addAction}
        </button>
      </fieldset>

      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2">
          <Toggle checked={form.enabled} onChange={(v) => setForm({ ...form, enabled: v })} label={t.common.enabled} />
          {form.enabled ? t.common.enabled : t.common.disabled}
        </label>
        <div className="flex-1" />
        <button type="button" className="btn-ghost" onClick={onCancel}>
          {t.common.cancel}
        </button>
        <button className="btn-primary" disabled={busy || !form.name.trim()}>
          {t.common.save}
        </button>
      </div>
    </form>
  );
}

export default function Automations() {
  const list = useAsync(api.automationsList);
  const logs = useAsync(() => api.automationsLogs(undefined, 30));
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<"new" | Automation | null>(null);
  const [showLogsFor, setShowLogsFor] = useState<string | null>(null);
  const names = new Map((list.data ?? []).map((a) => [a.id, a.name]));

  async function save(input: AutomationInput) {
    const result = await attempt(
      () => (editing && editing !== "new" ? api.automationsUpdate(editing.id, input) : api.automationsCreate(input)),
      setError,
    );
    list.reload();
    return result !== undefined;
  }

  async function act(fn: () => Promise<unknown>) {
    try {
      await fn();
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
    list.reload();
    logs.reload();
  }

  return (
    <>
      <PageHeader
        title={A.title}
        subtitle={A.subtitle}
        actions={
          editing === null && (
            <button className="btn-primary" onClick={() => setEditing("new")}>
              <Plus className="size-4" />
              {A.create}
            </button>
          )
        }
      />
      <ErrorBanner message={error ?? list.error} onClose={() => setError(null)} />

      {editing !== null && (
        <AutomationForm
          key={editing === "new" ? "new" : editing.id}
          initial={editing === "new" ? EMPTY : { name: editing.name, enabled: editing.enabled, trigger: editing.trigger, actions: editing.actions }}
          onSubmit={save}
          onCancel={() => setEditing(null)}
        />
      )}

      {list.data?.length === 0 && editing === null && <Empty>{A.empty}</Empty>}
      <ul className="flex flex-col gap-3">
        {list.data?.map((a) => (
          <li key={a.id} className="card">
            <div className="flex flex-wrap items-center gap-3">
              <Toggle checked={a.enabled} onChange={(v) => act(() => api.automationsSetEnabled(a.id, v))} label={a.name} />
              <div className="min-w-0 flex-1">
                <div className="truncate font-semibold">{a.name}</div>
                <div className="text-xs text-zinc-500">
                  {describeTrigger(a.trigger)} → {a.actions.map((x) => A.actionTypes[x.type]).join(", ")}
                </div>
              </div>
              <div className="text-xs text-zinc-500">
                {A.lastRun}: {formatDateTime(a.last_run_at)}
              </div>
              <div className="flex gap-1">
                <button className="btn-ghost px-2" onClick={() => act(() => api.automationsRunNow(a.id))} title={A.runNow} aria-label={A.runNow}>
                  <Play className="size-4" />
                </button>
                <button
                  className="btn-ghost px-2"
                  onClick={() => setShowLogsFor(showLogsFor === a.id ? null : a.id)}
                  title={A.logs}
                  aria-label={A.logs}
                  aria-expanded={showLogsFor === a.id}
                >
                  <ScrollText className="size-4" />
                </button>
                <button className="btn-ghost px-2" onClick={() => setEditing(a)} title={t.common.edit} aria-label={t.common.edit}>
                  <Pencil className="size-4" />
                </button>
                <button
                  className="btn-danger px-2"
                  onClick={() => confirm(t.common.confirmDelete) && act(() => api.automationsDelete(a.id))}
                  title={t.common.delete}
                  aria-label={t.common.delete}
                >
                  <Trash2 className="size-4" />
                </button>
              </div>
            </div>
            {showLogsFor === a.id && <AutomationLogs id={a.id} />}
          </li>
        ))}
      </ul>

      <section className="card mt-6">
        <h2 className="mb-3 font-semibold">{A.allLogs}</h2>
        {logs.data?.length === 0 && <p className="text-zinc-500">{A.noLogs}</p>}
        <ul className="divide-y divide-zinc-200 dark:divide-zinc-800">
          {logs.data?.map((l) => (
            <li key={l.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 py-2">
              <Badge tone={l.status === "ok" ? "ok" : l.status === "error" ? "error" : "warn"}>{A.status[l.status]}</Badge>
              <span className="font-medium">{names.get(l.automation_id) ?? "—"}</span>
              <span className="text-zinc-500">{l.message}</span>
              <span className="ml-auto text-xs text-zinc-500">{formatDateTime(l.created_at)}</span>
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}

function AutomationLogs({ id }: { id: string }) {
  const { data } = useAsync(() => api.automationsLogs(id, 20), [id]);
  return (
    <ul className="mt-3 border-t border-zinc-200 pt-2 text-xs dark:border-zinc-800">
      {data?.length === 0 && <li className="text-zinc-500">{A.noLogs}</li>}
      {data?.map((l) => (
        <li key={l.id} className="flex gap-2 py-0.5">
          <span className="text-zinc-500">{formatDateTime(l.created_at)}</span>
          <Badge tone={l.status === "ok" ? "ok" : l.status === "error" ? "error" : "warn"}>{A.status[l.status]}</Badge>
          <span>{l.message}</span>
        </li>
      ))}
    </ul>
  );
}
