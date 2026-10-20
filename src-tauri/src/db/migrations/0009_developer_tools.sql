-- Developer Tools: Git, Project Launcher, Terminal Quick Launcher, API Tester,
-- Regex Tester. JSON/YAML Tools keep no data. Every tool is its own module.

-- Git: repositories the user opened (the current one is the module setting `current_repo`).
CREATE TABLE dev_git_repos (
    path TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    last_opened_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Project Launcher.
CREATE TABLE dev_projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    -- node, rust, tauri, python, dotnet, go, java, php, ruby, generic
    kind TEXT NOT NULL DEFAULT 'generic',
    -- [{ "name": "dev", "command": "npm run dev" }, …]
    commands_json TEXT NOT NULL DEFAULT '[]',
    favorite INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_opened_at TEXT
);

-- Terminal Quick Launcher: command history and favourite commands.
CREATE TABLE terminal_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shell TEXT NOT NULL,
    cwd TEXT NOT NULL,
    command TEXT NOT NULL,
    exit_code INTEGER,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_terminal_history_created ON terminal_history(created_at DESC);

CREATE TABLE terminal_favorites (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    shell TEXT NOT NULL,
    command TEXT NOT NULL,
    cwd TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- API Tester.
CREATE TABLE api_collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE api_requests (
    id TEXT PRIMARY KEY,
    collection_id TEXT REFERENCES api_collections(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    method TEXT NOT NULL DEFAULT 'GET',
    url TEXT NOT NULL,
    -- [{ "name": …, "value": …, "enabled": true }]
    headers_json TEXT NOT NULL DEFAULT '[]',
    query_json TEXT NOT NULL DEFAULT '[]',
    body TEXT NOT NULL DEFAULT '',
    -- none, json, text, form
    body_type TEXT NOT NULL DEFAULT 'none',
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_api_requests_collection ON api_requests(collection_id, position);

CREATE TABLE api_environments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    -- [{ "name": …, "value": …, "enabled": true }]
    vars_json TEXT NOT NULL DEFAULT '[]',
    active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE api_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    method TEXT NOT NULL,
    url TEXT NOT NULL,
    status INTEGER,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    size INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    -- the request as sent (variables substituted), to repeat it
    request_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_api_history_created ON api_history(created_at DESC);

-- Regex Tester: saved expressions.
CREATE TABLE regex_saved (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    pattern TEXT NOT NULL,
    flags TEXT NOT NULL DEFAULT 'g',
    sample TEXT NOT NULL DEFAULT '',
    replacement TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
