-- FocusDeck initial schema.
-- Timestamps are ISO-8601 TEXT (UTC); JSON blobs are TEXT validated by the app.

CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE modules (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    description       TEXT NOT NULL DEFAULT '',
    category          TEXT NOT NULL,
    enabled           INTEGER NOT NULL DEFAULT 1,
    settings_json     TEXT NOT NULL DEFAULT '{}',
    requires_restart  INTEGER NOT NULL DEFAULT 0,
    permissions_json  TEXT NOT NULL DEFAULT '[]',
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE module_settings (
    module_id   TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value_json  TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (module_id, key)
);

CREATE TABLE widget_layouts (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    is_active    INTEGER NOT NULL DEFAULT 0,
    layout_json  TEXT NOT NULL DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE widgets (
    id             TEXT PRIMARY KEY,
    layout_id      TEXT NOT NULL REFERENCES widget_layouts(id) ON DELETE CASCADE,
    kind           TEXT NOT NULL,
    module_id      TEXT REFERENCES modules(id) ON DELETE SET NULL,
    position_json  TEXT NOT NULL DEFAULT '{}',
    config_json    TEXT NOT NULL DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE automations (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    description    TEXT NOT NULL DEFAULT '',
    enabled        INTEGER NOT NULL DEFAULT 0,
    trigger_json   TEXT NOT NULL,
    actions_json   TEXT NOT NULL DEFAULT '[]',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE automation_logs (
    id             TEXT PRIMARY KEY,
    automation_id  TEXT NOT NULL REFERENCES automations(id) ON DELETE CASCADE,
    status         TEXT NOT NULL,
    message        TEXT NOT NULL DEFAULT '',
    details_json   TEXT,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    path        TEXT,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE project_context (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,
    content     TEXT NOT NULL,
    source      TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE ai_sessions (
    id           TEXT PRIMARY KEY,
    title        TEXT NOT NULL DEFAULT '',
    profile_id   TEXT NOT NULL,
    provider     TEXT NOT NULL,
    model        TEXT NOT NULL,
    project_id   TEXT REFERENCES projects(id) ON DELETE SET NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE ai_messages (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL REFERENCES ai_sessions(id) ON DELETE CASCADE,
    role         TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
    content      TEXT NOT NULL,
    tokens_in    INTEGER,
    tokens_out   INTEGER,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_ai_messages_session ON ai_messages(session_id, created_at);

CREATE TABLE prompts (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    tags_json   TEXT NOT NULL DEFAULT '[]',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES projects(id) ON DELETE SET NULL,
    title       TEXT NOT NULL,
    notes       TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'todo',
    priority    INTEGER NOT NULL DEFAULT 0,
    due_at      TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE agent_tasks (
    id            TEXT PRIMARY KEY,
    skill_id      TEXT,
    session_id    TEXT REFERENCES ai_sessions(id) ON DELETE SET NULL,
    status        TEXT NOT NULL DEFAULT 'pending',
    input_json    TEXT NOT NULL DEFAULT '{}',
    plan_json     TEXT,
    result_json   TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE agent_logs (
    id             TEXT PRIMARY KEY,
    agent_task_id  TEXT NOT NULL REFERENCES agent_tasks(id) ON DELETE CASCADE,
    step           INTEGER NOT NULL,
    level          TEXT NOT NULL DEFAULT 'info',
    message        TEXT NOT NULL,
    details_json   TEXT,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE connectors (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    category       TEXT NOT NULL,
    auth_type      TEXT NOT NULL CHECK (auth_type IN ('none', 'api_key', 'oauth', 'local')),
    enabled        INTEGER NOT NULL DEFAULT 0,
    settings_json  TEXT NOT NULL DEFAULT '{}',
    last_status    TEXT,
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Secrets themselves live in the OS keyring; only metadata is stored here.
CREATE TABLE connector_credentials_metadata (
    connector_id  TEXT NOT NULL REFERENCES connectors(id) ON DELETE CASCADE,
    secret_key    TEXT NOT NULL,
    label         TEXT NOT NULL DEFAULT '',
    expires_at    TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (connector_id, secret_key)
);

CREATE TABLE skills (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    category       TEXT NOT NULL,
    enabled        INTEGER NOT NULL DEFAULT 1,
    safety_level   TEXT NOT NULL CHECK (safety_level IN ('read_only', 'confirm_before_action', 'dangerous')),
    manifest_json  TEXT NOT NULL,
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE permissions (
    id           TEXT PRIMARY KEY,
    subject      TEXT NOT NULL,           -- e.g. "module:clipboard", "skill:code-reviewer"
    permission   TEXT NOT NULL,           -- e.g. "fs:read", "shell:execute"
    granted      INTEGER NOT NULL DEFAULT 0,
    scope_json   TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (subject, permission)
);

CREATE TABLE permission_logs (
    id           TEXT PRIMARY KEY,
    subject      TEXT NOT NULL,
    permission   TEXT NOT NULL,
    decision     TEXT NOT NULL CHECK (decision IN ('allowed', 'denied', 'prompted')),
    context_json TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE clipboard_history (
    id          TEXT PRIMARY KEY,
    content     TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT 'text',
    pinned      INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_clipboard_created ON clipboard_history(created_at);

CREATE TABLE snippets (
    id          TEXT PRIMARY KEY,
    trigger     TEXT,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE notes (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES projects(id) ON DELETE SET NULL,
    title       TEXT NOT NULL DEFAULT '',
    body        TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
