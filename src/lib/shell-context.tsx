import { createContext, useContext } from "react";

/** App-wide overlays (palette, shortcuts help, onboarding), opened from anywhere. */
export interface Shell {
  openPalette: () => void;
  openShortcuts: () => void;
  openOnboarding: () => void;
}

export const ShellContext = createContext<Shell>({ openPalette() {}, openShortcuts() {}, openOnboarding() {} });

export const useShell = () => useContext(ShellContext);

/** localStorage can be unavailable; UI preferences then just aren't remembered. */
export const store = {
  get(key: string): string | null {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  },
  set(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      /* ignore */
    }
  },
};

export const ONBOARDING_KEY = "focusdeck.onboarding.v1";
