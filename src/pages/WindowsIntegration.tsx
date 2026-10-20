import { ReactNode, useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  AppWindow, Bell, Bluetooth, Camera, ChevronLeft, ChevronRight, ClipboardCopy, ClipboardPaste, Layers, Maximize2, Minimize2, Plus, RefreshCw,
  ScanSearch, Search, Square, Sun, Volume2, VolumeX, Wifi, WifiOff, X, Zap,
} from "lucide-react";
import { Badge, ErrorBanner, PageHeader } from "../components/ui";
import { t } from "../i18n";
import { api, errorText, InstalledApp, WindowInfo } from "../lib/api";
import { useGuarded } from "../lib/use-guarded";

const W = t.windows;

/** One card; loads its own data so a slow PowerShell call doesn't block the rest. */
function Card({ title, icon, children, onRefresh }: { title: string; icon: ReactNode; children: ReactNode; onRefresh?: () => void }) {
  return (
    <section className="card flex flex-col gap-3">
      <header className="flex items-center gap-2">
        <span className="text-accent-500">{icon}</span>
        <h2 className="flex-1 font-semibold">{title}</h2>
        {onRefresh && (
          <button className="btn-ghost px-1.5" onClick={onRefresh} title={W.refresh} aria-label={W.refresh}>
            <RefreshCw className="size-4" />
          </button>
        )}
      </header>
      {children}
    </section>
  );
}

/** Loads data; a failure (no permission, Safe Mode) is shown inside the card. */
function useLoad<T>(fn: () => Promise<T>) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const reload = useCallback(async () => {
    try {
      setData(await fn());
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);
  useEffect(() => {
    reload();
  }, [reload]);
  return { data, setData, error, reload };
}

function Problem({ error }: { error: string | null }) {
  if (!error) return null;
  return (
    <p className="text-xs text-amber-600 dark:text-amber-400">
      {error}{" "}
      <Link to="/permissions" className="text-accent-600 underline dark:text-accent-400">
        Permissions Center
      </Link>
    </p>
  );
}

export default function WindowsIntegration() {
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const { run, dialog } = useGuarded(setError);
  const flash = (m: string) => {
    setNotice(m);
    setTimeout(() => setNotice(null), 4000);
  };

  return (
    <>
      <PageHeader title={W.title} subtitle={W.subtitle} />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      {notice && <div className="mb-3 truncate rounded-lg bg-accent-500/10 px-3 py-2 text-accent-700 dark:text-accent-300">{notice}</div>}
      <div className="grid gap-4 xl:grid-cols-2">
        <Windows run={run} />
        <Apps run={run} flash={flash} />
        <System run={run} />
        <Network />
        <Desktops run={run} />
        <Tools run={run} flash={flash} />
      </div>
      {dialog}
    </>
  );
}

type Run = ReturnType<typeof useGuarded>["run"];

function Windows({ run }: { run: Run }) {
  const { data, error, reload } = useLoad(api.winWindows);
  const [uia, setUia] = useState<{ title: string; items: { depth: number; name: string; control_type: string }[] } | null>(null);
  const act = (w: WindowInfo, action: "focus" | "minimize" | "maximize" | "restore" | "close") => run((c) => api.winWindow(w.hwnd, action, c), () => setTimeout(reload, 300));
  const btn = "rounded p-1 text-zinc-400 hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200";
  return (
    <Card title={W.windows} icon={<AppWindow className="size-5" />} onRefresh={reload}>
      <Problem error={error} />
      <ul className="flex max-h-80 flex-col overflow-y-auto">
        {data?.map((w) => (
          <li key={w.hwnd} className="group flex items-center gap-2 border-b border-zinc-200 py-1.5 last:border-0 dark:border-zinc-800">
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm">{w.title}</div>
              <div className="text-xs text-zinc-500">
                {w.process}
                {w.active && <Badge tone="accent">{W.active}</Badge>}
                {w.minimized && " · ▁"}
                {w.maximized && " · ▢"}
              </div>
            </div>
            <span className="flex opacity-60 group-hover:opacity-100">
              <button className={btn} title={W.focus} onClick={() => act(w, "focus")}>
                <ChevronRight className="size-4" />
              </button>
              <button className={btn} title={W.minimize} onClick={() => act(w, "minimize")}>
                <Minimize2 className="size-4" />
              </button>
              <button className={btn} title={w.maximized ? W.restore : W.maximize} onClick={() => act(w, w.maximized ? "restore" : "maximize")}>
                {w.maximized ? <Square className="size-4" /> : <Maximize2 className="size-4" />}
              </button>
              <button
                className={btn}
                title={W.inspect}
                onClick={() => run(() => api.winUiaInspect(w.hwnd), (items) => setUia({ title: w.title, items }))}
              >
                <ScanSearch className="size-4" />
              </button>
              <button className={`${btn} hover:text-red-500`} title={W.close} onClick={() => act(w, "close")}>
                <X className="size-4" />
              </button>
            </span>
          </li>
        ))}
      </ul>
      {uia && (
        <div className="rounded-md bg-zinc-100 p-2 text-xs dark:bg-zinc-900">
          <div className="mb-1 flex items-center justify-between font-medium">
            {W.uiaTitle(uia.title)}
            <button onClick={() => setUia(null)} aria-label={t.widgets.close}>
              <X className="size-3.5" />
            </button>
          </div>
          <ul className="max-h-48 overflow-y-auto font-mono">
            {uia.items.map((e, i) => (
              <li key={i} style={{ paddingLeft: e.depth * 12 }} className="truncate">
                <span className="text-zinc-500">{e.control_type}</span> {e.name}
              </li>
            ))}
          </ul>
        </div>
      )}
    </Card>
  );
}

function Apps({ run, flash }: { run: Run; flash: (m: string) => void }) {
  const [query, setQuery] = useState("");
  const [apps, setApps] = useState<InstalledApp[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const id = setTimeout(async () => {
      setBusy(true);
      try {
        setApps(await api.winApps(query));
        setError(null);
      } catch (e) {
        setError(errorText(e));
      }
      setBusy(false);
    }, 250);
    return () => clearTimeout(id);
  }, [query]);
  return (
    <Card title={W.apps} icon={<Search className="size-5" />}>
      <label className="relative">
        <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400" />
        <input className="input pl-8" placeholder={W.appsSearch} value={query} onChange={(e) => setQuery(e.target.value)} />
      </label>
      <Problem error={error} />
      {busy && !apps && <p className="text-xs text-zinc-500">{t.common.loading}</p>}
      <ul className="flex max-h-64 flex-col overflow-y-auto">
        {apps?.map((a) => (
          <li key={a.app_id} className="flex items-center gap-2 py-1">
            <span className="min-w-0 flex-1 truncate text-sm" title={a.app_id}>
              {a.name}
            </span>
            <button className="btn-ghost px-2 text-xs" onClick={() => run((c) => api.winLaunch(a.app_id, c), () => flash(`${W.launch}: ${a.name}`))}>
              {W.launch}
            </button>
          </li>
        ))}
      </ul>
    </Card>
  );
}

