-- Clipboard Hub: rich clipboard history (text, links, files, images, HTML,
-- RTF) with the source app, and user-defined quick actions.

-- `content` stays the human-readable text (for images/files: a summary),
-- used for search and previews. The other formats are kept as copied.
ALTER TABLE clipboard_history ADD COLUMN source_app TEXT NOT NULL DEFAULT '';
ALTER TABLE clipboard_history ADD COLUMN hash TEXT NOT NULL DEFAULT '';
ALTER TABLE clipboard_history ADD COLUMN size INTEGER NOT NULL DEFAULT 0;      -- bytes of all formats
ALTER TABLE clipboard_history ADD COLUMN text TEXT;                            -- plain text, if any
ALTER TABLE clipboard_history ADD COLUMN html TEXT;                            -- CF_HTML, as copied
ALTER TABLE clipboard_history ADD COLUMN rtf TEXT;                             -- Rich Text Format
ALTER TABLE clipboard_history ADD COLUMN image BLOB;                           -- BMP file
ALTER TABLE clipboard_history ADD COLUMN image_width INTEGER;
ALTER TABLE clipboard_history ADD COLUMN image_height INTEGER;
ALTER TABLE clipboard_history ADD COLUMN files_json TEXT;                      -- ["C:\\…", …]
ALTER TABLE clipboard_history ADD COLUMN last_used_at TEXT;

-- Existing entries are plain text; links get their own kind.
UPDATE clipboard_history SET
    text = content,
    size = length(CAST(content AS BLOB)),
    kind = CASE WHEN content NOT LIKE '% %' AND (content LIKE 'http://%' OR content LIKE 'https://%') THEN 'link' ELSE 'text' END;

CREATE INDEX idx_clipboard_history_hash ON clipboard_history(hash);
CREATE INDEX idx_clipboard_history_kind ON clipboard_history(kind, id);

CREATE TABLE quick_actions (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    kind         TEXT NOT NULL CHECK (kind IN
                   ('open_app', 'open_url', 'open_folder', 'open_file', 'paste_text', 'copy_text', 'command')),
    target       TEXT NOT NULL,          -- path, URL, text or command line
    hotkey       TEXT,                   -- not allowed for commands
    position     INTEGER NOT NULL DEFAULT 0,
    last_run_at  TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
