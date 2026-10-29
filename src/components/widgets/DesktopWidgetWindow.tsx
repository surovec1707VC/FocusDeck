import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize, PhysicalSize } from "@tauri-apps/api/window";
import { MonitorDown } from "lucide-react";
import { t } from "../../i18n";
import { api, errorText, EVENTS, Widget } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import { appearanceProps } from "../../lib/widget-appearance";
import { MediaCompact } from "./MediaWidget";
import { WIDGET_UI } from "./registry";

const COMPACT = { width: 260, height: 60 };
const MIN = 160;

/**
 * Compact mode of the music widget: when focus goes to another window the
 * widget shrinks to a small always-on-top pill; clicking it focuses the
 * window and restores the full size.
 */
function useCompact(enabled: boolean) {
  const [compact, setCompact] = useState(false);
  const full = useRef<PhysicalSize | null>(null);
  const target = useRef(false);

  useEffect(() => {
    if (!enabled) return;
    const win = getCurrentWindow();
    // Serialized so fast focus changes end in the latest state.
    let chain = Promise.resolve();
    const apply = (want: boolean) => {
      target.current = want;
      chain = chain.then(async () => {
        if (target.current !== want) return;
        if (want) {
          const size = await win.innerSize();
          if (size.height > COMPACT.height * 2) full.current = size;
          await win.setMinSize(null);
          await win.setSize(new LogicalSize(COMPACT.width, COMPACT.height));
          await win.setAlwaysOnBottom(false);
          await win.setAlwaysOnTop(true);
        } else {
          if (full.current) await win.setSize(full.current);
          await win.setMinSize(new LogicalSize(MIN, MIN));
        }
        setCompact(want);
      }).catch(() => {});
    };
    if (!document.hasFocus()) apply(true);
    const unlisten = win.onFocusChanged(({ payload: focused }) => apply(!focused));
    return () => {
      unlisten.then((f) => f());
      if (target.current) {
        target.current = false;
        if (full.current) win.setSize(full.current).catch(() => {});
        win.setMinSize(new LogicalSize(MIN, MIN)).catch(() => {});
      }
    };
  }, [enabled]);

  return enabled && compact;
}

/**
 * Root of a desktop widget window (label `dw-<widget id>`): the widget body on
 * its own frameless, transparent surface. The title bar drags the window, the
 * corner resizes it; the backend saves the geometry.
 */
export default function DesktopWidgetWindow({ widgetId }: { widgetId: string }) {
  const [widget, setWidget] = useState<Widget | null>(null);
  const [error, setError] = useState<string | null>(null);
  const modules = useModules();
  const showTitles = modules.settingsOf("widgets").show_titles !== false;

  const load = useCallback(() => {
    api
      .desktopWidgetGet(widgetId)
      .then((w) => {
        setWidget(w);
        setError(null);
      })
      .catch((e) => setError(errorText(e)));
  }, [widgetId]);

  // Settings may have been changed in the main window meanwhile.
  useEffect(() => {
    load();
    const onFocus = () => load();
    window.addEventListener("focus", onFocus);
    const unlisten = listen(EVENTS.modulesChanged, load);
    return () => {
      window.removeEventListener("focus", onFocus);
      unlisten.then((f) => f());
    };
  }, [load]);

  const compact = useCompact(widget?.kind === "media" && modules.settingsOf("widget_media").auto_compact !== false);
  const returnToApp = () => api.desktopWidgetAttach(widgetId).catch((e) => setError(errorText(e)));
  const W = t.widgets;
  const { icon: Icon, Body } = (widget && WIDGET_UI[widget.kind]) || WIDGET_UI.status;
  const button = "rounded p-1 text-zinc-400 transition hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200";
  const { className: lookClass, ...look } = appearanceProps(widget?.config ?? {});
  const surface = "border border-zinc-300/70 bg-zinc-100/90 shadow-lg dark:border-zinc-700/70 dark:bg-zinc-900/85";

  if (compact && widget) {
    return (
      <article
        {...look}
        data-shape="pill"
        className={`flex h-full cursor-pointer items-center overflow-hidden rounded-full px-2.5 ${surface} ${lookClass}`}
        aria-label={widget.title}
        title={W.mediaExpand}
      >
        <MediaCompact widget={widget} />
      </article>
    );
  }

  return (
    <article {...look} className={`group/frame relative flex h-full flex-col overflow-hidden rounded-xl ${surface} ${lookClass}`} aria-label={widget?.title}>
      <header data-tauri-drag-region className="flex shrink-0 cursor-default items-center gap-1 px-2 pt-1.5 pb-1">
        <Icon className="pointer-events-none mx-1 size-3.5 shrink-0 text-accent-500" />
        <h1 data-tauri-drag-region className={`min-w-0 flex-1 truncate text-sm font-semibold ${showTitles ? "" : "opacity-0"}`}>
          {widget?.title}
        </h1>
        <button type="button" className={`${button} opacity-0 transition group-hover/frame:opacity-100 focus:opacity-100`} onClick={returnToApp} title={W.toApp} aria-label={W.toApp}>
          <MonitorDown className="size-3.5" />
        </button>
      </header>
      <div className="min-h-0 flex-1 overflow-auto px-3 pb-3">
        {widget ? <Body widget={widget} /> : <p className="text-xs text-zinc-500">{error ?? W.desktopGone}</p>}
      </div>
      <div
        className="absolute right-0 bottom-0 size-4 cursor-se-resize opacity-0 transition group-hover/frame:opacity-100"
        title={W.resize}
        onPointerDown={(e) => {
          if (e.button === 0) getCurrentWindow().startResizeDragging("SouthEast");
        }}
      >
        <svg viewBox="0 0 10 10" className="size-full text-zinc-400" aria-hidden>
          <path d="M9 3 3 9M9 6 6 9" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      </div>
    </article>
  );
}
