import { ReactNode, useEffect, useRef, useState } from "react";
import { BookOpen, Save, Trash2 } from "lucide-react";
import { ErrorBanner, Field } from "../../components/ui";
import { t } from "../../i18n";
import { api, errorText, RegexInput, SavedRegex } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import type { RegexJob, RegexMatch, RegexResult } from "../../lib/regex-worker";

const TEMPLATES: { name: string; pattern: string; flags: string; sample: string }[] = [
  { name: "Email", pattern: String.raw`(?<user>[\w.+-]+)@(?<domain>[\w-]+(?:\.[\w-]+)+)`, flags: "g", sample: "Пишите на support@focusdeck.local или ivan.petrov+news@mail.example.org" },
  { name: "URL", pattern: String.raw`(?<scheme>https?):\/\/(?<host>[^\s/:?#]+)(?::(?<port>\d+))?(?<path>[^\s?#]*)(?:\?(?<query>[^\s#]*))?`, flags: "gi", sample: "Документация: https://tauri.app/start/ и http://localhost:1420/dev?tab=git" },
  { name: "IPv4", pattern: String.raw`\b(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)\b`, flags: "g", sample: "Сервер 192.168.0.10, шлюз 10.0.0.1, неверный 999.1.1.1" },
  { name: "Дата ДД.ММ.ГГГГ", pattern: String.raw`\b(?<day>0[1-9]|[12]\d|3[01])\.(?<month>0[1-9]|1[0-2])\.(?<year>\d{4})\b`, flags: "g", sample: "Релиз 20.10.2026, ревью 01.11.2026, ошибка 32.13.2026" },
  { name: "ISO-дата и время", pattern: String.raw`(?<date>\d{4}-\d{2}-\d{2})[T ](?<time>\d{2}:\d{2}(?::\d{2})?)(?<tz>Z|[+-]\d{2}:?\d{2})?`, flags: "g", sample: "created_at: 2026-10-20T14:30:00+03:00, updated 2026-10-21 09:15" },
  { name: "Телефон (РФ)", pattern: String.raw`(?:\+7|8)[\s(-]*(?<code>\d{3})[\s)-]*(?<n1>\d{3})[\s-]*(?<n2>\d{2})[\s-]*(?<n3>\d{2})`, flags: "g", sample: "+7 (912) 345-67-89, 8 800 555 35 35" },
  { name: "UUID", pattern: String.raw`\b[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b`, flags: "gi", sample: "id: 3f2b1c9e-8d4a-4f6b-9c1d-2e5f7a8b9c0d" },
  { name: "HEX-цвет", pattern: String.raw`#(?:[0-9a-f]{3}){1,2}\b`, flags: "gi", sample: "color: #1E90FF; border: #fff; bad: #12345" },
  { name: "Semver", pattern: String.raw`\bv?(?<major>0|[1-9]\d*)\.(?<minor>0|[1-9]\d*)\.(?<patch>0|[1-9]\d*)(?:-(?<pre>[\w.-]+))?\b`, flags: "g", sample: "tauri 2.9.1, react v19.1.0, beta 1.0.0-rc.1" },
  { name: "Git-коммит (Conventional)", pattern: String.raw`^(?<type>feat|fix|docs|chore|refactor|test|perf)(?:\((?<scope>[^)]+)\))?(?<breaking>!)?: (?<subject>.+)$`, flags: "gm", sample: "feat(git): commit dialog\nfix!: drop old api\nupdate readme" },
  { name: "Пробелы в конце строк", pattern: String.raw`[ \t]+$`, flags: "gm", sample: "строка с пробелами   \nчистая строка\nтаб\t" },
  { name: "Путь Windows", pattern: String.raw`\b[A-Z]:\\(?:[^\\/:*?"<>|\r\n]+\\)*[^\\/:*?"<>|\r\n]*`, flags: "gi", sample: String.raw`Проект в E:\ProjectFocusDeck\src-tauri\src\lib.rs и C:\Windows` },
];

