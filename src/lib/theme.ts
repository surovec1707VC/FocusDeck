import type { Theme } from "./api";

const CACHE_KEY = "focusdeck.theme";
const media = () => window.matchMedia("(prefers-color-scheme: dark)");

let unsubscribe: (() => void) | null = null;

function paint(dark: boolean) {
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

/** Applies a theme; "system" follows the OS and updates live. */
export function applyTheme(theme: Theme) {
  unsubscribe?.();
  unsubscribe = null;
  if (theme === "system") {
    const m = media();
    paint(m.matches);
    const onChange = (e: MediaQueryListEvent) => paint(e.matches);
    m.addEventListener("change", onChange);
    unsubscribe = () => m.removeEventListener("change", onChange);
  } else {
    paint(theme === "dark");
  }
  try {
    localStorage.setItem(CACHE_KEY, theme);
  } catch {
    // Storage unavailable: the theme still applies for this session.
  }
}

/** Theme from the last session, used before settings load (dark by default). */
export function cachedTheme(): Theme {
  try {
    const v = localStorage.getItem(CACHE_KEY);
    if (v === "dark" || v === "light" || v === "system") return v;
  } catch {
    // ignore
  }
  return "dark";
}
