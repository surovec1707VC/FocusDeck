-- Productivity modules: notes (tags, trash, binding), tasks (priority,
-- status, category, recurrence), snippets (tags), timers and stopwatches.

-- Notes. Tags are a JSON array of strings; deleted_at != NULL = in the trash.
ALTER TABLE notes ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE notes ADD COLUMN project TEXT NOT NULL DEFAULT '';
ALTER TABLE notes ADD COLUMN layout_id TEXT REFERENCES widget_layouts(id) ON DELETE SET NULL;
ALTER TABLE notes ADD COLUMN deleted_at TEXT;
CREATE INDEX idx_notes_list ON notes(deleted_at, pinned, updated_at);

-- Tasks. `done` stays as a quick flag and always equals status = 'done'.
ALTER TABLE tasks ADD COLUMN priority INTEGER NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 3);
ALTER TABLE tasks ADD COLUMN status TEXT NOT NULL DEFAULT 'todo'
    CHECK (status IN ('todo', 'in_progress', 'done', 'cancelled'));
ALTER TABLE tasks ADD COLUMN category TEXT NOT NULL DEFAULT '';      -- "Работа/Отчёты"
ALTER TABLE tasks ADD COLUMN recurrence TEXT NOT NULL DEFAULT 'none'
    CHECK (recurrence IN ('none', 'daily', 'weekdays', 'weekly', 'monthly'));
ALTER TABLE tasks ADD COLUMN notes TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN completed_at TEXT;
UPDATE tasks SET status = 'done', completed_at = updated_at WHERE done = 1;
CREATE INDEX idx_tasks_status ON tasks(status, due_at);
CREATE INDEX idx_tasks_completed ON tasks(completed_at);

-- Snippets.
ALTER TABLE snippets ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE snippets ADD COLUMN use_count INTEGER NOT NULL DEFAULT 0;

-- Countdown timers and Pomodoro. State is timestamps, so timers keep
-- running while the window is closed or the app restarts.
CREATE TABLE timers (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    kind             TEXT NOT NULL CHECK (kind IN ('countdown', 'pomodoro')),
    duration_ms      INTEGER NOT NULL DEFAULT 300000,   -- countdown
    work_ms          INTEGER NOT NULL DEFAULT 1500000,  -- pomodoro intervals
    short_break_ms   INTEGER NOT NULL DEFAULT 300000,
    long_break_ms    INTEGER NOT NULL DEFAULT 900000,
    rounds           INTEGER NOT NULL DEFAULT 4,        -- work rounds before a long break
    phase            TEXT NOT NULL DEFAULT 'work' CHECK (phase IN ('work', 'short_break', 'long_break')),
    round            INTEGER NOT NULL DEFAULT 1,
    running          INTEGER NOT NULL DEFAULT 0,
    ends_at          INTEGER,                           -- epoch ms while running
    remaining_ms     INTEGER,                           -- while paused; NULL = full interval
    phase_started_at INTEGER,                           -- epoch ms, for session history
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE timer_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timer_id    TEXT,                                   -- kept after the timer is deleted
    timer_name  TEXT NOT NULL,
    kind        TEXT NOT NULL,                          -- countdown | work | short_break | long_break
    planned_ms  INTEGER NOT NULL,
    actual_ms   INTEGER NOT NULL,
    completed   INTEGER NOT NULL,                       -- 0 = stopped early / skipped
    started_at  TEXT NOT NULL,
    ended_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_timer_sessions_ended ON timer_sessions(ended_at);

CREATE TABLE stopwatches (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    elapsed_ms  INTEGER NOT NULL DEFAULT 0,
    started_at  INTEGER,                                -- epoch ms while running
    laps_json   TEXT NOT NULL DEFAULT '[]',
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE stopwatch_results (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    stopwatch_id  TEXT,
    name          TEXT NOT NULL,
    total_ms      INTEGER NOT NULL,
    laps_json     TEXT NOT NULL DEFAULT '[]',
    note          TEXT NOT NULL DEFAULT '',
    saved_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