const COLORS = ["bg-amber-300/60 dark:bg-amber-500/40", "bg-sky-300/60 dark:bg-sky-500/40", "bg-emerald-300/60 dark:bg-emerald-500/40", "bg-fuchsia-300/60 dark:bg-fuchsia-500/40", "bg-orange-300/60 dark:bg-orange-500/40", "bg-teal-300/60 dark:bg-teal-500/40"];
const FLAGS = ["g", "i", "m", "s", "u", "y"];

/** The sample text with matches underlined and capture groups coloured. */
function highlight(text: string, matches: RegexMatch[]): ReactNode[] {
  const marks = new Set<number>([0, text.length]);
  for (const m of matches) {
    marks.add(m.start).add(m.end);
    for (const g of m.groups) if (g.start !== null && g.end !== null) marks.add(g.start).add(g.end);
  }
  const cuts = [...marks].filter((x) => x <= text.length).sort((a, b) => a - b);
  const out: ReactNode[] = [];
  let mi = 0;
  for (let i = 0; i < cuts.length - 1; i++) {
    const [a, b] = [cuts[i], cuts[i + 1]];
    if (a === b) continue;
    while (mi < matches.length && matches[mi].end <= a && matches[mi].end !== matches[mi].start) mi++;
    const m = matches[mi] && matches[mi].start <= a && b <= matches[mi].end ? matches[mi] : null;
    const piece = text.slice(a, b);
    if (!m) {
      out.push(piece);
      continue;
    }
    // The innermost (last opened) group covering this piece decides the colour.
    const g = [...m.groups].reverse().find((g) => g.start !== null && g.end !== null && g.start <= a && b <= g.end);
    out.push(
      <mark key={i} className={`rounded-sm text-inherit ${g ? COLORS[(g.index - 1) % COLORS.length] : "bg-accent-500/20"} border-b-2 border-accent-500`} title={g ? `${t.dev.regex.group} ${g.name ?? g.index}` : t.dev.regex.match}>
        {piece}
      </mark>,
    );
  }
  return out;
}

