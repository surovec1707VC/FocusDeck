import { Badge, ErrorBanner, PageHeader, Toggle } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { api, Permission } from "../lib/api";
import { useSettings } from "../lib/settings-context";
import { attempt, useAsync } from "../lib/use-async";

export default function Permissions() {
  const { data, setData, error, setError } = useAsync(api.permissionsList);
  const logs = useAsync(() => api.permissionsLogs(50));
  const { reload: reloadSettings } = useSettings();
  const P = t.permissions;
  const names = new Map((data ?? []).map((p) => [p.id, p.name]));

  async function toggle(p: Permission) {
    const result = await attempt(() => api.permissionsSet(p.id, !p.granted), setError);
    if (result) {
      setData(result);
      logs.reload();
      // Revoking can switch off autostart / hotkeys in settings.
      reloadSettings();
    }
  }

  return (
    <>
      <PageHeader title={P.title} subtitle={P.subtitle} />
      <ErrorBanner message={error} onClose={() => setError(null)} />

      <ul className="card divide-y divide-zinc-200 py-0 dark:divide-zinc-800">
        {data?.map((p) => (
          <li key={p.id} className="flex items-center gap-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-medium">{p.name}</span>
                <code className="text-xs text-zinc-500">{p.id}</code>
              </div>
              <div className="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">{p.description}</div>
            </div>
            <Badge tone={p.granted ? "ok" : "neutral"}>{p.granted ? P.granted : P.denied}</Badge>
            <Toggle checked={p.granted} onChange={() => toggle(p)} label={p.name} />
          </li>
        ))}
      </ul>

      <section className="card mt-6">
        <h2 className="mb-3 font-semibold">{P.log}</h2>
        {logs.data?.length === 0 && <p className="text-zinc-500">{P.noLog}</p>}
        <div className="overflow-x-auto">
          <table className="w-full text-left">
            <tbody className="divide-y divide-zinc-200 dark:divide-zinc-800">
              {logs.data?.map((l) => (
                <tr key={l.id}>
                  <td className="py-2 pr-3 whitespace-nowrap text-xs text-zinc-500">{formatDateTime(l.created_at)}</td>
                  <td className="py-2 pr-3">{names.get(l.permission_id) ?? l.permission_id}</td>
                  <td className="py-2 pr-3">
                    <Badge tone={l.action === "granted" ? "ok" : l.action === "denied" ? "error" : "warn"}>{P.actions[l.action]}</Badge>
                  </td>
                  <td className="py-2 text-xs text-zinc-500">{P.sources[l.source] ?? l.source}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </>
  );
}
