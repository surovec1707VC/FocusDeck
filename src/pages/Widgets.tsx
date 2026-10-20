import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { ChevronDown, ChevronUp, Minus, Plus, Search, ShieldAlert, Trash2 } from "lucide-react";
import SettingsForm from "../components/SettingsForm";
import { WIDGET_UI } from "../components/widgets/registry";
import { Badge, Empty, ErrorBanner, PageHeader, Toggle } from "../components/ui";
import ConfirmDialog from "../components/ConfirmDialog";
import { t } from "../i18n";
import { api, errorText, Layout, ModuleInfo, Widget, WidgetCatalogEntry, WidgetCategory } from "../lib/api";
import { useModules } from "../lib/modules-context";
import { useModuleToggle } from "../lib/use-module-toggle";
import { useWidgetCatalog } from "../lib/widget-catalog";

const CATEGORIES: WidgetCategory[] = ["time", "productivity", "system", "information"];

export default function Widgets() {
  const W = t.widgets;
  const { catalog, error: catalogError, reload } = useWidgetCatalog();
  const { modules } = useModules();
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<WidgetCategory | "all">("all");
  const [open, setOpen] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [layout, setLayout] = useState<Layout | null>(null);
  const [instances, setInstances] = useState<Widget[]>([]);
  const [removing, setRemoving] = useState<Widget | null>(null);
  const { toggle, dialog } = useModuleToggle(() => reload(), setError);

  async function loadInstances() {
    try {
      const [layouts, list] = await Promise.all([api.layoutsList(), api.widgetsList()]);
      setLayout(layouts.find((l) => l.is_active) ?? null);
      setInstances(list);
    } catch (e) {
      setError(errorText(e));
    }
  }

  // The catalog reloads itself on module changes; instances follow it.
  useEffect(() => {
    loadInstances();
  }, [catalog]);

  async function add(c: WidgetCatalogEntry) {
    try {
      await api.widgetsAdd(c.kind);
      setNotice(W.added(c.title));
      setTimeout(() => setNotice(null), 2500);
      setError(null);
      reload();
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function remove(w: Widget) {
    setRemoving(null);
    try {
      await api.widgetsRemove(w.id);
      reload();
    } catch (e) {
      setError(errorText(e));
    }
  }

  const q = query.trim().toLowerCase();
  const list = (catalog ?? []).filter(
    (c) => (category === "all" || c.category === category) && (!q || `${c.title} ${c.name} ${c.description}`.toLowerCase().includes(q)),
  );
  const moduleOf = (c: WidgetCatalogEntry) => modules?.find((m) => m.id === c.module_id);
  const lastInstance = (c: WidgetCatalogEntry) => [...instances].reverse().find((w) => w.kind === c.kind);

  return (
    <>
      <PageHeader title={W.title} subtitle={W.subtitle} />
      <ErrorBanner message={error ?? catalogError} onClose={() => setError(null)} />
      {notice && <div className="mb-3 rounded-lg bg-accent-500/10 px-3 py-2 text-accent-700 dark:text-accent-300">{notice}</div>}

      <div className="mb-4 flex flex-wrap items-center gap-2">
        <label className="relative min-w-52 flex-1 sm:max-w-xs">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400" />
          <input className="input pl-8" placeholder={W.search} value={query} onChange={(e) => setQuery(e.target.value)} aria-label={W.search} />
        </label>
        <div className="flex flex-wrap gap-1" role="tablist">
          {(["all", ...CATEGORIES] as const).map((c) => (
            <button
              key={c}
              role="tab"
              aria-selected={category === c}
              className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                category === c ? "bg-accent-600 text-white" : "bg-zinc-200 text-zinc-700 hover:bg-zinc-300 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700"
              }`}
              onClick={() => setCategory(c)}
            >
              {c === "all" ? W.allCategories : W.categories[c]}
            </button>
          ))}
        </div>
      </div>

      {catalog && list.length === 0 && <Empty>{W.noMatches}</Empty>}
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
        {list.map((c) => {
          const Icon = WIDGET_UI[c.kind].icon;
          const m = moduleOf(c);
          const last = lastInstance(c);
          const expanded = open === c.kind;
          return (
            <article key={c.kind} className={`card flex flex-col gap-3 ${c.enabled ? "" : "opacity-75"}`}>
              <header className="flex items-start gap-3">
                <div className="rounded-lg bg-accent-500/15 p-2 text-accent-600 dark:text-accent-400">
                  <Icon className="size-5" />
                </div>
                <div className="min-w-0 flex-1">
                  <h2 className="truncate font-semibold">{c.title}</h2>
                  <p className="text-xs text-zinc-500">{c.description}</p>
                </div>
                {m && <Toggle checked={c.enabled} onChange={() => toggle(m as ModuleInfo)} label={c.name} />}
              </header>

              <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
                <dt className="text-zinc-500">{W.source}</dt>
                <dd>{W.sources[c.data_source]}</dd>
                <dt className="text-zinc-500">{W.refresh}</dt>
                <dd>{W.refreshEvery(c.refresh_secs)}</dd>
                <dt className="text-zinc-500">{W.size}</dt>
                <dd>{W.sizeValue(c.default_size.w, c.default_size.h)}</dd>
              </dl>

              <div className="flex flex-wrap gap-1">
                <Badge tone="neutral">{W.categories[c.category]}</Badge>
                {c.instances > 0 && <Badge tone="accent">{W.onDashboard(c.instances)}</Badge>}
              </div>

              {c.missing_modules.length > 0 && c.enabled && <p className="text-xs text-amber-600 dark:text-amber-400">{W.needsModules(c.missing_modules.join(", "))}</p>}
              {c.missing_permissions.length > 0 && (
                <p className="flex flex-wrap items-center gap-1 text-xs text-amber-600 dark:text-amber-400">
                  <ShieldAlert className="size-3.5" />
                  {W.needsPermissions} {c.permissions.filter((p) => !p.granted).map((p) => p.name).join(", ")}
                  <Link to="/permissions" className="text-accent-600 hover:underline dark:text-accent-400">
                    {t.widgets.openPermissions}
                  </Link>
                </p>
              )}

              <div className="mt-auto flex flex-wrap gap-1">
                <button className="btn-primary" disabled={!c.available} onClick={() => add(c)} title={c.available ? undefined : W.needsModules(c.missing_modules.join(", "))}>
                  <Plus className="size-4" />
                  {W.addToDashboard}
                </button>
                {last && (
                  <button className="btn-secondary" onClick={() => setRemoving(last)}>
                    <Minus className="size-4" />
                    {W.removeFromDashboard}
                  </button>
                )}
                <button className="btn-ghost" onClick={() => setOpen(expanded ? null : c.kind)} aria-expanded={expanded}>
                  {expanded ? <ChevronUp className="size-4" /> : <ChevronDown className="size-4" />}
                  {W.configure}
                </button>
              </div>

              {expanded && (
                <div className="border-t border-zinc-200 pt-3 dark:border-zinc-800">
                  <p className="mb-3 text-xs text-zinc-500">{c.details}</p>
                  <SettingsForm
                    schema={c.settings_schema}
                    value={c.settings}
                    onSave={(s) => api.updateModuleSettings(c.module_id, s).then(reload, (e) => setError(errorText(e)))}
                    onReset={() => api.resetModuleSettings(c.module_id).then(reload, (e) => setError(errorText(e)))}
                  />
                </div>
              )}
            </article>
          );
        })}
      </div>

      <section className="card mt-6">
        <h2 className="mb-3 font-semibold">{W.activeLayout(layout?.name ?? "—")}</h2>
        {instances.length === 0 && <p className="text-zinc-500">{W.noInstances}</p>}
        <ul className="divide-y divide-zinc-200 dark:divide-zinc-800">
          {instances.map((w) => {
            const Icon = WIDGET_UI[w.kind]?.icon;
            return (
              <li key={w.id} className="flex items-center gap-3 py-2">
                {Icon && <Icon className="size-4 text-accent-500" />}
                <span className="flex-1 truncate">{w.title}</span>
                {w.hidden && <Badge>{W.hiddenBadge}</Badge>}
                <span className="text-xs text-zinc-500 tabular-nums">{W.sizeValue(w.w, w.h)}</span>
                <button className="btn-ghost px-1.5 hover:text-red-500" onClick={() => setRemoving(w)} aria-label={`${W.remove}: ${w.title}`} title={W.remove}>
                  <Trash2 className="size-4" />
                </button>
              </li>
            );
          })}
        </ul>
      </section>

      {dialog}
      <ConfirmDialog
        open={removing !== null}
        title={W.remove}
        confirmLabel={t.common.delete}
        danger
        onCancel={() => setRemoving(null)}
        onConfirm={() => removing && remove(removing)}
      >
        {removing && W.removeConfirm(removing.title)}
      </ConfirmDialog>
    </>
  );
}
