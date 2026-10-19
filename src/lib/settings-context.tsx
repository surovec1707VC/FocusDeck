import { createContext, ReactNode, useCallback, useContext, useEffect, useState } from "react";
import { api, AppSettings } from "./api";
import { applyTheme } from "./theme";

interface SettingsContextValue {
  settings: AppSettings | null;
  save: (next: AppSettings) => Promise<AppSettings>;
  reload: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<AppSettings | null>(null);

  const reload = useCallback(async () => {
    const s = await api.settingsGet();
    setSettings(s);
    applyTheme(s.theme);
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const save = useCallback(async (next: AppSettings) => {
    const saved = await api.settingsUpdate(next);
    setSettings(saved);
    applyTheme(saved.theme);
    return saved;
  }, []);

  return <SettingsContext.Provider value={{ settings, save, reload }}>{children}</SettingsContext.Provider>;
}

export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings outside SettingsProvider");
  return ctx;
}
