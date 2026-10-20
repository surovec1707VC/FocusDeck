import { ReactNode } from "react";
import { Link } from "react-router-dom";
import { CloudOff, RefreshCw, ShieldAlert } from "lucide-react";
import { formatDateTime, locale, t } from "../../i18n";
import { Widget, WidgetData } from "../../lib/api";

/** Props of every widget body. */
export interface BodyProps {
  widget: Widget;
}

/** Shows why data isn't fresh (offline, missing permission); cached data stays visible above. */
export function DataStatus({ meta, error, onRetry }: { meta: WidgetData | null; error: string | null; onRetry?: () => void }) {
  const message = error ?? meta?.error;
  if (!message) return null;
  const cachedAt = meta?.from_cache && meta.fetched_at ? formatDateTime(meta.fetched_at) : null;
  return (
    <div className="mt-2 flex items-start gap-1.5 rounded-md bg-amber-500/10 px-2 py-1 text-xs text-amber-700 dark:text-amber-300">
      <CloudOff className="mt-0.5 size-3.5 shrink-0" />
      <span className="min-w-0 flex-1">
        {message}
        {cachedAt && <span className="block opacity-80">{t.widgets.offline(cachedAt)}</span>}
      </span>
      {onRetry && (
        <button className="shrink-0 opacity-70 hover:opacity-100" onClick={onRetry} title={t.widgets.retry} aria-label={t.widgets.retry}>
          <RefreshCw className="size-3.5" />
        </button>
      )}
    </div>
  );
}

export function PermissionNote({ name }: { name: string }) {
  return (
    <div className="flex flex-col items-start gap-1 rounded-md bg-zinc-200/60 p-2 text-xs dark:bg-zinc-800/60">
      <span className="flex items-center gap-1.5">
        <ShieldAlert className="size-3.5 text-amber-500" />
        {t.widgets.needPermission(name)}
      </span>
      <Link to="/permissions" className="text-accent-600 hover:underline dark:text-accent-400">
        {t.widgets.openPermissions}
      </Link>
    </div>
  );
}

export function Meter({ label, value, detail }: { label: string; value: number | null; detail?: ReactNode }) {
  const pct = value === null ? 0 : Math.max(0, Math.min(100, value));
  const tone = pct > 90 ? "bg-red-500" : pct > 70 ? "bg-amber-500" : "bg-accent-500";
  return (
    <div>
      <div className="flex items-baseline justify-between gap-2 text-xs">
        <span className="font-medium">{label}</span>
        <span className="truncate text-zinc-500 tabular-nums">{detail ?? (value === null ? "—" : `${Math.round(pct)}%`)}</span>
      </div>
      <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
        <div className={`h-full rounded-full transition-[width] duration-500 ${tone}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

export function Muted({ children }: { children: ReactNode }) {
  return <p className="text-zinc-500">{children}</p>;
}

const UNITS = ["Б", "КБ", "МБ", "ГБ", "ТБ"];

export function formatBytes(n: number): string {
  let i = 0;
  while (n >= 1024 && i < UNITS.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toLocaleString(locale, { maximumFractionDigits: n < 10 && i > 0 ? 1 : 0 })} ${UNITS[i]}`;
}

/** 3725000 → "1:02:05"; with `cs` → "1:02:05.00". */
export function formatDuration(ms: number, cs = false): string {
  ms = Math.max(0, ms);
  const h = Math.floor(ms / 3_600_000);
  const m = Math.floor((ms % 3_600_000) / 60_000);
  const s = Math.floor((ms % 60_000) / 1000);
  const pad = (v: number) => String(v).padStart(2, "0");
  const base = h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
  return cs ? `${base}.${pad(Math.floor((ms % 1000) / 10))}` : base;
}

export function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return d > 0 ? `${d} д ${h} ч` : h > 0 ? `${h} ч ${m} мин` : `${m} мин`;
}

/** Small icon button used inside widgets. */
export function IconButton({ label, onClick, children, danger, disabled }: { label: string; onClick: () => void; children: ReactNode; danger?: boolean; disabled?: boolean }) {
  return (
    <button
      type="button"
      className={`rounded p-1 text-zinc-400 transition hover:bg-zinc-200 disabled:opacity-40 dark:hover:bg-zinc-800 ${
        danger ? "hover:text-red-500" : "hover:text-zinc-700 dark:hover:text-zinc-200"
      }`}
      onClick={onClick}
      title={label}
      aria-label={label}
      disabled={disabled}
    >
      {children}
    </button>
  );
}
