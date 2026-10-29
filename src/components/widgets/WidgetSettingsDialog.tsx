import { FormEvent, useEffect, useRef, useState } from "react";
import { FolderOpen, Plus, X } from "lucide-react";
import SettingsForm from "../SettingsForm";
import { ErrorBanner, Field } from "../ui";
import { t } from "../../i18n";
import { api, errorText, Widget, WidgetCatalogEntry } from "../../lib/api";
import { Appearance, appearanceOf, appearanceProps, COLOR_PRESETS, SHAPES, TEXTURES } from "../../lib/widget-appearance";

interface Props {
  widget: Widget | null;
  entry: WidgetCatalogEntry | undefined;
  onClose: () => void;
  /** Called with the updated widget after a save. */
  onSaved: (w: Widget) => void;
}

type Link = { title: string; url: string };
type App = { name: string; path: string };

/** Settings of one widget instance plus the shared settings of its module. */
export default function WidgetSettingsDialog({ widget, entry, onClose, onSaved }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  const [title, setTitle] = useState("");
  const [links, setLinks] = useState<Link[]>([]);
  const [apps, setApps] = useState<App[]>([]);
  const [duration, setDuration] = useState({ h: 0, m: 5, s: 0 });
  const [label, setLabel] = useState("");
  const [look, setLook] = useState<Appearance | null>(null);
  const [error, setError] = useState<string | null>(null);
  const W = t.widgets;

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (widget && !d.open) d.showModal();
    if (!widget && d.open) d.close();
    if (!widget) return;
    const c = widget.config;
    setTitle(widget.title);
    setLinks(Array.isArray(c.links) ? (c.links as Link[]) : []);
    setApps(Array.isArray(c.apps) ? (c.apps as App[]) : []);
    const ms = Number(c.duration_ms ?? 300_000);
    setDuration({ h: Math.floor(ms / 3_600_000), m: Math.floor((ms % 3_600_000) / 60_000), s: Math.floor((ms % 60_000) / 1000) });
    setLabel(String(c.label ?? ""));
    setLook(appearanceOf(c));
    setError(null);
  }, [widget]);

  async function save(e: FormEvent) {
    e.preventDefault();
    if (!widget) return;
    try {
      const config = { ...(widget.kind === "quick_links" ? { links } : widget.kind === "app_launcher" ? { apps } : {}), appearance: look };
      let updated = await api.widgetsUpdate(widget.id, { title, config });
      if (widget.kind === "countdown") {
        const ms = ((duration.h * 60 + duration.m) * 60 + duration.s) * 1000;
        if (ms !== Number(widget.config.duration_ms) || label !== widget.config.label) {
          await api.widgetAction(widget.id, "set", { duration_ms: ms, label });
          updated = { ...updated, config: { ...updated.config, duration_ms: ms, label } };
        }
      }
      onSaved(updated);
      onClose();
    } catch (err) {
      setError(errorText(err));
    }
  }

  async function pickApp() {
    try {
      const picked = await api.widgetPickApp();
      if (picked) setApps([...apps, picked]);
    } catch (err) {
      setError(errorText(err));
    }
  }

  const num = (v: string, max: number) => Math.max(0, Math.min(max, Number(v) || 0));

  return (
    <dialog
      ref={ref}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      className="m-auto max-h-[85vh] w-[min(34rem,calc(100vw-2rem))] overflow-y-auto rounded-xl border border-zinc-200 bg-white p-0 text-zinc-900 shadow-2xl backdrop:bg-black/50 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-100"
    >
      {widget && (
        <div className="flex flex-col gap-4 p-5">
          <header className="flex items-center justify-between gap-2">
            <h2 className="text-base font-semibold">{W.dialogTitle(widget.title)}</h2>
            <button className="btn-ghost px-1.5" onClick={onClose} aria-label={W.close}>
              <X className="size-4" />
            </button>
          </header>
          <ErrorBanner message={error} onClose={() => setError(null)} />

          <form onSubmit={save} className="flex flex-col gap-3">
            <h3 className="text-xs font-semibold tracking-wide text-zinc-500 uppercase">{W.instanceSettings}</h3>
            <Field label={W.name}>
              <input className="input" value={title} maxLength={60} onChange={(e) => setTitle(e.target.value)} autoFocus />
            </Field>

            {widget.kind === "quick_links" && (
              <div className="flex flex-col gap-2">
                {links.map((l, i) => (
                  <div key={i} className="flex gap-1">
                    <input
                      className="input w-36"
                      placeholder={W.linkTitle}
                      value={l.title}
                      maxLength={60}
                      onChange={(e) => setLinks(links.map((x, j) => (j === i ? { ...x, title: e.target.value } : x)))}
                    />
                    <input
                      className="input min-w-0 flex-1"
                      placeholder={W.linkUrl}
                      value={l.url}
                      maxLength={2048}
                      onChange={(e) => setLinks(links.map((x, j) => (j === i ? { ...x, url: e.target.value } : x)))}
                    />
                    <button type="button" className="btn-ghost px-1.5 hover:text-red-500" onClick={() => setLinks(links.filter((_, j) => j !== i))} aria-label={t.common.delete}>
                      <X className="size-4" />
                    </button>
                  </div>
                ))}
                <button type="button" className="btn-secondary self-start" onClick={() => setLinks([...links, { title: "", url: "https://" }])} disabled={links.length >= 30}>
                  <Plus className="size-4" /> {W.addLink}
                </button>
              </div>
            )}

            {widget.kind === "app_launcher" && (
              <div className="flex flex-col gap-2">
                {apps.map((a, i) => (
                  <div key={i} className="flex items-center gap-1">
                    <input
                      className="input w-40"
                      placeholder={W.appName}
                      value={a.name}
                      maxLength={60}
                      onChange={(e) => setApps(apps.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)))}
                    />
                    <span className="min-w-0 flex-1 truncate text-xs text-zinc-500" title={a.path}>
                      {a.path}
                    </span>
                    <button type="button" className="btn-ghost px-1.5 hover:text-red-500" onClick={() => setApps(apps.filter((_, j) => j !== i))} aria-label={t.common.delete}>
                      <X className="size-4" />
                    </button>
                  </div>
                ))}
                <button type="button" className="btn-secondary self-start" onClick={pickApp} disabled={apps.length >= 30}>
                  <FolderOpen className="size-4" /> {W.addApp}
                </button>
              </div>
            )}

            {widget.kind === "countdown" && (
              <div className="flex flex-wrap items-end gap-2">
                <Field label={W.duration}>
                  <span className="flex items-center gap-1">
                    <input className="input w-16" type="number" min={0} max={99} value={duration.h} onChange={(e) => setDuration({ ...duration, h: num(e.target.value, 99) })} aria-label={W.hours} />
                    {W.hours}
                    <input className="input w-16" type="number" min={0} max={59} value={duration.m} onChange={(e) => setDuration({ ...duration, m: num(e.target.value, 59) })} aria-label={W.minutes} />
                    {W.minutes}
                    <input className="input w-16" type="number" min={0} max={59} value={duration.s} onChange={(e) => setDuration({ ...duration, s: num(e.target.value, 59) })} aria-label={W.seconds} />
                    {W.seconds}
                  </span>
                </Field>
                <Field label={W.label}>
                  <input className="input w-44" value={label} maxLength={60} onChange={(e) => setLabel(e.target.value)} />
                </Field>
              </div>
            )}

            <AppearanceEditor value={look} onChange={setLook} title={title} />

            <div className="flex justify-end gap-2">
              <button type="button" className="btn-secondary" onClick={onClose}>
                {t.common.cancel}
              </button>
              <button className="btn-primary" disabled={!title.trim()}>
                {t.common.save}
              </button>
            </div>
          </form>

          {entry && Object.keys(entry.settings_schema.properties ?? {}).length > 0 && (
            <section className="flex flex-col gap-3 border-t border-zinc-200 pt-4 dark:border-zinc-800">
              <h3 className="text-xs font-semibold tracking-wide text-zinc-500 uppercase">{W.typeSettings}</h3>
              <SettingsForm
                schema={entry.settings_schema}
                value={entry.settings}
                onSave={(s) => api.updateModuleSettings(entry.module_id, s).catch((err) => setError(errorText(err)))}
                onReset={() => api.resetModuleSettings(entry.module_id).catch((err) => setError(errorText(err)))}
              />
            </section>
          )}
        </div>
      )}
    </dialog>
  );
}

