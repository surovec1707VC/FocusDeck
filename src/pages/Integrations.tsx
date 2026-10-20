import { FormEvent, useEffect, useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  CalendarDays, CheckCircle2, Clapperboard, CodeXml, ExternalLink, FolderOpen, GitBranch, GitMerge, Globe, KeyRound, LucideIcon,
  MessagesSquare, Music, NotebookText, Plug, RadioTower, RefreshCw, Search, Send, SquareTerminal, Wifi, WifiOff, XCircle,
} from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import SettingsForm from "../components/SettingsForm";
import { Badge, Empty, ErrorBanner, PageHeader, Toggle } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { api, ConnectorInfo, ConnectorLog, ConnectorStatus } from "../lib/api";
import { attempt, useAsync } from "../lib/use-async";
import { useGuarded } from "../lib/use-guarded";

const T = t.integrations;

/** Manifest `icon` → icon (lucide has no brand logos). */
const ICONS: Record<string, LucideIcon> = {
  github: GitBranch, gitlab: GitMerge, vscode: CodeXml, terminal: SquareTerminal, obs: Clapperboard, twitch: RadioTower,
  discord: MessagesSquare, telegram: Send, notion: NotebookText, calendar: CalendarDays, spotify: Music, browser: Globe, files: FolderOpen,
};

const STATUS_TONE: Record<ConnectorStatus, "neutral" | "accent" | "ok" | "warn" | "error"> = {
  ok: "ok", warning: "warn", error: "error", not_configured: "warn", offline: "accent", disabled: "neutral", unknown: "neutral",
};
const RISK_TONE = { read: "neutral", change: "warn", dangerous: "error" } as const;
const FILTERS = ["all", "local", "cloud", "enabled"] as const;
const TABS = ["overview", "settings", "access", "actions", "logs"] as const;

function ConnectorIcon({ c, className = "size-5" }: { c: ConnectorInfo; className?: string }) {
  const Icon = ICONS[c.manifest.icon] ?? Plug;
  return <Icon className={className} />;
}

export function StatusBadge({ c }: { c: ConnectorInfo }) {
  return <Badge tone={STATUS_TONE[c.health_status] ?? "neutral"}>{T.status[c.health_status] ?? c.health_status}</Badge>;
}

