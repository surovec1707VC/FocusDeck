import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { t } from "../i18n";

const win = getCurrentWindow();

/** Custom title bar for the frameless window. Drag to move, double-click to maximize. */
export default function TitleBar({ onMaximizedChange }: { onMaximizedChange: (m: boolean) => void }) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const update = () =>
      win.isMaximized().then((m) => {
        setMaximized(m);
        onMaximizedChange(m);
      });
    update();
    const unlisten = win.onResized(update);
    return () => {
      unlisten.then((f) => f());
    };
  }, [onMaximizedChange]);

  const button = "grid h-full w-11 place-items-center text-zinc-500 transition hover:bg-zinc-200 dark:text-zinc-400 dark:hover:bg-zinc-800";

  return (
    <div data-tauri-drag-region className="flex h-9 shrink-0 items-center border-b border-zinc-200/70 dark:border-zinc-800/70">
      <div data-tauri-drag-region className="flex items-center gap-2 px-3 text-xs font-semibold tracking-wide text-zinc-600 dark:text-zinc-300">
        <span data-tauri-drag-region className="size-2.5 rounded-full bg-accent-500" />
        {t.app.name}
      </div>
      <div data-tauri-drag-region className="h-full flex-1" />
      <div className="flex h-full">
        <button className={button} onClick={() => win.minimize()} aria-label={t.titlebar.minimize} title={t.titlebar.minimize}>
          <Minus className="size-4" />
        </button>
        <button
          className={button}
          onClick={() => win.toggleMaximize()}
          aria-label={maximized ? t.titlebar.restore : t.titlebar.maximize}
          title={maximized ? t.titlebar.restore : t.titlebar.maximize}
        >
          {maximized ? <Copy className="size-3.5" /> : <Square className="size-3.5" />}
        </button>
        {/* Close goes through CloseRequested: hides to tray or quits per settings. */}
        <button
          className={`${button} hover:bg-red-600 hover:text-white dark:hover:bg-red-600 dark:hover:text-white`}
          onClick={() => win.close()}
          aria-label={t.titlebar.close}
          title={t.titlebar.close}
        >
          <X className="size-4" />
        </button>
      </div>
    </div>
  );
}
