import { FormEvent, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { DatabaseBackup, EyeOff, HardDriveDownload, Keyboard, Lock, RotateCcw, ShieldCheck } from "lucide-react";
import HotkeyInput from "../components/HotkeyInput";
import { ErrorBanner, Field, Kbd, PageHeader, Row, Toggle } from "../components/ui";
import { api, DataSection } from "../lib/api";
import { useShell } from "../lib/shell-context";
import { useGuarded } from "../lib/use-guarded";
import { t } from "../i18n";
import { AppSettings, Theme } from "../lib/api";
import { useSettings } from "../lib/settings-context";
import { attempt } from "../lib/use-async";

const THEMES: Theme[] = ["dark", "light", "system", "bento", "glass"];

export default function Settings() {
  const { settings, save } = useSettings();
  const [draft, setDraft] = useState<AppSettings | null>(settings);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const S = t.settings;
  const shell = useShell();
  const guarded = useGuarded(setError);
  const [backupNote, setBackupNote] = useState<string | null>(null);
  const [sections, setSections] = useState<DataSection[]>(["layouts", "notes", "tasks", "calendar", "snippets", "automations"]);
  const [withConfig, setWithConfig] = useState(true);

  async function exportData() {
    try {
      const path = await api.dataExport(sections, withConfig);
      if (path) setBackupNote(S.exportSaved(path));
      setError(null);
    } catch (e) {
      setError(String((e as Error).message ?? e));
    }
  }

  async function importData() {
    await guarded.run(
      (confirm) => api.dataImport(confirm),
      (r) => {
        if (!r) return;
        const n = Object.values(r.imported).reduce((a, b) => a + b, 0);
        const skipped = Object.values(r.skipped).reduce((a, b) => a + b, 0);
        setBackupNote(S.importDone(n, skipped, r.config_applied) + (r.warnings.length ? ` ${r.warnings.join("; ")}` : ""));
      },
    );
  }

  async function backup() {
    try {
      const path = await api.backupCreate();
      if (path) setBackupNote(S.backupSaved(path));
      setError(null);
    } catch (e) {
      setError(String((e as Error).message ?? e));
    }
  }

  async function restore() {
    await guarded.run(
      (confirm) => api.backupRestore(confirm),
      (path) => {
        if (!path) return;
        setBackupNote(S.restoreStaged);
        window.dispatchEvent(new Event("focusdeck:session"));
      },
    );
  }

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
            <div role="radiogroup" aria-label={S.theme} className="inline-flex flex-wrap rounded-lg border border-zinc-300 p-0.5 dark:border-zinc-700">
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
              <select className="input" value="ru" disabled aria-describedby="lang-hint">
                <option value="ru">{S.russian}</option>
              </select>
            </Field>
            <p id="lang-hint" className="mt-1 text-xs text-zinc-500">
              {S.languageHint}
            </p>
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
            <div className="label mb-1">{S.toggleHotkey}</div>
            <HotkeyInput
              label={S.toggleHotkey}
              value={draft.toggle_window_hotkey || null}
              disabled={!draft.global_hotkeys_enabled}
              onChange={(v) => set("toggle_window_hotkey", v ?? "")}
            />
            <p className="mt-1 text-xs text-zinc-500">{S.hotkeyHint}</p>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-3 py-3">
            <div>
              <div className="font-medium">{S.appShortcuts}</div>
              <div className="text-xs text-zinc-500">
                <Kbd>Ctrl+K</Kbd> {S.paletteHint} · <Kbd>F1</Kbd> {S.helpHint}
              </div>
            </div>
            <button type="button" className="btn-secondary" onClick={shell.openShortcuts}>
              <Keyboard className="size-4" /> {S.allShortcuts}
            </button>
          </div>
        </section>

        <section className="card flex flex-col gap-3">
          <h2 className="flex items-center gap-2 font-semibold">
            <DatabaseBackup className="size-4" /> {S.backupTitle}
          </h2>
          <p className="text-zinc-500 dark:text-zinc-400">{S.backupText}</p>
          <div className="flex flex-wrap gap-2">
            <button type="button" className="btn-secondary" onClick={backup}>
              <HardDriveDownload className="size-4" /> {S.backupCreate}
            </button>
            <button type="button" className="btn-secondary" onClick={restore}>
              <RotateCcw className="size-4" /> {S.backupRestore}
            </button>
          </div>
          <fieldset className="flex flex-col gap-2 rounded-lg border border-zinc-200 p-3 dark:border-zinc-800">
            <legend className="label px-1">{S.exportTitle}</legend>
            <div className="flex flex-wrap gap-x-4 gap-y-1">
              <label className="flex items-center gap-1.5">
                <input type="checkbox" className="accent-accent-600" checked={withConfig} onChange={(e) => setWithConfig(e.target.checked)} />
                {S.sections.config}
              </label>
              {(Object.keys(S.sections) as (DataSection | "config")[])
                .filter((k): k is DataSection => k !== "config")
                .map((k) => (
                  <label key={k} className="flex items-center gap-1.5">
                    <input
                      type="checkbox"
                      className="accent-accent-600"
                      checked={sections.includes(k)}
                      onChange={(e) => setSections(e.target.checked ? [...sections, k] : sections.filter((x) => x !== k))}
                    />
                    {S.sections[k]}
                  </label>
                ))}
            </div>
            <div className="flex flex-wrap gap-2">
              <button type="button" className="btn-secondary" disabled={!withConfig && sections.length === 0} onClick={exportData}>
                {S.exportData}
              </button>
              <button type="button" className="btn-secondary" onClick={importData}>
                {S.importData}
              </button>
            </div>
            <p className="text-xs text-zinc-500">{S.exportHint}</p>
          </fieldset>
          {backupNote && (
            <p role="status" className="text-xs break-all text-emerald-600 select-text dark:text-emerald-400">
              {backupNote}
            </p>
          )}
          <p className="text-xs text-zinc-500">
            {S.configHint}{" "}
            <Link className="text-accent-600 underline dark:text-accent-400" to="/control-center">
              {t.nav.controlCenter}
            </Link>
          </p>
        </section>

        <section className="card flex flex-col gap-3">
          <h2 className="flex items-center gap-2 font-semibold">
            <ShieldCheck className="size-4 text-emerald-500" /> {S.privacyTitle}
          </h2>
          <ul className="flex flex-col gap-2">
            {S.privacyItems.map((line) => (
              <li key={line} className="flex gap-2 text-zinc-600 dark:text-zinc-300">
                <Lock className="mt-0.5 size-3.5 shrink-0 text-zinc-400" aria-hidden /> {line}
              </li>
            ))}
          </ul>
          <div className="flex flex-wrap gap-2">
            <Link to="/permissions" className="btn-secondary">
              <ShieldCheck className="size-4" /> {S.privacyPermissions}
            </Link>
            <Link to="/integrations" className="btn-secondary">
              <EyeOff className="size-4" /> {S.privacyOffline}
            </Link>
          </div>
        </section>
      </div>
      {guarded.dialog}
    </form>
  );
}
