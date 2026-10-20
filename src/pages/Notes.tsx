import { useCallback, useEffect, useRef, useState } from "react";
import { Download, Eye, FileText, Pencil, Pin, PinOff, Plus, RotateCcw, Search, Trash2, X } from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import Markdown from "../components/Markdown";
import { Badge, Empty, ErrorBanner, PageHeader } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { api, errorText, Layout, Note, NoteFacets, NoteInput } from "../lib/api";
import { useModules } from "../lib/modules-context";

export default function Notes() {
  const N = t.notes;
  const preview = useModules().settingsOf("notes").preview !== false;
  const [notes, setNotes] = useState<Note[]>([]);
  const [facets, setFacets] = useState<NoteFacets | null>(null);
  const [layouts, setLayouts] = useState<Layout[]>([]);
  const [search, setSearch] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [project, setProject] = useState("");
  const [trash, setTrash] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [draft, setDraft] = useState<Note | null>(null);
  const [mode, setMode] = useState<"edit" | "preview">("edit");
  const [status, setStatus] = useState<"" | "saving" | "saved">("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<"forever" | "empty" | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const load = useCallback(async () => {
    try {
      const [list, f] = await Promise.all([
        api.notesList({ search: search || undefined, tag: tag ?? undefined, project: project || undefined, trash }),
        api.notesFacets(),
      ]);
      setNotes(list);
      setFacets(f);
    } catch (e) {
      setError(errorText(e));
    }
  }, [search, tag, project, trash]);

  useEffect(() => {
    const id = setTimeout(load, 150);
    return () => clearTimeout(id);
  }, [load]);

  useEffect(() => {
    api.layoutsList().then(setLayouts, () => undefined);
  }, []);

  // The editor works on a copy; switching notes flushes pending changes.
  useEffect(() => {
    setDraft(notes.find((n) => n.id === selected) ?? null);
  }, [selected]);

  async function save(next: Note, input: NoteInput) {
    setStatus("saving");
    try {
      const saved = await api.notesUpdate(next.id, input);
      setNotes((list) => list.map((n) => (n.id === saved.id ? saved : n)));
      setStatus("saved");
      setError(null);
    } catch (e) {
      setError(errorText(e));
      setStatus("");
    }
  }

  function edit(patch: Partial<Note>, immediate = false) {
    if (!draft) return;
    const next = { ...draft, ...patch };
    setDraft(next);
    setStatus("");
    clearTimeout(timer.current);
    const input: NoteInput = { title: next.title, body: next.body, tags: next.tags, project: next.project, pinned: next.pinned, layout_id: next.layout_id };
    if (immediate) save(next, input);
    else timer.current = setTimeout(() => save(next, input), 700);
  }

  async function create() {
    try {
      const n = await api.notesCreate({ title: N.untitled, body: "", tags: tag ? [tag] : [], project });
      setTrash(false);
      setNotes((list) => [n, ...list]);
      setSelected(n.id);
      setDraft(n);
      setMode("edit");
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function act(fn: () => Promise<unknown>) {
    try {
      await fn();
      setSelected(null);
      setError(null);
      load();
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function exportNote(format: "md" | "txt") {
    if (!draft) return;
    clearTimeout(timer.current);
    try {
      const path = await api.notesExport(draft.id, format);
      if (path) setNotice(N.exported(path));
    } catch (e) {
      setError(errorText(e));
    }
  }

  const layoutName = (id: string | null) => layouts.find((l) => l.id === id)?.name;

  return (
    <>
      <PageHeader
        title={N.title}
        subtitle={N.subtitle}
        actions={
          <button className="btn-primary" onClick={create}>
            <Plus className="size-4" /> {N.create}
          </button>
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />
      {notice && (
        <div className="mb-3 flex items-center justify-between rounded-lg bg-accent-500/10 px-3 py-2 text-accent-700 dark:text-accent-300">
          <span className="truncate">{notice}</span>
          <button onClick={() => setNotice(null)} aria-label={t.widgets.close}>
            <X className="size-4" />
          </button>
        </div>
      )}

      <div className="grid gap-4 lg:grid-cols-[18rem_1fr]">
        {/* List */}
        <aside className="flex min-w-0 flex-col gap-2">
          <label className="relative">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400" />
            <input className="input pl-8" placeholder={N.search} value={search} onChange={(e) => setSearch(e.target.value)} aria-label={N.search} />
          </label>
          <div className="flex gap-1">
            <button className={trash ? "btn-ghost flex-1" : "btn-secondary flex-1"} onClick={() => setTrash(false)}>
              <FileText className="size-4" /> {N.all}
            </button>
            <button className={trash ? "btn-secondary flex-1" : "btn-ghost flex-1"} onClick={() => setTrash(true)}>
              <Trash2 className="size-4" /> {N.trash} {facets && facets.trash > 0 && `(${facets.trash})`}
            </button>
          </div>
          {facets && facets.projects.length > 0 && (
            <select className="input" value={project} onChange={(e) => setProject(e.target.value)} aria-label={N.project}>
              <option value="">{N.allProjects}</option>
              {facets.projects.map((p) => (
                <option key={p}>{p}</option>
              ))}
            </select>
          )}
          {facets && facets.tags.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {facets.tags.map(([name, count]) => (
                <button
                  key={name}
                  onClick={() => setTag(tag === name ? null : name)}
                  className={`rounded-full px-2 py-0.5 text-xs ${tag === name ? "bg-accent-600 text-white" : "bg-zinc-200 hover:bg-zinc-300 dark:bg-zinc-800 dark:hover:bg-zinc-700"}`}
                >
                  #{name} <span className="opacity-60">{count}</span>
                </button>
              ))}
            </div>
          )}
          {trash && notes.length > 0 && (
            <button className="btn-danger self-start text-xs" onClick={() => setConfirm("empty")}>
              {N.emptyTrash}
            </button>
          )}
          <ul className="flex max-h-[60vh] flex-col gap-1 overflow-y-auto">
            {notes.map((n) => (
              <li key={n.id}>
                <button
                  onClick={() => setSelected(n.id)}
                  className={`w-full rounded-lg border px-3 py-2 text-left transition ${
                    n.id === selected ? "border-accent-500/60 bg-accent-500/10" : "border-transparent hover:bg-zinc-200/70 dark:hover:bg-zinc-800/70"
                  }`}
                >
                  <div className="flex items-center gap-1">
                    {n.pinned && <Pin className="size-3 shrink-0 text-accent-500" />}
                    <span className="truncate font-medium">{n.title || N.untitled}</span>
                  </div>
                  <div className="line-clamp-2 text-xs text-zinc-500">{n.body.slice(0, 160) || N.empty}</div>
                  <div className="mt-0.5 flex flex-wrap gap-1 text-[11px] text-zinc-500">
                    {n.tags.map((tg) => (
                      <span key={tg}>#{tg}</span>
                    ))}
                    {n.project && <span>· {n.project}</span>}
                  </div>
                </button>
              </li>
            ))}
            {notes.length === 0 && <li className="p-3 text-center text-zinc-500">{trash ? N.trashEmpty : N.none}</li>}
          </ul>
        </aside>

        {/* Editor */}
        {!draft ? (
          <Empty>{N.select}</Empty>
        ) : (
          <section className="card flex min-w-0 flex-col gap-3">
            <div className="flex flex-wrap items-center gap-2">
              <input
                className="min-w-0 flex-1 bg-transparent text-lg font-semibold outline-none"
                value={draft.title}
                maxLength={200}
                placeholder={N.untitled}
                readOnly={Boolean(draft.deleted_at)}
                onChange={(e) => edit({ title: e.target.value })}
                aria-label={N.noteTitle}
              />
              <span className="text-xs text-zinc-500">{status === "saving" ? t.widgets.saving : status === "saved" ? t.widgets.saved : formatDateTime(draft.updated_at)}</span>
            </div>

            {draft.deleted_at ? (
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone="warn">{N.inTrash(formatDateTime(draft.deleted_at))}</Badge>
                <button className="btn-secondary" onClick={() => act(() => api.notesRestore(draft.id))}>
                  <RotateCcw className="size-4" /> {N.restore}
                </button>
                <button className="btn-danger" onClick={() => setConfirm("forever")}>
                  <Trash2 className="size-4" /> {N.deleteForever}
                </button>
              </div>
            ) : (
              <div className="flex flex-wrap items-center gap-2 text-xs">
                <input
                  className="input w-56"
                  placeholder={N.tagsPlaceholder}
                  defaultValue={draft.tags.join(", ")}
                  key={`tags-${draft.id}`}
                  onBlur={(e) => edit({ tags: e.target.value.split(/[,\s]+/).filter(Boolean) }, true)}
                  aria-label={N.tags}
                />
                <input
                  className="input w-40"
                  placeholder={N.project}
                  value={draft.project}
                  maxLength={60}
                  list="note-projects"
                  onChange={(e) => edit({ project: e.target.value })}
                  aria-label={N.project}
                />
                <datalist id="note-projects">
                  {facets?.projects.map((p) => (
                    <option key={p} value={p} />
                  ))}
                </datalist>
                <select className="input w-44" value={draft.layout_id ?? ""} onChange={(e) => edit({ layout_id: e.target.value || null }, true)} aria-label={N.layout}>
                  <option value="">{N.noLayout}</option>
                  {layouts.map((l) => (
                    <option key={l.id} value={l.id}>
                      {N.layoutPrefix} {l.name}
                    </option>
                  ))}
                </select>
                <button className="btn-ghost px-2" onClick={() => edit({ pinned: !draft.pinned }, true)} aria-pressed={draft.pinned} title={draft.pinned ? t.widgets.unpin : t.widgets.pin}>
                  {draft.pinned ? <PinOff className="size-4 text-accent-500" /> : <Pin className="size-4" />}
                </button>
                {preview && (
                  <button className="btn-ghost px-2" onClick={() => setMode(mode === "edit" ? "preview" : "edit")}>
                    {mode === "edit" ? <Eye className="size-4" /> : <Pencil className="size-4" />}
                    {mode === "edit" ? N.preview : N.edit}
                  </button>
                )}
                <span className="ml-auto flex gap-1">
                  <button className="btn-ghost px-2" onClick={() => exportNote("md")} title={N.export}>
                    <Download className="size-4" /> .md
                  </button>
                  <button className="btn-ghost px-2" onClick={() => exportNote("txt")} title={N.export}>
                    <Download className="size-4" /> .txt
                  </button>
                  {!draft.widget && (
                    <button className="btn-ghost px-2 hover:text-red-500" onClick={() => act(() => api.notesTrash(draft.id))} title={N.toTrash}>
                      <Trash2 className="size-4" />
                    </button>
                  )}
                </span>
              </div>
            )}
            {draft.layout_id && layoutName(draft.layout_id) && <p className="text-xs text-zinc-500">{N.boundTo(layoutName(draft.layout_id)!)}</p>}

            {mode === "preview" || draft.deleted_at ? (
              <div className="min-h-80 rounded-lg border border-zinc-200 p-3 dark:border-zinc-800">
                <Markdown source={draft.body || N.empty} />
              </div>
            ) : (
              <textarea
                className="input min-h-80 flex-1 resize-y font-mono text-sm"
                value={draft.body}
                placeholder={N.bodyPlaceholder}
                onChange={(e) => edit({ body: e.target.value })}
                aria-label={N.body}
              />
            )}
          </section>
        )}
      </div>

      <ConfirmDialog
        open={confirm !== null}
        title={confirm === "empty" ? N.emptyTrash : N.deleteForever}
        confirmLabel={t.common.delete}
        danger
        onCancel={() => setConfirm(null)}
        onConfirm={() => {
          const which = confirm;
          setConfirm(null);
          if (which === "empty") act(() => api.notesEmptyTrash());
          else if (draft) act(() => api.notesDeleteForever(draft.id));
        }}
      >
        {confirm === "empty" ? N.emptyTrashConfirm : N.deleteForeverConfirm}
      </ConfirmDialog>
    </>
  );
}
