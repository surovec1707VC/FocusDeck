-- Module Registry v2: versions, JSON settings, critical flag, restart flag,
-- module action log, and module ownership of automations.

ALTER TABLE modules RENAME COLUMN is_core TO is_critical;
ALTER TABLE modules ADD COLUMN version TEXT NOT NULL DEFAULT '1.0.0';
ALTER TABLE modules ADD COLUMN requires_restart INTEGER NOT NULL DEFAULT 0;
-- The module's whole settings object; validated against the module's schema.
ALTER TABLE modules ADD COLUMN settings_json TEXT NOT NULL DEFAULT '{}';

-- Every action on a module (enable, disable, settings, health, import).
CREATE TABLE module_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    module_id   TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    level       TEXT NOT NULL CHECK (level IN ('info', 'warn', 'error')),
    action      TEXT NOT NULL,
    message     TEXT NOT NULL DEFAULT '',
    source      TEXT NOT NULL DEFAULT 'user',
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_module_logs ON module_logs(module_id, id);

-- The module an automation belongs to; it doesn't run while that module is off.
ALTER TABLE automations ADD COLUMN module_id TEXT NOT NULL DEFAULT 'automations';
