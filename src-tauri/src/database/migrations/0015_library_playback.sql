CREATE TABLE IF NOT EXISTS library_playback (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    position_seconds REAL NOT NULL DEFAULT 0 CHECK (position_seconds >= 0),
    duration_seconds REAL NOT NULL DEFAULT 0 CHECK (duration_seconds >= 0),
    completed INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_library_playback_continue
    ON library_playback(completed, updated_at DESC);
