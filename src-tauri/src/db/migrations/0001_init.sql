-- FocusDeck schema v1. Timestamps are UTC ISO-8601 TEXT.

CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE modules (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    category     TEXT NOT NULL,
    enabled      INTEGER NOT NULL DEFAULT 1,
    is_core      INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE module_settings (
    module_id   TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value_json  TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (module_id, key)
);

CREATE TABLE widget_layouts (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    is_active   INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE widgets (
    id           TEXT PRIMARY KEY,
    layout_id    TEXT NOT NULL REFERENCES widget_layouts(id) ON DELETE CASCADE,
    kind         TEXT NOT NULL,
    title        TEXT NOT NULL DEFAULT '',
    size         TEXT NOT NULL DEFAULT 'md' CHECK (size IN ('sm', 'md', 'lg')),
    position     INTEGER NOT NULL DEFAULT 0,
    config_json  TEXT NOT NULL DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_widgets_layout ON widgets(layout_id, position);

CREATE TABLE automations (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 1,
    trigger_json  TEXT NOT NULL,
    actions_json  TEXT NOT NULL DEFAULT '[]',
    last_run_at   TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE automation_logs (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    automation_id  TEXT NOT NULL REFERENCES automations(id) ON DELETE CASCADE,
    status         TEXT NOT NULL CHECK (status IN ('ok', 'error', 'skipped')),
    message        TEXT NOT NULL DEFAULT '',
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_automation_logs ON automation_logs(automation_id, id);

CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    done        INTEGER NOT NULL DEFAULT 0,
    position    INTEGER NOT NULL DEFAULT 0,
    due_at      TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE notes (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL DEFAULT '',
    body        TEXT NOT NULL DEFAULT '',
    pinned      INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE snippets (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    shortcut    TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE clipboard_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    content     TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT 'text',
    pinned      INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE permissions (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    granted      INTEGER NOT NULL DEFAULT 0,
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE permission_logs (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    permission_id  TEXT NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    action         TEXT NOT NULL CHECK (action IN ('granted', 'revoked', 'denied')),
    source         TEXT NOT NULL,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_permission_logs ON permission_logs(permission_id, id);

-- Local integrations only (e.g. apps on this PC); no cloud services.
CREATE TABLE connectors (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE connector_settings (
    connector_id  TEXT NOT NULL REFERENCES connectors(id) ON DELETE CASCADE,
    key           TEXT NOT NULL,
    value_json    TEXT NOT NULL,
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (connector_id, key)
);
