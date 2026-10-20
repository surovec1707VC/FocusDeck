import { useEffect, useRef, useState } from "react";
import { AlertCircle, ArrowLeftToLine, CheckCircle2, Search } from "lucide-react";
import { Badge, ErrorBanner } from "../../components/ui";
import { t } from "../../i18n";
import { api, errorText, JsonOp, JsonProcessed, JsonQueryResult } from "../../lib/api";
import { CopyButton } from "./common";

const SAMPLE = `{
  "store": {
    "book": [
      { "title": "Рефакторинг", "author": "Фаулер", "price": 8.95 },
      { "title": "Чистый код", "author": "Мартин", "price": 12.99, "isbn": "978-5-4461" },
      { "title": "SICP", "author": "Абельсон", "price": 22.5 }
    ],
    "bicycle": { "color": "red", "price": 19.95 }
  }
}`;

export default function JsonTool() {
  const J = t.dev.json;
  const [text, setText] = useState(SAMPLE);
  const [from, setFrom] = useState<"" | "json" | "yaml">("");
  const [check, setCheck] = useState<JsonProcessed | null>(null);
  const [out, setOut] = useState<{ op: JsonOp; result: JsonProcessed } | null>(null);
  const [path, setPath] = useState("$.store.book[?(@.price < 20)].title");
  const [query, setQuery] = useState<JsonQueryResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const area = useRef<HTMLTextAreaElement>(null);
  const fmt = from || undefined;

  // Live validation.
  useEffect(() => {
    const id = setTimeout(() => api.devjsonProcess(text, "validate", fmt).then(setCheck, (e) => setError(errorText(e))), 250);
    return () => clearTimeout(id);
  }, [text, fmt]);

  useEffect(() => {
    if (!path.trim()) return setQuery(null);
    const id = setTimeout(() => api.devjsonQuery(text, path, fmt).then(setQuery, (e) => setError(errorText(e))), 250);
    return () => clearTimeout(id);
  }, [text, path, fmt]);

  async function runOp(op: JsonOp) {
    try {
      setOut({ op, result: await api.devjsonProcess(text, op, fmt) });
    } catch (e) {
      setError(errorText(e));
    }
  }

  /** Puts the cursor at the error position. */
  function goTo(line: number | null, col: number | null) {
    const el = area.current;
    if (!el || !line) return;
    const lines = text.split("\n");
    const pos = lines.slice(0, line - 1).reduce((n, l) => n + l.length + 1, 0) + Math.max(0, (col ?? 1) - 1);
    el.focus();
    el.setSelectionRange(pos, Math.min(pos + 1, text.length));
    el.scrollTop = Math.max(0, (line - 3) * 18);
  }

  const err = check?.error;
  const lineCount = text.split("\n").length;

  return (
    <div className="flex flex-col gap-3">
      <ErrorBanner message={error} onClose={() => setError(null)} />
      <div className="flex flex-wrap items-center gap-2">
        <select className="input w-32" value={from} onChange={(e) => setFrom(e.target.value as "" | "json" | "yaml")} aria-label={J.input}>
          <option value="">{J.auto}</option>
          <option value="json">JSON</option>
          <option value="yaml">YAML</option>
        </select>
        <button className="btn-primary" onClick={() => runOp("format")}>
          {J.format}
        </button>
        <button className="btn-secondary" onClick={() => runOp("minify")}>
          {J.minify}
        </button>
        <button className="btn-secondary" onClick={() => runOp("to_json")}>
          {J.toJson}
        </button>
        <button className="btn-secondary" onClick={() => runOp("to_yaml")}>
          {J.toYaml}
        </button>
        <span className="flex-1" />
        {check && (
          <span className={`flex items-center gap-1.5 text-xs ${check.ok ? "text-emerald-600" : "text-red-600"}`}>
            {check.ok ? <CheckCircle2 className="size-4" /> : <AlertCircle className="size-4" />}
            {check.ok ? J.valid : J.errorAt(err?.line ?? null, err?.column ?? null)}
            <Badge>{J.detected(check.detected)}</Badge>
          </span>
        )}
      </div>

      <div className="grid gap-3 lg:grid-cols-2">
        <div className="card flex flex-col overflow-hidden">
          <div className="border-b border-zinc-200 px-3 py-1.5 text-xs font-semibold text-zinc-500 dark:border-zinc-800">
            {J.input} <span className="font-normal">· {lineCount}</span>
          </div>
          <div className="flex min-h-96 flex-1">
            <div aria-hidden className="w-10 shrink-0 overflow-hidden bg-zinc-100 py-2 pr-1.5 text-right font-mono text-xs leading-[18px] text-zinc-400 select-none dark:bg-zinc-900">
              {Array.from({ length: Math.min(lineCount, 5000) }, (_, i) => (
                <div key={i} className={err?.line === i + 1 ? "bg-red-500/20 text-red-600" : ""}>
                  {i + 1}
                </div>
              ))}
            </div>
            <textarea
              ref={area}
              className="min-w-0 flex-1 resize-none bg-transparent p-2 font-mono text-xs leading-[18px] outline-none"
              value={text}
              onChange={(e) => setText(e.target.value)}
              onScroll={(e) => {
                const gutter = e.currentTarget.previousElementSibling as HTMLElement | null;
                if (gutter) gutter.scrollTop = e.currentTarget.scrollTop;
              }}
              spellCheck={false}
              wrap="off"
            />
          </div>
          {err && (
            <button className="flex items-start gap-1.5 border-t border-red-500/30 bg-red-500/10 px-3 py-2 text-left text-xs text-red-700 dark:text-red-300" onClick={() => goTo(err.line, err.column)}>
              <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
              <span>
                {err.line && <b>{J.errorAt(err.line, err.column)}: </b>}
                {err.message}
              </span>
            </button>
          )}
          {check?.stats && <div className="border-t border-zinc-200 px-3 py-1.5 text-xs text-zinc-500 dark:border-zinc-800">{J.stats(check.stats.objects, check.stats.arrays, check.stats.values, check.stats.depth)}</div>}
        </div>

        <div className="card flex flex-col overflow-hidden">
          <div className="flex items-center gap-2 border-b border-zinc-200 px-3 py-1.5 text-xs font-semibold text-zinc-500 dark:border-zinc-800">
            {J.output}
            <span className="flex-1" />
            {out?.result.ok && out.result.output && (
              <>
                <button className="btn-ghost px-1.5 py-0.5 text-xs font-normal" onClick={() => (setText(out.result.output), setOut(null))}>
                  <ArrowLeftToLine className="size-3.5" />
                  {J.useOutput}
                </button>
                <CopyButton text={out.result.output} />
              </>
            )}
          </div>
          {out && !out.result.ok && out.result.error && (
            <div className="px-3 py-2 text-xs text-red-600">
              {J.errorAt(out.result.error.line, out.result.error.column)} {out.result.error.message}
            </div>
          )}
          <pre className="min-h-96 flex-1 overflow-auto p-2 font-mono text-xs leading-[18px] select-text">{out?.result.output}</pre>
        </div>
      </div>

      <div className="card flex flex-col gap-2 p-3">
        <div className="flex items-center gap-2">
          <Search className="size-4 text-zinc-500" />
          <span className="text-sm font-medium">{J.jsonPath}</span>
          <input className="input min-w-0 flex-1 font-mono text-xs" value={path} onChange={(e) => setPath(e.target.value)} placeholder={J.jsonPathHint} spellCheck={false} />
        </div>
        <p className="text-xs text-zinc-500">{J.jsonPathHint}</p>
        {query?.error && !query.in_document && <p className="text-xs text-red-600">{query.error.message}</p>}
        {query && !query.error && (
          <>
            <div className="text-xs text-zinc-500">{J.results(query.matches.length)}</div>
            <ul className="flex max-h-72 flex-col gap-1 overflow-auto font-mono text-xs">
              {query.matches.map((m, i) => (
                <li key={i} className="flex gap-3 rounded bg-zinc-100 px-2 py-1 dark:bg-zinc-900">
                  <span className="shrink-0 text-accent-600 dark:text-accent-400">{m.path}</span>
                  <span className="min-w-0 break-all select-text">{JSON.stringify(m.value)}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </div>
  );
}
