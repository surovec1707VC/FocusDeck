import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Flag, Pause, Pencil, Play, Plus, RotateCcw, Save, SkipForward, Trash2 } from "lucide-react";
import { formatDuration } from "../components/widgets/common";
import { Badge, ErrorBanner, Field, PageHeader } from "../components/ui";
import { formatDateTime, t } from "../i18n";
import { api, errorText, EVENTS, StopwatchResult, Timer, TimerInput, TimerSession, TimersState } from "../lib/api";
import { useTicker } from "../lib/widget-data";

const MIN = 60_000;

export default function Timers() {
  const T = t.timers;
  const [state, setState] = useState<TimersState | null>(null);
  const [sessions, setSessions] = useState<TimerSession[]>([]);
  const [results, setResults] = useState<StopwatchResult[]>([]);
  const [tab, setTab] = useState<"timers" | "stopwatches" | "history">("timers");
  const [form, setForm] = useState<{ id?: string; input: TimerInput } | null>(null);
  const [swName, setSwName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [s, h, r] = await Promise.all([api.timersState(), api.timerSessions(50), api.stopwatchResults(50)]);
      setState(s);
      setSessions(h);
      setResults(r);
    } catch (e) {
      setError(errorText(e));
    }
  }, []);

  useEffect(() => {
    load();
    const un = listen(EVENTS.timersChanged, () => load());
    return () => {
      un.then((f) => f());
    };
  }, [load]);

  // Sessions are written by actions and by the background task: refresh on open.
  useEffect(() => {
    if (tab === "history") load();
  }, [tab, load]);

  const skew = useMemo(() => (state ? state.now_ms - Date.now() : 0), [state]);
  const anyRunning = Boolean(state?.timers.some((x) => x.running) || state?.stopwatches.some((s) => s.started_at));
  useTicker(tab === "stopwatches" ? 47 : 250, anyRunning);
  const now = Date.now() + skew;

  async function run(fn: () => Promise<TimersState | unknown>) {
    try {
      const r = await fn();
      if (r && typeof r === "object" && "timers" in (r as object)) setState(r as TimersState);
      setError(null);
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  }

  async function saveTimer(e: FormEvent) {
    e.preventDefault();
    if (form && (await run(() => api.timersSave(form.input, form.id)))) setForm(null);
  }

  const remaining = (x: Timer) => (x.running ? Math.max(0, (x.ends_at ?? now) - now) : (x.remaining_ms ?? interval(x)));
  const interval = (x: Timer) => (x.kind === "countdown" ? x.duration_ms : x.phase === "work" ? x.work_ms : x.phase === "short_break" ? x.short_break_ms : x.long_break_ms);
  const minutes = (ms: number | undefined) => Math.round((ms ?? 0) / MIN);
  const setMin = (key: keyof TimerInput, v: string) => form && setForm({ ...form, input: { ...form.input, [key]: Math.max(1, Number(v) || 1) * MIN } });

  return (
    <>
      <PageHeader
        title={T.title}
        subtitle={T.subtitle}
        actions={
          state && (
            <div className="flex gap-3 text-xs text-zinc-500">
              <span>{T.pomodorosToday(state.stats.pomodoros_today)}</span>
              <span>{T.focusToday(formatDuration(state.stats.focus_ms_today))}</span>
            </div>
          )
        }
      />
      <ErrorBanner message={error} onClose={() => setError(null)} />

      <div className="mb-4 flex gap-1" role="tablist">
        {(["timers", "stopwatches", "history"] as const).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "btn-secondary" : "btn-ghost"} onClick={() => setTab(k)}>
            {T.tabs[k]}
          </button>
        ))}
      </div>

      {tab === "timers" && (
        <>
          <div className="mb-3 flex flex-wrap gap-2">
            <button className="btn-primary" onClick={() => setForm({ input: { name: T.pomodoro, kind: "pomodoro", work_ms: 25 * MIN, short_break_ms: 5 * MIN, long_break_ms: 15 * MIN, rounds: 4 } })}>
              <Plus className="size-4" /> {T.newPomodoro}
            </button>
            <button className="btn-secondary" onClick={() => setForm({ input: { name: T.timer, kind: "countdown", duration_ms: 10 * MIN } })}>
              <Plus className="size-4" /> {T.newTimer}
            </button>
          </div>

          {form && (
            <form onSubmit={saveTimer} className="card mb-4 flex flex-wrap items-end gap-3">
              <Field label={T.name}>
                <input className="input w-48" autoFocus value={form.input.name} maxLength={60} onChange={(e) => setForm({ ...form, input: { ...form.input, name: e.target.value } })} />
              </Field>
              {form.input.kind === "countdown" ? (
                <Field label={T.durationMin}>
                  <input className="input w-24" type="number" min={1} max={5999} value={minutes(form.input.duration_ms)} onChange={(e) => setMin("duration_ms", e.target.value)} />
                </Field>
              ) : (
                <>
                  <Field label={T.workMin}>
                    <input className="input w-24" type="number" min={1} max={600} value={minutes(form.input.work_ms)} onChange={(e) => setMin("work_ms", e.target.value)} />
                  </Field>
                  <Field label={T.shortMin}>
                    <input className="input w-24" type="number" min={1} max={600} value={minutes(form.input.short_break_ms)} onChange={(e) => setMin("short_break_ms", e.target.value)} />
                  </Field>
                  <Field label={T.longMin}>
                    <input className="input w-24" type="number" min={1} max={600} value={minutes(form.input.long_break_ms)} onChange={(e) => setMin("long_break_ms", e.target.value)} />
                  </Field>
                  <Field label={T.rounds}>
                    <input className="input w-20" type="number" min={1} max={12} value={form.input.rounds} onChange={(e) => setForm({ ...form, input: { ...form.input, rounds: Number(e.target.value) || 1 } })} />
                  </Field>
                </>
              )}
              <button className="btn-primary" disabled={!form.input.name.trim()}>
                {t.common.save}
              </button>
              <button type="button" className="btn-secondary" onClick={() => setForm(null)}>
                {t.common.cancel}
              </button>
              {form.id && <p className="w-full text-xs text-zinc-500">{T.editResets}</p>}
            </form>
          )}

          {state?.timers.length === 0 && <p className="text-zinc-500">{T.noTimers}</p>}
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {state?.timers.map((x) => {
              const left = remaining(x);
              const progress = 1 - left / Math.max(1, interval(x));
              return (
                <article key={x.id} className={`card flex flex-col gap-3 ${x.running ? "border-accent-500/60 dark:border-accent-500/60" : ""}`}>
                  <header className="flex items-center gap-2">
                    <h2 className="min-w-0 flex-1 truncate font-semibold">{x.name}</h2>
                    {x.kind === "pomodoro" && (
                      <Badge tone={x.phase === "work" ? "accent" : "ok"}>
                        {T.phases[x.phase]} · {x.round}/{x.rounds}
                      </Badge>
                    )}
                    <button
                      className="btn-ghost px-1.5"
                      onClick={() =>
                        setForm({ id: x.id, input: { name: x.name, kind: x.kind, duration_ms: x.duration_ms, work_ms: x.work_ms, short_break_ms: x.short_break_ms, long_break_ms: x.long_break_ms, rounds: x.rounds } })
                      }
                      aria-label={t.common.edit}
                    >
                      <Pencil className="size-4" />
                    </button>
                    <button className="btn-ghost px-1.5 hover:text-red-500" onClick={() => run(() => api.timersDelete(x.id))} aria-label={t.common.delete}>
                      <Trash2 className="size-4" />
                    </button>
                  </header>
                  <div className="text-4xl font-semibold tabular-nums">{formatDuration(left + (x.running ? 999 : 0))}</div>
                  <div className="h-1.5 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
                    <div className={`h-full ${x.phase === "work" || x.kind === "countdown" ? "bg-accent-500" : "bg-emerald-500"}`} style={{ width: `${Math.min(100, progress * 100)}%` }} />
                  </div>
                  <div className="flex flex-wrap gap-1">
                    {x.running ? (
                      <button className="btn-secondary" onClick={() => run(() => api.timersControl(x.id, "pause"))}>
                        <Pause className="size-4" /> {t.widgets.pause}
                      </button>
                    ) : (
                      <button className="btn-primary" onClick={() => run(() => api.timersControl(x.id, "start"))}>
                        <Play className="size-4" /> {t.widgets.start}
                      </button>
                    )}
                    <button className="btn-ghost" onClick={() => run(() => api.timersControl(x.id, "reset"))}>
                      <RotateCcw className="size-4" /> {t.widgets.reset}
                    </button>
                    {x.kind === "pomodoro" && (
                      <button className="btn-ghost" onClick={() => run(() => api.timersControl(x.id, "skip"))}>
                        <SkipForward className="size-4" /> {T.skip}
                      </button>
                    )}
                  </div>
                </article>
              );
            })}
          </div>
        </>
      )}

      {tab === "stopwatches" && (
        <>
          <form
            className="mb-3 flex gap-2"
            onSubmit={async (e) => {
              e.preventDefault();
              if (await run(() => api.stopwatchesCreate(swName || T.stopwatch))) setSwName("");
            }}
          >
            <input className="input max-w-xs" placeholder={T.stopwatchName} value={swName} maxLength={60} onChange={(e) => setSwName(e.target.value)} />
            <button className="btn-primary">
              <Plus className="size-4" /> {T.newStopwatch}
            </button>
          </form>
          {state?.stopwatches.length === 0 && <p className="text-zinc-500">{T.noStopwatches}</p>}
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {state?.stopwatches.map((s) => {
              const total = s.elapsed_ms + (s.started_at ? now - s.started_at : 0);
              return (
                <article key={s.id} className="card flex flex-col gap-2">
                  <header className="flex items-center gap-2">
                    <input
                      className="min-w-0 flex-1 bg-transparent font-semibold outline-none"
                      defaultValue={s.name}
                      maxLength={60}
                      onBlur={(e) => e.target.value.trim() && e.target.value !== s.name && run(() => api.stopwatchesRename(s.id, e.target.value))}
                      aria-label={T.name}
                    />
                    <button className="btn-ghost px-1.5 hover:text-red-500" onClick={() => run(() => api.stopwatchesDelete(s.id))} aria-label={t.common.delete}>
                      <Trash2 className="size-4" />
                    </button>
                  </header>
                  <div className="text-4xl font-semibold tabular-nums">{formatDuration(total, true)}</div>
                  <div className="flex flex-wrap gap-1">
                    {s.started_at ? (
                      <button className="btn-secondary" onClick={() => run(() => api.stopwatchesControl(s.id, "pause"))}>
                        <Pause className="size-4" /> {t.widgets.pause}
                      </button>
                    ) : (
                      <button className="btn-primary" onClick={() => run(() => api.stopwatchesControl(s.id, "start"))}>
                        <Play className="size-4" /> {t.widgets.start}
                      </button>
                    )}
                    <button className="btn-ghost" disabled={!s.started_at} onClick={() => run(() => api.stopwatchesControl(s.id, "lap"))}>
                      <Flag className="size-4" /> {t.widgets.lap}
                    </button>
                    <button className="btn-ghost" onClick={() => run(() => api.stopwatchesControl(s.id, "reset"))}>
                      <RotateCcw className="size-4" />
                    </button>
                    <button
                      className="btn-ghost"
                      disabled={total <= 0}
                      onClick={async () => {
                        try {
                          setResults(await api.stopwatchesSaveResult(s.id));
                          setError(null);
                        } catch (e) {
                          setError(errorText(e));
                        }
                      }}
                    >
                      <Save className="size-4" /> {T.saveResult}
                    </button>
                  </div>
                  {s.laps.length > 0 && (
                    <ol className="max-h-32 overflow-y-auto text-xs">
                      {s.laps
                        .map((lap, i) => ({ lap, i, split: lap - (s.laps[i - 1] ?? 0) }))
                        .reverse()
                        .map(({ lap, i, split }) => (
                          <li key={i} className="flex justify-between border-t border-zinc-200 py-0.5 tabular-nums dark:border-zinc-800">
                            <span className="text-zinc-500">{t.widgets.lapN(i + 1)}</span>
                            <span>+{formatDuration(split, true)}</span>
                            <span className="font-medium">{formatDuration(lap, true)}</span>
                          </li>
                        ))}
                    </ol>
                  )}
                </article>
              );
            })}
          </div>
        </>
      )}

      {tab === "history" && (
        <div className="grid gap-4 lg:grid-cols-2">
          <section className="card">
            <header className="mb-2 flex items-center justify-between">
              <h2 className="font-semibold">{T.sessions}</h2>
              {sessions.length > 0 && (
                <button className="text-xs text-zinc-500 hover:text-red-500" onClick={() => run(() => api.timerSessionsClear()).then(load)}>
                  {T.clearHistory}
                </button>
              )}
            </header>
            {sessions.length === 0 && <p className="text-zinc-500">{T.noSessions}</p>}
            <ul className="divide-y divide-zinc-200 text-sm dark:divide-zinc-800">
              {sessions.map((s) => (
                <li key={s.id} className="flex flex-wrap items-center gap-2 py-1.5">
                  <Badge tone={s.completed ? "ok" : "warn"}>{s.kind === "countdown" ? T.timer : T.phases[s.kind]}</Badge>
                  <span className="flex-1 truncate">{s.timer_name}</span>
                  <span className="tabular-nums">{formatDuration(s.actual_ms)}</span>
                  <span className="text-xs text-zinc-500">{formatDateTime(s.ended_at)}</span>
                </li>
              ))}
            </ul>
          </section>
          <section className="card">
            <h2 className="mb-2 font-semibold">{T.results}</h2>
            {results.length === 0 && <p className="text-zinc-500">{T.noResults}</p>}
            <ul className="divide-y divide-zinc-200 text-sm dark:divide-zinc-800">
              {results.map((r) => (
                <li key={r.id} className="flex flex-wrap items-center gap-2 py-1.5">
                  <span className="flex-1 truncate">{r.name}</span>
                  <span className="text-xs text-zinc-500">{T.laps((JSON.parse(r.laps_json) as number[]).length)}</span>
                  <span className="font-medium tabular-nums">{formatDuration(r.total_ms, true)}</span>
                  <span className="text-xs text-zinc-500">{formatDateTime(r.saved_at)}</span>
                  <button className="btn-ghost px-1 hover:text-red-500" onClick={async () => setResults(await api.stopwatchResultsDelete(r.id))} aria-label={t.common.delete}>
                    <Trash2 className="size-3.5" />
                  </button>
                </li>
              ))}
            </ul>
          </section>
        </div>
      )}
    </>
  );
}
