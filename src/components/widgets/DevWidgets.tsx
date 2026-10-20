import { Link } from "react-router-dom";
import { GitBranch } from "lucide-react";
import { formatDateTime, t } from "../../i18n";
import { GitCommit, GitStatus } from "../../lib/api";
import { useWidgetData } from "../../lib/widget-data";
import { BodyProps, DataStatus, Muted } from "./common";

interface GitData {
  repo: string | null;
  name?: string;
  error?: string;
  status?: GitStatus;
  commits?: GitCommit[];
}

/** Branch, changes and last commits of the current Git repository. */
export function GitWidget({ widget }: BodyProps) {
  const G = t.dev.git;
  const { data, meta, error } = useWidgetData<GitData>(widget);
  if (!data) return <DataStatus meta={meta} error={error} />;
  if (!data.repo || !data.status) {
    return (
      <div className="flex flex-col items-start gap-2">
        <Muted>{data.error ?? G.widgetNoRepo}</Muted>
        <Link to="/dev/git" className="text-xs text-accent-600 hover:underline dark:text-accent-400">
          {G.widgetOpen}
        </Link>
      </div>
    );
  }
  const s = data.status;
  const changes = s.staged.length + s.unstaged.length + s.untracked.length + s.conflicted.length;
  return (
    <div className="flex h-full flex-col gap-2">
      <Link to="/dev/git" className="flex items-center gap-1.5 font-semibold hover:underline" title={data.repo}>
        <GitBranch className="size-4 text-accent-500" />
        <span className="truncate">{data.name}</span>
        <span className="truncate font-normal text-zinc-500">· {s.detached ? G.detached : s.branch}</span>
      </Link>
      <div className="flex flex-wrap gap-x-3 text-xs">
        {s.clean ? <span className="text-emerald-600">{G.clean}</span> : <span className="text-amber-600">{G.changes(changes)}</span>}
        {s.staged.length > 0 && <span>staged {s.staged.length}</span>}
        {s.ahead > 0 && <span>{G.ahead(s.ahead)}</span>}
        {s.behind > 0 && <span>{G.behind(s.behind)}</span>}
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto text-xs">
        {(data.commits ?? []).map((c) => (
          <li key={c.hash} className="flex gap-1.5 py-0.5" title={`${c.author} · ${formatDateTime(c.date)}`}>
            <span className="font-mono text-accent-600 dark:text-accent-400">{c.short}</span>
            <span className="truncate">{c.subject}</span>
          </li>
        ))}
      </ul>
      <DataStatus meta={meta} error={error} />
    </div>
  );
}
