import { useState } from "react";
import { CheckCircle2, ShieldCheck, Sparkles } from "lucide-react";
import Modal from "../Modal";
import { Kbd, Toggle } from "../ui";
import { t } from "../../i18n";
import { api, Theme } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import { useSettings } from "../../lib/settings-context";

const O = t.shell.onboarding;
const STEPS = ["welcome", "theme", "modules", "keys", "done"] as const;
const THEMES: Theme[] = ["dark", "light", "system"];
/** Modules worth deciding on the first day; everything else lives in the Control Center. */
const STARTER = ["widgets", "notes", "tasks", "timers", "clipboard_history", "automations", "integrations"];

/** First-run tour. Every choice here can be changed later. */
export default function Onboarding({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [step, setStep] = useState(0);
  const { settings, save } = useSettings();
  const { modules, reload } = useModules();
  const [error, setError] = useState<string | null>(null);
  const key = STEPS[step];
  const last = step === STEPS.length - 1;

  const finish = () => {
    setStep(0);
    onClose();
  };

  async function toggleModule(id: string, on: boolean) {
    try {
      await (on ? api.enableModule(id) : api.disableModule(id));
      setError(null);
      await reload();
    } catch (e) {
      setError(String((e as Error).message ?? e));
    }
  }

  const title = { welcome: O.welcomeTitle, theme: O.themeTitle, modules: O.modulesTitle, keys: O.keysTitle, done: O.doneTitle }[key];

  return (
    <Modal
      open={open}
      title={title}
      onClose={finish}
      footer={
        <div className="flex w-full items-center gap-2">
          <span className="flex-1 text-xs text-zinc-500" aria-live="polite">
            {O.step(step + 1, STEPS.length)}
          </span>
          {!last && (
            <button className="btn-ghost" onClick={finish}>
              {O.skip}
            </button>
          )}
          {step > 0 && (
            <button className="btn-secondary" onClick={() => setStep(step - 1)}>
              {O.back}
            </button>
          )}
          <button className="btn-primary" data-autofocus onClick={() => (last ? finish() : setStep(step + 1))}>
            {last ? O.finish : O.next}
          </button>
        </div>
      }
    >
      <div className="mb-4 flex gap-1" aria-hidden>
        {STEPS.map((s, i) => (
          <span key={s} className={`h-1 flex-1 rounded-full ${i <= step ? "bg-accent-500" : "bg-zinc-200 dark:bg-zinc-800"}`} />
        ))}
      </div>

      {key === "welcome" && (
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-3">
            <div className="grid size-12 place-items-center rounded-xl bg-accent-600 text-xl font-bold text-white">F</div>
            <p className="text-zinc-600 dark:text-zinc-300">{O.welcome}</p>
          </div>
          <ul className="flex flex-col gap-2">
            {O.privacy.map((line) => (
              <li key={line} className="flex gap-2">
                <ShieldCheck className="mt-0.5 size-4 shrink-0 text-emerald-500" aria-hidden /> {line}
              </li>
            ))}
          </ul>
        </div>
      )}

      {key === "theme" && settings && (
        <div className="flex flex-col gap-3">
          <p className="text-zinc-600 dark:text-zinc-300">{O.themeText}</p>
          <div role="radiogroup" aria-label={t.settings.theme} className="grid grid-cols-3 gap-2">
            {THEMES.map((theme) => (
              <button
                key={theme}
                role="radio"
                aria-checked={settings.theme === theme}
                onClick={() => save({ ...settings, theme }).catch((e) => setError(String(e)))}
                className={`rounded-xl border p-3 text-left transition ${settings.theme === theme ? "border-accent-500 ring-2 ring-accent-500/30" : "border-zinc-300 hover:border-zinc-400 dark:border-zinc-700"}`}
              >
                <span className={`mb-2 block h-10 rounded-lg ${theme === "dark" ? "bg-zinc-900" : theme === "light" ? "border border-zinc-200 bg-zinc-50" : "bg-gradient-to-r from-zinc-50 to-zinc-900"}`} />
                {t.settings.themes[theme]}
              </button>
            ))}
          </div>
        </div>
      )}

      {key === "modules" && (
        <div className="flex flex-col gap-1">
          <p className="mb-2 text-zinc-600 dark:text-zinc-300">{O.modulesText}</p>
          {error && <p className="text-xs text-red-600">{error}</p>}
          {modules
            ?.filter((m) => STARTER.includes(m.id))
            .map((m) => (
              <div key={m.id} className="flex items-center gap-3 border-b border-zinc-200/70 py-2 last:border-0 dark:border-zinc-800/70">
                <div className="min-w-0 flex-1">
                  <div className="font-medium">{m.name}</div>
                  <div className="text-xs text-zinc-500">{m.description}</div>
                </div>
                <Toggle checked={m.enabled} onChange={(v) => toggleModule(m.id, v)} label={m.name} />
              </div>
            ))}
        </div>
      )}

      {key === "keys" && (
        <div className="flex flex-col gap-3">
          <p className="text-zinc-600 dark:text-zinc-300">{O.keysText}</p>
          <ul className="flex flex-col gap-2">
            {t.shell.shortcuts.items.slice(0, 5).map(([k, what]) => (
              <li key={k} className="flex items-center gap-3">
                <Kbd>{k}</Kbd>
                <span>{what}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {key === "done" && (
        <div className="flex flex-col items-center gap-3 py-4 text-center">
          <span className="relative">
            <CheckCircle2 className="size-12 text-emerald-500" aria-hidden />
            <Sparkles className="absolute -top-1 -right-3 size-5 text-amber-400" aria-hidden />
          </span>
          <p className="max-w-sm text-zinc-600 dark:text-zinc-300">{O.doneText}</p>
        </div>
      )}
    </Modal>
  );
}
