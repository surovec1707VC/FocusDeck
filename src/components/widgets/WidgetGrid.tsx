import { KeyboardEvent, PointerEvent, useEffect, useLayoutEffect, useRef, useState } from "react";
import { t } from "../../i18n";
import { GridItem, Widget } from "../../lib/api";
import { arrange, Box, changed, MAX_HEIGHT, rows } from "../../lib/grid";
import WidgetFrame, { FrameActions } from "./WidgetFrame";

interface Props {
  widgets: Widget[];
  /** Minimum sizes by kind (from the catalog). */
  minSize: (w: Widget) => { w: number; h: number };
  cols: number;
  rowHeight: number;
  compact: boolean;
  gap?: number;
  /** Persists moved/resized widgets. */
  onCommit: (items: GridItem[]) => void;
  actions: (w: Widget) => FrameActions;
}

interface Drag {
  id: string;
  mode: "move" | "resize";
  startX: number;
  startY: number;
  orig: Box;
  base: Box[];
}

/** Below this width widgets are shown as a plain list. */
const LIST_BREAKPOINT = 640;

/** Collapsed widgets take one row in the layout; their stored height is kept. */
const toBox = (w: Widget): Box => ({ id: w.id, x: w.x, y: w.y, w: w.w, h: w.collapsed ? 1 : w.h, pinned: w.pinned });

/**
 * Dashboard grid: drag by the grip, resize from the corner, or use the
 * keyboard on the grip (arrows move, Shift+arrows resize). Collisions are
 * resolved locally; only the final positions are sent to the backend.
 */
export default function WidgetGrid({ widgets, minSize, cols, rowHeight, compact, gap = 12, onCommit, actions }: Props) {
  const container = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [boxes, setBoxes] = useState<Box[]>(() => widgets.map(toBox));
  const [drag, setDrag] = useState<Drag | null>(null);
  const dragRef = useRef<Drag | null>(null);
  dragRef.current = drag;

  // New data replaces the local layout (not while dragging). Overlaps that
  // appear on their own (a widget expanded, fewer columns) are resolved and saved.
  const key = widgets.map((w) => `${w.id}:${w.x},${w.y},${w.w},${w.h},${w.pinned ? 1 : 0}${w.collapsed ? 1 : 0}`).join("|");
  useEffect(() => {
    if (dragRef.current) return;
    const base = widgets.map(toBox);
    const fixed = arrange(base, cols, compact);
    setBoxes(fixed);
    commit(base, fixed);
  }, [key, cols, compact]);

  useLayoutEffect(() => {
    const el = container.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    ro.observe(el);
    setWidth(el.getBoundingClientRect().width);
    return () => ro.disconnect();
  }, []);

  const listMode = width > 0 && width < LIST_BREAKPOINT;
  const colW = Math.max(1, (width - gap * (cols - 1)) / cols);
  const stepX = colW + gap;
  const stepY = rowHeight + gap;
  const byId = new Map(widgets.map((w) => [w.id, w]));

  /** Final positions back to stored values (collapsed widgets keep their height). */
  function commit(before: Box[], after: Box[]) {
    const items = changed(before, after).map((b) => {
      const w = byId.get(b.id)!;
      return { id: b.id, x: b.x, y: b.y, w: b.w, h: w.collapsed ? w.h : b.h };
    });
    if (items.length) onCommit(items);
  }

  function propose(d: Drag, dx: number, dy: number): Box[] {
    const w = byId.get(d.id)!;
    const min = minSize(w);
    const next =
      d.mode === "move"
        ? { ...d.orig, x: Math.max(0, Math.min(cols - d.orig.w, d.orig.x + dx)), y: Math.max(0, d.orig.y + dy) }
        : { ...d.orig, w: Math.max(min.w, Math.min(cols - d.orig.x, d.orig.w + dx)), h: Math.max(min.h, Math.min(MAX_HEIGHT, d.orig.h + dy)) };
    return arrange(
      d.base.map((b) => (b.id === d.id ? next : b)),
      cols,
      compact,
      d.id,
    );
  }

  function start(e: PointerEvent<HTMLElement>, id: string, mode: Drag["mode"]) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    const orig = boxes.find((b) => b.id === id)!;
    setDrag({ id, mode, startX: e.clientX, startY: e.clientY, orig, base: boxes });
  }

  function move(e: PointerEvent<HTMLElement>) {
    const d = dragRef.current;
    if (!d) return;
    const dx = Math.round((e.clientX - d.startX) / stepX);
    const dy = Math.round((e.clientY - d.startY) / stepY);
    setBoxes(propose(d, dx, dy));
  }

  function end() {
    const d = dragRef.current;
    if (!d) return;
    setDrag(null);
    // Final pass also compacts the dragged widget.
    const final = arrange(boxes, cols, compact);
    setBoxes(final);
    commit(d.base, final);
  }

  function keyboard(e: KeyboardEvent<HTMLElement>, id: string) {
    const delta: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const step = delta[e.key];
    if (!step) return;
    e.preventDefault();
    const d: Drag = { id, mode: e.shiftKey ? "resize" : "move", startX: 0, startY: 0, orig: boxes.find((b) => b.id === id)!, base: boxes };
    const next = arrange(propose(d, step[0], step[1]), cols, compact, id);
    setBoxes(next);
    commit(boxes, next);
  }

  const height = Math.max(1, rows(boxes)) * stepY - gap;

  if (listMode) {
    return (
      <div ref={container} className="flex flex-col gap-3">
        <p className="text-xs text-zinc-500">{t.dashboard.narrow}</p>
        {[...widgets]
          .sort((a, b) => a.y - b.y || a.x - b.x)
          .map((w) => (
            <div key={w.id} className="relative" style={{ height: w.collapsed ? 40 : Math.max(2, w.h) * rowHeight }}>
              <WidgetFrame widget={w} {...actions(w)} />
            </div>
          ))}
      </div>
    );
  }

  return (
    <div ref={container} className="relative select-none" style={{ height }} onPointerMove={move} onPointerUp={end} onPointerCancel={end}>
      {/* Drop preview: the grid cells under the dragged widget. */}
      {drag &&
        (() => {
          const b = boxes.find((x) => x.id === drag.id)!;
          return (
            <div
              className="absolute rounded-xl border-2 border-dashed border-accent-500/50 bg-accent-500/5"
              style={{ left: b.x * stepX, top: b.y * stepY, width: b.w * stepX - gap, height: b.h * stepY - gap }}
            />
          );
        })()}
      {boxes.map((b) => {
        const w = byId.get(b.id);
        if (!w) return null;
        const isDragged = drag?.id === b.id;
        return (
          <div
            key={b.id}
            className={`absolute ${isDragged ? "z-20" : "z-10 transition-[left,top,width,height] duration-150"}`}
            style={{ left: b.x * stepX, top: b.y * stepY, width: b.w * stepX - gap, height: b.h * stepY - gap }}
          >
            <WidgetFrame
              widget={w}
              dragging={isDragged}
              dragHandle={{ onPointerDown: (e) => start(e, b.id, "move"), onKeyDown: (e) => keyboard(e, b.id) }}
              resizeHandle={{ onPointerDown: (e) => start(e, b.id, "resize") }}
              {...actions(w)}
            />
          </div>
        );
      })}
    </div>
  );
}
