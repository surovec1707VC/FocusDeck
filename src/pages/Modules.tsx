import { Link } from "react-router-dom";
import { ShieldAlert } from "lucide-react";
import { Badge, ErrorBanner, PageHeader, Toggle } from "../components/ui";
import { t } from "../i18n";
import { api, ModuleInfo } from "../lib/api";
import { attempt, useAsync } from "../lib/use-async";

export default function Modules() {
  const { data, setData, error, setError } = useAsync(api.modulesList);
  const perms = useAsync(api.permissionsList);
  const permName = new Map((perms.data ?? []).map((p) => [p.id, p.name]));
  const M = t.modules;

  const groups = new Map<string, ModuleInfo[]>();
  for (const m of data ?? []) groups.set(m.category, [...(groups.get(m.category) ?? []), m]);

  async function toggle(m: ModuleInfo) {
    const result = await attempt(() => api.modulesSetEnabled(m.id, !m.enabled), setError);
    if (result) setData(result);
  }

  return (
    <>
      <PageHeader title={M.title} subtitle={M.subtitle} />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="flex flex-col gap-6">
        {[...groups].map(([category, modules]) => (
          <section key={category}>
            <h2 className="mb-2 text-xs font-semibold tracking-wide text-zinc-500 uppercase">{category}</h2>
            <ul className="grid grid-cols-1 gap-3 lg:grid-cols-2">
              {modules.map((m) => (
                <li key={m.id} className="card flex flex-col gap-2">
                  <div className="flex items-center gap-2">
                    <h3 className="flex-1 truncate font-semibold">{m.name}</h3>
                    {m.is_core && <Badge>{M.core}</Badge>}
                    <Toggle checked={m.enabled} disabled={m.is_core} onChange={() => toggle(m)} label={m.name} />
                  </div>
                  <p className="text-zinc-600 dark:text-zinc-400">{m.description}</p>
                  {m.enabled && m.missing_permissions.length > 0 && (
                    <div className="flex flex-wrap items-center gap-2 text-xs text-amber-700 dark:text-amber-400">
                      <ShieldAlert className="size-4" />
                      {M.missingPermissions} {m.missing_permissions.map((p) => permName.get(p) ?? p).join(", ")}
                      <Link to="/permissions" className="underline">
                        {M.openPermissions}
                      </Link>
                    </div>
                  )}
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </>
  );
}
