import { ReactNode, useEffect, useRef } from "react";
import { X } from "lucide-react";

interface Props {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  /** Buttons in the footer; omit for none. */
  footer?: ReactNode;
  size?: "sm" | "md" | "lg";
}

/** Generic modal on <dialog>: focus trap, Esc and backdrop come from the browser.
 *  Mark the element to focus first with `data-autofocus`. */
export default function Modal({ open, title, onClose, children, footer, size = "md" }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) {
      d.showModal();
      // React handles autoFocus itself, so <dialog> would focus its first button: move focus explicitly.
      d.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    }
    if (!open && d.open) d.close();
  }, [open]);
  const width = { sm: "28rem", md: "36rem", lg: "48rem" }[size];

  return (
    <dialog
      ref={ref}
      aria-labelledby="modal-title"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onClick={(e) => e.target === ref.current && onClose()}
      style={{ width: `min(${width}, calc(100vw - 2rem))` }}
      className="m-auto max-h-[calc(100vh-2rem)] rounded-xl border border-zinc-200 bg-white p-0 text-zinc-900 shadow-2xl backdrop:bg-black/50 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-100"
    >
      {open && (
        <div className="flex max-h-[calc(100vh-2rem)] flex-col">
          <header className="flex items-center gap-3 border-b border-zinc-200 px-5 py-3 dark:border-zinc-800">
            <h2 id="modal-title" className="flex-1 text-base font-semibold">
              {title}
            </h2>
            <button type="button" className="icon-btn" aria-label="Закрыть" onClick={onClose}>
              <X className="size-4" />
            </button>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-5">{children}</div>
          {footer && <footer className="flex justify-end gap-2 border-t border-zinc-200 px-5 py-3 dark:border-zinc-800">{footer}</footer>}
        </div>
      )}
    </dialog>
  );
}
