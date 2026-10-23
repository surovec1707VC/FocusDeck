import { NavLink } from "react-router-dom";
import { Blocks, Braces, ClipboardList, Code2, FolderTree, Monitor, Info, Plug, LayoutDashboard, LayoutGrid, ListChecks, NotebookPen, Puzzle, Search, Settings, ShieldCheck, SlidersHorizontal, Timer, Workflow } from "lucide-react";
import { t } from "../i18n";
import { useModules } from "../lib/modules-context";
import { DEV_MODULES } from "../lib/dev-modules";
import { useShell } from "../lib/shell-context";

/** `module`: the page is hidden from the menu while that module is off;
 *  `anyOf`: shown while at least one of these modules is on. */
export const ITEMS: { to: string; label: string; icon: typeof Info; module?: string; anyOf?: string[] }[] = [
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
  { to: "/dev", label: t.nav.dev, icon: Code2, anyOf: DEV_MODULES },
  { to: "/integrations", label: t.nav.integrations, icon: Plug, module: "integrations" },
  { to: "/automations", label: t.nav.automations, icon: Workflow, module: "automations" },
  { to: "/control-center", label: t.nav.controlCenter, icon: SlidersHorizontal },
  { to: "/modules", label: t.nav.modules, icon: Puzzle, module: "module_registry" },
  { to: "/settings", label: t.nav.settings, icon: Settings, module: "settings" },
  { to: "/permissions", label: t.nav.permissions, icon: ShieldCheck, module: "security" },
  { to: "/about", label: t.nav.about, icon: Info },
];

/** Menu items visible with the current modules (also used by the palette and Ctrl+1…9). */
export function useNavItems() {
  const { isEnabled } = useModules();
  return ITEMS.filter((item) => (!item.module || isEnabled(item.module)) && (!item.anyOf || item.anyOf.some(isEnabled)));
}

/** Icons only below `lg`, icons with labels from `lg` up. */
export default function Sidebar() {
  const items = useNavItems();
  const shell = useShell();
  return (
    <nav aria-label="Основная навигация" className="flex w-14 shrink-0 flex-col gap-1 overflow-y-auto border-r border-zinc-200/70 p-2 lg:w-52 dark:border-zinc-800/70">
      <button
        type="button"
        onClick={shell.openPalette}
        title={`${t.shell.palette.title} (Ctrl+K)`}
        aria-keyshortcuts="Control+K"
        className="mb-1 flex items-center gap-3 rounded-lg border border-zinc-300/70 px-2.5 py-1.5 text-zinc-500 transition hover:bg-zinc-200/70 focus-visible:outline-2 focus-visible:outline-accent-500 dark:border-zinc-700/70 dark:hover:bg-zinc-800"
      >
        <Search className="size-4.5 shrink-0" />
        <span className="hidden flex-1 text-left lg:inline">{t.dashboard.search}</span>
        <kbd className="kbd hidden lg:inline-flex">Ctrl K</kbd>
      </button>
      {items.map(({ to, label, icon: Icon }, i) => (
        <NavLink
          key={to}
          to={to}
          end={to === "/"}
          title={i < 9 ? `${label} (Ctrl+${i + 1})` : label}
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
