import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { Eye, Plus } from "lucide-react";
import WidgetGrid from "../components/widgets/WidgetGrid";
import WidgetSettingsDialog from "../components/widgets/WidgetSettingsDialog";
import { WIDGET_UI } from "../components/widgets/registry";
import { Empty, ErrorBanner, PageHeader } from "../components/ui";
import ConfirmDialog from "../components/ConfirmDialog";
import { t } from "../i18n";
import { api, errorText, EVENTS, GridItem, Layout, Widget, WidgetKind } from "../lib/api";
import { useModules } from "../lib/modules-context";
import { useWidgetCatalog } from "../lib/widget-catalog";

/** Closes a popover on outside click or Esc. */
function usePopover() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => ref.current && !ref.current.contains(e.target as Node) && setOpen(false);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);
  return { open, setOpen, ref };
}

export default function Dashboard() {
  const D = t.dashboard;
  const { isEnabled, settingsOf, modules } = useModules();
  const { catalog } = useWidgetCatalog();
  const [layouts, setLayouts] = useState<Layout[] | null>(null);
  const [widgets, setWidgets] = useState<Widget[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<Widget | null>(null);
  const [removing, setRemoving] = useState<Widget | null>(null);
  const add = usePopover();
  const hiddenMenu = usePopover();

  const grid = settingsOf("dashboard");
  const cols = Number(grid.columns ?? 12);
  const rowHeight = Number(grid.row_height ?? 72);
  const compact = grid.compact !== false;
  const active = layouts?.find((l) => l.is_active);

  const load = useCallback(async () => {
    try {
      const [l, w] = await Promise.all([api.layoutsList(), api.widgetsList()]);
      setLayouts(l);
      setWidgets(w);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  // Widgets of turned-off modules disappear, turned-on ones come back.
  const modulesKey = modules?.map((m) => (m.enabled ? "1" : "0")).join("") ?? "";
  useEffect(() => {
    load();
  }, [load, modulesKey]);

  // Layout switched by a global hotkey.
  useEffect(() => {
    const unlisten = listen(EVENTS.layoutChanged, () => load());
    return () => {
      unlisten.then((f) => f());
    };
  }, [load]);

  const replace = (w: Widget) => setWidgets((list) => list?.map((x) => (x.id === w.id ? w : x)) ?? null);

  async function run<T>(fn: () => Promise<T>) {
    try {
      const result = await fn();
      setError(null);
      return result;
    } catch (e) {
      setError(errorText(e));
      return undefined;
    }
  }

  async function switchLayout(id: string) {
    if (await run(() => api.layoutsActivate(id))) load();
  }

  async function addWidget(kind: WidgetKind) {
    add.setOpen(false);
    const w = await run(() => api.widgetsAdd(kind));
    if (w) setWidgets((list) => [...(list ?? []), w]);
  }

  async function commit(items: GridItem[]) {
    if (!active) return;
    // Optimistic: the grid already shows the new positions.
    setWidgets((list) => list?.map((w) => ({ ...w, ...items.find((i) => i.id === w.id) })) ?? null);
    if ((await run(() => api.widgetsSaveGrid(active.id, items))) === undefined) load();
  }

  async function patch(w: Widget, p: Parameters<typeof api.widgetsUpdate>[1]) {
    const updated = await run(() => api.widgetsUpdate(w.id, p));
    if (updated) replace(updated);
  }

  async function remove(w: Widget) {
    setRemoving(null);
    if ((await run(() => api.widgetsRemove(w.id))) !== undefined) setWidgets((list) => list?.filter((x) => x.id !== w.id) ?? null);
  }

  const entry = (kind: WidgetKind) => catalog?.find((c) => c.kind === kind);
  const minSize = (w: Widget) => entry(w.kind)?.min_size ?? { w: 2, h: 2 };
  const visible = widgets?.filter((w) => !w.hidden) ?? [];
  const hidden = widgets?.filter((w) => w.hidden) ?? [];
  const addable = catalog?.filter((c) => c.available) ?? [];
  const widgetsOn = isEnabled("widgets");

  return (
    <>
      <PageHeader
        title={active?.name ?? D.title}
        subtitle={D.subtitle}
        actions={
          <>
            {layouts && (
              <label className="flex items-center gap-2 text-zinc-500">
                <span className="hidden sm:inline">{D.layout}</span>
                <select className="input w-44" value={active?.id ?? ""} onChange={(e) => switchLayout(e.target.value)} aria-label={D.layout}>
                  {layouts.map((l) => (
                    <option key={l.id} value={l.id}>
                      {l.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {hidden.length > 0 && (
              <div className="relative" ref={hiddenMenu.ref}>
                <button className="btn-secondary" onClick={() => hiddenMenu.setOpen(!hiddenMenu.open)} aria-expanded={hiddenMenu.open}>
                  <Eye className="size-4" />
                  {D.hidden(hidden.length)}
                </button>
                {hiddenMenu.open && (
                  <ul className="card absolute right-0 z-30 mt-1 flex w-60 flex-col gap-0.5 p-1 shadow-xl">
                    {hidden.map((w) => (
                      <li key={w.id}>
                        <button
                          className="flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left hover:bg-zinc-200 dark:hover:bg-zinc-800"
                          onClick={() => {
                            hiddenMenu.setOpen(false);
                            patch(w, { hidden: false });
                          }}
                        >
                          <span className="truncate">{w.title}</span>
                          <span className="text-xs text-accent-600 dark:text-accent-400">{D.showAgain}</span>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
            {widgetsOn && (
              <div className="relative" ref={add.ref}>
                <button className="btn-primary" onClick={() => add.setOpen(!add.open)} aria-expanded={add.open}>
                  <Plus className="size-4" />
                  {D.addWidget}
                </button>
                {add.open && (
                  <ul className="card absolute right-0 z-30 mt-1 flex max-h-96 w-64 flex-col gap-0.5 overflow-y-auto p-1 shadow-xl">
                    {addable.length === 0 && <li className="p-2 text-zinc-500">{D.nothingToAdd}</li>}
                    {addable.map((c) => {
                      const Icon = WIDGET_UI[c.kind].icon;
                      return (
                        <li key={c.kind}>
                          <button className="flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-zinc-200 dark:hover:bg-zinc-800" onClick={() => addWidget(c.kind)}>
                            <Icon className="mt-0.5 size-4 shrink-0 text-accent-500" />
                            <span className="min-w-0">
                              <span className="block font-medium">{c.title}</span>
                              <span className="block truncate text-xs text-zinc-500">{c.description}</span>
                            </span>
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                )}
              </div>
            )}
          </>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />

      {!widgetsOn && (
        <Empty>
          {D.widgetsOff}{" "}
          <Link to="/control-center" className="text-accent-600 hover:underline dark:text-accent-400">
            {t.gate.open}
          </Link>
        </Empty>
      )}
      {widgetsOn && widgets && visible.length === 0 && (
        <Empty>
          <p>{D.empty}</p>
          <p className="mt-1 text-xs">{D.emptyHint}</p>
        </Empty>
      )}
      {widgetsOn && visible.length > 0 && (
        <WidgetGrid
          widgets={visible}
          minSize={minSize}
          cols={cols}
          rowHeight={rowHeight}
          compact={compact}
          onCommit={commit}
          actions={(w) => ({
            onPin: () => patch(w, { pinned: !w.pinned }),
            onCollapse: () => patch(w, { collapsed: !w.collapsed }),
            onHide: () => patch(w, { hidden: true }),
            onRemove: () => setRemoving(w),
            onSettings: () => setEditing(w),
          })}
        />
      )}

      <WidgetSettingsDialog widget={editing} entry={editing ? entry(editing.kind) : undefined} onClose={() => setEditing(null)} onSaved={replace} />
      <ConfirmDialog
        open={removing !== null}
        title={t.widgets.remove}
        confirmLabel={t.common.delete}
        danger
        onCancel={() => setRemoving(null)}
        onConfirm={() => removing && remove(removing)}
      >
        {removing && t.widgets.removeConfirm(removing.title)}
      </ConfirmDialog>
    </>
  );
}