export default function RegexTool() {
  const R = t.dev.regex;
  const settings = useModules().settingsOf("dev_regex");
  const timeoutMs = Number(settings.timeout_ms ?? 1000);
  const maxMatches = Number(settings.max_matches ?? 1000);
  const [input, setInput] = useState<RegexInput>({ name: "", pattern: TEMPLATES[0].pattern, flags: "g", sample: TEMPLATES[0].sample, replacement: "" });
  const [savedId, setSavedId] = useState<string | undefined>();
  const [saved, setSaved] = useState<SavedRegex[]>([]);
  const [result, setResult] = useState<RegexResult | null>(null);
  const [timedOut, setTimedOut] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const worker = useRef<Promise<Worker> | null>(null);
  const jobId = useRef(0);

  useEffect(() => {
    api.regexList().then(setSaved, (e) => setError(errorText(e)));
    return () => {
      worker.current?.then((w) => w.terminate());
      worker.current = null;
    };
  }, []);

  // Live matching (debounced); a worker that doesn't answer in time is replaced.
  useEffect(() => {
    const id = ++jobId.current;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let cancelled = false;
    const start = setTimeout(async () => {
      if (!input.pattern) return setResult(null);
      const w = await ready();
      if (cancelled || id !== jobId.current) return;
      w.onmessage = (e: MessageEvent<RegexResult>) => {
        if (e.data.id !== jobId.current) return;
        clearTimeout(timer);
        setTimedOut(false);
        setResult(e.data);
      };
      // The timeout counts only the matching itself, not loading the worker.
      timer = setTimeout(() => {
        w.terminate();
        worker.current = null;
        setTimedOut(true);
        setResult(null);
      }, timeoutMs);
      const job: RegexJob = { id, pattern: input.pattern, flags: input.flags.replace("d", ""), text: input.sample, replacement: input.replacement || null, max: maxMatches };
      w.postMessage(job);
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(start);
      clearTimeout(timer);
    };
  }, [input.pattern, input.flags, input.sample, input.replacement, timeoutMs, maxMatches]);

  /** The worker, once it has loaded. */
  function ready(): Promise<Worker> {
    if (worker.current) return worker.current;
    const w = new Worker(new URL("../../lib/regex-worker.ts", import.meta.url), { type: "module" });
    worker.current = new Promise((resolve) => {
      w.onmessage = (e) => e.data?.ready && resolve(w);
    });
    return worker.current;
  }

  const set = (patch: Partial<RegexInput>) => setInput((i) => ({ ...i, ...patch }));
  const toggleFlag = (f: string) => set({ flags: input.flags.includes(f) ? input.flags.replace(f, "") : input.flags + f });

  async function save() {
    try {
      const name = input.name.trim() || input.pattern.slice(0, 40);
      const r = await api.regexSave({ ...input, name }, savedId);
      setSavedId(r.id);
      set({ name: r.name });
      setSaved(await api.regexList());
    } catch (e) {
      setError(errorText(e));
    }
  }

  return (
    <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_16rem]">
      <div className="flex min-w-0 flex-col gap-3">
        <ErrorBanner message={error} onClose={() => setError(null)} />
        <div className="card flex flex-col gap-3 p-3">
          <Field label={R.pattern}>
            <div className="flex items-center gap-1 font-mono">
              <span className="text-zinc-400">/</span>
              <input className="input min-w-0 flex-1 font-mono text-sm" value={input.pattern} onChange={(e) => set({ pattern: e.target.value })} spellCheck={false} />
              <span className="text-zinc-400">/{input.flags}</span>
            </div>
          </Field>
          <div className="flex flex-wrap gap-3 text-xs">
            {FLAGS.map((f) => (
              <label key={f} className="flex items-center gap-1" title={R.flagNames[f]}>
                <input type="checkbox" checked={input.flags.includes(f)} onChange={() => toggleFlag(f)} />
                <code>{f}</code>
                <span className="text-zinc-500">{R.flagNames[f]}</span>
              </label>
            ))}
          </div>
          {result?.error && <ErrorBanner message={result.error} />}
          {timedOut && <ErrorBanner message={R.timeout(timeoutMs)} />}
          <Field label={R.sample}>
            <textarea className="input min-h-32 font-mono text-xs" value={input.sample} onChange={(e) => set({ sample: e.target.value })} spellCheck={false} />
          </Field>
          <div>
            <div className="mb-1 flex items-center gap-2 text-xs text-zinc-500">
              {result && !result.error && (result.matches.length ? R.matches(result.matches.length) : R.noMatches)}
              {result && <span>· {result.ms} мс</span>}
              {result?.truncated && <span className="text-amber-600">{R.truncated(maxMatches)}</span>}
            </div>
            <div className="max-h-64 overflow-auto rounded-md border border-zinc-200 bg-white p-2 font-mono text-xs leading-6 whitespace-pre-wrap select-text dark:border-zinc-800 dark:bg-zinc-950">
              {result && !result.error ? highlight(input.sample, result.matches) : input.sample}
            </div>
          </div>
          <Field label={R.replacement}>
            <input className="input font-mono text-xs" value={input.replacement} onChange={(e) => set({ replacement: e.target.value })} spellCheck={false} />
          </Field>
          {input.replacement && result?.replaced !== null && result?.replaced !== undefined && (
            <Field label={R.replaced}>
              <pre className="max-h-40 overflow-auto rounded-md bg-zinc-100 p-2 font-mono text-xs whitespace-pre-wrap select-text dark:bg-zinc-900">{result.replaced}</pre>
            </Field>
          )}
        </div>

        {result && result.matches.length > 0 && (
          <div className="card overflow-auto p-3">
            <table className="w-full text-left text-xs">
              <thead className="text-zinc-500">
                <tr>
                  <th className="py-1 pr-3">#</th>
                  <th className="py-1 pr-3">{R.match}</th>
                  <th className="py-1 pr-3">[start, end)</th>
                  <th className="py-1">{R.group}</th>
                </tr>
              </thead>
              <tbody className="font-mono">
                {result.matches.slice(0, 200).map((m, i) => (
                  <tr key={i} className="border-t border-zinc-200 align-top dark:border-zinc-800">
                    <td className="py-1 pr-3 text-zinc-500">{i + 1}</td>
                    <td className="py-1 pr-3 break-all">{m.text || <span className="text-zinc-400">""</span>}</td>
                    <td className="py-1 pr-3 text-zinc-500">
                      {m.start}–{m.end}
                    </td>
                    <td className="py-1">
                      {m.groups.map((g) => (
                        <div key={g.index} className="flex gap-1.5">
                          <span className={`rounded-sm px-1 ${COLORS[(g.index - 1) % COLORS.length]}`}>{g.name ?? g.index}</span>
                          <span className="break-all">{g.text ?? <span className="text-zinc-400">undefined</span>}</span>
                        </div>
                      ))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <aside className="flex flex-col gap-3">
        <div className="card flex flex-col gap-2 p-3">
          <input className="input text-sm" placeholder={R.name} value={input.name} onChange={(e) => set({ name: e.target.value })} />
          <div className="flex gap-1">
            <button className="btn-primary flex-1" onClick={save}>
              <Save className="size-4" />
              {t.dev.save}
            </button>
            {savedId && (
              <button className="btn-ghost" onClick={() => (setSavedId(undefined), set({ name: "" }))}>
                {t.dev.saveAs}
              </button>
            )}
          </div>
        </div>
        <div className="card p-3">
          <h3 className="mb-2 text-xs font-semibold tracking-wide text-zinc-500 uppercase">{R.saved}</h3>
          {saved.length === 0 && <p className="text-xs text-zinc-500">{R.savedEmpty}</p>}
          <ul className="flex flex-col">
            {saved.map((s) => (
              <li key={s.id} className="group flex items-center">
                <button
                  className={`min-w-0 flex-1 rounded px-1.5 py-1 text-left ${savedId === s.id ? "bg-accent-500/10" : "hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"}`}
                  onClick={() => {
                    setSavedId(s.id);
                    setInput({ name: s.name, pattern: s.pattern, flags: s.flags, sample: s.sample, replacement: s.replacement });
                  }}
                >
                  <div className="truncate text-sm">{s.name}</div>
                  <code className="block truncate text-xs text-zinc-500">
                    /{s.pattern}/{s.flags}
                  </code>
                </button>
                <button
                  className="px-1 text-red-500 opacity-0 group-hover:opacity-60 hover:opacity-100"
                  title={t.common.delete}
                  onClick={async () => {
                    await api.regexDelete(s.id);
                    if (savedId === s.id) setSavedId(undefined);
                    setSaved(await api.regexList());
                  }}
                >
                  <Trash2 className="size-3.5" />
                </button>
              </li>
            ))}
          </ul>
        </div>
        <div className="card p-3">
          <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold tracking-wide text-zinc-500 uppercase">
            <BookOpen className="size-3.5" />
            {R.templates}
          </h3>
          <ul className="flex flex-col">
            {TEMPLATES.map((tpl) => (
              <li key={tpl.name}>
                <button
                  className="w-full truncate rounded px-1.5 py-1 text-left text-sm hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"
                  title={tpl.pattern}
                  onClick={() => {
                    setSavedId(undefined);
                    setInput({ name: tpl.name, pattern: tpl.pattern, flags: tpl.flags, sample: tpl.sample, replacement: "" });
                  }}
                >
                  {tpl.name}
                </button>
              </li>
            ))}
          </ul>
        </div>
      </aside>
    </div>
  );
}
