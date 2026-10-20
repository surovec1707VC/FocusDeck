import { Link, NavLink, Navigate, useParams } from "react-router-dom";
import { Braces, FileJson, FolderGit2, GitBranch, Send, SquareTerminal, LucideIcon } from "lucide-react";
import ModuleGate from "../components/ModuleGate";
import { Empty, PageHeader } from "../components/ui";
import { t } from "../i18n";
import { useModules } from "../lib/modules-context";
import GitTool from "./dev/GitTool";
import ProjectsTool from "./dev/ProjectsTool";
import TerminalTool from "./dev/TerminalTool";
import ApiTool from "./dev/ApiTool";
import RegexTool from "./dev/RegexTool";
import JsonTool from "./dev/JsonTool";

/** Every tool is its own module: its tab is shown only while the module is on. */
export const DEV_TOOLS: { key: string; module: string; icon: LucideIcon; Page: () => React.JSX.Element }[] = [
  { key: "git", module: "dev_git", icon: GitBranch, Page: GitTool },
  { key: "projects", module: "dev_projects", icon: FolderGit2, Page: ProjectsTool },
  { key: "terminal", module: "dev_terminal", icon: SquareTerminal, Page: TerminalTool },
  { key: "api", module: "dev_api", icon: Send, Page: ApiTool },
  { key: "regex", module: "dev_regex", icon: Braces, Page: RegexTool },
  { key: "json", module: "dev_json", icon: FileJson, Page: JsonTool },
];

export default function DevTools() {
  const D = t.dev;
  const { tool } = useParams();
  const { isEnabled } = useModules();
  const enabled = DEV_TOOLS.filter((d) => isEnabled(d.module));
  const current = DEV_TOOLS.find((d) => d.key === tool);

  if (!current) {
    return enabled.length ? <Navigate to={`/dev/${enabled[0].key}`} replace /> : (
      <>
        <PageHeader title={D.title} subtitle={D.subtitle} />
        <Empty>
          {D.noTools}{" "}
          <Link className="text-accent-600 hover:underline dark:text-accent-400" to="/control-center">
            {D.openControlCenter}
          </Link>
        </Empty>
      </>
    );
  }

  return (
    <>
      <PageHeader title={D.title} subtitle={D.subtitle} />
      <div role="tablist" className="mb-4 flex flex-wrap gap-1 border-b border-zinc-200 dark:border-zinc-800">
        {enabled.map(({ key, icon: Icon }) => (
          <NavLink
            key={key}
            to={`/dev/${key}`}
            role="tab"
            className={({ isActive }) =>
              `-mb-px flex items-center gap-1.5 border-b-2 px-3 py-2 font-medium transition ${
                isActive ? "border-accent-500 text-accent-600 dark:text-accent-400" : "border-transparent text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200"
              }`
            }
          >
            <Icon className="size-4" />
            {D.tabs[key]}
          </NavLink>
        ))}
      </div>
      <ModuleGate id={current.module}>
        <current.Page />
      </ModuleGate>
    </>
  );
}
