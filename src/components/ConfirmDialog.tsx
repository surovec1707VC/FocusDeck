import { ReactNode, useEffect, useRef } from "react";
import { AlertTriangle } from "lucide-react";
import { t } from "../i18n";

interface Props {
  open: boolean;
  title: string;
  children: ReactNode;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Modal confirmation built on <dialog> (focus trap and Esc come for free). */
export default function ConfirmDialog({ open, title, children, confirmLabel, danger, onConfirm, onCancel }: Props) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
      className="m-auto w-[min(28rem,calc(100vw-2rem))] rounded-xl border border-zinc-200 bg-white p-0 text-zinc-900 shadow-2xl backdrop:bg-black/50 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-100"
    >
      <div className="flex gap-3 p-5">
        <AlertTriangle className={`mt-0.5 size-5 shrink-0 ${danger ? "text-red-500" : "text-amber-500"}`} />
        <div className="min-w-0">
          <h2 className="text-base font-semibold">{title}</h2>
          <div className="mt-2 text-zinc-600 dark:text-zinc-300">{children}</div>
        </div>
      </div>
      <div className="flex justify-end gap-2 border-t border-zinc-200 px-5 py-3 dark:border-zinc-800">
        <button className="btn-secondary" onClick={onCancel} autoFocus>
          {t.common.cancel}
        </button>
        <button className={danger ? "btn bg-red-600 text-white hover:bg-red-500" : "btn-primary"} onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
    </dialog>
  );
}