function System({ run }: { run: Run }) {
  const vol = useLoad(api.winVolume);
  const bright = useLoad(api.winBrightness);
  const plans = useLoad(api.winPowerPlans);
  const [level, setLevel] = useState<number | null>(null);
  const [b, setB] = useState<number | null>(null);
  const shown = level ?? (vol.data ? Math.round(vol.data.level * 100) : 0);
  const shownB = b ?? bright.data ?? 0;
  return (
    <Card title={`${W.sound} · ${W.brightness} · ${W.power}`} icon={<Volume2 className="size-5" />}>
      <div className="flex items-center gap-3">
        <button
          className="btn-ghost px-1.5"
          title={W.mute}
          disabled={!vol.data}
          onClick={() => run((c) => api.winSetVolume(undefined, !vol.data!.muted, c), vol.setData)}
        >
          {vol.data?.muted ? <VolumeX className="size-5 text-red-500" /> : <Volume2 className="size-5" />}
        </button>
        <input
          type="range"
          min={0}
          max={100}
          className="flex-1 accent-accent-600"
          value={shown}
          disabled={!vol.data}
          aria-label={W.sound}
          onChange={(e) => setLevel(Number(e.target.value))}
          onPointerUp={() => level !== null && run((c) => api.winSetVolume(level, undefined, c), (v) => (vol.setData(v), setLevel(null)))}
          onKeyUp={() => level !== null && run((c) => api.winSetVolume(level, undefined, c), (v) => (vol.setData(v), setLevel(null)))}
        />
        <span className="w-10 text-right text-sm tabular-nums">{shown}%</span>
      </div>
      <Problem error={vol.error} />

      <div className="flex items-center gap-3">
        <Sun className="size-5 shrink-0 text-zinc-500" />
        {bright.data === null && !bright.error ? (
          <span className="text-xs text-zinc-500">{W.brightnessNa}</span>
        ) : (
          <>
            <input
              type="range"
              min={0}
              max={100}
              className="flex-1 accent-accent-600"
              value={shownB}
              aria-label={W.brightness}
              onChange={(e) => setB(Number(e.target.value))}
              onPointerUp={() => b !== null && run((c) => api.winSetBrightness(b, c), () => (bright.setData(b), setB(null)))}
            />
            <span className="w-10 text-right text-sm tabular-nums">{shownB}%</span>
          </>
        )}
      </div>
      <Problem error={bright.error} />

      <div className="flex flex-wrap items-center gap-2">
        <Zap className="size-5 text-zinc-500" />
        {plans.data?.map((p) => (
          <button key={p.guid} className={p.active ? "btn-primary" : "btn-secondary"} disabled={p.active} onClick={() => run((c) => api.winSetPowerPlan(p.guid, c), plans.setData)}>
            {p.name}
          </button>
        ))}
      </div>
      <Problem error={plans.error} />
    </Card>
  );
}

