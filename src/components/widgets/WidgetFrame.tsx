import { HTMLAttributes, KeyboardEvent, PointerEvent } from "react";
import { ChevronDown, ChevronUp, EyeOff, GripVertical, MonitorUp, Pin, PinOff, Settings2, Trash2 } from "lucide-react";
import { t } from "../../i18n";
import { Widget } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import { WIDGET_UI } from "./registry";

export interface FrameActions {
  onPin: () => void;
  onCollapse: () => void;
  onHide: () => void;
  onRemove: () => void;
  onSettings: () => void;
  /** Detach into a desktop window; absent when the Desktop widgets module is off. */
  onDesktop?: () => void;
}

interface Props extends FrameActions {
  widget: Widget;
  /** Drag handle wiring from the grid; undefined in list mode. */
  dragHandle?: {
    onPointerDown: (e: PointerEvent<HTMLElement>) => void;
    onKeyDown: (e: KeyboardEvent<HTMLElement>) => void;
  };
  resizeHandle?: HTMLAttributes<HTMLDivElement>;
  dragging?: boolean;
}

/** A widget card: title bar with controls, the body, and a resize corner. */
export default function WidgetFrame({ widget, dragHandle, resizeHandle, dragging, onPin, onCollapse, onHide, onRemove, onSettings, onDesktop }: Props) {
  const showTitles = useModules().settingsOf("widgets").show_titles !== false;
  const W = t.widgets;
  const { icon: Icon, Body } = WIDGET_UI[widget.kind] ?? WIDGET_UI.status;
  const button = "rounded p-1 text-zinc-400 transition hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200";

  return (
    <article
      className={`card group/frame flex h-full flex-col overflow-hidden p-0 ${dragging ? "shadow-2xl ring-2 ring-accent-500/60" : ""} ${
        widget.pinned ? "border-accent-500/40 dark:border-accent-500/40" : ""
      }`}
      aria-label={widget.title}
    >
      <header className="flex shrink-0 items-center gap-1 px-2 pt-1.5 pb-1">
        {dragHandle && !widget.pinned ? (
          <button
            type="button"
            className={`${button} cursor-grab touch-none active:cursor-grabbing`}
            title={W.drag}
            aria-label={`${W.drag}: ${widget.title}`}
            onPointerDown={dragHandle.onPointerDown}
            onKeyDown={dragHandle.onKeyDown}
          >
            <GripVertical className="size-3.5" />
          </button>
        ) : (
          <Icon className="mx-1 size-3.5 shrink-0 text-accent-500" />
        )}
        <h2 className={`min-w-0 flex-1 truncate text-sm font-semibold ${showTitles || widget.collapsed ? "" : "sr-only"}`}>{widget.title}</h2>
        {!showTitles && !widget.collapsed && <div className="flex-1" />}
        <div className="flex opacity-60 transition group-hover/frame:opacity-100 focus-within:opacity-100">
          <button type="button" className={button} onClick={onSettings} title={W.settings} aria-label={W.settings}>
            <Settings2 className="size-3.5" />
          </button>
          <button type="button" className={button} onClick={onPin} title={widget.pinned ? W.unpin : W.pin} aria-label={widget.pinned ? W.unpin : W.pin} aria-pressed={widget.pinned}>
            {widget.pinned ? <PinOff className="size-3.5 text-accent-500" /> : <Pin className="size-3.5" />}
          </button>
          <button
            type="button"
            className={button}
            onClick={onCollapse}
            title={widget.collapsed ? W.expand : W.collapse}
            aria-label={widget.collapsed ? W.expand : W.collapse}
            aria-expanded={!widget.collapsed}
          >
            {widget.collapsed ? <ChevronDown className="size-3.5" /> : <ChevronUp className="size-3.5" />}
          </button>
          {onDesktop && (
            <button type="button" className={button} onClick={onDesktop} title={W.toDesktop} aria-label={W.toDesktop}>
              <MonitorUp className="size-3.5" />
            </button>
          )}
          <button type="button" className={button} onClick={onHide} title={W.hide} aria-label={W.hide}>
            <EyeOff className="size-3.5" />
          </button>
          <button type="button" className={`${button} hover:text-red-500 dark:hover:text-red-400`} onClick={onRemove} title={W.remove} aria-label={W.remove}>
            <Trash2 className="size-3.5" />
          </button>
        </div>
      </header>
      {!widget.collapsed && (
        <div className="min-h-0 flex-1 overflow-auto px-3 pb-3">
          <Body widget={widget} />
        </div>
      )}
      {resizeHandle && !widget.collapsed && !widget.pinned && (
        <div
          {...resizeHandle}
          className="absolute right-0 bottom-0 size-4 cursor-se-resize touch-none opacity-0 transition group-hover/frame:opacity-100"
          title={W.resize}
        >
          <svg viewBox="0 0 10 10" className="size-full text-zinc-400" aria-hidden>
            <path d="M9 3 3 9M9 6 6 9" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </div>
      )}
    </article>
  );
}
