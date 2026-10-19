import { FormEvent, useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Maximize2, Plus, Trash2 } from "lucide-react";
import { ClockWidget, NoteWidget, StatusWidget, TasksWidget } from "../components/widgets/WidgetBodies";
import { Empty, ErrorBanner, Field, PageHeader } from "../components/ui";
import { t } from "../i18n";
import { api, errorText, Widget, WidgetKind, WidgetSize } from "../lib/api";
import { attempt, useAsync } from "../lib/use-async";
import { useModules } from "../lib/modules-context";

const KINDS: WidgetKind[] = ["clock", "note", "tasks", "status"];
/** Module that owns each widget kind (mirrors the backend catalog). */
const OWNER: Record<WidgetKind, string> = { clock: "widget_clock", note: "notes", tasks: "tasks", status: "widget_status" };
const SIZES: WidgetSize[] = ["sm", "md", "lg"];
const SPAN: Record<WidgetSize, string> = {
  sm: "col-span-1",
  md: "col-span-1 md:col-span-2",
  lg: "col-span-1 md:col-span-2 xl:col-span-4",
};

export default function Widgets() {
  const layouts = useAsync(api.layoutsList);
  const widgets = useAsync(() => api.widgetsList());
  const [error, setError] = useState<string | null>(null);
  const [kind, setKind] = useState<WidgetKind>("note");
  const [title, setTitle] = useState<string>(t.widgets.kinds.note);
  const W = t.widgets;
  const { settingsOf, isEnabled, modules } = useModules();
  const showTitles = settingsOf("widgets").show_titles !== false;
  // Re-fetch when modules change: widgets of a disabled module are hidden by the backend.
  const modulesKey = modules?.map((m) => (m.enabled ? "1" : "0")).join("") ?? "";
  useEffect(() => {
    widgets.reload();
  }, [modulesKey]); // widgets.reload is stable for this page

  // Only kinds whose module is on can be added; keep the selection valid.
  const available = KINDS.filter((k) => isEnabled(OWNER[k]));
  useEffect(() => {
    if (available.length > 0 && !available.includes(kind)) {
      setKind(available[0]);
      setTitle(W.kinds[available[0]]);
    }
  }, [available.join(), kind]);
  const active = layouts.data?.find((l) => l.is_active);

  async function switchLayout(id: string) {
    const result = await attempt(() => api.layoutsActivate(id), setError);
    if (result) {
      layouts.setData(result);
      widgets.reload();
    }
  }

  async function add(e: FormEvent) {
    e.preventDefault();
    const size: WidgetSize = kind === "clock" || kind === "status" ? "sm" : "md";
    const result = await attempt(() => api.widgetsAdd(kind, title, size), setError);
    if (result) widgets.setData(result);
  }

  async function act(fn: () => Promise<unknown>) {
    try {
      await fn();
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
    widgets.reload();
  }

  const nextSize = (w: Widget) => SIZES[(SIZES.indexOf(w.size) + 1) % SIZES.length];

  return (
    <>
      <PageHeader
        title={W.title}
        subtitle={W.subtitle}
        actions={
          layouts.data && (
            <label className="flex items-center gap-2 text-zinc-500">
              {W.layout}
              <select className="input w-48" value={active?.id ?? ""} onChange={(e) => switchLayout(e.target.value)}>
                {layouts.data.map((l) => (
                  <option key={l.id} value={l.id}>
                    {l.name}
                  </option>
                ))}
              </select>
            </label>
          )
        }
      />
      <ErrorBanner message={error ?? widgets.error} onClose={() => setError(null)} />

      <form onSubmit={add} className="card mb-4 flex flex-wrap items-end gap-3">
        <Field label={W.kind}>
          <select
            className="input w-40"
            value={kind}
            onChange={(e) => {
              const k = e.target.value as WidgetKind;
              setKind(k);
              setTitle(W.kinds[k]);
            }}
          >
            {available.map((k) => (
              <option key={k} value={k}>
                {W.kinds[k]}
              </option>
            ))}
          </select>
        </Field>
        <Field label={W.name}>
          <input className="input w-56" value={title} maxLength={60} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <button className="btn-primary" disabled={!title.trim() || available.length === 0}>
          <Plus className="size-4" />
          {W.add}
        </button>
      </form>

      {widgets.data?.length === 0 && <Empty>{W.empty}</Empty>}
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
        {widgets.data?.map((w, i, all) => (
          <article key={w.id} className={`card flex flex-col gap-3 ${SPAN[w.size]}`}>
            <header className="flex items-center gap-1">
              <h2 className={`flex-1 truncate font-semibold ${showTitles ? "" : "sr-only"}`}>{w.title}</h2>
              {!showTitles && <div className="flex-1" />}
              <button className="btn-ghost px-1.5" disabled={i === 0} onClick={() => act(() => api.widgetsMove(w.id, -1))} aria-label={t.common.moveUp} title={t.common.moveUp}>
                <ArrowUp className="size-3.5" />
              </button>
              <button
                className="btn-ghost px-1.5"
                disabled={i === all.length - 1}
                onClick={() => act(() => api.widgetsMove(w.id, 1))}
                aria-label={t.common.moveDown}
                title={t.common.moveDown}
              >
                <ArrowDown className="size-3.5" />
              </button>
              <button
                className="btn-ghost px-1.5"
                onClick={() => act(() => api.widgetsUpdate(w.id, { size: nextSize(w) }))}
                aria-label={W.changeSize}
                title={`${W.changeSize}: ${W.sizes[nextSize(w)]}`}
              >
                <Maximize2 className="size-3.5" />
              </button>
              <button
                className="btn-ghost px-1.5 hover:text-red-500"
                onClick={() => confirm(t.common.confirmDelete) && act(() => api.widgetsRemove(w.id))}
                aria-label={t.common.delete}
                title={t.common.delete}
              >
                <Trash2 className="size-3.5" />
              </button>
            </header>
            {w.kind === "clock" && <ClockWidget />}
            {w.kind === "note" && <NoteWidget widget={w} onError={setError} />}
            {w.kind === "tasks" && <TasksWidget onError={setError} />}
            {w.kind === "status" && <StatusWidget />}
          </article>
        ))}
      </div>
    </>
  );
}
