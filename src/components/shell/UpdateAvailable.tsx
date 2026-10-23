import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Download } from "lucide-react";
import Markdown from "../Markdown";
import { api, EVENTS, UpdateInfo } from "../../lib/api";

type Available = NonNullable<UpdateInfo["available"]>;

/** Dialog shown when the startup check finds a newer release on GitHub. */
export default function UpdateAvailable() {
  const ref = useRef<HTMLDialogElement>(null);
  const [update, setUpdate] = useState<Available | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<Available>(EVENTS.updateAvailable, (e) => setUpdate(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (update && !d.open) d.showModal();
    if (!update && d.open) d.close();
  }, [update]);

  const install = async () => {
    setBusy(true);
    setError(null);
    try {
      // Clicking «Обновить» is the confirmation; the app restarts when done.
      await api.updatesInstall(true);
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <dialog
      ref={ref}
      onCancel={(e) => {
        e.preventDefault();
        if (!busy) setUpdate(null);
      }}
      className="m-auto w-[min(32rem,calc(100vw-2rem))] rounded-xl border border-zinc-200 bg-white p-0 text-zinc-900 shadow-2xl backdrop:bg-black/50 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-100"
    >
      {update && (
        <div className="flex gap-3 p-5">
          <Download className="mt-0.5 size-5 shrink-0 text-accent-600" />
          <div className="min-w-0 flex-1">
            <h2 className="text-base font-semibold">Доступно обновление FocusDeck {update.version}</h2>
            {update.notes && (
              <div className="mt-2 max-h-60 overflow-y-auto text-sm text-zinc-600 dark:text-zinc-300">
                <Markdown source={update.notes} />
              </div>
            )}
            <p className="mt-2 text-xs text-zinc-500">Перед установкой база данных будет скопирована. Приложение перезапустится.</p>
            {error && <p className="mt-2 text-sm text-red-600">{error}</p>}
            <div className="mt-4 flex justify-end gap-2">
              <button className="btn-secondary" disabled={busy} onClick={() => setUpdate(null)}>
                Позже
              </button>
              <button className="btn-primary" disabled={busy} onClick={install}>
                {busy ? "Загрузка…" : "Обновить"}
              </button>
            </div>
          </div>
        </div>
      )}
    </dialog>
  );
}
