/** System and information widgets: system info, clipboard, notifications, weather, status. */
import { Bell, CheckCheck, Cloud, CloudDrizzle, CloudFog, CloudLightning, CloudRain, CloudSnow, CloudSun, Copy, Moon, Pin, PinOff, RefreshCw, Sun, Trash2, X } from "lucide-react";
import { useState } from "react";
import { formatDateTime, locale, t } from "../../i18n";
import { DashboardStats, EVENTS } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import { useWidgetData } from "../../lib/widget-data";
import { BodyProps, DataStatus, formatBytes, formatUptime, IconButton, Meter, Muted, PermissionNote } from "./common";

interface SystemData {
  cpu_percent: number | null;
  cores: number;
  ram_total: number;
  ram_used: number;
  disks: { mount: string; total: number; free: number }[];
  battery: { percent: number | null; on_ac: boolean; charging: boolean; seconds_left: number | null } | null;
  uptime_secs: number;
}

export function SystemInfoWidget({ widget }: BodyProps) {
  const s = useModules().settingsOf("widget_system_info");
  const { data, meta, error } = useWidgetData<SystemData>(widget);
  const W = t.widgets;
  if (!data) return <DataStatus meta={meta} error={error} />;
  const ramPct = data.ram_total ? (data.ram_used / data.ram_total) * 100 : null;
  return (
    <div className="flex flex-col gap-2.5">
      {s.show_cpu !== false && <Meter label={W.cpu} value={data.cpu_percent} detail={data.cpu_percent === null ? "—" : `${Math.round(data.cpu_percent)}% · ${W.cores(data.cores)}`} />}
      {s.show_ram !== false && <Meter label={W.ram} value={ramPct} detail={`${formatBytes(data.ram_used)} / ${formatBytes(data.ram_total)}`} />}
      {s.show_disk !== false &&
        data.disks.map((d) => (
          <Meter key={d.mount} label={d.mount} value={d.total ? ((d.total - d.free) / d.total) * 100 : null} detail={W.free(formatBytes(d.free))} />
        ))}
      {s.show_battery !== false && data.battery && (
        <Meter
          label={W.battery}
          value={data.battery.percent}
          detail={[
            data.battery.percent === null ? "—" : `${data.battery.percent}%`,
            data.battery.charging ? W.charging : data.battery.on_ac ? W.onAc : data.battery.seconds_left ? formatUptime(data.battery.seconds_left) : null,
          ]
            .filter(Boolean)
            .join(" · ")}
        />
      )}
      <div className="text-xs text-zinc-500">
        {W.uptime}: {formatUptime(data.uptime_secs)}
      </div>
      <DataStatus meta={meta} error={error} />
    </div>
  );
}

interface ClipData {
  items: { id: number; content: string; pinned: boolean; created_at: string }[];
  permission: boolean;
  recording: boolean;
}

