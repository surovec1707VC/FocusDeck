import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { Activity, Download, RefreshCw, ShieldAlert, Upload } from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import SettingsForm from "../components/SettingsForm";
import { Badge, ErrorBanner, PageHeader, Toggle } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { api, ApiError, errorText, HealthState, ImportReport, ModuleCategory, ModuleInfo, ModuleLog, ModuleStatus } from "../lib/api";
import { useModules } from "../lib/modules-context";
import { useSettings } from "../lib/settings-context";
import { useModuleToggle } from "../lib/use-module-toggle";

const C = t.controlCenter;
type Tab = keyof typeof C.tabs;
const HEALTH_TONE: Record<HealthState, "ok" | "warn" | "error" | "neutral"> = { ok: "ok", degraded: "warn", error: "error", disabled: "neutral" };

export default function ControlCenter() {
  const { modules, reload } = useModules();
  const { reload: reloadSettings } = useSettings();
  const [params, setParams] = useSearchParams();
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<ModuleCategory | "all">("all");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [pendingImport, setPendingImport] = useState<{ json: string; message: string } | null>(null);
  const selectedId = params.get("module");
  const { toggle, dialog } = useModuleToggle(() => reload(), setError);

  const categories = useMemo(() => Array.from(new Set((modules ?? []).map((m) => m.category))), [modules]);
  const visible = (modules ?? []).filter((m) => {
    if (category !== "all" && m.category !== category) return false;
    const q = query.trim().toLowerCase();
    return !q || [m.name, m.description, m.id].some((s) => s.toLowerCase().includes(q));
  });
  const selected = modules?.find((m) => m.id === selectedId) ?? null;

  async function exportConfig() {
    setError(null);
    try {
      const path = await api.exportConfigToFile();
      if (path) setNotice(C.exported(path));
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function runImport(json: string, confirm: boolean) {
    try {
      const r = await api.importConfig(json, confirm);
      setReport(r);
      setNotice(`${C.imported}: ${C.importSummary(r.modules_applied, r.permissions_applied)}`);
      await Promise.all([reload(), reloadSettings()]);
    } catch (e) {
      if (e instanceof ApiError && e.kind === "confirmation_required") setPendingImport({ json, message: e.message });
      else setError(errorText(e));
    }
  }

  async function importConfig() {
    setError(null);
    setReport(null);
    try {
      const json = await api.readConfigFile();
      if (json) await runImport(json, false);
    } catch (e) {
      setError(errorText(e));
    }
  }

  return (
    <>
      <PageHeader
        title={C.title}
        subtitle={C.subtitle}
        actions={
          <>
            <button className="btn-secondary" onClick={exportConfig}>
              <Download className="size-4" />
              {C.export}
            </button>
            <button className="btn-secondary" onClick={importConfig}>
              <Upload className="size-4" />
              {C.import}
            </button>
          </>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      {notice && (
        <div className="mb-4 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-emerald-700 dark:text-emerald-300">
          <div className="flex justify-between gap-2">
            <span className="select-text">{notice}</span>
            <button className="text-xs opacity-70 hover:opacity-100" onClick={() => (setNotice(null), setReport(null))} aria-label="Скрыть">
              ✕
            </button>
          </div>
          {report && [...report.warnings, ...report.skipped].length > 0 && (
            <ul className="mt-1 list-disc pl-5 text-xs text-amber-700 dark:text-amber-300">
              {report.warnings.map((w) => (
                <li key={w}>{w}</li>
              ))}
              {report.skipped.map((s) => (
                <li key={s}>
                  {C.skipped}: {s}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,22rem)_1fr]">
        <section className="flex min-w-0 flex-col gap-2">
          <div className="flex gap-2">
            <input type="search" className="input" placeholder={C.search} value={query} onChange={(e) => setQuery(e.target.value)} aria-label={C.search} />
          </div>
          <select className="input" value={category} onChange={(e) => setCategory(e.target.value as ModuleCategory | "all")} aria-label={C.allCategories}>
            <option value="all">{C.allCategories}</option>
            {categories.map((c) => (
              <option key={c} value={c}>
                {t.categories[c]}
              </option>
            ))}
          </select>
          {visible.length === 0 && modules && <p className="p-3 text-zinc-500">{C.noMatches}</p>}
          <ul className="flex flex-col gap-1.5 lg:max-h-[calc(100vh-15rem)] lg:overflow-y-auto lg:pr-1">
            {visible.map((m) => (
              <li key={m.id}>
                <div
                  className={`card flex cursor-pointer items-center gap-3 p-3 transition ${
                    m.id === selectedId ? "border-accent-500/70 dark:border-accent-500/70" : "hover:border-zinc-300 dark:hover:border-zinc-700"
                  }`}
                  onClick={() => setParams({ module: m.id })}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <span className="truncate font-medium">{m.name}</span>
                      {m.critical && <ShieldAlert className="size-3.5 shrink-0 text-amber-500" aria-label={t.modules.critical} />}
                    </div>
                    <div className="truncate text-xs text-zinc-500">{t.categories[m.category]}</div>
                  </div>
                  <span onClick={(e) => e.stopPropagation()}>
                    <Toggle checked={m.enabled} onChange={() => toggle(m)} label={m.name} />
                  </span>
                </div>
              </li>
            ))}
          </ul>
        </section>

        {selected ? (
          <ModuleDetails key={selected.id} module={selected} onToggle={() => toggle(selected)} onChanged={reload} onError={setError} />
        ) : (
          <div className="card grid place-items-center p-10 text-zinc-500">{C.selectHint}</div>
        )}
      </div>

      {dialog}
      <ConfirmDialog
        open={pendingImport !== null}
        title={C.confirmImportTitle}
        confirmLabel={C.importAnyway}
        danger
        onCancel={() => setPendingImport(null)}
        onConfirm={() => {
          const p = pendingImport!;
          setPendingImport(null);
          runImport(p.json, true);
        }}
      >
        {pendingImport?.message}
      </ConfirmDialog>
    </>
  );
}

function ModuleDetails({
  module: m,
  onToggle,
  onChanged,
  onError,
}: {
  module: ModuleInfo;
  onToggle: () => void;
  onChanged: () => Promise<void>;
  onError: (msg: string | null) => void;
}) {
  const [tab, setTab] = useState<Tab>("settings");
  const [status, setStatus] = useState<ModuleStatus | null>(null);
  const [logs, setLogs] = useState<ModuleLog[] | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);

  const check = useCallback(async () => {
    try {
      setStatus(await api.getModuleStatus(m.id));
    } catch (e) {
      onError(errorText(e));
    }
  }, [m.id, onError]);

  const loadLogs = useCallback(async () => {
    try {
      setLogs(await api.getModuleLogs(m.id, 100));
    } catch (e) {
      onError(errorText(e));
    }
  }, [m.id, onError]);

  useEffect(() => {
    if (tab === "status") check();
    if (tab === "logs") loadLogs();
  }, [tab, check, loadLogs, m.enabled, m.updated_at]);

  async function save(settings: Record<string, unknown>) {
    try {
      await api.updateModuleSettings(m.id, settings);
      onError(null);
      await onChanged();
    } catch (e) {
      onError(errorText(e));
    }
  }

  async function reset() {
    setConfirmReset(false);
    try {
      await api.resetModuleSettings(m.id);
      onError(null);
      await onChanged();
    } catch (e) {
      onError(errorText(e));
    }
  }

  return (
    <section className="card flex min-w-0 flex-col gap-4">
      <header className="flex flex-wrap items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-lg font-semibold">{m.name}</h2>
            {m.critical && <Badge tone="warn">{t.modules.critical}</Badge>}
            <Badge>{t.categories[m.category]}</Badge>
            <Badge>
              {t.modules.version} {m.version}
            </Badge>
          </div>
          <p className="mt-1 text-zinc-600 dark:text-zinc-300">{m.description}</p>
        </div>
        <Toggle checked={m.enabled} onChange={onToggle} label={m.name} />
      </header>

      <p className="text-zinc-600 dark:text-zinc-400">{m.details}</p>
      {m.requires_restart && <p className="text-xs text-amber-600 dark:text-amber-400">{m.pending_restart ? C.pendingRestart : C.requiresRestart}</p>}
      {!m.enabled && <p className="text-xs text-zinc-500">{C.disabledNote}</p>}
      {m.missing_permissions.length > 0 && (
        <p className="flex flex-wrap items-center gap-2 text-xs text-amber-700 dark:text-amber-400">
          <ShieldAlert className="size-4" />
          {t.modules.missingPermissions} {m.permissions.filter((p) => !p.granted).map((p) => p.name).join(", ")}
          <Link to="/permissions" className="underline">
            {t.modules.openPermissions}
          </Link>
        </p>
      )}

      <nav className="flex gap-1 border-b border-zinc-200 dark:border-zinc-800" role="tablist">
        {(Object.keys(C.tabs) as Tab[]).map((k) => (
          <button
            key={k}
            role="tab"
            aria-selected={tab === k}
            onClick={() => setTab(k)}
            className={`-mb-px border-b-2 px-3 py-1.5 font-medium transition ${
              tab === k ? "border-accent-500 text-accent-600 dark:text-accent-400" : "border-transparent text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200"
            }`}
          >
            {C.tabs[k]}
          </button>
        ))}
      </nav>

      {tab === "settings" && <SettingsForm schema={m.settings_schema} value={m.settings} onSave={save} onReset={() => setConfirmReset(true)} />}

      {tab === "status" && (
        <div className="flex flex-col gap-3">
          {status && (
            <div className="flex flex-wrap items-center gap-2">
              <Activity className="size-4 text-zinc-400" />
              <Badge tone={HEALTH_TONE[status.state]}>{t.health[status.state]}</Badge>
              <span className="select-text">{status.message}</span>
            </div>
          )}
          {status && (
            <div className="text-xs text-zinc-500">
              {C.checkedAt}: {formatDateTime(status.checked_at)}
            </div>
          )}
          <button className="btn-secondary self-start" onClick={check}>
            <RefreshCw className="size-4" />
            {C.checkNow}
          </button>
        </div>
      )}

      {tab === "permissions" &&
        (m.permissions.length === 0 ? (
          <p className="text-zinc-500">{C.noPermissions}</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {m.permissions.map((p) => (
              <li key={p.id} className="flex flex-wrap items-center gap-2">
                <Badge tone={p.granted ? "ok" : "neutral"}>{p.granted ? t.permissions.granted : t.permissions.denied}</Badge>
                <span className="font-medium">{p.name}</span>
                <code className="text-xs text-zinc-500">{p.id}</code>
              </li>
            ))}
            <li>
              <Link to="/permissions" className="text-xs text-accent-600 underline dark:text-accent-400">
                {t.modules.openPermissions}
              </Link>
            </li>
          </ul>
        ))}

      {tab === "logs" && (
        <div className="overflow-x-auto">
          {logs?.length === 0 && <p className="text-zinc-500">{C.noLogs}</p>}
          <table className="w-full text-left text-xs">
            <tbody className="divide-y divide-zinc-200 dark:divide-zinc-800">
              {logs?.map((l) => (
                <tr key={l.id}>
                  <td className="py-1.5 pr-3 whitespace-nowrap text-zinc-500">{formatDateTime(l.created_at)}</td>
                  <td className="py-1.5 pr-3">
                    <Badge tone={l.level === "error" ? "error" : l.level === "warn" ? "warn" : "neutral"}>{C.actions[l.action] ?? l.action}</Badge>
                  </td>
                  <td className="py-1.5 pr-3 select-text">{l.message}</td>
                  <td className="py-1.5 whitespace-nowrap text-zinc-500">{C.sources[l.source] ?? l.source}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <ConfirmDialog open={confirmReset} title={C.reset} confirmLabel={C.reset} onCancel={() => setConfirmReset(false)} onConfirm={reset}>
        {C.resetConfirm}
      </ConfirmDialog>
    </section>
  );
}
