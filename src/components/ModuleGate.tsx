import { ReactNode, useState } from "react";
import { Link } from "react-router-dom";
import { PowerOff } from "lucide-react";
import { ErrorBanner } from "./ui";
import { t } from "../i18n";
import { useModules } from "../lib/modules-context";
import { useModuleToggle } from "../lib/use-module-toggle";

/** Renders the page only while its module is on; otherwise offers to enable it. */
export default function ModuleGate({ id, children }: { id: string; children: ReactNode }) {
  const { modules, isEnabled, reload } = useModules();
  const [error, setError] = useState<string | null>(null);
  const { toggle, dialog } = useModuleToggle(() => reload(), setError);
  const module = modules?.find((m) => m.id === id);

  if (isEnabled(id) || !module) return <>{children}</>;
  return (
    <div className="card mx-auto mt-10 flex max-w-lg flex-col items-center gap-3 p-8 text-center">
      <PowerOff className="size-8 text-zinc-400" />
      <h1 className="text-lg font-semibold">{t.gate.disabled(module.name)}</h1>
      <p className="text-zinc-500">{t.gate.hint}</p>
      <ErrorBanner message={error} />
      <div className="flex flex-wrap justify-center gap-2">
        <button className="btn-primary" onClick={() => toggle(module)}>
          {t.gate.enable}
        </button>
        <Link className="btn-secondary" to={`/control-center?module=${id}`}>
          {t.gate.open}
        </Link>
      </div>
      {dialog}
    </div>
  );
}
