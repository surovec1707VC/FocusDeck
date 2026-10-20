/** Grid geometry for the dashboard. Positions and sizes are in cells. */

export interface Box {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Pinned boxes never move. */
  pinned?: boolean;
}

export const MAX_HEIGHT = 24;

export const overlaps = (a: Box, b: Box) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** Keeps a box inside the columns. */
export function clampBox(b: Box, cols: number): Box {
  const w = Math.max(1, Math.min(b.w, cols));
  return { ...b, w, x: Math.max(0, Math.min(b.x, cols - w)), y: Math.max(0, b.y) };
}

/**
 * Resolves collisions: pinned boxes stay, the `moving` box keeps its spot
 * (pushed below pinned ones if needed), the rest are pushed down — or, with
 * `compact`, pulled up as far as they fit. Returns boxes in the input order.
 */
export function arrange(boxes: Box[], cols: number, compact: boolean, movingId?: string): Box[] {
  const placed: Box[] = boxes.filter((b) => b.pinned).map((b) => clampBox(b, cols));
  const moving = boxes.find((b) => b.id === movingId && !b.pinned);
  if (moving) {
    const m = clampBox(moving, cols);
    while (placed.some((p) => overlaps(p, m))) m.y++;
    placed.push(m);
  }
  const rest = boxes
    .filter((b) => !b.pinned && b.id !== moving?.id)
    .map((b) => clampBox(b, cols))
    .sort((a, b) => a.y - b.y || a.x - b.x);
  for (const b of rest) {
    const c = { ...b, y: compact ? 0 : b.y };
    while (placed.some((p) => overlaps(p, c))) c.y++;
    placed.push(c);
  }
  const byId = new Map(placed.map((p) => [p.id, p]));
  return boxes.map((b) => byId.get(b.id)!);
}

/** Boxes whose position or size differs. */
export function changed(before: Box[], after: Box[]): Box[] {
  const prev = new Map(before.map((b) => [b.id, b]));
  return after.filter((b) => {
    const p = prev.get(b.id);
    return !p || p.x !== b.x || p.y !== b.y || p.w !== b.w || p.h !== b.h;
  });
}

export const rows = (boxes: Box[]) => boxes.reduce((max, b) => Math.max(max, b.y + b.h), 0);
