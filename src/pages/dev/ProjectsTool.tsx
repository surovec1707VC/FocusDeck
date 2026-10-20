import { FormEvent, useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Code2, FolderOpen, FolderSearch, GitBranch, Pencil, Play, Plus, RefreshCw, Search, SquareTerminal, Star, Trash2, X } from "lucide-react";
import ConfirmDialog from "../../components/ConfirmDialog";
import { Badge, Empty, ErrorBanner, Field } from "../../components/ui";
import { formatDateTime, t } from "../../i18n";
import { api, errorText, Project, ProjectCandidate, ProjectCommand, ShellId } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import { useGuarded } from "../../lib/use-guarded";

export const SHELL_NAMES: Record<ShellId, string> = { powershell: "PowerShell", cmd: "CMD", git_bash: "Git Bash", wsl: "WSL", custom: "Свой shell" };
const KINDS = ["node", "tauri", "rust", "python", "dotnet", "go", "java", "php", "ruby", "generic"];

export default function ProjectsTool() {
  const P = t.dev.projects;
  const navigate = useNavigate();
  const { isEnabled } = useModules();
  const [items, setItems] = useState<Project[]>([]);
  const [search, setSearch] = useState("");
  const [shell, setShell] = useState<ShellId | "">("");
  const [candidates, setCandidates] = useState<{ list: ProjectCandidate[]; chosen: Set<string> } | null>(null);
  const [editing, setEditing] = useState<Project | null>(null);
  const [removing, setRemoving] = useState<Project | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { run, dialog } = useGuarded(setError);

  const load = useCallback(() => api.projectsList().then(setItems, (e) => setError(errorText(e))), []);
  useEffect(() => {
    load();
  }, [load]);

  const replace = (p: Project) => setItems((list) => list.map((x) => (x.id === p.id ? p : x)));

  async function add() {
    const dir = await api.devPickFolder(P.add);
    if (!dir) return;
    try {
      await api.projectsAdd(dir);
      load();
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function scan() {
    const dir = await api.devPickFolder(P.scan);
    if (!dir) return;
    try {
      const list = await api.projectsScan(dir);
      setCandidates({ list, chosen: new Set(list.filter((c) => !c.added).map((c) => c.path)) });
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function addChosen() {
    if (!candidates) return;
    for (const path of candidates.chosen) {
      try {
        await api.projectsAdd(path);
      } catch (e) {
        setError(errorText(e));
      }
    }
    setCandidates(null);
    load();
  }

  const sh = shell || undefined;
  const q = search.trim().toLowerCase();
  const shown = items.filter((p) => !q || p.name.toLowerCase().includes(q) || p.path.toLowerCase().includes(q) || p.kind.includes(q));

  return (
    <div className="flex flex-col gap-4">
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-56 flex-1">
          <Search className="absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400" />
          <input className="input w-full pl-8" placeholder={P.search} value={search} onChange={(e) => setSearch(e.target.value)} />
        </div>
        <label className="flex items-center gap-1.5 text-xs text-zinc-500">
          {P.shell}
          <select className="input w-40" value={shell} onChange={(e) => setShell(e.target.value as ShellId | "")}>
            <option value="">по умолчанию</option>
            {Object.entries(SHELL_NAMES).map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
          </select>
        </label>
        <button className="btn-secondary" onClick={scan}>
          <FolderSearch className="size-4" />
          {P.scan}
        </button>
        <button className="btn-primary" onClick={add}>
          <Plus className="size-4" />
          {P.add}
        </button>
      </div>

      {items.length === 0 && <Empty>{P.empty}</Empty>}
      <div className="grid gap-3 md:grid-cols-2">
        {shown.map((p) => (
          <div key={p.id} className={`card flex flex-col gap-2 p-3 ${p.exists ? "" : "opacity-60"}`}>
            <div className="flex items-start gap-2">
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="truncate font-semibold">{p.name}</span>
                  <Badge tone="accent">{P.kinds[p.kind] ?? p.kind}</Badge>
                  {p.is_git && <Badge>git</Badge>}
                  {!p.exists && <Badge tone="error">{P.missing}</Badge>}
                </div>
                <div className="truncate text-xs text-zinc-500" title={p.path}>
                  {p.path}
                </div>
                {p.last_opened_at && (
                  <div className="text-xs text-zinc-400">
                    {P.lastOpened} {formatDateTime(p.last_opened_at)}
                  </div>
                )}
              </div>
              <button className="btn-ghost px-1.5" title={p.favorite ? P.unfavorite : P.favorite} onClick={() => api.projectsUpdate(p.id, { favorite: !p.favorite }).then(load, (e) => setError(errorText(e)))}>
                <Star className={`size-4 ${p.favorite ? "fill-amber-400 text-amber-400" : ""}`} />
              </button>
              <button className="btn-ghost px-1.5" title={P.edit} onClick={() => setEditing(p)}>
                <Pencil className="size-4" />
              </button>
              <button className="btn-ghost px-1.5" title={P.redetect} onClick={() => api.projectsRedetect(p.id).then(replace, (e) => setError(errorText(e)))}>
                <RefreshCw className="size-4" />
              </button>
              <button className="btn-danger px-1.5" title={t.dev.remove} onClick={() => setRemoving(p)}>
                <Trash2 className="size-4" />
              </button>
            </div>
            <div className="flex flex-wrap gap-1.5">
              <button className="btn-secondary py-1 text-xs" disabled={!p.exists} onClick={() => run((c) => api.projectsOpen(p.id, "editor", undefined, c), load)}>
                <Code2 className="size-3.5" />
                {P.editor}
              </button>
              <button className="btn-secondary py-1 text-xs" disabled={!p.exists} onClick={() => run((c) => api.projectsOpen(p.id, "terminal", sh, c), load)}>
                <SquareTerminal className="size-3.5" />
                {P.terminal}
              </button>
              <button className="btn-secondary py-1 text-xs" disabled={!p.exists} onClick={() => run((c) => api.projectsOpen(p.id, "explorer", undefined, c), load)}>
                <FolderOpen className="size-3.5" />
                {P.explorer}
              </button>
              {p.is_git && isEnabled("dev_git") && (
                <button className="btn-secondary py-1 text-xs" onClick={() => navigate(`/dev/git?repo=${encodeURIComponent(p.path)}`)}>
                  <GitBranch className="size-3.5" />
                  {P.git}
                </button>
              )}
            </div>
            {p.commands.length > 0 && (
              <ul className="flex flex-col gap-0.5 border-t border-zinc-200 pt-2 dark:border-zinc-800">
                {p.commands.map((c, i) => (
                  <li key={i} className="flex items-center gap-2">
                    <button className="btn-ghost px-1.5 py-0.5" title={P.run} disabled={!p.exists} onClick={() => run((cf) => api.projectsRun(p.id, i, sh, cf), load)}>
                      <Play className="size-3.5 text-emerald-600" />
                    </button>
                    <span className="w-28 shrink-0 truncate text-xs font-medium">{c.name}</span>
                    <code className="min-w-0 flex-1 truncate text-xs text-zinc-500">{c.command}</code>
                  </li>
                ))}
              </ul>
            )}
          </div>
        ))}
      </div>

      {candidates && (
        <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-label={P.scanTitle}>
          <div className="card flex max-h-[80vh] w-full max-w-xl flex-col gap-3 p-4">
            <h2 className="text-lg font-semibold">{P.scanTitle}</h2>
            {candidates.list.length === 0 && <p className="text-zinc-500">{P.scanEmpty}</p>}
            <ul className="flex flex-col gap-1 overflow-y-auto">
              {candidates.list.map((c) => (
                <li key={c.path}>
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      disabled={c.added}
                      checked={candidates.chosen.has(c.path)}
                      onChange={(e) => {
                        const chosen = new Set(candidates.chosen);
                        if (e.target.checked) chosen.add(c.path);
                        else chosen.delete(c.path);
                        setCandidates({ ...candidates, chosen });
                      }}
                    />
                    <span className="font-medium">{c.name}</span>
                    <Badge>{P.kinds[c.kind] ?? c.kind}</Badge>
                    {c.added && <span className="text-xs text-zinc-500">{P.added}</span>}
                    <span className="min-w-0 flex-1 truncate text-right text-xs text-zinc-500">{c.path}</span>
                  </label>
                </li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <button className="btn-secondary" onClick={() => setCandidates(null)}>
                {t.common.cancel}
              </button>
              <button className="btn-primary" disabled={candidates.chosen.size === 0} onClick={addChosen}>
                {P.addSelected} ({candidates.chosen.size})
              </button>
            </div>
          </div>
        </div>
      )}

      {editing && <EditProject project={editing} onClose={() => setEditing(null)} onSaved={(p) => (replace(p), setEditing(null))} onError={setError} />}

      <ConfirmDialog
        open={removing !== null}
        title={t.dev.remove}
        confirmLabel={t.dev.remove}
        danger
        onCancel={() => setRemoving(null)}
        onConfirm={async () => {
          const p = removing!;
          setRemoving(null);
          await api.projectsRemove(p.id).catch((e) => setError(errorText(e)));
          load();
        }}
      >
        {removing?.name} — {removing?.path}
      </ConfirmDialog>
      {dialog}
    </div>
  );
}

function EditProject({ project, onClose, onSaved, onError }: { project: Project; onClose: () => void; onSaved: (p: Project) => void; onError: (m: string) => void }) {
  const P = t.dev.projects;
  const [name, setName] = useState(project.name);
  const [kind, setKind] = useState(project.kind);
  const [commands, setCommands] = useState<ProjectCommand[]>(project.commands);
  const set = (i: number, patch: Partial<ProjectCommand>) => setCommands((list) => list.map((c, j) => (j === i ? { ...c, ...patch } : c)));

  async function save(e: FormEvent) {
    e.preventDefault();
    try {
      onSaved(await api.projectsUpdate(project.id, { name, kind, commands: commands.filter((c) => c.name.trim() || c.command.trim()) }));
    } catch (err) {
      onError(errorText(err));
    }
  }

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-label={P.edit}>
      <form className="card flex max-h-[85vh] w-full max-w-2xl flex-col gap-3 overflow-y-auto p-4" onSubmit={save}>
        <h2 className="text-lg font-semibold">{P.edit}</h2>
        <div className="grid grid-cols-[2fr_1fr] gap-3">
          <Field label={P.name}>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
          <Field label={P.kind}>
            <select className="input" value={kind} onChange={(e) => setKind(e.target.value)}>
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {P.kinds[k]}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <h3 className="font-medium">{P.commands}</h3>
        {commands.length === 0 && <p className="text-xs text-zinc-500">{P.noCommands}</p>}
        {commands.map((c, i) => (
          <div key={i} className="flex items-center gap-1.5">
            <input className="input w-36" placeholder={P.commandName} value={c.name} onChange={(e) => set(i, { name: e.target.value })} />
            <input className="input flex-1 font-mono text-xs" placeholder={P.command} value={c.command} onChange={(e) => set(i, { command: e.target.value, custom: true })} />
            <button type="button" className="btn-ghost px-1.5" onClick={() => setCommands(commands.filter((_, j) => j !== i))} aria-label={t.common.delete}>
              <X className="size-4" />
            </button>
          </div>
        ))}
        <button type="button" className="btn-ghost self-start" onClick={() => setCommands([...commands, { name: "", command: "", custom: true }])}>
          <Plus className="size-4" />
          {P.addCommand}
        </button>
        <div className="flex justify-end gap-2">
          <button type="button" className="btn-secondary" onClick={onClose}>
            {t.common.cancel}
          </button>
          <button className="btn-primary">{t.dev.save}</button>
        </div>
      </form>
    </div>
  );
}
