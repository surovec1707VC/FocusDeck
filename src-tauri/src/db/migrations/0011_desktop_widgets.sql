-- Widgets detached from the grid into their own window on the Windows desktop.
-- Geometry is in physical pixels of the virtual screen; a row exists while the
-- widget is on the desktop.
CREATE TABLE desktop_widgets (
    widget_id  TEXT PRIMARY KEY REFERENCES widgets(id) ON DELETE CASCADE,
    x          INTEGER,
    y          INTEGER,
    width      INTEGER NOT NULL DEFAULT 320,
    height     INTEGER NOT NULL DEFAULT 240,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
