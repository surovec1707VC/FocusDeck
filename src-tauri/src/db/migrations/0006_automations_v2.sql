-- Automations v2: description, conditions, cooldown, template, and richer
-- run logs. Trigger/action JSON stays compatible with v1.

ALTER TABLE automations ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE automations ADD COLUMN conditions_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE automations ADD COLUMN cooldown_secs INTEGER NOT NULL DEFAULT 0;
ALTER TABLE automations ADD COLUMN template TEXT;
ALTER TABLE automations ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

-- What started the run (trigger kind, manual, test, button, confirmation),
-- how long it took and the result of each action.
ALTER TABLE automation_logs ADD COLUMN source TEXT NOT NULL DEFAULT '';
ALTER TABLE automation_logs ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE automation_logs ADD COLUMN details_json TEXT NOT NULL DEFAULT '[]';
