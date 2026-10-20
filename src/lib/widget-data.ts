import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, EVENTS, Widget, WidgetData } from "./api";

/**
 * Last data of every widget (per params). A remounted widget shows it at once
 * and keeps showing it when a refresh fails (offline, module turned off).
 */
const cache = new Map<string, WidgetData>();

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
      cache.set(key, d);
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
  const refreshSecs = state?.refresh_secs ?? 0;
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
    const unlisteners = eventsKey.split("|").map((name) => listen(name, () => reload()));
    return () => {
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
    const id = setInterval(() => setTick((n) => n + 1), ms);
    return () => clearInterval(id);
  }, [ms, active]);
}
