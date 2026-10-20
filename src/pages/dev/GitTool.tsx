import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { ArrowDownToLine, ArrowUpFromLine, FolderOpen, GitCommitHorizontal, Minus, Plus, RefreshCw, X } from "lucide-react";
import { Badge, ErrorBanner } from "../../components/ui";
import { formatDateTime, t } from "../../i18n";
import { api, errorText, GitCommit, GitDiff, GitFileChange, GitRepo, GitStatus } from "../../lib/api";
import { useGuarded } from "../../lib/use-guarded";
import { DiffView } from "./common";

type Selected = { kind: "file"; file: GitFileChange; staged: boolean; untracked: boolean } | { kind: "commit"; commit: GitCommit } | null;

export default function GitTool() {
  const G = t.dev.git;
  const [params, setParams] = useSearchParams();
  const [repos, setRepos] = useState<GitRepo[]>([]);
  const [repo, setRepo] = useState<string | null>(null);
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [log, setLog] = useState<GitCommit[]>([]);
  const [selected, setSelected] = useState<Selected>(null);
  const [diff, setDiff] = useState<GitDiff | null>(null);
  const [message, setMessage] = useState("");
  const [output, setOutput] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { run, dialog } = useGuarded(setError);

  const open = useCallback(async (path: string) => {
    try {
      const r = await api.gitOpenRepo(path);
      setRepo(r.path);
      setRepos(await api.gitRepos());
      setSelected(null);
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  // First load: a repository passed from Project Launcher, else the last one.
  useEffect(() => {
    const fromUrl = params.get("repo");
    if (fromUrl) {
      open(fromUrl);
      setParams({}, { replace: true });
      return;
    }
    api.gitRepos().then((list) => {
      setRepos(list);
      if (list[0]) setRepo(list[0].path);
    }, (e) => setError(errorText(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const refresh = useCallback(async () => {
    if (!repo) return;
    setBusy(true);
    try {
      const [s, l] = await Promise.all([api.gitStatus(repo), api.gitLog(repo)]);
      setStatus(s);
      setLog(l);
      setError(null);
    } catch (e) {
      setStatus(null);
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }, [repo]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // Diff of the selected file or commit.
  useEffect(() => {
    if (!repo || !selected) return setDiff(null);
    const req =
      selected.kind === "commit"
        ? api.gitDiff(repo, { commit: selected.commit.hash })
        : api.gitDiff(repo, { path: selected.file.path, staged: selected.staged, untracked: selected.untracked });
    req.then(setDiff, (e) => setError(errorText(e)));
  }, [repo, selected, status]);

  async function pick() {
    const dir = await api.devPickFolder(G.openRepo);
    if (dir) open(dir);
  }

  const stage = (paths: string[], on: boolean) => run((c) => api.gitStage(repo!, paths, on, c), setStatus);

  function commit() {
    run(
      (c) => api.gitCommit(repo!, message, c),
      (cm) => {
        setMessage("");
        setOutput(G.committed(cm.short));
        refresh();
      },
    );
  }

  const sync = (kind: "pull" | "push") =>
    run(
      (c) => (kind === "pull" ? api.gitPull(repo!, c) : api.gitPush(repo!, c)),
      (r) => {
        setOutput(r.output || "OK");
        setStatus(r.status);
        api.gitLog(repo!).then(setLog);
      },
    );

  const fileRow = (f: GitFileChange, list: "staged" | "unstaged" | "untracked" | "conflicted") => {
    const staged = list === "staged";
    const isSel = selected?.kind === "file" && selected.file.path === f.path && selected.staged === staged;
    return (
      <li key={`${list}:${f.path}`} className={`group flex items-center gap-2 rounded px-2 py-1 ${isSel ? "bg-accent-500/10" : "hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"}`}>
        <span className="w-4 shrink-0 text-center font-mono text-xs font-bold text-zinc-500" title={G.status[f.status] ?? f.status}>
          {f.status}
        </span>
        <button className="min-w-0 flex-1 truncate text-left font-mono text-xs" title={f.orig_path ? `${f.orig_path} → ${f.path}` : f.path} onClick={() => setSelected({ kind: "file", file: f, staged, untracked: list === "untracked" })}>
          {f.path}
        </button>
        {list !== "conflicted" && (
          <button className="btn-ghost px-1.5 py-0.5 opacity-0 group-hover:opacity-100 focus:opacity-100" title={staged ? G.unstage : G.stage} onClick={() => stage([f.path], !staged)}>
            {staged ? <Minus className="size-3.5" /> : <Plus className="size-3.5" />}
          </button>
        )}
      </li>
    );
  };

  const section = (title: string, files: GitFileChange[], list: "staged" | "unstaged" | "untracked" | "conflicted", action?: { label: string; on: boolean }) =>
    files.length > 0 && (
      <div>
        <div className="mb-1 flex items-center justify-between">
          <h3 className="text-xs font-semibold tracking-wide text-zinc-500 uppercase">
            {title} <span className="font-normal">({files.length})</span>
          </h3>
          {action && (
            <button className="text-xs text-accent-600 hover:underline dark:text-accent-400" onClick={() => stage(files.map((f) => f.path), action.on)}>
              {action.label}
            </button>
          )}
        </div>
        <ul>{files.map((f) => fileRow(f, list))}</ul>
      </div>
    );

  return (
    <div className="flex flex-col gap-4">
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="flex flex-wrap items-center gap-2">
        <select className="input min-w-64 flex-1" value={repo ?? ""} onChange={(e) => e.target.value && open(e.target.value)} aria-label={G.repos}>
          {!repo && <option value="">{G.noRepo}</option>}
          {repos.map((r) => (
            <option key={r.path} value={r.path}>
              {r.name} — {r.path}
            </option>
          ))}
        </select>
        <button className="btn-secondary" onClick={pick}>
          <FolderOpen className="size-4" />
          {G.openRepo}
        </button>
        {repo && (
          <button
            className="btn-ghost"
            title={t.dev.remove}
            onClick={async () => {
              await api.gitForgetRepo(repo);
              const list = await api.gitRepos();
              setRepos(list);
              setRepo(list[0]?.path ?? null);
              setStatus(null);
            }}
          >
            <X className="size-4" />
          </button>
        )}
      </div>

      {repo && status && (
        <>
          <div className="card flex flex-wrap items-center gap-3 p-3">
            <GitCommitHorizontal className="size-5 text-accent-500" />
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-semibold">{status.detached ? G.detached : status.branch}</span>
                {status.no_commits && <Badge>{G.noCommits}</Badge>}
                <span className="text-xs text-zinc-500">{status.upstream ? G.upstream(status.upstream) : G.noUpstream}</span>
                {status.ahead > 0 && <Badge tone="accent">{G.ahead(status.ahead)}</Badge>}
                {status.behind > 0 && <Badge tone="warn">{G.behind(status.behind)}</Badge>}
              </div>
              <div className="truncate text-xs text-zinc-500">{status.root}</div>
            </div>
            <button className="btn-ghost" onClick={refresh} disabled={busy} title={t.dev.refresh}>
              <RefreshCw className={`size-4 ${busy ? "animate-spin" : ""}`} />
            </button>
            <button className="btn-secondary" onClick={() => sync("pull")} title={G.pullHint}>
              <ArrowDownToLine className="size-4" />
              {G.pull}
            </button>
            <button className="btn-secondary" onClick={() => sync("push")} title={G.pushHint} disabled={status.no_commits}>
              <ArrowUpFromLine className="size-4" />
              {G.push}
            </button>
          </div>
          {output && (
            <pre className="card relative max-h-40 overflow-auto p-3 pr-8 font-mono text-xs whitespace-pre-wrap select-text">
              {output}
              <button className="absolute top-2 right-2 opacity-60 hover:opacity-100" onClick={() => setOutput(null)} aria-label={t.dev.close}>
                <X className="size-4" />
              </button>
            </pre>
          )}

          <div className="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
            <div className="flex flex-col gap-4">
              <div className="card flex flex-col gap-3 p-3">
                {status.clean && <p className="text-zinc-500">{G.clean}</p>}
                {section(G.conflicted, status.conflicted, "conflicted")}
                {section(G.staged, status.staged, "staged", { label: G.unstageAll, on: false })}
                {section(G.unstaged, status.unstaged, "unstaged", { label: G.stageAll, on: true })}
                {section(G.untracked, status.untracked, "untracked", { label: G.stageAll, on: true })}
              </div>
              <div className="card flex flex-col gap-2 p-3">
                <textarea className="input min-h-20 font-mono text-xs" placeholder={G.commitMessage} value={message} onChange={(e) => setMessage(e.target.value)} />
                <button className="btn-primary self-end" disabled={!message.trim() || status.staged.length === 0} onClick={commit}>
                  {G.commit} ({status.staged.length})
                </button>
                <p className="text-xs text-zinc-500">{G.permission}</p>
              </div>
              <div className="card p-3">
                <h3 className="mb-2 text-xs font-semibold tracking-wide text-zinc-500 uppercase">{G.commits}</h3>
                <ul className="flex flex-col">
                  {log.map((c) => (
                    <li key={c.hash}>
                      <button
                        className={`flex w-full items-baseline gap-2 rounded px-2 py-1 text-left ${selected?.kind === "commit" && selected.commit.hash === c.hash ? "bg-accent-500/10" : "hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"}`}
                        onClick={() => setSelected({ kind: "commit", commit: c })}
                      >
                        <span className="font-mono text-xs text-accent-600 dark:text-accent-400">{c.short}</span>
                        <span className="min-w-0 flex-1 truncate">{c.subject}</span>
                        <span className="shrink-0 text-xs text-zinc-500" title={`${c.author} <${c.email}>`}>
                          {formatDateTime(c.date)}
                        </span>
                      </button>
                      {c.refs && <div className="truncate pl-14 text-xs text-zinc-500">{c.refs}</div>}
                    </li>
                  ))}
                </ul>
              </div>
            </div>
            <div className="card min-h-64 overflow-hidden">
              <div className="border-b border-zinc-200 px-3 py-2 text-xs font-semibold text-zinc-500 dark:border-zinc-800">
                {G.diff}
                {selected?.kind === "file" && `: ${selected.file.path}${selected.staged ? " (staged)" : ""}`}
                {selected?.kind === "commit" && `: ${selected.commit.short} ${selected.commit.subject}`}
              </div>
              {!selected ? <p className="p-3 text-zinc-500">{G.diffEmpty}</p> : diff && <DiffView text={diff.text} />}
              {diff?.truncated && <p className="px-3 pb-2 text-xs text-amber-600">{G.truncated}</p>}
            </div>
          </div>
        </>
      )}
      {!repo && <p className="text-zinc-500">{G.noRepo}</p>}
      {dialog}
    </div>
  );
}
