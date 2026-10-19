import { createContext, ReactNode, useCallback, useContext, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, EVENTS, ModuleInfo, Settings } from "./api";

interface ModulesContextValue {
  modules: ModuleInfo[] | null;
  reload: () => Promise<void>;
  isEnabled: (id: string) => boolean;
  /** Effective settings of a module (defaults until loaded). */
  settingsOf: (id: string) => Settings;
}

const ModulesContext = createContext<ModulesContextValue | null>(null);

/** Keeps the module list current; refreshes on `focusdeck://modules-changed`. */
export function ModulesProvider({ children }: { children: ReactNode }) {
  const [modules, setModules] = useState<ModuleInfo[] | null>(null);

  const reload = useCallback(async () => {
    try {
      setModules(await api.listModules());
    } catch (e) {
      console.error("modules", e);
    }
  }, []);

  useEffect(() => {
    reload();
    const unlisten = listen(EVENTS.modulesChanged, () => reload());
    return () => {
      unlisten.then((f) => f());
    };
  }, [reload]);

  // Until the list arrives, treat modules as enabled to avoid flicker.
  const isEnabled = useCallback((id: string) => modules?.find((m) => m.id === id)?.enabled ?? modules === null, [modules]);
  const settingsOf = useCallback((id: string) => modules?.find((m) => m.id === id)?.settings ?? {}, [modules]);

  return <ModulesContext.Provider value={{ modules, reload, isEnabled, settingsOf }}>{children}</ModulesContext.Provider>;
}

export function useModules() {
  const ctx = useContext(ModulesContext);
  if (!ctx) throw new Error("useModules outside ModulesProvider");
  return ctx;
}
