import { FormEvent, useCallback, useEffect, useState } from "react";
import { ClipboardCopy, ClipboardPaste, Download, Pencil, Plus, Search, Trash2, Upload, X } from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import { Badge, Empty, ErrorBanner, Field, PageHeader } from "../components/ui";
import { t } from "../i18n";
import { api, errorText, Snippet, SnippetInput } from "../lib/api";

const EMPTY: SnippetInput = { title: "", body: "", shortcut: "", tags: [] };

export default function Snippets() {
  const S = t.snippets;
  const [items, setItems] = useState<Snippet[]>([]);
  const [search, setSearch] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [editing, setEditing] = useState<{ id?: string; input: SnippetInput; tags: string } | null>(null);
  const [using, setUsing] = useState<{ snippet: Snippet; mode: "copy" | "paste"; values: Record<string, string> } | null>(null);
  const [deleting, setDeleting] = useState<Snippet | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setItems(await api.snippetsList(search || undefined, tag ?? undefined));
    } catch (e) {
      setError(errorText(e));
    }
  }, [search, tag]);

  useEffect(() => {
    const id = setTimeout(load, 120);
    return () => clearTimeout(id);
  }, [load]);

  const flash = (msg: string) => {
    setNotice(msg);
    setTimeout(() => setNotice(null), 3000);
  };

  async function run<T>(fn: () => Promise<T>) {
    try {
      const r = await fn();
      setError(null);
      return r;
    } catch (e) {
      setError(errorText(e));
      return undefined;
    }
  }

  async function save(e: FormEvent) {
    e.preventDefault();
    if (!editing) return;
    const input = { ...editing.input, tags: editing.tags.split(/[,\s]+/).filter(Boolean) };
    if (await run(() => api.snippetsSave(input, editing.id))) {
      setEditing(null);
      load();
    }
  }

  /** Snippets with variables ask for values first. */
  function use(snippet: Snippet, mode: "copy" | "paste") {
    if (snippet.variables.length) setUsing({ snippet, mode, values: Object.fromEntries(snippet.variables.map((v) => [v, ""])) });
    else finish(snippet, mode, {});
  }

  async function finish(snippet: Snippet, mode: "copy" | "paste", values: Record<string, string>) {
    setUsing(null);
    if (mode === "copy") {
      if ((await run(() => api.snippetsCopy(snippet.id, values))) !== undefined) flash(S.copied(snippet.title));
    } else {
      await run(() => api.snippetsPaste(snippet.id, values));
    }
    load();
  }

  async function exportAll() {
    const path = await run(() => api.snippetsExport());
    if (path) flash(S.exported(path));
  }

  async function importFile() {
    const report = await run(() => api.snippetsImport());
    if (report) {
      flash(S.imported(report.imported, report.duplicates));
      if (report.skipped.length) setError(`${S.skipped}: ${report.skipped.join("; ")}`);
      load();
    }
  }

  const allTags = [...new Set(items.flatMap((s) => s.tags))].sort();

  return (
    <>
      <PageHeader
        title={S.title}
        subtitle={S.subtitle}
        actions={
          <>
            <button className="btn-ghost" onClick={importFile}>
              <Upload className="size-4" /> {S.import}
            </button>
            <button className="btn-ghost" onClick={exportAll}>
              <Download className="size-4" /> {S.export}
            </button>
            <button className="btn-primary" onClick={() => setEditing({ input: { ...EMPTY }, tags: tag ?? "" })}>
              <Plus className="size-4" /> {S.create}
            </button>
          </>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      {notice && <div className="mb-3 truncate rounded-lg bg-accent-500/10 px-3 py-2 text-accent-700 dark:text-accent-300">{notice}</div>}

      <div className="mb-3 flex flex-wrap items-center gap-2">
        <label className="relative min-w-52 flex-1 sm:max-w-xs">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400" />
          <input className="input pl-8" placeholder={S.search} value={search} onChange={(e) => setSearch(e.target.value)} aria-label={S.search} />
        </label>
        {allTags.map((tg) => (
          <button
            key={tg}
            onClick={() => setTag(tag === tg ? null : tg)}
            className={`rounded-full px-2 py-0.5 text-xs ${tag === tg ? "bg-accent-600 text-white" : "bg-zinc-200 hover:bg-zinc-300 dark:bg-zinc-800 dark:hover:bg-zinc-700"}`}
          >
            #{tg}
          </button>
        ))}
        {tag && !allTags.includes(tag) && (
          <button className="text-xs text-accent-600" onClick={() => setTag(null)}>
            #{tag} ✕
          </button>
        )}
      </div>

      {editing && (
        <form onSubmit={save} className="card mb-4 grid gap-3 md:grid-cols-3">
          <Field label={S.name}>
            <input className="input" autoFocus value={editing.input.title} maxLength={100} onChange={(e) => setEditing({ ...editing, input: { ...editing.input, title: e.target.value } })} />
          </Field>
          <Field label={S.shortcut}>
            <input className="input" value={editing.input.shortcut ?? ""} maxLength={30} onChange={(e) => setEditing({ ...editing, input: { ...editing.input, shortcut: e.target.value } })} />
          </Field>
          <Field label={S.tags}>
            <input className="input" value={editing.tags} onChange={(e) => setEditing({ ...editing, tags: e.target.value })} />
          </Field>
          <div className="md:col-span-3">
            <Field label={S.body}>
              <textarea className="input min-h-32 font-mono text-sm" value={editing.input.body} onChange={(e) => setEditing({ ...editing, input: { ...editing.input, body: e.target.value } })} />
            </Field>
            <p className="mt-1 text-xs text-zinc-500">{S.variablesHint}</p>
          </div>
          <div className="flex gap-2 md:col-span-3">
            <button className="btn-primary" disabled={!editing.input.title.trim() || !editing.input.body}>
              {t.common.save}
            </button>
            <button type="button" className="btn-secondary" onClick={() => setEditing(null)}>
              {t.common.cancel}
            </button>
          </div>
        </form>
      )}

      {items.length === 0 && !editing && <Empty>{S.none}</Empty>}
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {items.map((s) => (
          <article key={s.id} className="card flex flex-col gap-2">
            <header className="flex items-start gap-2">
              <div className="min-w-0 flex-1">
                <h2 className="truncate font-semibold">{s.title}</h2>
                <div className="flex flex-wrap gap-1 text-xs text-zinc-500">
                  {s.shortcut && <code className="rounded bg-zinc-200 px-1 dark:bg-zinc-800">{s.shortcut}</code>}
                  {s.tags.map((tg) => (
                    <button key={tg} onClick={() => setTag(tg)} className="hover:text-accent-500">
                      #{tg}
                    </button>
                  ))}
                  <span>· {S.used(s.use_count)}</span>
                </div>
              </div>
              <button className="btn-ghost px-1.5" onClick={() => setEditing({ id: s.id, input: { title: s.title, body: s.body, shortcut: s.shortcut, tags: s.tags }, tags: s.tags.join(", ") })} aria-label={t.common.edit}>
                <Pencil className="size-4" />
              </button>
              <button className="btn-ghost px-1.5 hover:text-red-500" onClick={() => setDeleting(s)} aria-label={t.common.delete}>
                <Trash2 className="size-4" />
              </button>
            </header>
            <pre className="line-clamp-4 rounded-md bg-zinc-200/50 p-2 font-mono text-xs whitespace-pre-wrap dark:bg-zinc-800/50">{s.body}</pre>
            {s.variables.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {s.variables.map((v) => (
                  <Badge key={v} tone="accent">{`{{${v}}}`}</Badge>
                ))}
              </div>
            )}
            <div className="mt-auto flex gap-1">
              <button className="btn-secondary" onClick={() => use(s, "copy")}>
                <ClipboardCopy className="size-4" /> {S.copy}
              </button>
              <button className="btn-ghost" onClick={() => use(s, "paste")} title={S.pasteHint}>
                <ClipboardPaste className="size-4" /> {S.paste}
              </button>
            </div>
          </article>
        ))}
      </div>

      {using && (
        <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-label={S.fill}>
          <form
            className="card flex w-full max-w-md flex-col gap-3 bg-white dark:bg-zinc-900"
            onSubmit={(e) => {
              e.preventDefault();
              finish(using.snippet, using.mode, using.values);
            }}
          >
            <header className="flex items-center justify-between">
              <h2 className="font-semibold">{S.fill}</h2>
              <button type="button" onClick={() => setUsing(null)} aria-label={t.common.cancel}>
                <X className="size-4" />
              </button>
            </header>
            {using.snippet.variables.map((v, i) => (
              <Field key={v} label={v}>
                <input className="input" autoFocus={i === 0} value={using.values[v]} onChange={(e) => setUsing({ ...using, values: { ...using.values, [v]: e.target.value } })} />
              </Field>
            ))}
            <button className="btn-primary self-end">{using.mode === "copy" ? S.copy : S.paste}</button>
          </form>
        </div>
      )}

      <ConfirmDialog
        open={deleting !== null}
        title={t.common.delete}
        confirmLabel={t.common.delete}
        danger
        onCancel={() => setDeleting(null)}
        onConfirm={() => {
          const s = deleting!;
          setDeleting(null);
          run(() => api.snippetsDelete(s.id)).then(load);
        }}
      >
        {deleting && S.deleteConfirm(deleting.title)}
      </ConfirmDialog>
    </>
  );
}
