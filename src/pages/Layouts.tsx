import { FormEvent, useState } from "react";
import { Briefcase, Check, Code2, Copy, Crosshair, Gamepad2, LayoutTemplate, LucideIcon, Pencil, Plus, Radio, Trash2 } from "lucide-react";
import HotkeyInput from "../components/HotkeyInput";
import ConfirmDialog from "../components/ConfirmDialog";
import { Badge, ErrorBanner, Field, PageHeader } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { api, Layout, LayoutProfile } from "../lib/api";
import { useModules } from "../lib/modules-context";
import { useSettings } from "../lib/settings-context";
import { attempt, useAsync } from "../lib/use-async";

const PROFILES: LayoutProfile[] = ["work", "coding", "streaming", "gaming", "focus", "custom"];
export const PROFILE_ICONS: Record<LayoutProfile, LucideIcon> = {
  work: Briefcase,
  coding: Code2,
  streaming: Radio,
  gaming: Gamepad2,
  focus: Crosshair,
  custom: LayoutTemplate,
};

export default function Layouts() {
  const { data, setData, error, setError } = useAsync(api.layoutsList);
  const [name, setName] = useState("");
  const [profile, setProfile] = useState<LayoutProfile>("custom");
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);
  const [deleting, setDeleting] = useState<Layout | null>(null);
  const { isEnabled } = useModules();
  const { settings } = useSettings();
  const L = t.layouts;
  const hotkeysOn = Boolean(settings?.global_hotkeys_enabled) && isEnabled("hotkeys");

  const run = async (fn: () => Promise<Layout[]>) => {
    const result = await attempt(fn, setError);
    if (result) setData(result);
    return result;
  };

  async function create(e: FormEvent) {
    e.preventDefault();
    if (await run(() => api.layoutsCreate(name, profile))) setName("");
  }

  async function rename(e: FormEvent) {
    e.preventDefault();
    if (editing && (await run(() => api.layoutsRename(editing.id, editing.name)))) setEditing(null);
  }

  const single = (data?.length ?? 0) <= 1;

  return (
    <>
      <PageHeader title={L.title} subtitle={L.subtitle} />
      <ErrorBanner message={error} onClose={() => setError(null)} />

      <form onSubmit={create} className="card mb-4 flex flex-wrap items-end gap-3">
        <Field label={L.newLayout}>
          <input className="input w-56" placeholder={L.namePlaceholder} value={name} maxLength={60} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label={L.profile}>
          <select
            className="input w-48"
            value={profile}
            onChange={(e) => {
              const p = e.target.value as LayoutProfile;
              setProfile(p);
              if (!name.trim() || PROFILES.some((x) => L.profiles[x] === name)) setName(L.profiles[p]);
            }}
          >
            {PROFILES.map((p) => (
              <option key={p} value={p}>
                {L.profiles[p]}
              </option>
            ))}
          </select>
        </Field>
        <button className="btn-primary" disabled={!name.trim()}>
          <Plus className="size-4" />
          {L.create}
        </button>
        <p className="w-full text-xs text-zinc-500">{L.profileHints[profile]}</p>
      </form>

      {!hotkeysOn && <p className="mb-3 text-xs text-amber-600 dark:text-amber-400">{L.hotkeyOff}</p>}

      <ul className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {data?.map((l) => {
          const Icon = PROFILE_ICONS[l.profile] ?? LayoutTemplate;
          return (
            <li key={l.id} className={`card flex flex-col gap-3 ${l.is_active ? "border-accent-500/60 dark:border-accent-500/60" : ""}`}>
              <div className="flex items-start gap-3">
                <div className="rounded-lg bg-accent-500/15 p-2 text-accent-600 dark:text-accent-400">
                  <Icon className="size-5" />
                </div>
                <div className="min-w-0 flex-1">
                  {editing?.id === l.id ? (
                    <form onSubmit={rename} className="flex gap-2">
                      <input
                        className="input"
                        autoFocus
                        maxLength={60}
                        value={editing.name}
                        onChange={(e) => setEditing({ ...editing, name: e.target.value })}
                        onKeyDown={(e) => e.key === "Escape" && setEditing(null)}
                      />
                      <button className="btn-primary" disabled={!editing.name.trim()}>
                        {t.common.save}
                      </button>
                    </form>
                  ) : (
                    <div className="flex flex-wrap items-center gap-2">
                      <h2 className="truncate text-base font-semibold">{l.name}</h2>
                      {l.is_active && <Badge tone="accent">{L.active}</Badge>}
                    </div>
                  )}
                  <div className="mt-0.5 text-xs text-zinc-500">
                    {L.profiles[l.profile]} · {L.widgets(l.widget_count)} · {formatDateTime(l.updated_at)}
                  </div>
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-2 text-xs">
                <span className="text-zinc-500">{L.hotkey}</span>
                <HotkeyInput label={`${L.hotkey}: ${l.name}`} value={l.hotkey} onChange={(h) => run(() => api.layoutsSetHotkey(l.id, h))} />
              </div>

              <div className="flex flex-wrap gap-1">
                {!l.is_active && (
                  <button className="btn-secondary" onClick={() => run(() => api.layoutsActivate(l.id))}>
                    <Check className="size-4" />
                    {L.activate}
                  </button>
                )}
                <button className="btn-ghost" onClick={() => setEditing({ id: l.id, name: l.name })}>
                  <Pencil className="size-4" />
                  {t.common.rename}
                </button>
                <button className="btn-ghost" onClick={() => run(() => api.layoutsDuplicate(l.id))}>
                  <Copy className="size-4" />
                  {L.duplicate}
                </button>
                <button className="btn-danger" disabled={single} title={single ? L.lastLayout : undefined} onClick={() => setDeleting(l)}>
                  <Trash2 className="size-4" />
                  {t.common.delete}
                </button>
              </div>
            </li>
          );
        })}
      </ul>

      <ConfirmDialog
        open={deleting !== null}
        title={t.common.delete}
        confirmLabel={t.common.delete}
        danger
        onCancel={() => setDeleting(null)}
        onConfirm={() => {
          const l = deleting!;
          setDeleting(null);
          run(() => api.layoutsDelete(l.id));
        }}
      >
        {deleting && L.deleteConfirm(deleting.name)}
      </ConfirmDialog>
    </>
  );
}
