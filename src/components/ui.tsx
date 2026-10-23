import { ReactNode } from "react";
import { AlertCircle, Inbox } from "lucide-react";

export function PageHeader({ title, subtitle, actions }: { title: string; subtitle?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="mb-5 flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        {subtitle && <div className="mt-1 text-zinc-500 dark:text-zinc-400">{subtitle}</div>}
      </div>
      {actions && <div className="flex flex-wrap gap-2">{actions}</div>}
    </header>
  );
}

export function ErrorBanner({ message, onClose }: { message: string | null; onClose?: () => void }) {
  if (!message) return null;
  return (
    <div
      role="alert"
      className="mb-4 flex items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-red-700 dark:text-red-300"
    >
      <AlertCircle className="mt-0.5 size-4 shrink-0" />
      <span className="flex-1 select-text">{message}</span>
      {onClose && (
        <button className="text-xs opacity-70 hover:opacity-100" onClick={onClose} aria-label="Скрыть">
          ✕
        </button>
      )}
    </div>
  );
}

/** Simple empty state (text only); use EmptyState for icon + action. */
export function Empty({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-2 rounded-xl border border-dashed border-zinc-300 p-8 text-center text-zinc-500 dark:border-zinc-700">
      <Inbox className="size-6 text-zinc-400" aria-hidden />
      <div>{children}</div>
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition
        focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-500
        disabled:cursor-not-allowed disabled:opacity-50
        ${checked ? "bg-accent-600" : "bg-zinc-300 dark:bg-zinc-700"}`}
    >
      <span
        className={`inline-block size-4 rounded-full bg-white shadow transition-transform ${checked ? "translate-x-4.5" : "translate-x-0.5"}`}
      />
    </button>
  );
}

/** A labelled settings row with a control on the right. */
export function Row({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-4 py-3">
      <div className="min-w-0">
        <div className="font-medium">{title}</div>
        {hint && <div className="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-zinc-600 dark:text-zinc-400">
      {label}
      {children}
    </label>
  );
}

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "accent" | "ok" | "warn" | "error" }) {
  const tones = {
    neutral: "bg-zinc-200 text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300",
    accent: "bg-accent-500/15 text-accent-600 dark:text-accent-400",
    ok: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-400",
    warn: "bg-amber-500/15 text-amber-700 dark:text-amber-400",
    error: "bg-red-500/15 text-red-700 dark:text-red-400",
  };
  return <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${tones[tone]}`}>{children}</span>;
}

// --- design system additions (docs/design-system.md) ---

/** Hover/focus hint for icon buttons and truncated text. Wraps one focusable child. */
export function Tooltip({ text, children, side = "top" }: { text: string; children: ReactNode; side?: "top" | "bottom" | "right" }) {
  const pos = { top: "bottom-full left-1/2 mb-1.5 -translate-x-1/2", bottom: "top-full left-1/2 mt-1.5 -translate-x-1/2", right: "left-full top-1/2 ml-2 -translate-y-1/2" }[side];
  return (
    <span className="group/tip relative inline-flex">
      {children}
      <span
        role="tooltip"
        className={`pointer-events-none absolute z-50 w-max max-w-64 rounded-md bg-zinc-900 px-2 py-1 text-xs font-normal text-white opacity-0 shadow-lg transition-opacity delay-300
          group-hover/tip:opacity-100 group-focus-within/tip:opacity-100 dark:bg-zinc-100 dark:text-zinc-900 ${pos}`}
      >
        {text}
      </span>
    </span>
  );
}

/** Icon-only button with a tooltip and an accessible name. */
export function IconButton({ label, onClick, children, disabled, side }: { label: string; onClick: () => void; children: ReactNode; disabled?: boolean; side?: "top" | "bottom" | "right" }) {
  return (
    <Tooltip text={label} side={side}>
      <button type="button" className="icon-btn" aria-label={label} onClick={onClick} disabled={disabled}>
        {children}
      </button>
    </Tooltip>
  );
}

export function Spinner({ label = "Загрузка…" }: { label?: string }) {
  return (
    <span role="status" className="inline-flex items-center gap-2 text-zinc-500">
      <span className="size-4 animate-spin rounded-full border-2 border-zinc-300 border-t-accent-500 dark:border-zinc-700" />
      <span className="sr-only">{label}</span>
    </span>
  );
}

/** Empty state with an icon, explanation and an optional next step. */
export function EmptyState({ icon: Icon, title, hint, action }: { icon?: typeof AlertCircle; title: string; hint?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-2 rounded-xl border border-dashed border-zinc-300 px-6 py-10 text-center dark:border-zinc-700">
      {Icon && <Icon className="size-8 text-zinc-400" aria-hidden />}
      <div className="font-medium">{title}</div>
      {hint && <div className="max-w-md text-zinc-500 dark:text-zinc-400">{hint}</div>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}

/** Error state for a failed load, with retry. */
export function ErrorState({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div role="alert" className="flex flex-col items-center gap-2 rounded-xl border border-red-500/30 bg-red-500/5 px-6 py-8 text-center">
      <AlertCircle className="size-7 text-red-500" aria-hidden />
      <div className="font-medium">Не удалось загрузить данные</div>
      <div className="max-w-md text-zinc-600 select-text dark:text-zinc-300">{message}</div>
      {onRetry && (
        <button className="btn-secondary mt-2" onClick={onRetry}>
          Повторить
        </button>
      )}
    </div>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="kbd">{children}</kbd>;
}

/** Accessible tabs: arrows / Home / End move between tabs. */
export function Tabs<K extends string>({ tabs, value, onChange, label }: { tabs: { key: K; label: ReactNode }[]; value: K; onChange: (k: K) => void; label: string }) {
  function onKey(e: React.KeyboardEvent, i: number) {
    const n = tabs.length;
    const next = { ArrowRight: (i + 1) % n, ArrowLeft: (i - 1 + n) % n, Home: 0, End: n - 1 }[e.key];
    if (next === undefined) return;
    e.preventDefault();
    onChange(tabs[next].key);
    (e.currentTarget.parentElement?.children[next] as HTMLElement | undefined)?.focus();
  }
  return (
    <div role="tablist" aria-label={label} className="flex flex-wrap gap-1 border-b border-zinc-200 dark:border-zinc-800">
      {tabs.map((tab, i) => (
        <button
          key={tab.key}
          type="button"
          role="tab"
          aria-selected={value === tab.key}
          tabIndex={value === tab.key ? 0 : -1}
          onClick={() => onChange(tab.key)}
          onKeyDown={(e) => onKey(e, i)}
          className={`-mb-px border-b-2 px-3 py-2 font-medium transition ${
            value === tab.key ? "border-accent-500 text-accent-600 dark:text-accent-400" : "border-transparent text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200"
          }`}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