export function ClipboardWidget({ widget }: BodyProps) {
  const { data, meta, error, act } = useWidgetData<ClipData>(widget, { events: [EVENTS.clipboardChanged] });
  const [copied, setCopied] = useState<number | null>(null);
  const W = t.widgets;
  if (!data) return <DataStatus meta={meta} error={error} />;

  async function copy(id: number) {
    if (await act("copy", { id })) {
      setCopied(id);
      setTimeout(() => setCopied(null), 1200);
    }
  }

  return (
    <div className="flex h-full flex-col gap-2">
      {!data.permission && <PermissionNote name="Чтение буфера обмена" />}
      {data.permission && !data.recording && <Muted>{W.clipboardPaused}</Muted>}
      {data.items.length === 0 && data.permission && <Muted>{W.clipboardEmpty}</Muted>}
      <ul className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
        {data.items.map((c) => (
          <li key={c.id} className="group flex items-start gap-1 rounded-md px-1 py-0.5 hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60">
            <button className="min-w-0 flex-1 text-left" onClick={() => copy(c.id)} title={W.copy}>
              <span className="line-clamp-2 text-xs break-all whitespace-pre-wrap">{c.content}</span>
              {copied === c.id && <span className="text-xs text-accent-600 dark:text-accent-400">{W.copied}</span>}
            </button>
            <span className={`flex shrink-0 ${c.pinned ? "" : "opacity-0 group-hover:opacity-100 focus-within:opacity-100"}`}>
              <IconButton label={W.copy} onClick={() => copy(c.id)}>
                <Copy className="size-3.5" />
              </IconButton>
              <IconButton label={c.pinned ? W.unpin : W.pin} onClick={() => act("pin", { id: c.id, pinned: !c.pinned })}>
                {c.pinned ? <PinOff className="size-3.5 text-accent-500" /> : <Pin className="size-3.5" />}
              </IconButton>
              <IconButton label={t.common.delete} danger onClick={() => act("delete", { id: c.id })}>
                <X className="size-3.5" />
              </IconButton>
            </span>
          </li>
        ))}
      </ul>
      {data.items.some((c) => !c.pinned) && (
        <button className="self-end text-xs text-zinc-500 hover:text-red-500" onClick={() => act("clear")}>
          {W.clearHistory}
        </button>
      )}
      <DataStatus meta={meta} error={error} />
    </div>
  );
}

interface NotificationsData {
  items: { id: number; level: string; title: string; body: string; source: string; read: boolean; created_at: string }[];
  unread: number;
}

export function NotificationsWidget({ widget }: BodyProps) {
  const { data, meta, error, act } = useWidgetData<NotificationsData>(widget, { events: [EVENTS.notify] });
  const W = t.widgets;
  if (!data) return <DataStatus meta={meta} error={error} />;
  return (
    <div className="flex h-full flex-col gap-2">
      <div className="flex items-center justify-between text-xs text-zinc-500">
        <span>{W.unread(data.unread)}</span>
        <span className="flex gap-1">
          <IconButton label={W.readAll} onClick={() => act("read_all")} disabled={data.unread === 0}>
            <CheckCheck className="size-3.5" />
          </IconButton>
          <IconButton label={W.clearAll} danger onClick={() => act("clear")} disabled={data.items.length === 0}>
            <Trash2 className="size-3.5" />
          </IconButton>
        </span>
      </div>
      {data.items.length === 0 && <Muted>{W.noNotifications}</Muted>}
      <ul className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto">
        {data.items.map((n) => (
          <li key={n.id} className="group flex gap-2 rounded-md px-1 py-1 hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60">
            <Bell className={`mt-0.5 size-3.5 shrink-0 ${n.read ? "text-zinc-400" : "text-accent-500"}`} />
            <div className="min-w-0 flex-1">
              <div className={`truncate ${n.read ? "" : "font-semibold"}`}>{n.title}</div>
              {n.body && <div className="line-clamp-2 text-xs text-zinc-600 dark:text-zinc-300">{n.body}</div>}
              <div className="text-xs text-zinc-500">
                {n.source} · {formatDateTime(n.created_at)}
              </div>
            </div>
            <span className="opacity-0 group-hover:opacity-100 focus-within:opacity-100">
              <IconButton label={t.common.delete} danger onClick={() => act("delete", { id: n.id })}>
                <X className="size-3.5" />
              </IconButton>
            </span>
          </li>
        ))}
      </ul>
      <DataStatus meta={meta} error={error} />
    </div>
  );
}

interface WeatherData {
  city: string;
  country?: string;
  units: "celsius" | "fahrenheit";
  current: { temperature: number; apparent: number; humidity: number; wind: number; code: number; is_day: boolean };
  daily: { date: string; code: number; max: number; min: number }[];
}

