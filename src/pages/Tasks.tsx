import { FormEvent, useCallback, useEffect, useState } from "react";
import { CalendarClock, ChevronDown, ChevronUp, Flag, Repeat, Search, Trash2, Zap } from "lucide-react";
import { Badge, ErrorBanner, Field, PageHeader } from "../components/ui";
import { locale, t } from "../i18n";
import { api, errorText, QuickTask, Recurrence, Task, TaskFilter, TaskInput, TaskStats, TaskStatus } from "../lib/api";

const STATUSES: TaskStatus[] = ["todo", "in_progress", "done", "cancelled"];
const RECURRENCES: Recurrence[] = ["none", "daily", "weekdays", "weekly", "monthly"];
const PRIORITY_COLOR = ["text-zinc-400", "text-sky-500", "text-amber-500", "text-red-500"];

const todayStr = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

const formatDue = (due: string) => new Date(`${due}T00:00:00`).toLocaleDateString(locale, { day: "numeric", month: "short" });

export default function Tasks() {
  const T = t.tasks;
  const [tasks, setTasks] = useState<Task[]>([]);
  const [stats, setStats] = useState<TaskStats | null>(null);
  const [filter, setFilter] = useState<TaskFilter>({ status: "open" });
  const [line, setLine] = useState("");
  const [preview, setPreview] = useState<QuickTask | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [list, s] = await Promise.all([api.tasksQuery(filter), api.tasksStats()]);
      setTasks(list);
      setStats(s);
    } catch (e) {
      setError(errorText(e));
    }
  }, [filter]);

  useEffect(() => {
    const id = setTimeout(load, 120);
    return () => clearTimeout(id);
  }, [load]);

  useEffect(() => {
    if (!line.trim()) return setPreview(null);
    const id = setTimeout(() => api.tasksQuickParse(line).then(setPreview, () => undefined), 120);
    return () => clearTimeout(id);
  }, [line]);

  async function run(fn: () => Promise<unknown>) {
    try {
      await fn();
      setError(null);
      load();
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  }

  async function quickAdd(e: FormEvent) {
    e.preventDefault();
    if (line.trim() && (await run(() => api.tasksQuickAdd(line)))) setLine("");
  }

  const update = (id: string, input: TaskInput) => run(() => api.tasksUpdate(id, input));
  const set = (patch: TaskFilter) => setFilter({ ...filter, ...patch });
  const today = todayStr();
  const maxDay = Math.max(1, ...(stats?.last_7_days.map((d) => d.count) ?? [1]));

  return (
    <>
      <PageHeader title={T.title} subtitle={T.subtitle} />
      <ErrorBanner message={error} onClose={() => setError(null)} />

      {stats && (
        <div className="mb-4 grid grid-cols-2 gap-3 md:grid-cols-5">
          {[
            [T.completedToday, stats.completed_today],
            [T.open, stats.open],
            [T.inProgress, stats.in_progress],
            [T.dueToday, stats.due_today],
            [T.overdue, stats.overdue],
          ].map(([label, value]) => (
            <div key={label} className="card py-3">
              <div className="text-xs text-zinc-500">{label}</div>
              <div className={`text-2xl font-semibold ${label === T.overdue && Number(value) > 0 ? "text-red-500" : ""}`}>{value}</div>
            </div>
          ))}
          <div className="card col-span-2 py-3 md:col-span-5">
            <div className="mb-2 text-xs text-zinc-500">{T.week}</div>
            <div className="flex h-16 items-end gap-2">
              {stats.last_7_days.map((d) => (
                <div key={d.date} className="flex flex-1 flex-col items-center gap-1" title={`${formatDue(d.date)}: ${d.count}`}>
                  <span className="text-[10px] text-zinc-500 tabular-nums">{d.count || ""}</span>
                  <div className={`w-full rounded-t ${d.date === today ? "bg-accent-500" : "bg-accent-500/40"}`} style={{ height: `${(d.count / maxDay) * 40 + 2}px` }} />
                  <span className="text-[10px] text-zinc-500">{new Date(`${d.date}T00:00:00`).toLocaleDateString(locale, { weekday: "short" })}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      <form onSubmit={quickAdd} className="card mb-3 flex flex-col gap-2">
        <div className="flex gap-2">
          <Zap className="mt-2.5 size-4 shrink-0 text-accent-500" />
          <input className="input flex-1" placeholder={T.quickPlaceholder} value={line} onChange={(e) => setLine(e.target.value)} aria-label={T.quickAdd} />
          <button className="btn-primary" disabled={!line.trim()}>
            {t.common.add}
          </button>
        </div>
        {preview ? (
          <div className="flex flex-wrap items-center gap-1 pl-6 text-xs">
            <span className="font-medium">{preview.title || "…"}</span>
            {preview.priority !== null && <Badge tone="warn">{T.priorities[preview.priority]}</Badge>}
            {preview.due && <Badge tone="accent">{formatDue(preview.due)}</Badge>}
            {preview.category && <Badge>{preview.category}</Badge>}
            {preview.recurrence && <Badge>{T.recurrences[preview.recurrence]}</Badge>}
          </div>
        ) : (
          <p className="pl-6 text-xs text-zinc-500">{T.quickHint}</p>
        )}
      </form>

      <div className="mb-3 flex flex-wrap gap-2">
        <label className="relative min-w-48 flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400" />
          <input className="input pl-8" placeholder={T.search} value={filter.search ?? ""} onChange={(e) => set({ search: e.target.value || undefined })} />
        </label>
        <select className="input w-40" value={filter.status ?? "open"} onChange={(e) => set({ status: e.target.value as TaskFilter["status"] })} aria-label={T.status}>
          <option value="open">{T.filterOpen}</option>
          <option value="all">{T.filterAll}</option>
          {STATUSES.map((s) => (
            <option key={s} value={s}>
              {T.statuses[s]}
            </option>
          ))}
        </select>
        <select className="input w-40" value={filter.priority ?? ""} onChange={(e) => set({ priority: e.target.value ? Number(e.target.value) : undefined })} aria-label={T.priority}>
          <option value="">{T.anyPriority}</option>
          {[1, 2, 3].map((p) => (
            <option key={p} value={p}>
              {T.atLeast(T.priorities[p])}
            </option>
          ))}
        </select>
        <select className="input w-40" value={filter.due ?? ""} onChange={(e) => set({ due: (e.target.value || undefined) as TaskFilter["due"] })} aria-label={T.due}>
          <option value="">{T.anyDue}</option>
          {(["today", "overdue", "week", "none"] as const).map((d) => (
            <option key={d} value={d}>
              {T.dues[d]}
            </option>
          ))}
        </select>
        {stats && stats.categories.length > 0 && (
          <select className="input w-44" value={filter.category ?? ""} onChange={(e) => set({ category: e.target.value || undefined })} aria-label={T.category}>
            <option value="">{T.anyCategory}</option>
            {stats.categories.map((c) => (
              <option key={c} value={c}>
                {"  ".repeat(c.split("/").length - 1) + c.split("/").pop()}
              </option>
            ))}
          </select>
        )}
      </div>

      <ul className="card divide-y divide-zinc-200 p-0 dark:divide-zinc-800">
        {tasks.length === 0 && <li className="p-6 text-center text-zinc-500">{T.none}</li>}
        {tasks.map((task) => {
          const overdue = task.due_at && task.due_at < today && !task.done;
          const expanded = open === task.id;
          return (
            <li key={task.id} className="px-3 py-2">
              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  className="size-4 accent-accent-600"
                  checked={task.done}
                  onChange={() => update(task.id, { status: task.done ? "todo" : "done" })}
                  aria-label={task.title}
                />
                <Flag className={`size-3.5 shrink-0 ${PRIORITY_COLOR[task.priority]}`} aria-label={T.priorities[task.priority]} />
                <span className={`min-w-0 flex-1 truncate ${task.done || task.status === "cancelled" ? "text-zinc-400 line-through" : ""}`}>{task.title}</span>
                {task.category && <Badge>{task.category}</Badge>}
                {task.recurrence !== "none" && <Repeat className="size-3.5 text-zinc-500" aria-label={T.recurrences[task.recurrence]} />}
                {task.due_at && (
                  <span className={`flex items-center gap-1 text-xs ${overdue ? "text-red-500" : "text-zinc-500"}`}>
                    <CalendarClock className="size-3.5" />
                    {formatDue(task.due_at)}
                  </span>
                )}
                <select className="input w-32 py-0.5 text-xs" value={task.status} onChange={(e) => update(task.id, { status: e.target.value as TaskStatus })} aria-label={T.status}>
                  {STATUSES.map((s) => (
                    <option key={s} value={s}>
                      {T.statuses[s]}
                    </option>
                  ))}
                </select>
                <button className="btn-ghost px-1.5" onClick={() => setOpen(expanded ? null : task.id)} aria-expanded={expanded} aria-label={t.common.edit}>
                  {expanded ? <ChevronUp className="size-4" /> : <ChevronDown className="size-4" />}
                </button>
                <button className="btn-ghost px-1.5 hover:text-red-500" onClick={() => run(() => api.tasksRemove(task.id))} aria-label={t.common.delete}>
                  <Trash2 className="size-4" />
                </button>
              </div>
              {expanded && <TaskEditor task={task} onSave={(input) => update(task.id, input)} />}
            </li>
          );
        })}
      </ul>
    </>
  );
}

function TaskEditor({ task, onSave }: { task: Task; onSave: (input: TaskInput) => Promise<boolean> }) {
  const T = t.tasks;
  const [d, setD] = useState<TaskInput>({ title: task.title, notes: task.notes, priority: task.priority, due_at: task.due_at, category: task.category, recurrence: task.recurrence });
  return (
    <form
      className="mt-2 grid gap-2 pl-6 sm:grid-cols-2 lg:grid-cols-4"
      onSubmit={(e) => {
        e.preventDefault();
        onSave(d);
      }}
    >
      <Field label={T.titleLabel}>
        <input className="input" value={d.title} maxLength={200} onChange={(e) => setD({ ...d, title: e.target.value })} />
      </Field>
      <Field label={T.priority}>
        <select className="input" value={d.priority} onChange={(e) => setD({ ...d, priority: Number(e.target.value) })}>
          {[0, 1, 2, 3].map((p) => (
            <option key={p} value={p}>
              {T.priorities[p]}
            </option>
          ))}
        </select>
      </Field>
      <Field label={T.due}>
        <input className="input" type="date" value={d.due_at ?? ""} onChange={(e) => setD({ ...d, due_at: e.target.value || null })} />
      </Field>
      <Field label={T.recurrence}>
        <select className="input" value={d.recurrence} onChange={(e) => setD({ ...d, recurrence: e.target.value as Recurrence })}>
          {RECURRENCES.map((r) => (
            <option key={r} value={r}>
              {T.recurrences[r]}
            </option>
          ))}
        </select>
      </Field>
      <Field label={T.categoryHint}>
        <input className="input" value={d.category} maxLength={80} onChange={(e) => setD({ ...d, category: e.target.value })} />
      </Field>
      <div className="sm:col-span-2 lg:col-span-3">
        <Field label={T.notes}>
          <textarea className="input min-h-16" value={d.notes} maxLength={5000} onChange={(e) => setD({ ...d, notes: e.target.value })} />
        </Field>
      </div>
      <div className="flex items-end">
        <button className="btn-primary" disabled={!d.title?.trim()}>
          {t.common.save}
        </button>
      </div>
    </form>
  );
}
