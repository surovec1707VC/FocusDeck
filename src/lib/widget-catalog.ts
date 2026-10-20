import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, EVENTS, WidgetCatalogEntry } from "./api";

/** The widget catalog; reloads when modules change (on/off, settings, permissions). */
export function useWidgetCatalog() {
  const [catalog, setCatalog] = useState<WidgetCatalogEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setCatalog(await api.widgetCatalog());
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  useEffect(() => {
    reload();
    const unlisten = listen(EVENTS.modulesChanged, () => reload());
    return () => {
      unlisten.then((f) => f());
    };
  }, [reload]);

  return { catalog, error, setError, reload };
}
