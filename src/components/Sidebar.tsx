import { NavLink } from "react-router-dom";
import { Blocks, Info, LayoutDashboard, LayoutGrid, Puzzle, Settings, ShieldCheck, Workflow } from "lucide-react";
import { t } from "../i18n";

const ITEMS = [
  { to: "/", label: t.nav.dashboard, icon: LayoutDashboard },
  { to: "/widgets", label: t.nav.widgets, icon: LayoutGrid },
  { to: "/layouts", label: t.nav.layouts, icon: Blocks },
  { to: "/automations", label: t.nav.automations, icon: Workflow },
  { to: "/modules", label: t.nav.modules, icon: Puzzle },
  { to: "/settings", label: t.nav.settings, icon: Settings },
  { to: "/permissions", label: t.nav.permissions, icon: ShieldCheck },
  { to: "/about", label: t.nav.about, icon: Info },
];

/** Icons only below `lg`, icons with labels from `lg` up. */
export default function Sidebar() {
  return (
    <nav aria-label="Основная навигация" className="flex w-14 shrink-0 flex-col gap-1 border-r border-zinc-200/70 p-2 lg:w-52 dark:border-zinc-800/70">
      {ITEMS.map(({ to, label, icon: Icon }) => (
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
