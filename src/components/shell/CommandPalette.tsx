import { KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Blocks, Database, GraduationCap, Keyboard, LucideIcon, Moon, Puzzle, Search, Zap } from "lucide-react";
import { t } from "../../i18n";
import { api, Layout, QuickAction } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import { useSettings } from "../../lib/settings-context";
import { useShell } from "../../lib/shell-context";
import { useGuarded } from "../../lib/use-guarded";
import { useNavItems } from "../Sidebar";

interface Item {
  id: string;
  group: string;
  label: string;
  hint?: string;
  icon: LucideIcon;
  run: () => void;
}

const P = t.shell.palette;

/** Lowercase, ё → е: «тема» finds «Тёмная». */
const norm = (s: string) => s.toLowerCase().replace(/ё/g, "е");

/** Every word of the query must occur somewhere in the label or hint. */
export function matches(item: { label: string; hint?: string; group: string }, query: string) {
  const hay = norm(`${item.label} ${item.hint ?? ""} ${P.groups[item.group] ?? ""}`);
  // Russian endings vary («тема» / «тему»): longer words match without their last letter.
  return norm(query).split(/\s+/).filter(Boolean).every((w) => hay.includes(w.length > 3 ? w.slice(0, -1) : w));
}

/** Ctrl+K: pages, layouts, modules, quick actions and app actions in one list. */
export default function CommandPalette({ open, onClose, onError }: { open: boolean; onClose: () => void; onError: (m: string) => void }) {
  const navigate = useNavigate();
  const nav = useNavItems();
  const { modules, isEnabled } = useModules();
  const { settings, save } = useSettings();
  const shell = useShell();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [layouts, setLayouts] = useState<Layout[]>([]);
  const [quick, setQuick] = useState<QuickAction[]>([]);
  const ref = useRef<HTMLDialogElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const guarded = useGuarded((m) => m && onError(m));

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) {
      d.showModal();
      setQuery("");
      setActive(0);
      // Optional sources: a turned-off module just contributes nothing.
      api.layoutsList().then(setLayouts).catch(() => setLayouts([]));
      if (isEnabled("clipboard_history")) api.quickActionsList().then(setQuick).catch(() => setQuick([]));
      else setQuick([]);
    }
    if (!open && d.open) d.close();
  }, [open, isEnabled]);

  const items = useMemo<Item[]>(() => {
    const go = (to: string) => () => navigate(to);
    const all: Item[] = [
      ...nav.map((n) => ({ id: `page:${n.to}`, group: "page", label: n.label, icon: n.icon, run: go(n.to) })),
      ...layouts.map((l) => ({
        id: `layout:${l.id}`,
        group: "layout",
        label: P.layoutActivate(l.name),
        hint: l.is_active ? t.dashboard.activeBadge : undefined,
        icon: Blocks,
        run: () => api.layoutsActivate(l.id).then(() => navigate("/")).catch((e) => onError(String(e.message ?? e))),
      })),
      ...quick.map((q) => ({
        id: `quick:${q.id}`,
        group: "quick",
        label: q.name,
        icon: Zap,
        run: () => guarded.run((confirm) => api.quickActionsRun(q.id, confirm)),
      })),
      {
        id: "action:theme",
        group: "action",
        label: P.themeToggle,
        icon: Moon,
        run: () => settings && save({ ...settings, theme: document.documentElement.classList.contains("dark") ? "light" : "dark" }).catch((e) => onError(String(e))),
      },
      { id: "action:backup", group: "action", label: P.backup, icon: Database, run: () => api.backupCreate().catch((e) => onError(String(e.message ?? e))) },
      { id: "action:shortcuts", group: "action", label: P.shortcuts, hint: "F1", icon: Keyboard, run: shell.openShortcuts },
      { id: "action:onboarding", group: "action", label: P.onboarding, icon: GraduationCap, run: shell.openOnboarding },
      ...(modules ?? []).map((m) => ({
        id: `module:${m.id}`,
        group: "module",
        label: P.moduleOpen(m.name, m.enabled),
        hint: m.description,
        icon: Puzzle,
        run: go(`/control-center?module=${m.id}`),
      })),
    ];
    return query.trim() ? all.filter((i) => matches(i, query)).slice(0, 50) : all.filter((i) => i.group !== "module").slice(0, 30);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nav, layouts, quick, modules, query, settings, save, navigate, shell, onError]);

  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    listRef.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  function choose(i: Item | undefined) {
    if (!i) return;
    onClose();
    i.run();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") setActive((a) => Math.min(a + 1, items.length - 1));
    else if (e.key === "ArrowUp") setActive((a) => Math.max(a - 1, 0));
    else if (e.key === "Enter") choose(items[active]);
    else return;
    e.preventDefault();
  }

  let lastGroup = "";
  return (
    <>
    {guarded.dialog}
    <dialog
      ref={ref}
      aria-label={P.title}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onClick={(e) => e.target === ref.current && onClose()}
      className="mx-auto mt-[12vh] w-[min(36rem,calc(100vw-2rem))] overflow-hidden rounded-xl border border-zinc-200 bg-white p-0 text-zinc-900 shadow-2xl backdrop:bg-black/40 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-100"
    >
      {open && (
        <div onKeyDown={onKey}>
          <div className="flex items-center gap-2 border-b border-zinc-200 px-3 dark:border-zinc-800">
            <Search className="size-4 text-zinc-400" aria-hidden />
            <input
              autoFocus
              role="combobox"
              aria-expanded
              aria-controls="palette-list"
              aria-activedescendant={items[active] ? `palette-${active}` : undefined}
              className="w-full bg-transparent py-3 text-base outline-none"
              placeholder={P.placeholder}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <ul ref={listRef} id="palette-list" role="listbox" className="max-h-[50vh] overflow-y-auto p-1">
            {items.length === 0 && <li className="px-3 py-6 text-center text-zinc-500">{P.empty}</li>}
            {items.map((item, i) => {
              const header = item.group !== lastGroup;
              lastGroup = item.group;
              const Icon = item.icon;
              return (
                <li key={item.id} role="presentation">
                  {header && <div className="px-2 pt-2 pb-1 text-[11px] font-semibold tracking-wide text-zinc-500 uppercase">{P.groups[item.group]}</div>}
                  <div
                    id={`palette-${i}`}
                    data-index={i}
                    role="option"
                    aria-selected={i === active}
                    className="menu-item cursor-pointer"
                    onMouseMove={() => setActive(i)}
                    onClick={() => choose(item)}
                  >
                    <Icon className="size-4 shrink-0 text-zinc-500" aria-hidden />
                    <span className="min-w-0 flex-1 truncate">{item.label}</span>
                    {item.hint && <span className="max-w-[40%] truncate text-xs text-zinc-500">{item.hint}</span>}
                  </div>
                </li>
              );
            })}
          </ul>
          <div className="border-t border-zinc-200 px-3 py-2 text-xs text-zinc-500 dark:border-zinc-800">{P.hint}</div>
        </div>
      )}
    </dialog>
    </>
  );
}
