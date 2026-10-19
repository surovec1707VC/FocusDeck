import { FormEvent, useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { locale, t } from "../../i18n";
import { api, Widget } from "../../lib/api";
import { attempt, useAsync } from "../../lib/use-async";

export function ClockWidget() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);
  return (
    <div>
      <div className="text-4xl font-semibold tabular-nums">
        {now.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
      </div>
      <div className="mt-1 text-zinc-500 first-letter:uppercase">
        {now.toLocaleDateString(locale, { weekday: "long", day: "numeric", month: "long" })}
      </div>
    </div>
  );
}

/** Note text is stored in the widget's config and saved on blur. */
export function NoteWidget({ widget, onError }: { widget: Widget; onError: (m: string | null) => void }) {
  const initial = typeof widget.config.text === "string" ? widget.config.text : "";
  const [text, setText] = useState(initial);
  return (
    <textarea
      className="input min-h-28 resize-y"
      value={text}
      placeholder={t.widgets.notePlaceholder}
      maxLength={5000}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => text !== initial && attempt(() => api.widgetsUpdate(widget.id, { config: { ...widget.config, text } }), onError)}
    />
  );
}

export function TasksWidget({ onError }: { onError: (m: string | null) => void }) {
  const { data, setData } = useAsync(api.tasksList);
  const [title, setTitle] = useState("");

  async function add(e: FormEvent) {
    e.preventDefault();
    const result = await attempt(() => api.tasksAdd(title), onError);
    if (result) {
      setData(result);
      setTitle("");
    }
  }

  const mutate = async (fn: () => Promise<typeof data>) => {
    const result = await attempt(fn, onError);
    if (result) setData(result);
  };

  return (
    <div className="flex flex-col gap-2">
      <form onSubmit={add}>
        <input
          className="input"
          placeholder={t.widgets.taskPlaceholder}
          value={title}
          maxLength={200}
          onChange={(e) => setTitle(e.target.value)}
        />
      </form>
      {data?.length === 0 && <p className="text-zinc-500">{t.widgets.noTasks}</p>}
      <ul className="flex max-h-60 flex-col gap-1 overflow-y-auto">
        {data?.map((task) => (
          <li key={task.id} className="group flex items-center gap-2 rounded-md px-1 py-0.5 hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60">
            <input
              type="checkbox"
              className="size-4 accent-accent-600"
              checked={task.done}
              onChange={() => mutate(() => api.tasksToggle(task.id))}
              aria-label={task.title}
            />
            <span className={`flex-1 truncate ${task.done ? "text-zinc-400 line-through" : ""}`}>{task.title}</span>
            <button
              className="text-zinc-400 opacity-0 transition group-hover:opacity-100 hover:text-red-500 focus:opacity-100"
              onClick={() => mutate(() => api.tasksDelete(task.id))}
              aria-label={`${t.common.delete}: ${task.title}`}
            >
              <Trash2 className="size-3.5" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function StatusWidget() {
  const { data } = useAsync(api.dashboardStats);
  if (!data) return null;
  const d = t.dashboard;
  const rows: [string, string | number][] = [
    [d.modules, `${data.modules_enabled} ${d.of} ${data.modules_total}`],
    [d.automations, `${data.automations_enabled} ${d.of} ${data.automations_total}`],
    [d.hotkeys, data.hotkeys_registered],
    [d.autostart, data.autostart ? t.common.enabled : t.common.disabled],
  ];
  return (
    <dl className="grid grid-cols-[1fr_auto] gap-y-1">
      {rows.map(([k, v]) => (
        <div key={k} className="contents">
          <dt className="text-zinc-500">{k}</dt>
          <dd className="text-right font-medium">{v}</dd>
        </div>
      ))}
    </dl>
  );
}
