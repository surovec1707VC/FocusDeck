import { useState } from "react";
import ConfirmDialog from "../components/ConfirmDialog";
import { t } from "../i18n";
import { api, ApiError, errorText, ModuleInfo, ToggleResult } from "./api";

/**
 * Enables/disables modules. Critical modules ask for confirmation first
 * (the backend answers `confirmation_required` otherwise).
 */
export function useModuleToggle(onDone: (result: ToggleResult) => void, onError: (msg: string | null) => void) {
  const [pending, setPending] = useState<ModuleInfo | null>(null);

  async function run(m: ModuleInfo, confirm: boolean) {
    try {
      const result = m.enabled ? await api.disableModule(m.id, confirm) : await api.enableModule(m.id);
      onError(result.warnings.length ? result.warnings.join("; ") : null);
      onDone(result);
    } catch (e) {
      if (e instanceof ApiError && e.kind === "confirmation_required") setPending(m);
      else onError(errorText(e));
    }
  }

  const toggle = (m: ModuleInfo) => (m.enabled && m.critical ? setPending(m) : run(m, false));

  const dialog = (
    <ConfirmDialog
      open={pending !== null}
      title={pending ? t.controlCenter.confirmDisableTitle(pending.name) : ""}
      confirmLabel={t.controlCenter.disableAnyway}
      danger
      onCancel={() => setPending(null)}
      onConfirm={() => {
        const m = pending!;
        setPending(null);
        run(m, true);
      }}
    >
      <p>{pending?.details}</p>
      <p className="mt-2">{t.controlCenter.confirmDisableBody}</p>
    </ConfirmDialog>
  );

  return { toggle, dialog };
}
