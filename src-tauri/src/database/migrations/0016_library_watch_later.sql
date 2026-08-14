CREATE TABLE IF NOT EXISTS library_watch_later (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_library_watch_later_added
    ON library_watch_later(added_at DESC, item_id ASC);
