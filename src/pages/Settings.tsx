import { FormEvent, useEffect, useState } from "react";
import { ErrorBanner, Field, PageHeader, Row, Toggle } from "../components/ui";
import { t } from "../i18n";
import { AppSettings, Theme } from "../lib/api";
import { useSettings } from "../lib/settings-context";
import { attempt } from "../lib/use-async";

const THEMES: Theme[] = ["dark", "light", "system"];

export default function Settings() {
  const { settings, save } = useSettings();
  const [draft, setDraft] = useState<AppSettings | null>(settings);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const S = t.settings;

  useEffect(() => setDraft(settings), [settings]);
  if (!draft || !settings) return null;

  const set = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    setSaved(false);
    setDraft({ ...draft, [key]: value });
  };
  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (await attempt(() => save(draft!), setError)) setSaved(true);
  }

  return (
    <form onSubmit={submit}>
      <PageHeader
        title={S.title}
        subtitle={S.subtitle}
        actions={
          <>
            {dirty && (
              <button type="button" className="btn-ghost" onClick={() => setDraft(settings)}>
                {t.common.cancel}
              </button>
            )}
            <button className="btn-primary" disabled={!dirty}>
              {t.common.save}
            </button>
          </>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      {dirty && <p className="mb-3 text-xs text-amber-600 dark:text-amber-400">{S.unsaved}</p>}
      {saved && !dirty && <p className="mb-3 text-xs text-emerald-600 dark:text-emerald-400">{t.common.saved}</p>}

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <section className="card">
          <h2 className="mb-1 font-semibold">{S.appearance}</h2>
          <div className="py-3">
            <div className="mb-2 font-medium">{S.theme}</div>
            <div role="radiogroup" aria-label={S.theme} className="inline-flex rounded-lg border border-zinc-300 p-0.5 dark:border-zinc-700">
              {THEMES.map((theme) => (
                <button
                  key={theme}
                  type="button"
                  role="radio"
                  aria-checked={draft.theme === theme}
                  onClick={() => set("theme", theme)}
                  className={`rounded-md px-3 py-1 transition ${
                    draft.theme === theme ? "bg-accent-600 text-white" : "text-zinc-600 hover:bg-zinc-200 dark:text-zinc-400 dark:hover:bg-zinc-800"
                  }`}
                >
                  {S.themes[theme]}
                </button>
              ))}
            </div>
          </div>
          <div className="max-w-xs py-3">
            <Field label={S.language}>
              <select className="input" value="ru" disabled>
                <option value="ru">{S.russian}</option>
              </select>
            </Field>
          </div>
        </section>

        <section className="card divide-y divide-zinc-200 dark:divide-zinc-800">
          <h2 className="pb-1 font-semibold">{S.window}</h2>
          <Row title={S.closeToTray} hint={S.closeToTrayHint}>
            <Toggle checked={draft.close_to_tray} onChange={(v) => set("close_to_tray", v)} label={S.closeToTray} />
          </Row>
          <Row title={S.startMinimized}>
            <Toggle checked={draft.start_minimized} onChange={(v) => set("start_minimized", v)} label={S.startMinimized} />
          </Row>
        </section>

        <section className="card divide-y divide-zinc-200 xl:col-span-2 dark:divide-zinc-800">
          <h2 className="pb-1 font-semibold">{S.system}</h2>
          <Row title={S.autostart} hint={S.autostartHint}>
            <Toggle checked={draft.autostart} onChange={(v) => set("autostart", v)} label={S.autostart} />
          </Row>
          <Row title={S.hotkeys} hint={S.hotkeysHint}>
            <Toggle checked={draft.global_hotkeys_enabled} onChange={(v) => set("global_hotkeys_enabled", v)} label={S.hotkeys} />
          </Row>
          <div className="max-w-sm py-3">
            <Field label={S.toggleHotkey}>
              <input
                className="input font-mono"
                value={draft.toggle_window_hotkey}
                disabled={!draft.global_hotkeys_enabled}
                spellCheck={false}
                onChange={(e) => set("toggle_window_hotkey", e.target.value)}
              />
            </Field>
            <p className="mt-1 text-xs text-zinc-500">{S.hotkeyHint}</p>
          </div>
        </section>
      </div>
    </form>
  );
}
