import { useState } from "react";
import { Check, Copy, Plus, X } from "lucide-react";
import { t } from "../../i18n";
import { ApiPair } from "../../lib/api";

/** Unified diff with added/removed/hunk lines coloured. */
export function DiffView({ text }: { text: string }) {
  if (!text.trim()) return <p className="p-3 text-zinc-500">{t.dev.git.noChanges}</p>;
  const lines = text.split("\n");
  return (
    <pre className="overflow-auto p-2 font-mono text-xs leading-5 select-text">
      {lines.map((l, i) => {
        const tone =
          l.startsWith("+++") || l.startsWith("---") || l.startsWith("diff ") || l.startsWith("index ")
            ? "font-semibold text-zinc-500"
            : l.startsWith("+")
              ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-400"
              : l.startsWith("-")
                ? "bg-red-500/10 text-red-700 dark:text-red-400"
                : l.startsWith("@@")
                  ? "text-sky-600 dark:text-sky-400"
                  : "";
        return (
          <div key={i} className={`whitespace-pre px-1 ${tone}`}>
            {l || " "}
          </div>
        );
      })}
    </pre>
  );
}

export function CopyButton({ text, label }: { text: string; label?: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      className="btn-ghost"
      title={t.dev.copy}
      onClick={async () => {
        await navigator.clipboard.writeText(text);
        setDone(true);
        setTimeout(() => setDone(false), 1500);
      }}
    >
      {done ? <Check className="size-4 text-emerald-500" /> : <Copy className="size-4" />}
      {label && <span>{done ? t.dev.copied : label}</span>}
    </button>
  );
}

/** Name/value rows with an enabled checkbox (headers, query, variables). */
export function PairsEditor({ rows, onChange }: { rows: ApiPair[]; onChange: (rows: ApiPair[]) => void }) {
  const A = t.dev.api;
  const set = (i: number, patch: Partial<ApiPair>) => onChange(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  return (
    <div className="flex flex-col gap-1.5">
      {rows.map((r, i) => (
        <div key={i} className="flex items-center gap-1.5">
          <input type="checkbox" checked={r.enabled} onChange={(e) => set(i, { enabled: e.target.checked })} aria-label={A.name} />
          <input className="input flex-1 font-mono text-xs" placeholder={A.name} value={r.name} onChange={(e) => set(i, { name: e.target.value })} />
          <input className="input flex-[2] font-mono text-xs" placeholder={A.value} value={r.value} onChange={(e) => set(i, { value: e.target.value })} />
          <button className="btn-ghost px-1.5" onClick={() => onChange(rows.filter((_, j) => j !== i))} aria-label={t.common.delete}>
            <X className="size-4" />
          </button>
        </div>
      ))}
      <button className="btn-ghost self-start" onClick={() => onChange([...rows, { name: "", value: "", enabled: true }])}>
        <Plus className="size-4" />
        {A.addRow}
      </button>
    </div>
  );
}

export const newId = () => (crypto.randomUUID ? crypto.randomUUID() : String(Date.now() + Math.random()));
