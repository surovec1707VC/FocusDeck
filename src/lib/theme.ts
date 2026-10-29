import type { Theme } from "./api";

const CACHE_KEY = "focusdeck.theme";
const media = () => window.matchMedia("(prefers-color-scheme: dark)");

let unsubscribe: (() => void) | null = null;

/** Styled themes on top of dark/light (`[data-theme]` in index.css). */
const STYLED: Partial<Record<Theme, boolean>> = { bento: true, glass: false };

function paint(dark: boolean, styled?: Theme) {
  const root = document.documentElement;
  root.classList.toggle("dark", dark);
  root.style.colorScheme = dark ? "dark" : "light";
  if (styled) root.dataset.theme = styled;
  else delete root.dataset.theme;
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
  } else if (theme in STYLED) {
    paint(STYLED[theme]!, theme);
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
    if (v === "dark" || v === "light" || v === "system" || v === "bento" || v === "glass") return v;
  } catch {
    // ignore
  }
  return "dark";
}
