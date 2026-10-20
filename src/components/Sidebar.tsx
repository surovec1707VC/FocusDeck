import { NavLink } from "react-router-dom";
import { Blocks, Braces, ClipboardList, FolderTree, Monitor, Info, LayoutDashboard, LayoutGrid, ListChecks, NotebookPen, Puzzle, Settings, ShieldCheck, SlidersHorizontal, Timer, Workflow } from "lucide-react";
import { t } from "../i18n";
import { useModules } from "../lib/modules-context";

/** `module`: the page is hidden from the menu while that module is off. */
const ITEMS = [
  { to: "/", label: t.nav.dashboard, icon: LayoutDashboard, module: "dashboard" },
  { to: "/widgets", label: t.nav.widgets, icon: LayoutGrid, module: "widgets" },
  { to: "/layouts", label: t.nav.layouts, icon: Blocks, module: "layouts" },
  { to: "/notes", label: t.nav.notes, icon: NotebookPen, module: "notes" },
  { to: "/tasks", label: t.nav.tasks, icon: ListChecks, module: "tasks" },
  { to: "/snippets", label: t.nav.snippets, icon: Braces, module: "snippets" },
  { to: "/timers", label: t.nav.timers, icon: Timer, module: "timers" },
  { to: "/clipboard", label: t.nav.clipboard, icon: ClipboardList, module: "clipboard_history" },
  { to: "/organizer", label: t.nav.organizer, icon: FolderTree, module: "desktop_organizer" },
  { to: "/windows", label: t.nav.windows, icon: Monitor, module: "windows_integration" },
  { to: "/automations", label: t.nav.automations, icon: Workflow, module: "automations" },
  { to: "/control-center", label: t.nav.controlCenter, icon: SlidersHorizontal },
  { to: "/modules", label: t.nav.modules, icon: Puzzle, module: "module_registry" },
  { to: "/settings", label: t.nav.settings, icon: Settings, module: "settings" },
  { to: "/permissions", label: t.nav.permissions, icon: ShieldCheck, module: "security" },
  { to: "/about", label: t.nav.about, icon: Info },
];

/** Icons only below `lg`, icons with labels from `lg` up. */
export default function Sidebar() {
  const { isEnabled } = useModules();
  return (
    <nav aria-label="Основная навигация" className="flex w-14 shrink-0 flex-col gap-1 border-r border-zinc-200/70 p-2 lg:w-52 dark:border-zinc-800/70">
      {ITEMS.filter((item) => !item.module || isEnabled(item.module)).map(({ to, label, icon: Icon }) => (
        <NavLink
          key={to}
          to={to}
          end={to === "/"}
          title={label}
          className={({ isActive }) =>
            `flex items-center gap-3 rounded-lg px-2.5 py-2 font-medium transition ${
              isActive
                ? "bg-accent-500/15 text-accent-600 dark:text-accent-400"
                : "text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-400 dark:hover:bg-zinc-800"
            }`
          }
        >
          <Icon className="size-4.5 shrink-0" />
          <span className="hidden truncate lg:inline">{label}</span>
        </NavLink>
      ))}
    </nav>
  );
}
