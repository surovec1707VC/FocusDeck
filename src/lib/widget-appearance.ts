import { CSSProperties } from "react";

/** Look of one widget card, stored in `widget.config.appearance` (validated by the backend). */
export interface Appearance {
  shape?: Shape;
  color?: string | null;
  texture?: Texture;
  opacity?: number;
}

export const SHAPES = ["rounded", "square", "soft", "pill", "cut"] as const;
export const TEXTURES = ["none", "glass", "gradient", "dots", "grid", "lines", "noise"] as const;
export type Shape = (typeof SHAPES)[number];
export type Texture = (typeof TEXTURES)[number];

export const COLOR_PRESETS = ["#ffffff", "#18181b", "#1e3a8a", "#0f766e", "#15803d", "#a16207", "#b91c1c", "#7e22ce", "#db2777", "#f5e6c8"];

export function appearanceOf(config: Record<string, unknown>): Appearance | null {
  const a = config.appearance;
  return a && typeof a === "object" ? (a as Appearance) : null;
}

/** Relative luminance of a `#rrggbb` color, 0..1. */
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/**
 * Attributes for a widget card (`.widget-look` in index.css). Without a saved
 * appearance the card keeps its default look.
 */
export function appearanceProps(config: Record<string, unknown>): { className: string; style?: CSSProperties; "data-shape"?: string; "data-texture"?: string; "data-ink"?: string } {
  const a = appearanceOf(config);
  if (!a) return { className: "" };
  const style: Record<string, string | number> = { "--wa-opacity": `${Math.round((a.opacity ?? 0.85) * 100)}%` };
  let ink: string | undefined;
  if (a.color) {
    style["--wa-color"] = a.color;
    ink = luminance(a.color) > 0.4 ? "dark" : "light";
  }
  return { className: "widget-look", style: style as CSSProperties, "data-shape": a.shape ?? "rounded", "data-texture": a.texture ?? "none", "data-ink": ink };
}
