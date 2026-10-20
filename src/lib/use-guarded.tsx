import { useState } from "react";
import ConfirmDialog from "../components/ConfirmDialog";
import { t } from "../i18n";
import { ApiError, errorText } from "./api";

/**
 * Runs guarded backend actions. When the backend answers
 * `confirmation_required` (dangerous action or Safe Mode), the user sees the
 * backend's message and the call is repeated with `confirm = true`.
 */
export function useGuarded(onError: (m: string | null) => void) {
  const [pending, setPending] = useState<{ message: string; retry: () => void } | null>(null);

  async function run<T>(fn: (confirm: boolean) => Promise<T>, onOk?: (r: T) => void) {
    try {
      const r = await fn(false);
      onError(null);
      onOk?.(r);
    } catch (e) {
      if (e instanceof ApiError && e.kind === "confirmation_required") {
        setPending({
          message: e.message,
          retry: async () => {
            try {
              const r = await fn(true);
              onError(null);
              onOk?.(r);
            } catch (err) {
              onError(errorText(err));
            }
          },
        });
      } else onError(errorText(e));
    }
  }

  const dialog = (
    <ConfirmDialog
      open={pending !== null}
      title={t.windows.confirmTitle}
      confirmLabel={t.windows.confirm}
      danger
      onCancel={() => setPending(null)}
      onConfirm={() => {
        const p = pending!;
        setPending(null);
        p.retry();
      }}
    >
      {pending?.message}
    </ConfirmDialog>
  );

  return { run, dialog };
}
