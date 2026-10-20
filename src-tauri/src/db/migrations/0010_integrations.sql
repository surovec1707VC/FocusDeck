-- Integrations: state of each connector (manifests live in the binary, see
-- src/integrations/manifests). `connectors` and `connector_settings` come
-- from 0001; secrets are never stored here (Windows Credential Manager).

-- Connector permissions the user granted: ["repos.read", …].
ALTER TABLE connectors ADD COLUMN granted_json TEXT NOT NULL DEFAULT '[]';
-- Last health check: ok, warning, error, not_configured, offline, disabled, unknown.
ALTER TABLE connectors ADD COLUMN health_status TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE connectors ADD COLUMN health_message TEXT NOT NULL DEFAULT '';
ALTER TABLE connectors ADD COLUMN health_checked_at TEXT;

CREATE TABLE connector_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    connector_id TEXT NOT NULL REFERENCES connectors(id) ON DELETE CASCADE,
    -- info, warn, error
    level TEXT NOT NULL,
    -- enable, disable, settings, permission, secret, health, offline
    action TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_connector_logs ON connector_logs(connector_id, id DESC);
