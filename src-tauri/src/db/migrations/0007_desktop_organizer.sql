-- Desktop Organizer: sorting rules, operations and every single file move
-- (for undo). Files are only ever moved, never deleted.

CREATE TABLE organizer_rules (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    position    INTEGER NOT NULL DEFAULT 0,
    match_json  TEXT NOT NULL,           -- { categories, extensions, name_pattern, older_than_days, … }
    target      TEXT NOT NULL,           -- folder template: {root}\Скриншоты\{year}-{month}
    builtin     TEXT,                    -- key of a default rule, NULL for user rules
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE organizer_operations (
    id            TEXT PRIMARY KEY,
    source        TEXT NOT NULL,         -- manual | button | schedule | login | limit
    status        TEXT NOT NULL DEFAULT 'done'
                  CHECK (status IN ('done', 'partial', 'undone', 'partially_undone')),
    moved_count   INTEGER NOT NULL DEFAULT 0,
    failed_count  INTEGER NOT NULL DEFAULT 0,
    summary       TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    undone_at     TEXT
);

CREATE TABLE organizer_moves (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id   TEXT NOT NULL REFERENCES organizer_operations(id) ON DELETE CASCADE,
    original_path  TEXT NOT NULL,
    new_path       TEXT NOT NULL,
    rule_name      TEXT NOT NULL DEFAULT '',
    size           INTEGER NOT NULL DEFAULT 0,
    status         TEXT NOT NULL CHECK (status IN ('moved', 'failed', 'undone', 'undo_failed')),
    error          TEXT NOT NULL DEFAULT '',
    moved_at       TEXT NOT NULL DEFAULT (datetime('now')),
    undone_at      TEXT,
    restored_path  TEXT                  -- where undo put it (differs if the original name was taken)
);
CREATE INDEX idx_organizer_moves_op ON organizer_moves(operation_id, id);