export default function Integrations() {
  const { data, setData, error, setError, reload } = useAsync(api.integrationsList, []);
  const guarded = useGuarded(setError);
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (data?.connectors ?? []).filter(
      (c) =>
        (filter === "all" || (filter === "enabled" ? c.enabled : c.manifest.kind === filter)) &&
        (!q || c.manifest.name.toLowerCase().includes(q) || c.manifest.description.toLowerCase().includes(q)),
    );
  }, [data, filter, query]);
  const current = data?.connectors.find((c) => c.manifest.id === selected) ?? null;

  /** Replaces one connector in the list after a mutation. */
  const put = (c: ConnectorInfo) => setData((d) => d && { ...d, connectors: d.connectors.map((x) => (x.manifest.id === c.manifest.id ? c : x)) });

  async function toggle(c: ConnectorInfo, on: boolean) {
    await guarded.run((confirm) => api.integrationsSetEnabled(c.manifest.id, on, confirm), put);
  }

  async function setOffline(on: boolean) {
    if ((await attempt(() => api.integrationsSetOffline(on), setError)) !== undefined) reload();
  }

  async function checkAll() {
    setChecking(true);
    const all = await attempt(() => api.integrationsCheck(), setError);
    if (all) setData((d) => d && { ...d, connectors: all });
    setChecking(false);
  }

  const enabledCount = data?.connectors.filter((c) => c.enabled).length ?? 0;

  return (
    <>
      <PageHeader
        title={T.title}
        subtitle={T.subtitle}
        actions={
          <button className="btn-secondary" onClick={checkAll} disabled={checking}>
            <RefreshCw className={`size-4 ${checking ? "animate-spin" : ""}`} />
            {T.checkAll}
          </button>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />

      {data && (
        <section className="card mb-4 flex items-center gap-3">
          {data.offline_mode ? <WifiOff className="size-5 shrink-0 text-accent-500" /> : <Wifi className="size-5 shrink-0 text-zinc-400" />}
          <div className="min-w-0 flex-1">
            <div className="font-medium">
              {T.offlineTitle} <span className="ml-1 text-xs font-normal text-zinc-500">{T.enabledCount}: {enabledCount} / {data.connectors.length}</span>
            </div>
            <div className="text-xs text-zinc-500 dark:text-zinc-400">{data.offline_mode ? T.offlineOn : T.offlineOff}</div>
          </div>
          <Toggle checked={data.offline_mode} onChange={setOffline} label={T.offlineTitle} />
        </section>
      )}

      <div className="mb-3 flex flex-wrap items-center gap-2">
        <div className="relative min-w-48 flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400" />
          <input className="input w-full pl-8" placeholder={T.search} aria-label={T.search} value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <div role="radiogroup" className="flex gap-1">
          {FILTERS.map((f) => (
            <button key={f} role="radio" aria-checked={filter === f} className={filter === f ? "btn-primary" : "btn-secondary"} onClick={() => setFilter(f)}>
              {T.filters[f]}
            </button>
          ))}
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
        <ul className="flex flex-col gap-2" aria-label={T.title}>
          {list.length === 0 && data && <Empty>{T.nothing}</Empty>}
          {list.map((c) => (
            <li key={c.manifest.id}>
              <div
                className={`card flex cursor-pointer items-center gap-3 transition hover:border-accent-500/50 ${selected === c.manifest.id ? "border-accent-500 ring-1 ring-accent-500/40" : ""}`}
                onClick={() => setSelected(c.manifest.id)}
              >
                <div className={`rounded-lg p-2 ${c.enabled ? "bg-accent-500/15 text-accent-600 dark:text-accent-400" : "bg-zinc-200 text-zinc-500 dark:bg-zinc-800"}`}>
                  <ConnectorIcon c={c} />
                </div>
                <button className="min-w-0 flex-1 text-left" onClick={() => setSelected(c.manifest.id)}>
                  <div className="flex flex-wrap items-center gap-1.5 font-medium">
                    {c.manifest.name}
                    <Badge tone={c.manifest.kind === "cloud" ? "accent" : "neutral"}>{T.kind[c.manifest.kind]}</Badge>
                  </div>
                  <div className="truncate text-xs text-zinc-500 dark:text-zinc-400">{c.manifest.description}</div>
                </button>
                <StatusBadge c={c} />
                <span onClick={(e) => e.stopPropagation()}>
                  <Toggle
                    checked={c.enabled}
                    onChange={(v) => toggle(c, v)}
                    disabled={!c.enabled && c.manifest.kind === "cloud" && data!.offline_mode}
                    label={c.manifest.name}
                  />
                </span>
              </div>
            </li>
          ))}
        </ul>

        <div className="lg:sticky lg:top-0 lg:self-start">
          {current ? (
            <Detail key={current.manifest.id} c={current} offline={data!.offline_mode} onChange={put} onToggle={(v) => toggle(current, v)} onError={setError} />
          ) : (
            <Empty>{T.choose}</Empty>
          )}
        </div>
      </div>
      {guarded.dialog}
    </>
  );
}

