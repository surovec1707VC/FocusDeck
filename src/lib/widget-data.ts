import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, EVENTS, Widget, WidgetData } from "./api";

/**
 * Last data of every widget (per params). A remounted widget shows it at once
 * and keeps showing it when a refresh fails (offline, module turned off).
 */
const cache = new Map<string, WidgetData>();
/** Bounded cache: oldest entries go first (removed widgets, old calendar months). */
const CACHE_LIMIT = 200;
function remember(key: string, d: WidgetData) {
  cache.delete(key);
  cache.set(key, d);
  if (cache.size > CACHE_LIMIT) cache.delete(cache.keys().next().value!);
}

/** Event-driven refreshes are coalesced: at most one per widget every THROTTLE_MS. */
export const THROTTLE_MS = 1000;
/** Floor for periodic refresh, whatever a widget asks for. */
export const MIN_REFRESH_SECS = 2;

/** Leading + trailing throttle: the first call runs at once, a burst ends with one more call. */
export function throttle(fn: () => void, ms: number) {
  let last = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const run = () => {
    last = Date.now();
    timer = null;
    fn();
  };
  const call = () => {
    const wait = last + ms - Date.now();
    if (wait <= 0 && !timer) run();
    else if (!timer) timer = setTimeout(run, Math.max(wait, 0));
  };
  call.cancel = () => timer && clearTimeout(timer);
  return call;
}

interface Options {
  /** Kind-specific request parameters (calendar: `{ month }`). */
  params?: Record<string, unknown>;
  /** Extra Tauri events that should trigger a refresh. */
  events?: string[];
}

/**
 * Loads a widget's data through `widget_data`, refreshes it every
 * `refresh_secs` while the window is visible, and runs actions through
 * `widget_action`. Requests are async: the UI never waits on them.
 */
export function useWidgetData<T = any>(widget: Widget, { params, events = [] }: Options = {}) {
  const paramsJson = JSON.stringify(params ?? {});
  const key = `${widget.id}:${paramsJson}`;
  const [state, setState] = useState<WidgetData<T> | null>(() => (cache.get(key) as WidgetData<T>) ?? null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  // Only the newest request may update the state.
  const seq = useRef(0);

  useEffect(() => {
    setState((cache.get(key) as WidgetData<T>) ?? null);
  }, [key]);

  const accept = useCallback(
    (d: WidgetData<T>) => {
      remember(key, d);
      setState(d);
      setError(null);
    },
    [key],
  );

  const reload = useCallback(
    async (force = false) => {
      const mine = ++seq.current;
      setLoading(true);
      try {
        const d = await api.widgetData<T>(widget.id, JSON.parse(paramsJson), force);
        if (mine === seq.current) accept(d);
      } catch (e) {
        if (mine === seq.current) setError(errorText(e));
      } finally {
        if (mine === seq.current) setLoading(false);
      }
    },
    [widget.id, paramsJson, accept],
  );

  const act = useCallback(
    async (action: string, payload?: Record<string, unknown>) => {
      const mine = ++seq.current;
      try {
        const d = await api.widgetAction<T>(widget.id, action, payload, JSON.parse(paramsJson));
        if (mine === seq.current) accept(d);
        return d;
      } catch (e) {
        setError(errorText(e));
        return undefined;
      }
    },
    [widget.id, paramsJson, accept],
  );

  // First load and reload when the widget's config changes (settings dialog).
  const configJson = JSON.stringify(widget.config);
  useEffect(() => {
    reload();
  }, [reload, configJson]);

  // Periodic refresh, paused while the window is hidden (tray).
  const refreshSecs = state?.refresh_secs ? Math.max(state.refresh_secs, MIN_REFRESH_SECS) : 0;
  useEffect(() => {
    if (!refreshSecs) return;
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") reload();
    }, refreshSecs * 1000);
    const onVisible = () => document.visibilityState === "visible" && reload();
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [refreshSecs, reload]);

  // Module settings and other sources of change.
  const eventsKey = [EVENTS.modulesChanged, ...events].join("|");
  useEffect(() => {
    // Bursts (clipboard changes, several modules toggled) cause one refresh, not dozens.
    const throttled = throttle(() => document.visibilityState === "visible" && reload(), THROTTLE_MS);
    const unlisteners = eventsKey.split("|").map((name) => listen(name, throttled));
    return () => {
      throttled.cancel();
      unlisteners.forEach((u) => u.then((f) => f()));
    };
  }, [eventsKey, reload]);

  return { data: state?.data as T | undefined, meta: state, error, setError, loading, reload, act };
}

/** Re-renders every `ms` while `active` (clocks, timers). */
export function useTicker(ms: number, active = true) {
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!active) return;
    // Hidden window (tray): no re-renders at all.
    const id = setInterval(() => document.visibilityState === "visible" && setTick((n) => n + 1), ms);
    return () => clearInterval(id);
  }, [ms, active]);
}
