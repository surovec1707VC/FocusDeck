import { ReactNode } from "react";
import { Link } from "react-router-dom";
import { Keyboard, LayoutGrid, ListTodo, Power, Puzzle, ShieldCheck, Workflow } from "lucide-react";
import { Badge, ErrorBanner, PageHeader } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { api } from "../lib/api";
import { useAsync } from "../lib/use-async";

function Stat({ to, icon, label, value, hint }: { to: string; icon: ReactNode; label: string; value: ReactNode; hint?: string }) {
  return (
    <Link to={to} className="card flex items-start gap-3 transition hover:border-accent-500/50">
      <div className="rounded-lg bg-accent-500/15 p-2 text-accent-600 dark:text-accent-400">{icon}</div>
      <div className="min-w-0">
        <div className="text-xs text-zinc-500 dark:text-zinc-400">{label}</div>
        <div className="mt-0.5 truncate text-xl font-semibold">{value}</div>
        {hint && <div className="truncate text-xs text-zinc-500">{hint}</div>}
      </div>
    </Link>
  );
}

export default function Dashboard() {
  const stats = useAsync(api.dashboardStats);
  const logs = useAsync(() => api.automationsLogs(undefined, 6));
  const automations = useAsync(api.automationsList);
  const names = new Map((automations.data ?? []).map((a) => [a.id, a.name]));
  const s = stats.data;
  const d = t.dashboard;

  return (
    <>
      <PageHeader title={d.title} subtitle={d.subtitle} />
      <ErrorBanner message={stats.error ?? logs.error} />
      {s && (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
          <Stat to="/modules" icon={<Puzzle className="size-5" />} label={d.modules} value={`${s.modules_enabled} ${d.of} ${s.modules_total}`} hint={d.on} />
          <Stat
            to="/automations"
            icon={<Workflow className="size-5" />}
            label={d.automations}
            value={`${s.automations_enabled} ${d.of} ${s.automations_total}`}
            hint={d.on}
          />
          <Stat
            to="/widgets"
            icon={<LayoutGrid className="size-5" />}
            label={d.activeLayout}
            value={s.active_layout ?? "—"}
            hint={`${s.widgets_active} ${d.widgets}`}
          />
          <Stat to="/widgets" icon={<ListTodo className="size-5" />} label={d.openTasks} value={s.tasks_open} />
          <Stat
            to="/permissions"
            icon={<ShieldCheck className="size-5" />}
            label={d.permissions}
            value={`${s.permissions_granted} ${d.of} ${s.permissions_total}`}
          />
          <Stat to="/settings" icon={<Keyboard className="size-5" />} label={d.hotkeys} value={s.hotkeys_registered} hint={d.registered} />
          <Stat to="/settings" icon={<Power className="size-5" />} label={d.autostart} value={s.autostart ? t.common.enabled : t.common.disabled} />
        </div>
      )}

      <section className="card mt-6">
        <h2 className="mb-3 font-semibold">{d.recentRuns}</h2>
        {logs.data && logs.data.length === 0 && <p className="text-zinc-500">{d.noRuns}</p>}
        <ul className="divide-y divide-zinc-200 dark:divide-zinc-800">
          {logs.data?.map((l) => (
            <li key={l.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 py-2">
              <Badge tone={l.status === "ok" ? "ok" : l.status === "error" ? "error" : "warn"}>{t.automations.status[l.status]}</Badge>
              <span className="font-medium">{names.get(l.automation_id) ?? l.automation_id}</span>
              <span className="text-zinc-500">{l.message}</span>
              <span className="ml-auto text-xs text-zinc-500">{formatDateTime(l.created_at)}</span>
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}