function WeatherIcon({ code, day = true, className }: { code: number; day?: boolean; className: string }) {
  const Icon =
    code === 0 ? (day ? Sun : Moon)
    : code <= 2 ? CloudSun
    : code === 3 ? Cloud
    : code <= 48 ? CloudFog
    : code <= 57 ? CloudDrizzle
    : code <= 67 || (code >= 80 && code <= 82) ? CloudRain
    : code <= 77 || code === 85 || code === 86 ? CloudSnow
    : CloudLightning;
  return <Icon className={className} />;
}

export function WeatherWidget({ widget }: BodyProps) {
  const { data, meta, error, reload, loading } = useWidgetData<WeatherData | null>(widget);
  const W = t.widgets;
  const deg = (v: number) => `${Math.round(v)}°${data?.units === "fahrenheit" ? "F" : ""}`;
  const retry = () => reload(true);

  if (!data) {
    const noPermission = (meta?.error ?? "").includes("разрешения");
    return (
      <div className="flex flex-col gap-2">
        {noPermission ? <PermissionNote name="Сеть: прогноз погоды" /> : <Muted>{W.weatherNoData}</Muted>}
        <DataStatus meta={meta} error={noPermission ? null : error} onRetry={retry} />
      </div>
    );
  }
  const c = data.current;
  return (
    <div className="flex h-full flex-col gap-2">
      <div className="flex items-center gap-3">
        <WeatherIcon code={c.code} day={c.is_day} className="size-10 shrink-0 text-accent-500" />
        <div className="min-w-0 flex-1">
          <div className="text-3xl font-semibold tabular-nums">{deg(c.temperature)}</div>
          <div className="truncate text-xs text-zinc-500">
            {t.weatherCodes[c.code] ?? "—"} · {W.feelsLike(deg(c.apparent))}
          </div>
        </div>
        <IconButton label={W.retry} onClick={retry} disabled={loading}>
          <RefreshCw className={`size-3.5 ${loading ? "animate-spin" : ""}`} />
        </IconButton>
      </div>
      <div className="flex justify-between text-xs text-zinc-500">
        <span className="truncate font-medium text-zinc-700 dark:text-zinc-300">{data.city}</span>
        <span>
          {W.humidity} {Math.round(c.humidity)}% · {W.wind} {Math.round(c.wind)} {W.windUnit}
        </span>
      </div>
      <div className="grid grid-cols-3 gap-1 text-center text-xs">
        {data.daily.map((d, i) => (
          <div key={d.date} className="rounded-md bg-zinc-200/50 p-1 dark:bg-zinc-800/50">
            <div className="text-zinc-500 first-letter:uppercase">{i === 0 ? W.today : new Date(d.date).toLocaleDateString(locale, { weekday: "short" })}</div>
            <WeatherIcon code={d.code} className="mx-auto my-0.5 size-4" />
            <div className="tabular-nums">
              {deg(d.max)} <span className="text-zinc-500">{deg(d.min)}</span>
            </div>
          </div>
        ))}
      </div>
      <DataStatus meta={meta} error={error} onRetry={retry} />
    </div>
  );
}

export function StatusWidget({ widget }: BodyProps) {
  const { data, meta, error } = useWidgetData<DashboardStats>(widget);
  if (!data) return <DataStatus meta={meta} error={error} />;
  const d = t.dashboard;
  const rows: [string, string | number][] = [
    [d.modules, `${data.modules_enabled} ${d.of} ${data.modules_total}`],
    [d.automations, `${data.automations_enabled} ${d.of} ${data.automations_total}`],
    [d.openTasks, data.tasks_open],
    [d.permissions, `${data.permissions_granted} ${d.of} ${data.permissions_total}`],
    [d.hotkeys, data.hotkeys_registered],
    [d.autostart, data.autostart ? t.common.enabled : t.common.disabled],
  ];
  return (
    <dl className="grid grid-cols-[1fr_auto] gap-y-1">
      {rows.map(([k, v]) => (
        <div key={k} className="contents">
          <dt className="text-zinc-500">{k}</dt>
          <dd className="text-right font-medium">{v}</dd>
        </div>
      ))}
    </dl>
  );
}
