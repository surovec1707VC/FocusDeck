/** Time widgets: clock, date, calendar, countdown timer, stopwatch. */
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Flag, Pause, Play, RotateCcw, Trash2 } from "lucide-react";
import { locale, t } from "../../i18n";
import { useModules } from "../../lib/modules-context";
import { useTicker, useWidgetData } from "../../lib/widget-data";
import { BodyProps, DataStatus, formatDuration, IconButton } from "./common";

interface TimeData {
  epoch_ms: number;
  iso_week: number;
  day_of_year: number;
}

/** Offset between the backend clock and this WebView, measured on each refresh. */
function useServerNow(widget: BodyProps["widget"], tickMs: number) {
  const { data, meta, error } = useWidgetData<TimeData>(widget);
  const skew = useMemo(() => (data ? data.epoch_ms - Date.now() : 0), [data]);
  useTicker(tickMs);
  return { now: new Date(Date.now() + skew), data, meta, error };
}

export function ClockWidget({ widget }: BodyProps) {
  const { show_seconds = true, show_date = true, hour12 = false } = useModules().settingsOf("widget_clock");
  const { now } = useServerNow(widget, show_seconds ? 1000 : 10_000);
  return (
    <div className="flex h-full flex-col justify-center">
      <div className="text-4xl font-semibold tracking-tight tabular-nums">
        {now.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit", hour12: Boolean(hour12), ...(show_seconds ? { second: "2-digit" } : {}) })}
      </div>
      {Boolean(show_date) && (
        <div className="mt-1 text-zinc-500 first-letter:uppercase">{now.toLocaleDateString(locale, { weekday: "long", day: "numeric", month: "long" })}</div>
      )}
    </div>
  );
}

export function DateWidget({ widget }: BodyProps) {
  const { show_weekday = true, show_week_number = true } = useModules().settingsOf("widget_date");
  const { now, data } = useServerNow(widget, 30_000);
  const W = t.widgets;
  return (
    <div className="flex h-full items-center gap-4">
      <div className="text-5xl font-semibold tabular-nums">{now.getDate()}</div>
      <div className="min-w-0">
        <div className="font-medium first-letter:uppercase">{now.toLocaleDateString(locale, { month: "long", year: "numeric" })}</div>
        {Boolean(show_weekday) && <div className="text-zinc-500 first-letter:uppercase">{now.toLocaleDateString(locale, { weekday: "long" })}</div>}
        {Boolean(show_week_number) && data && (
          <div className="text-xs text-zinc-500">
            {W.week(data.iso_week)} · {W.dayOfYear(data.day_of_year)}
          </div>
        )}
      </div>
    </div>
  );
}

// --- calendar ---

interface CalEvent {
  id: string;
  title: string;
  date: string;
  time: string | null;
  note: string;
}

interface CalendarData {
  month: string;
  today: string;
  events: CalEvent[];
  upcoming: CalEvent[];
}

const ymd = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

export function CalendarWidget({ widget }: BodyProps) {
  const { week_starts = "monday", show_events = true } = useModules().settingsOf("widget_calendar");
  const [month, setMonth] = useState(() => ymd(new Date()).slice(0, 7));
  const [selected, setSelected] = useState(() => ymd(new Date()));
  const [title, setTitle] = useState("");
  const [time, setTime] = useState("");
  const { data, meta, error, act } = useWidgetData<CalendarData>(widget, { params: { month } });
  const W = t.widgets;

  const [y, m] = month.split("-").map(Number);
  const first = new Date(y, m - 1, 1);
  const offset = (first.getDay() - (week_starts === "sunday" ? 0 : 1) + 7) % 7;
  const daysInMonth = new Date(y, m, 0).getDate();
  const cells: (number | null)[] = [...Array(offset).fill(null), ...Array.from({ length: daysInMonth }, (_, i) => i + 1)];
  const weekdays = Array.from({ length: 7 }, (_, i) => new Date(2024, 0, (week_starts === "sunday" ? 7 : 1) + i).toLocaleDateString(locale, { weekday: "short" }));
  const eventsOn = (date: string) => data?.events.filter((e) => e.date === date) ?? [];
  const shift = (delta: number) => {
    const d = new Date(y, m - 1 + delta, 1);
    setMonth(ymd(d).slice(0, 7));
  };

  async function add(e: FormEvent) {
    e.preventDefault();
    if (await act("add", { title, date: selected, time: time || null })) {
      setTitle("");
      setTime("");
    }
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <IconButton label={W.prevMonth} onClick={() => shift(-1)}>
          <ChevronLeft className="size-4" />
        </IconButton>
        <span className="font-medium first-letter:uppercase">{first.toLocaleDateString(locale, { month: "long", year: "numeric" })}</span>
        <IconButton label={W.nextMonth} onClick={() => shift(1)}>
          <ChevronRight className="size-4" />
        </IconButton>
      </div>
      <div className="grid grid-cols-7 gap-0.5 text-center text-xs">
        {weekdays.map((d) => (
          <div key={d} className="py-0.5 text-zinc-500">
            {d}
          </div>
        ))}
        {cells.map((day, i) => {
          if (day === null) return <div key={`e${i}`} />;
          const date = `${month}-${String(day).padStart(2, "0")}`;
          const has = Boolean(show_events) && eventsOn(date).length > 0;
          return (
            <button
              key={date}
              onClick={() => setSelected(date)}
              className={`relative rounded py-1 tabular-nums transition hover:bg-zinc-200 dark:hover:bg-zinc-800 ${
                date === selected ? "bg-accent-500/20 font-semibold text-accent-600 dark:text-accent-400" : ""
              } ${date === data?.today ? "ring-1 ring-accent-500" : ""}`}
            >
              {day}
              {has && <span className="absolute bottom-0.5 left-1/2 size-1 -translate-x-1/2 rounded-full bg-accent-500" />}
            </button>
          );
        })}
      </div>
      {Boolean(show_events) && (
        <>
          <ul className="flex flex-col gap-0.5">
            {eventsOn(selected).map((ev) => (
              <li key={ev.id} className="group flex items-center gap-2 text-xs">
                <span className="w-10 shrink-0 text-zinc-500 tabular-nums">{ev.time ?? "—"}</span>
                <span className="flex-1 truncate">{ev.title}</span>
                <span className="opacity-0 group-hover:opacity-100 focus-within:opacity-100">
                  <IconButton label={t.common.delete} danger onClick={() => act("delete", { id: ev.id })}>
                    <Trash2 className="size-3" />
                  </IconButton>
                </span>
              </li>
            ))}
            {eventsOn(selected).length === 0 && <li className="text-xs text-zinc-500">{W.noEvents}</li>}
          </ul>
          <form onSubmit={add} className="flex gap-1">
            <input className="input min-w-0 flex-1 py-1 text-xs" placeholder={W.eventPlaceholder} value={title} maxLength={100} onChange={(e) => setTitle(e.target.value)} />
            <input className="input w-20 py-1 text-xs" type="time" value={time} onChange={(e) => setTime(e.target.value)} aria-label="Время" />
          </form>
        </>
      )}
      <DataStatus meta={meta} error={error} />
    </div>
  );
}

// --- countdown ---

interface CountdownData {
  duration_ms: number;
  label: string;
  running: boolean;
  ends_at?: number;
  remaining_ms: number;
  finished: boolean;
  now_ms: number;
}

const PRESETS_MIN = [5, 15, 25];

export function CountdownWidget({ widget }: BodyProps) {
  const { data, meta, error, act, reload } = useWidgetData<CountdownData>(widget);
  const skew = useMemo(() => (data ? data.now_ms - Date.now() : 0), [data]);
  useTicker(250, Boolean(data?.running));
  const W = t.widgets;
  const now = Date.now() + skew;
  const remaining = !data ? 0 : data.running ? Math.max(0, (data.ends_at ?? now) - now) : data.remaining_ms;
  // The backend finishes the timer (and sends the notification); ask once per run.
  const asked = useRef<number | null>(null);
  const due = Boolean(data?.running) && remaining <= 0;
  useEffect(() => {
    if (due && asked.current !== data?.ends_at) {
      asked.current = data?.ends_at ?? null;
      reload();
    }
  }, [due, data?.ends_at, reload]);
  if (!data) return <DataStatus meta={meta} error={error} />;
  const progress = data.duration_ms > 0 ? 1 - remaining / data.duration_ms : 0;

  return (
    <div className="flex h-full flex-col justify-between gap-2">
      <div>
        {data.label && <div className="truncate text-xs text-zinc-500">{data.label}</div>}
        <div className={`text-4xl font-semibold tabular-nums ${data.finished ? "text-accent-600 dark:text-accent-400" : ""}`}>
          {data.finished ? W.finished : formatDuration(remaining + (data.running ? 999 : 0))}
        </div>
        <div className="mt-2 h-1 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
          <div className="h-full bg-accent-500 transition-[width]" style={{ width: `${Math.min(100, progress * 100)}%` }} />
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-1">
        {data.running ? (
          <button className="btn-secondary px-2 py-1" onClick={() => act("pause")}>
            <Pause className="size-4" /> {W.pause}
          </button>
        ) : (
          <button className="btn-primary px-2 py-1" onClick={() => act("start")}>
            <Play className="size-4" /> {W.start}
          </button>
        )}
        <IconButton label={W.reset} onClick={() => act("reset")}>
          <RotateCcw className="size-4" />
        </IconButton>
        {!data.running && (
          <span className="ml-auto flex items-center gap-1 text-xs text-zinc-500">
            {W.presets}
            {PRESETS_MIN.map((min) => (
              <button key={min} className="rounded px-1.5 py-0.5 hover:bg-zinc-200 dark:hover:bg-zinc-800" onClick={() => act("set", { duration_ms: min * 60_000, label: data.label })}>
                {min}
              </button>
            ))}
          </span>
        )}
      </div>
      <DataStatus meta={meta} error={error} />
    </div>
  );
}

// --- stopwatch ---

interface StopwatchData {
  elapsed_ms: number;
  started_at: number | null;
  laps: number[];
  now_ms: number;
}

export function StopwatchWidget({ widget }: BodyProps) {
  const { show_ms = true } = useModules().settingsOf("widget_stopwatch");
  const { data, meta, error, act } = useWidgetData<StopwatchData>(widget);
  const skew = useMemo(() => (data ? data.now_ms - Date.now() : 0), [data]);
  const running = data?.started_at != null;
  useTicker(show_ms ? 47 : 500, running);
  const W = t.widgets;
  if (!data) return <DataStatus meta={meta} error={error} />;

  const total = data.elapsed_ms + (running ? Date.now() + skew - data.started_at! : 0);
  return (
    <div className="flex h-full flex-col gap-2">
      <div className="text-4xl font-semibold tabular-nums">{formatDuration(total, Boolean(show_ms))}</div>
      <div className="flex items-center gap-1">
        {running ? (
          <button className="btn-secondary px-2 py-1" onClick={() => act("pause")}>
            <Pause className="size-4" /> {W.pause}
          </button>
        ) : (
          <button className="btn-primary px-2 py-1" onClick={() => act("start")}>
            <Play className="size-4" /> {W.start}
          </button>
        )}
        <IconButton label={W.lap} onClick={() => act("lap")} disabled={!running}>
          <Flag className="size-4" />
        </IconButton>
        <IconButton label={W.reset} onClick={() => act("reset")}>
          <RotateCcw className="size-4" />
        </IconButton>
      </div>
      {data.laps.length > 0 ? (
        <ol className="min-h-0 flex-1 overflow-y-auto text-xs">
          {data.laps
            .map((lap, i) => ({ lap, i, split: lap - (data.laps[i - 1] ?? 0) }))
            .reverse()
            .map(({ lap, i, split }) => (
              <li key={i} className="flex justify-between border-t border-zinc-200 py-0.5 tabular-nums dark:border-zinc-800">
                <span className="text-zinc-500">{W.lapN(i + 1)}</span>
                <span>+{formatDuration(split, true)}</span>
                <span className="font-medium">{formatDuration(lap, true)}</span>
              </li>
            ))}
        </ol>
      ) : null}
      <DataStatus meta={meta} error={error} />
    </div>
  );
}