function Detail({ c, offline, onChange, onToggle, onError }: {
  c: ConnectorInfo;
  offline: boolean;
  onChange: (c: ConnectorInfo) => void;
  onToggle: (on: boolean) => void;
  onError: (m: string | null) => void;
}) {
  const m = c.manifest;
  const [tab, setTab] = useState<(typeof TABS)[number]>("overview");
  const [busy, setBusy] = useState(false);
  const blocked = !c.enabled && m.kind === "cloud" && offline;

  async function check() {
    setBusy(true);
    const r = await attempt(() => api.integrationsCheck(m.id), onError);
    if (r?.[0]) onChange(r[0]);
    setBusy(false);
  }

  return (
    <section className="card flex flex-col gap-4">
      <header className="flex items-start gap-3">
        <div className="rounded-lg bg-accent-500/15 p-2.5 text-accent-600 dark:text-accent-400">
          <ConnectorIcon c={c} className="size-6" />
        </div>
        <div className="min-w-0 flex-1">
          <h2 className="flex flex-wrap items-center gap-2 text-lg font-semibold">
            {m.name} <Badge tone={m.kind === "cloud" ? "accent" : "neutral"}>{T.kind[m.kind]}</Badge>
          </h2>
          <p className="text-zinc-500 dark:text-zinc-400">{m.description}</p>
          <div className="mt-1 flex flex-wrap items-center gap-3 text-xs text-zinc-500">
            <span>{T.version} {m.version}</span>
            <button className="inline-flex items-center gap-1 text-accent-600 hover:underline dark:text-accent-400" onClick={() => openUrl(m.docs_url).catch(() => undefined)}>
              {T.docs} <ExternalLink className="size-3" />
            </button>
          </div>
        </div>
        <Toggle checked={c.enabled} onChange={onToggle} disabled={blocked} label={m.name} />
      </header>
      {blocked && <p className="rounded-lg bg-accent-500/10 px-3 py-2 text-xs text-accent-700 dark:text-accent-300">{T.cloudBlocked}</p>}

      <div role="tablist" className="flex flex-wrap gap-1 border-b border-zinc-200 dark:border-zinc-800">
        {TABS.map((k) => (
          <button
            key={k}
            role="tab"
            aria-selected={tab === k}
            onClick={() => setTab(k)}
            className={`-mb-px border-b-2 px-3 py-2 font-medium transition ${tab === k ? "border-accent-500 text-accent-600 dark:text-accent-400" : "border-transparent text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200"}`}
          >
            {T.tabs[k]}
          </button>
        ))}
      </div>

      {tab === "overview" && (
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <StatusBadge c={c} />
            <span className="min-w-0 flex-1 select-text text-zinc-600 dark:text-zinc-300">{c.health_message}</span>
            <button className="btn-secondary" onClick={check} disabled={busy}>
              <RefreshCw className={`size-4 ${busy ? "animate-spin" : ""}`} />
              {T.check}
            </button>
          </div>
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm">
            <dt className="text-zinc-500">{T.lastCheck}</dt>
            <dd>{c.health_checked_at ? formatDateTime(c.health_checked_at) : T.never}</dd>
            <dt className="text-zinc-500">{T.auth}</dt>
            <dd>
              {T.authTypes[m.auth.type]}
              {m.auth.optional && ` (${T.optional})`}
              {m.auth.type !== "none" && <> · {c.has_secret ? T.secretSaved : T.secretMissing}</>}
            </dd>
            <dt className="text-zinc-500">{T.permissions}</dt>
            <dd>{c.granted.length} / {m.permissions.length}</dd>
            <dt className="text-zinc-500">{T.actions}</dt>
            <dd>{m.actions.length}</dd>
            <dt className="text-zinc-500">{T.events}</dt>
            <dd>{m.events.length}</dd>
          </dl>
        </div>
      )}

      {tab === "settings" && (
        <SettingsForm
          schema={m.settings}
          value={c.settings}
          onSave={async (s) => {
            const r = await attempt(() => api.integrationsUpdateSettings(m.id, s), onError);
            if (r) onChange(r);
          }}
          onReset={async () => {
            const r = await attempt(() => api.integrationsResetSettings(m.id), onError);
            if (r) onChange(r);
          }}
        />
      )}

      {tab === "access" && <Access c={c} onChange={onChange} onError={onError} />}

      {tab === "actions" && (
        <div className="flex flex-col gap-4">
          <p className="text-xs text-zinc-500">{T.stageNote}</p>
          <div>
            <h3 className="mb-2 font-medium">{T.actions}</h3>
            <ul className="flex flex-col gap-2">
              {m.actions.map((a) => {
                const missing = a.permissions.filter((p) => !c.granted.includes(p));
                return (
                  <li key={a.id} className="rounded-lg border border-zinc-200 px-3 py-2 dark:border-zinc-800">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium">{a.name}</span>
                      <code className="text-xs text-zinc-500">{a.id}</code>
                      <span className={`ml-auto inline-flex items-center gap-1 text-xs ${missing.length ? "text-amber-600 dark:text-amber-400" : "text-emerald-600 dark:text-emerald-400"}`}>
                        {missing.length ? <XCircle className="size-3.5" /> : <CheckCircle2 className="size-3.5" />}
                        {missing.length ? T.missing : T.ready}
                      </span>
                    </div>
                    <div className="text-xs text-zinc-500 dark:text-zinc-400">{a.description}</div>
                    {a.permissions.length > 0 && (
                      <div className="mt-1 text-xs text-zinc-500">
                        {T.needs}: {a.permissions.map((p) => m.permissions.find((x) => x.id === p)?.name ?? p).join(", ")}
                      </div>
                    )}
                  </li>
                );
              })}
            </ul>
          </div>
          <div>
            <h3 className="mb-2 font-medium">{T.events}</h3>
            {m.events.length === 0 ? (
              <p className="text-zinc-500">{T.noEvents}</p>
            ) : (
              <ul className="flex flex-col gap-2">
                {m.events.map((e) => (
                  <li key={e.id} className="rounded-lg border border-zinc-200 px-3 py-2 dark:border-zinc-800">
                    <span className="font-medium">{e.name}</span> <code className="text-xs text-zinc-500">{e.id}</code>
                    <div className="text-xs text-zinc-500 dark:text-zinc-400">{e.description}</div>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )}

      {tab === "logs" && <Logs c={c} />}
    </section>
  );
}

function Access({ c, onChange, onError }: { c: ConnectorInfo; onChange: (c: ConnectorInfo) => void; onError: (m: string | null) => void }) {
  const m = c.manifest;
  const [secret, setSecret] = useState("");
  const [confirmRemove, setConfirmRemove] = useState(false);

  async function save(e: FormEvent) {
    e.preventDefault();
    const r = await attempt(() => api.integrationsSetSecret(m.id, secret), onError);
    setSecret("");
    if (r) onChange(r);
  }

  return (
    <div className="flex flex-col gap-5">
      <div>
        <h3 className="mb-1 flex items-center gap-2 font-medium">
          <KeyRound className="size-4" /> {T.secret}
          {m.auth.type !== "none" && <Badge tone={c.has_secret ? "ok" : m.auth.optional ? "neutral" : "warn"}>{c.has_secret ? T.secretSaved : T.secretMissing}</Badge>}
        </h3>
        {m.auth.type === "none" ? (
          <p className="text-zinc-500">{T.noSecret}</p>
        ) : (
          <form onSubmit={save} className="flex flex-col gap-2">
            <label className="text-xs font-medium text-zinc-600 dark:text-zinc-400" htmlFor={`secret-${m.id}`}>
              {m.auth.label || T.authTypes[m.auth.type]}
              {m.auth.optional && ` (${T.optional})`}
            </label>
            <input
              id={`secret-${m.id}`}
              type="password"
              autoComplete="off"
              spellCheck={false}
              className="input"
              placeholder={T.secretPlaceholder}
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
            />
            {m.auth.hint && <p className="text-xs text-zinc-500">{m.auth.hint}</p>}
            <div className="flex flex-wrap gap-2">
              <button className="btn-primary" disabled={!secret.trim()}>{T.secretSave}</button>
              {c.has_secret && (
                <button type="button" className="btn-secondary" onClick={() => setConfirmRemove(true)}>
                  {T.secretRemove}
                </button>
              )}
            </div>
            <p className="text-xs text-zinc-500">{T.secretNote}</p>
          </form>
        )}
      </div>

      <div>
        <h3 className="mb-1 font-medium">{T.permissions}</h3>
        <p className="mb-2 text-xs text-zinc-500">{T.permissionsNote}</p>
        <ul className="flex flex-col divide-y divide-zinc-200 dark:divide-zinc-800">
          {m.permissions.map((p) => (
            <li key={p.id} className="flex items-start gap-3 py-2">
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2 font-medium">
                  {p.name} <Badge tone={RISK_TONE[p.risk]}>{T.risk[p.risk]}</Badge>
                </div>
                <div className="text-xs text-zinc-500 dark:text-zinc-400">{p.description}</div>
              </div>
              <Toggle
                checked={c.granted.includes(p.id)}
                label={p.name}
                onChange={async (v) => {
                  const r = await attempt(() => api.integrationsSetPermission(m.id, p.id, v), onError);
                  if (r) onChange(r);
                }}
              />
            </li>
          ))}
        </ul>
      </div>

      <ConfirmDialog
        open={confirmRemove}
        title={T.secretRemove}
        confirmLabel={T.secretRemove}
        danger
        onCancel={() => setConfirmRemove(false)}
        onConfirm={async () => {
          setConfirmRemove(false);
          const r = await attempt(() => api.integrationsClearSecret(m.id), onError);
          if (r) onChange(r);
        }}
      >
        {T.secretRemoveConfirm}
      </ConfirmDialog>
    </div>
  );
}

const LEVEL_TONE = { info: "neutral", warn: "warn", error: "error" } as const;

function Logs({ c }: { c: ConnectorInfo }) {
  const [logs, setLogs] = useState<ConnectorLog[] | null>(null);
  // Reload whenever the connector changes (a mutation writes a log entry).
  useEffect(() => {
    api.integrationsLogs(c.manifest.id).then(setLogs).catch(() => setLogs([]));
  }, [c]);
  if (!logs) return null;
  if (logs.length === 0) return <p className="text-zinc-500">{T.logsEmpty}</p>;
  return (
    <ul className="flex max-h-96 flex-col gap-1 overflow-y-auto text-sm">
      {logs.map((l) => (
        <li key={l.id} className="flex items-start gap-2 rounded px-1 py-1 hover:bg-zinc-100 dark:hover:bg-zinc-900">
          <span className="shrink-0 text-xs whitespace-nowrap text-zinc-500 tabular-nums">{formatDateTime(l.created_at)}</span>
          <Badge tone={LEVEL_TONE[l.level] ?? "neutral"}>{T.logAction[l.action] ?? l.action}</Badge>
          <span className="min-w-0 flex-1 select-text">{l.message}</span>
        </li>
      ))}
    </ul>
  );
}
