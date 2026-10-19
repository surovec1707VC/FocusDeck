import { FormEvent, useState } from "react";
import { Check, Pencil, Plus, Trash2 } from "lucide-react";
import { Badge, ErrorBanner, PageHeader } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { api, Layout } from "../lib/api";
import { attempt, useAsync } from "../lib/use-async";

export default function Layouts() {
  const { data, setData, error, setError } = useAsync(api.layoutsList);
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);
  const L = t.layouts;

  const run = async (fn: () => Promise<Layout[]>) => {
    const result = await attempt(fn, setError);
    if (result) setData(result);
    return result;
  };

  async function create(e: FormEvent) {
    e.preventDefault();
    if (await run(() => api.layoutsCreate(name))) setName("");
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

      <form onSubmit={create} className="card mb-4 flex flex-wrap items-center gap-2">
        <input className="input max-w-sm flex-1" placeholder={L.namePlaceholder} value={name} maxLength={60} onChange={(e) => setName(e.target.value)} />
        <button className="btn-primary" disabled={!name.trim()}>
          <Plus className="size-4" />
          {L.create}
        </button>
      </form>

      <ul className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {data?.map((l) => (
          <li key={l.id} className={`card flex flex-col gap-3 ${l.is_active ? "border-accent-500/60 dark:border-accent-500/60" : ""}`}>
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
              <div className="flex items-center gap-2">
                <h2 className="truncate text-base font-semibold">{l.name}</h2>
                {l.is_active && <Badge tone="accent">{L.active}</Badge>}
              </div>
            )}
            <div className="text-xs text-zinc-500">{formatDateTime(l.updated_at)}</div>
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
              <button
                className="btn-danger"
                disabled={single}
                title={single ? L.lastLayout : undefined}
                onClick={() => confirm(t.common.confirmDelete) && run(() => api.layoutsDelete(l.id))}
              >
                <Trash2 className="size-4" />
                {t.common.delete}
              </button>
            </div>
          </li>
        ))}
      </ul>
    </>
  );
}
