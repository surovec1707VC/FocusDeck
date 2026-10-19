import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Bell, X } from "lucide-react";
import { EVENTS, Notification } from "../lib/api";
import { t } from "../i18n";

interface Toast extends Notification {
  id: number;
}

const LIFETIME_MS = 6000;

/** Shows notifications sent by automations (`focusdeck://notify`). */
export default function Toasts() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  useEffect(() => {
    let next = 0;
    const unlisten = listen<Notification>(EVENTS.notify, (e) => {
      const toast = { ...e.payload, id: next++ };
      setToasts((list) => [...list.slice(-3), toast]);
      setTimeout(() => setToasts((list) => list.filter((x) => x.id !== toast.id)), LIFETIME_MS);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div aria-live="polite" className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2">
      {toasts.map((toast) => (
        <div key={toast.id} className="card pointer-events-auto flex gap-3 bg-white/95 dark:bg-zinc-900/95">
          <Bell className="mt-0.5 size-4 shrink-0 text-accent-500" />
          <div className="min-w-0 flex-1">
            <div className="font-semibold">{toast.title}</div>
            {toast.body && <div className="mt-0.5 text-zinc-600 dark:text-zinc-300">{toast.body}</div>}
            <div className="mt-1 text-xs text-zinc-500">
              {t.toast.from}: {toast.source}
            </div>
          </div>
          <button
            className="self-start text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200"
            onClick={() => setToasts((list) => list.filter((x) => x.id !== toast.id))}
            aria-label="Закрыть уведомление"
          >
            <X className="size-4" />
          </button>
        </div>
      ))}
    </div>
  );
}