/** Shape, color, texture and opacity of the widget card, with a live preview. */
function AppearanceEditor({ value, onChange, title }: { value: Appearance | null; onChange: (a: Appearance | null) => void; title: string }) {
  const W = t.widgets;
  const a: Appearance = value ?? {};
  const set = (patch: Partial<Appearance>) => onChange({ shape: "rounded", texture: "none", opacity: 0.85, ...a, ...patch });
  const chip = (active: boolean) =>
    `rounded-lg border px-2 py-1 text-xs transition ${active ? "border-accent-500 bg-accent-500/10 text-accent-600 dark:text-accent-400" : "border-zinc-300 hover:border-zinc-400 dark:border-zinc-700"}`;
  const { className, ...preview } = appearanceProps({ appearance: value });

  return (
    <fieldset className="flex flex-col gap-3 border-t border-zinc-200 pt-3 dark:border-zinc-800">
      <legend className="sr-only">{W.look}</legend>
      <label className="flex items-center gap-2 text-sm font-medium">
        <input type="checkbox" checked={!!value} onChange={(e) => onChange(e.target.checked ? { shape: "rounded", texture: "none", opacity: 0.85 } : null)} />
        {W.lookCustom}
      </label>
      {value && (
        <>
          <Field label={W.lookShape}>
            <div className="flex flex-wrap gap-1">
              {SHAPES.map((s) => (
                <button key={s} type="button" className={chip((a.shape ?? "rounded") === s)} onClick={() => set({ shape: s })} aria-pressed={(a.shape ?? "rounded") === s}>
                  {W.shapes[s]}
                </button>
              ))}
            </div>
          </Field>
          <Field label={W.lookColor}>
            <div className="flex flex-wrap items-center gap-1.5">
              <button type="button" className={chip(!a.color)} onClick={() => set({ color: null })}>
                {W.lookThemeColor}
              </button>
              {COLOR_PRESETS.map((c) => (
                <button
                  key={c}
                  type="button"
                  className={`size-6 rounded-full border-2 ${a.color === c ? "border-accent-500" : "border-zinc-300 dark:border-zinc-700"}`}
                  style={{ background: c }}
                  onClick={() => set({ color: c })}
                  title={c}
                  aria-label={c}
                />
              ))}
              <input type="color" className="h-6 w-8 cursor-pointer rounded border-0 bg-transparent p-0" value={a.color ?? "#ffffff"} onChange={(e) => set({ color: e.target.value })} title={W.lookCustomColor} aria-label={W.lookCustomColor} />
            </div>
          </Field>
          <Field label={W.lookTexture}>
            <div className="flex flex-wrap gap-1">
              {TEXTURES.map((x) => (
                <button key={x} type="button" className={chip((a.texture ?? "none") === x)} onClick={() => set({ texture: x })} aria-pressed={(a.texture ?? "none") === x}>
                  {W.textures[x]}
                </button>
              ))}
            </div>
          </Field>
          <Field label={`${W.lookOpacity}: ${Math.round((a.opacity ?? 0.85) * 100)}%`}>
            <input type="range" min={20} max={100} value={Math.round((a.opacity ?? 0.85) * 100)} onChange={(e) => set({ opacity: Number(e.target.value) / 100 })} />
          </Field>
          <div className="rounded-lg bg-[repeating-conic-gradient(#a1a1aa33_0_25%,transparent_0_50%)] bg-[length:16px_16px] p-3">
            <div {...preview} className={`card flex h-20 flex-col justify-center p-0 px-4 text-sm ${className}`} aria-label={W.lookPreview}>
              <span className="font-semibold">{title || W.lookPreview}</span>
              <span className="text-xs text-zinc-500">{W.lookPreview}</span>
            </div>
          </div>
        </>
      )}
    </fieldset>
  );
}
