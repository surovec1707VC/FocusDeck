/** Productivity widgets: notes, todo list, quick links, app launcher. */
import { FormEvent, useEffect, useRef, useState } from "react";
import { AlertTriangle, ExternalLink, Rocket, Trash2 } from "lucide-react";
import { t } from "../../i18n";
import { Task } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import { useWidgetData } from "../../lib/widget-data";
import { BodyProps, DataStatus, IconButton, Muted, PermissionNote } from "./common";

interface NoteData {
  note: { id: string; title: string; body: string; updated_at: string };
}

/** Saves after a pause in typing and when the field loses focus. */
export function NotesWidget({ widget }: BodyProps) {
  const { monospace = false, autosave_ms = 800 } = useModules().settingsOf("widget_notes");
  const { data, meta, error, act } = useWidgetData<NoteData>(widget);
  const [text, setText] = useState<string | null>(null);
  const [status, setStatus] = useState<"idle" | "saving" | "saved">("idle");
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const saved = data?.note.body ?? "";
  const value = text ?? saved;

  async function save(body: string) {
    clearTimeout(timer.current);
    if (body === saved) return;
    setStatus("saving");
    const ok = await act("save", { body });
    setStatus(ok ? "saved" : "idle");
    if (ok) setText(null);
  }

  useEffect(() => () => clearTimeout(timer.current), []);

  return (
    <div className="flex h-full flex-col gap-1">
      <textarea
        className={`input min-h-16 flex-1 resize-none ${monospace ? "font-mono text-xs" : ""}`}
        value={value}
        placeholder={t.widgets.notePlaceholder}
        maxLength={100_000}
        onChange={(e) => {
          const body = e.target.value;
          setText(body);
          setStatus("idle");
          clearTimeout(timer.current);
          timer.current = setTimeout(() => save(body), Number(autosave_ms));
        }}
        onBlur={() => text !== null && save(text)}
        aria-label={widget.title}
      />
      <div className="h-4 text-right text-xs text-zinc-500">{status === "saving" ? t.widgets.saving : status === "saved" ? t.widgets.saved : ""}</div>
      <DataStatus meta={meta} error={error} />
    </div>
  );
}

interface TodoData {
  tasks: Task[];
  open: number;
  done: number;
  hide_completed: boolean;
}

export function TodoWidget({ widget }: BodyProps) {
  const { show_counter = true } = useModules().settingsOf("widget_todo");
  const { data, meta, error, act } = useWidgetData<TodoData>(widget);
  const [title, setTitle] = useState("");
  const W = t.widgets;

  async function add(e: FormEvent) {
    e.preventDefault();
    if (title.trim() && (await act("add", { title }))) setTitle("");
  }

  const tasks = data?.tasks.filter((task) => !(data.hide_completed && task.done)) ?? [];
  return (
    <div className="flex h-full flex-col gap-2">
      <form onSubmit={add}>
        <input className="input" placeholder={W.taskPlaceholder} value={title} maxLength={200} onChange={(e) => setTitle(e.target.value)} />
      </form>
      {data && tasks.length === 0 && <Muted>{W.noTasks}</Muted>}
      <ul className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
        {tasks.map((task) => (
          <li key={task.id} className="group flex items-center gap-2 rounded-md px-1 py-0.5 hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60">
            <input type="checkbox" className="size-4 accent-accent-600" checked={task.done} onChange={() => act("toggle", { id: task.id })} aria-label={task.title} />
            <span className={`flex-1 truncate ${task.done ? "text-zinc-400 line-through" : ""}`}>{task.title}</span>
            <span className="opacity-0 group-hover:opacity-100 focus-within:opacity-100">
              <IconButton label={`${t.common.delete}: ${task.title}`} danger onClick={() => act("delete", { id: task.id })}>
                <Trash2 className="size-3.5" />
              </IconButton>
            </span>
          </li>
        ))}
      </ul>
      {data && Boolean(show_counter) && (
        <div className="flex items-center justify-between text-xs text-zinc-500">
          <span>{W.tasksLeft(data.open, data.done)}</span>
          {data.done > 0 && (
            <button className="hover:text-red-500" onClick={() => act("clear_done")}>
              {W.clearDone}
            </button>
          )}
        </div>
      )}
      <DataStatus meta={meta} error={error} />
    </div>
  );
}

interface LinksData {
  links: { title: string; url: string }[];
}

export function QuickLinksWidget({ widget }: BodyProps) {
  const { show_urls = false } = useModules().settingsOf("widget_quick_links");
  const { data, meta, error, act } = useWidgetData<LinksData>(widget);
  if (data && data.links.length === 0) return <Muted>{t.widgets.noLinks}</Muted>;
  return (
    <div className="flex flex-col gap-1">
      <ul className="flex flex-col gap-0.5">
        {data?.links.map((l, index) => (
          <li key={index}>
            <button
              className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left hover:bg-zinc-200/70 dark:hover:bg-zinc-800"
              onClick={() => act("open", { index })}
              title={l.url}
            >
              <ExternalLink className="size-3.5 shrink-0 text-accent-500" />
              <span className="min-w-0 flex-1">
                <span className="block truncate">{l.title}</span>
                {Boolean(show_urls) && <span className="block truncate text-xs text-zinc-500">{l.url}</span>}
              </span>
            </button>
          </li>
        ))}
      </ul>
      <DataStatus meta={meta} error={error} />
    </div>
  );
}

interface LauncherData {
  apps: { name: string; path: string; exists: boolean }[];
  permission: boolean;
}

export function AppLauncherWidget({ widget }: BodyProps) {
  const { confirm_launch = false } = useModules().settingsOf("widget_app_launcher");
  const { data, meta, error, act } = useWidgetData<LauncherData>(widget);
  const W = t.widgets;
  if (data && data.apps.length === 0) return <Muted>{W.noApps}</Muted>;

  const launch = (index: number, name: string) => {
    if (confirm_launch && !window.confirm(W.launchConfirm(name))) return;
    act("launch", { index });
  };

  return (
    <div className="flex flex-col gap-2">
      {data && !data.permission && <PermissionNote name="Запуск программ" />}
      <div className="grid grid-cols-[repeat(auto-fill,minmax(5.5rem,1fr))] gap-1.5">
        {data?.apps.map((a, index) => (
          <button
            key={index}
            className="flex flex-col items-center gap-1 rounded-lg p-2 text-xs transition hover:bg-zinc-200/70 disabled:opacity-50 dark:hover:bg-zinc-800"
            disabled={!a.exists || !data.permission}
            onClick={() => launch(index, a.name)}
            title={a.exists ? a.path : `${W.appMissing}: ${a.path}`}
          >
            {a.exists ? <Rocket className="size-5 text-accent-500" /> : <AlertTriangle className="size-5 text-amber-500" />}
            <span className="w-full truncate text-center">{a.name}</span>
          </button>
        ))}
      </div>
      <DataStatus meta={meta} error={error} />
    </div>
  );
}