function Network() {
  const wifi = useLoad(api.winWifi);
  const bt = useLoad(api.winBluetooth);
  return (
    <Card title={W.network} icon={<Wifi className="size-5" />} onRefresh={() => (wifi.reload(), bt.reload())}>
      <div className="flex items-center gap-2 text-sm">
        {wifi.data?.connected ? <Wifi className="size-4 text-emerald-500" /> : <WifiOff className="size-4 text-zinc-400" />}
        {!wifi.data ? (
          <span className="text-zinc-500">{wifi.error ? W.loadFailed : t.common.loading}</span>
        ) : !wifi.data.available ? (
          W.wifiNone
        ) : wifi.data.connected ? (
          <span>
            {wifi.data.ssid ?? wifi.data.adapter}
            {wifi.data.signal !== null && <span className="text-zinc-500"> · {wifi.data.signal}%</span>}
          </span>
        ) : (
          W.wifiOff
        )}
      </div>
      <div className="flex items-start gap-2 text-sm">
        <Bluetooth className={`mt-0.5 size-4 ${bt.data?.enabled ? "text-accent-500" : "text-zinc-400"}`} />
        <div>
          {!bt.data ? (
            <span className="text-zinc-500">{bt.error ? W.loadFailed : t.common.loading}</span>
          ) : !bt.data.available ? (
            W.btNone
          ) : (
            <>
              {bt.data.enabled ? W.btOn : W.btOff}
              {bt.data.adapter && <span className="text-zinc-500"> · {bt.data.adapter}</span>}
              {bt.data.devices.length > 0 && (
                <div className="text-xs text-zinc-500">
                  {W.devices}: {bt.data.devices.join(", ")}
                </div>
              )}
            </>
          )}
        </div>
      </div>
      <Problem error={wifi.error ?? bt.error} />
    </Card>
  );
}

function Desktops({ run }: { run: Run }) {
  const d = useLoad(api.winDesktops);
  const act = (a: "next" | "previous" | "new" | "close") => run((c) => api.winDesktop(a, c), () => setTimeout(d.reload, 600));
  return (
    <Card title={W.desktops} icon={<Layers className="size-5" />} onRefresh={d.reload}>
      {d.data && <p className="text-sm">{W.desktopsInfo(d.data.count, d.data.current)}</p>}
      <Problem error={d.error} />
      <div className="flex flex-wrap gap-2">
        <button className="btn-secondary" onClick={() => act("previous")}>
          <ChevronLeft className="size-4" /> {W.prev}
        </button>
        <button className="btn-secondary" onClick={() => act("next")}>
          {W.next} <ChevronRight className="size-4" />
        </button>
        <button className="btn-ghost" onClick={() => act("new")}>
          <Plus className="size-4" /> {W.newDesktop}
        </button>
        <button className="btn-ghost hover:text-red-500" onClick={() => act("close")}>
          <X className="size-4" /> {W.closeDesktop}
        </button>
      </div>
    </Card>
  );
}

function Tools({ run, flash }: { run: Run; flash: (m: string) => void }) {
  const [text, setText] = useState("");
  return (
    <Card title={W.tools} icon={<Camera className="size-5" />}>
      <div className="flex flex-wrap gap-2">
        <button className="btn-secondary" onClick={() => run(() => api.winScreenshot(false), (s) => flash(W.saved(s.path)))}>
          <Camera className="size-4" /> {W.screenshot}
        </button>
        <button className="btn-secondary" onClick={() => run(() => api.winScreenshot(true), (s) => flash(W.saved(s.path)))}>
          <Camera className="size-4" /> {W.screenshotCopy}
        </button>
      </div>
      <div className="flex gap-2">
        <input className="input flex-1" placeholder={W.clipboardText} value={text} onChange={(e) => setText(e.target.value)} />
        <button className="btn-secondary" title={W.writeClipboard} disabled={!text} onClick={() => run((c) => api.winClipboardWrite(text, c), () => flash(W.writeClipboard))}>
          <ClipboardCopy className="size-4" />
        </button>
        <button className="btn-secondary" title={W.readClipboard} onClick={() => run(() => api.winClipboardRead(), (v) => setText(v ?? ""))}>
          <ClipboardPaste className="size-4" />
        </button>
      </div>
      <div className="flex flex-wrap gap-2">
        <button className="btn-ghost" onClick={() => run(() => api.winNotify("FocusDeck", text || "Проверка уведомления", false), () => flash(W.notifyApp))}>
          <Bell className="size-4" /> {W.notifyApp}
        </button>
        <button className="btn-ghost" onClick={() => run(() => api.winNotify("FocusDeck", text || "Проверка уведомления", true), () => flash(W.notifyNative))}>
          <Bell className="size-4" /> {W.notifyNative}
        </button>
      </div>
    </Card>
  );
}
