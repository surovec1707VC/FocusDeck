import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { Activity, Eye, LayoutGrid, Plus, Search, Zap } from "lucide-react";
import WidgetGrid from "../components/widgets/WidgetGrid";
import WidgetSettingsDialog from "../components/widgets/WidgetSettingsDialog";
import { WIDGET_UI } from "../components/widgets/registry";
import { Badge, EmptyState, ErrorBanner, PageHeader, Spinner, Tooltip } from "../components/ui";
import ConfirmDialog from "../components/ConfirmDialog";
import { t } from "../i18n";
import { api, errorText, EVENTS, GridItem, Layout, ModuleStatus, QuickAction, Widget, WidgetKind } from "../lib/api";
import { useShell } from "../lib/shell-context";
import { useGuarded } from "../lib/use-guarded";
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
  const quickMenu = usePopover();
  const statusMenu = usePopover();
  const shell = useShell();
  const [quick, setQuick] = useState<QuickAction[] | null>(null);
  const [health, setHealth] = useState<ModuleStatus[] | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const quickOn = isEnabled("clipboard_history");
  const enabledModules = modules?.filter((m) => m.enabled) ?? [];
  const attention = enabledModules.filter((m) => m.missing_permissions.length > 0 || m.pending_restart);

  useEffect(() => {
    if (quickMenu.open && quickOn) api.quickActionsList().then(setQuick).catch((e) => setError(errorText(e)));
  }, [quickMenu.open, quickOn]);

  // Health of enabled modules, checked on demand (one call per module).
  useEffect(() => {
    if (!statusMenu.open) return;
    setHealth(null);
    Promise.all(enabledModules.map((m) => api.getModuleStatus(m.id).catch(() => null))).then((r) =>
      setHealth(r.filter((x): x is ModuleStatus => x !== null)),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [statusMenu.open]);

  const guarded = useGuarded(setError);
  // Commands still need their confirmation: the backend asks, the dialog answers.
  async function runQuick(q: QuickAction) {
    quickMenu.setOpen(false);
    await guarded.run(
      (confirm) => api.quickActionsRun(q.id, confirm),
      () => {
        setNotice(D.ran(q.name));
        setTimeout(() => setNotice(null), 3000);
      },
    );
  }

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

  // Layout switched by a global hotkey; a widget detached or returned from the desktop.
  useEffect(() => {
    const unlisteners = [EVENTS.layoutChanged, EVENTS.desktopWidgetsChanged].map((e) => listen(e, () => load()));
    return () => {
      unlisteners.forEach((u) => u.then((f) => f()));
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
  const visible = widgets?.filter((w) => !w.hidden && !w.desktop) ?? [];
  // Desktop widgets are listed with the hidden ones: that is where they are returned from.
  const hidden = widgets?.filter((w) => w.hidden || w.desktop) ?? [];
  const desktopOn = isEnabled("desktop_widgets");
  const addable = catalog?.filter((c) => c.available) ?? [];
  const widgetsOn = isEnabled("widgets");

  return (
    <>
      <PageHeader
        title={active?.name ?? D.title}
        subtitle={
          <span className="flex flex-wrap items-center gap-2">
            {active && (
              <Badge tone="accent">
                <LayoutGrid className="mr-1 size-3" aria-hidden /> {D.activeLayout}: {active.name}
              </Badge>
            )}
            {modules && (
              <span className="relative" ref={statusMenu.ref}>
                <button
                  className="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium transition hover:bg-zinc-200 dark:hover:bg-zinc-800"
                  onClick={() => statusMenu.setOpen(!statusMenu.open)}
                  aria-expanded={statusMenu.open}
                  aria-haspopup="dialog"
                >
                  <span className={`size-2 rounded-full ${attention.length ? "bg-amber-500" : "bg-emerald-500"}`} aria-hidden />
                  {D.moduleStatus(enabledModules.length, modules.length)}
                  {attention.length > 0 && <span className="text-amber-600 dark:text-amber-400">· {D.moduleIssues(attention.length)}</span>}
                </button>
                {statusMenu.open && (
                  <div role="dialog" aria-label={D.modules} className="popover animate-fade-in absolute left-0 z-30 mt-1 w-80 p-2 shadow-xl">
                    {!health && (
                      <div className="flex items-center gap-2 p-2 text-zinc-500">
                        <Spinner /> {D.checking}
                      </div>
                    )}
                    {health && health.filter((h) => h.state !== "ok").length === 0 && <div className="p-2 text-emerald-600 dark:text-emerald-400">{D.moduleAllOk}</div>}
                    <ul className="flex max-h-72 flex-col gap-0.5 overflow-y-auto">
                      {health
                        ?.filter((h) => h.state !== "ok")
                        .map((h) => (
                          <li key={h.module_id}>
                            <Link to={`/control-center?module=${h.module_id}`} className="menu-item">
                              <span className={`size-2 shrink-0 rounded-full ${h.state === "error" ? "bg-red-500" : "bg-amber-500"}`} aria-hidden />
                              <span className="min-w-0 flex-1">
                                <span className="block font-medium">{modules.find((m) => m.id === h.module_id)?.name}</span>
                                <span className="block truncate text-xs text-zinc-500">{h.message}</span>
                              </span>
                            </Link>
                          </li>
                        ))}
                    </ul>
                    <Link to="/control-center" className="btn-ghost btn-sm mt-1 w-full">
                      {D.openControlCenter}
                    </Link>
                  </div>
                )}
              </span>
            )}
          </span>
        }
        actions={
          <>
            <Tooltip text={`${D.search} (Ctrl+K)`} side="bottom">
              <button className="btn-secondary" onClick={shell.openPalette} aria-label={D.search}>
                <Search className="size-4" />
              </button>
            </Tooltip>
            <div className="relative" ref={quickMenu.ref}>
              <button className="btn-secondary" onClick={() => quickMenu.setOpen(!quickMenu.open)} aria-expanded={quickMenu.open} aria-haspopup="menu">
                <Zap className="size-4" />
                <span className="hidden sm:inline">{D.quickActions}</span>
              </button>
              {quickMenu.open && (
                <div className="popover animate-fade-in absolute left-0 mt-1 w-72">
                  {!quickOn && <p className="p-2 text-xs text-zinc-500">{D.quickActionsOff}</p>}
                  {quickOn && quick === null && (
                    <div className="p-2">
                      <Spinner />
                    </div>
                  )}
                  {quickOn && quick?.length === 0 && <p className="p-2 text-zinc-500">{D.quickActionsEmpty}</p>}
                  {quickOn && quick && quick.length > 0 && (
                    <ul role="menu" className="flex max-h-80 flex-col gap-0.5 overflow-y-auto">
                      {quick.map((q) => (
                        <li key={q.id} role="none">
                          <button role="menuitem" className="menu-item" onClick={() => runQuick(q)}>
                            <Zap className="size-4 shrink-0 text-accent-500" aria-hidden />
                            <span className="min-w-0 flex-1 truncate">{q.name}</span>
                            {q.hotkey && <kbd className="kbd">{q.hotkey}</kbd>}
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                  <Link to={quickOn ? "/clipboard" : "/control-center?module=clipboard_history"} className="btn-ghost btn-sm mt-1 w-full">
                    {quickOn ? D.quickActionsManage : D.openControlCenter}
                  </Link>
                </div>
              )}
            </div>
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
                  <ul className="popover absolute right-0 mt-1 flex w-60 flex-col gap-0.5 p-1 shadow-xl">
                    {hidden.map((w) => (
                      <li key={w.id}>
                        <button
                          className="flex w-full items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left hover:bg-zinc-200 dark:hover:bg-zinc-800"
                          onClick={() => {
                            hiddenMenu.setOpen(false);
                            if (w.desktop) run(() => api.desktopWidgetAttach(w.id)).then(load);
                            else patch(w, { hidden: false });
                          }}
                        >
                          <span className="min-w-0 truncate">
                            {w.title}
                            {w.desktop && <span className="block text-xs text-zinc-500">{t.widgets.onDesktopBadge}</span>}
                          </span>
                          <span className="shrink-0 text-xs text-accent-600 dark:text-accent-400">{w.desktop ? t.widgets.toApp : D.showAgain}</span>
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
                  <ul className="popover absolute right-0 mt-1 flex max-h-96 w-64 flex-col gap-0.5 overflow-y-auto p-1 shadow-xl">
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
      {notice && (
        <p role="status" className="mb-3 text-xs text-emerald-600 dark:text-emerald-400">
          {notice}
        </p>
      )}

      {!widgetsOn && (
        <EmptyState
          icon={LayoutGrid}
          title={D.widgetsOff}
          action={
            <Link to="/control-center?module=widgets" className="btn-primary">
              {t.gate.open}
            </Link>
          }
        />
      )}
      {widgetsOn && widgets && visible.length === 0 && (
        <EmptyState
          icon={Activity}
          title={D.empty}
          hint={D.emptyHint}
          action={
            <button className="btn-primary" onClick={() => add.setOpen(true)}>
              <Plus className="size-4" /> {D.addWidget}
            </button>
          }
        />
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
            onDesktop: desktopOn ? () => run(() => api.desktopWidgetDetach(w.id)).then((p) => p && load()) : undefined,
          })}
        />
      )}

      {guarded.dialog}
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
