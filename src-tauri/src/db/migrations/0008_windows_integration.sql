-- Windows Integration and the Action Log: every sensitive action of any
-- module (window control, system settings, launches, file moves…) with its
-- result and, when possible, the data needed to undo it.

CREATE TABLE action_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    module_id   TEXT NOT NULL,
    action      TEXT NOT NULL,          -- id from the action catalog, e.g. windows.minimize
    target      TEXT NOT NULL DEFAULT '',
    result      TEXT NOT NULL CHECK (result IN ('ok', 'error', 'denied', 'blocked', 'cancelled', 'undone')),
    message     TEXT NOT NULL DEFAULT '',
    permission  TEXT,                   -- permission used (for the per-permission access log)
    undo_json   TEXT,                   -- how to revert; NULL = can't be undone
    undone_at   TEXT
);
CREATE INDEX idx_action_log_created ON action_log(id);
CREATE INDEX idx_action_log_permission ON action_log(permission, id);
CREATE INDEX idx_action_log_module ON action_log(module_id, id);
