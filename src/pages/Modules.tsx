import { useState } from "react";
import { Link } from "react-router-dom";
import { Settings2, ShieldAlert } from "lucide-react";
import { Badge, ErrorBanner, PageHeader, Toggle } from "../components/ui";
import { t } from "../i18n";
import { useModules } from "../lib/modules-context";
import { useModuleToggle } from "../lib/use-module-toggle";

/** Compact list of all modules with quick toggles. */
export default function Modules() {
  const { modules, reload } = useModules();
  const [error, setError] = useState<string | null>(null);
  const { toggle, dialog } = useModuleToggle(() => reload(), setError);
  const M = t.modules;
  const on = modules?.filter((m) => m.enabled).length ?? 0;

  return (
    <>
      <PageHeader title={M.title} subtitle={M.subtitle} actions={modules && <Badge>{M.enabledCount(on, modules.length)}</Badge>} />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="card overflow-x-auto p-0">
        <table className="w-full text-left">
          <thead className="text-xs text-zinc-500">
            <tr className="border-b border-zinc-200 dark:border-zinc-800">
              <th className="px-4 py-2 font-medium">{M.colModule}</th>
              <th className="hidden px-2 py-2 font-medium md:table-cell">{M.colCategory}</th>
              <th className="hidden px-2 py-2 font-medium sm:table-cell">{M.version}</th>
              <th className="px-2 py-2 font-medium">
                <span className="sr-only">{M.colEnabled}</span>
              </th>
              <th className="px-2 py-2">
                <span className="sr-only">{M.configure}</span>
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-zinc-200 dark:divide-zinc-800">
            {modules?.map((m) => (
              <tr key={m.id} className={m.enabled ? "" : "text-zinc-500"}>
                <td className="px-4 py-2">
                  <div className="flex items-center gap-1.5">
                    <span className="font-medium">{m.name}</span>
                    {m.critical && <ShieldAlert className="size-3.5 text-amber-500" aria-label={M.critical} />}
                    {m.pending_restart && <Badge tone="warn">{M.restart}</Badge>}
                  </div>
                  <div className="text-xs text-zinc-500">{m.description}</div>
                </td>
                <td className="hidden px-2 py-2 text-xs md:table-cell">{t.categories[m.category]}</td>
                <td className="hidden px-2 py-2 text-xs sm:table-cell">{m.version}</td>
                <td className="px-2 py-2">
                  <Toggle checked={m.enabled} onChange={() => toggle(m)} label={m.name} />
                </td>
                <td className="px-2 py-2 text-right">
                  <Link to={`/control-center?module=${m.id}`} className="btn-ghost px-2" title={M.configure} aria-label={`${M.configure}: ${m.name}`}>
                    <Settings2 className="size-4" />
                  </Link>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {dialog}
    </>
  );
}
