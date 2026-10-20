-- Widget system: grid placement and state flags, layout profiles with
-- hotkeys, cached widget data, calendar events and the notification history.

-- Grid placement in cells; x = -1 means "not placed yet" (placed on next load).
ALTER TABLE widgets ADD COLUMN x INTEGER NOT NULL DEFAULT -1;
ALTER TABLE widgets ADD COLUMN y INTEGER NOT NULL DEFAULT -1;
ALTER TABLE widgets ADD COLUMN w INTEGER NOT NULL DEFAULT 4;
ALTER TABLE widgets ADD COLUMN h INTEGER NOT NULL DEFAULT 3;
ALTER TABLE widgets ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE widgets ADD COLUMN collapsed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE widgets ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;

-- Old fixed sizes → grid sizes (12 columns).
UPDATE widgets SET
    w = CASE size WHEN 'sm' THEN 3 WHEN 'md' THEN 6 ELSE 12 END,
    h = CASE size WHEN 'sm' THEN 2 ELSE 4 END;
ALTER TABLE widgets DROP COLUMN size;

-- Renamed kinds.
UPDATE widgets SET kind = 'todo' WHERE kind = 'tasks';
UPDATE widgets SET kind = 'notes' WHERE kind = 'note';

-- Note text moves from the widget config to the notes table.
INSERT INTO notes (id, title, body)
    SELECT id, title, COALESCE(json_extract(config_json, '$.text'), '') FROM widgets WHERE kind = 'notes';
UPDATE widgets SET config_json = json_set(json_remove(config_json, '$.text'), '$.note_id', id)
    WHERE kind = 'notes';

ALTER TABLE widget_layouts ADD COLUMN profile TEXT NOT NULL DEFAULT 'custom'
    CHECK (profile IN ('work', 'coding', 'streaming', 'gaming', 'focus', 'custom'));
ALTER TABLE widget_layouts ADD COLUMN hotkey TEXT;
ALTER TABLE widget_layouts ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

-- Last good data of widgets that fetch it (weather): shown offline.
CREATE TABLE widget_cache (
    key         TEXT PRIMARY KEY,
    data_json   TEXT NOT NULL,
    fetched_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE calendar_events (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    date        TEXT NOT NULL,          -- YYYY-MM-DD
    time        TEXT,                   -- HH:MM, NULL = all day
    note        TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_calendar_events_date ON calendar_events(date);

CREATE TABLE notifications (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    level       TEXT NOT NULL DEFAULT 'info' CHECK (level IN ('info', 'warn', 'error')),
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    source      TEXT NOT NULL DEFAULT '',
    read        INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_clipboard_history_created ON clipboard_history(pinned, id);
