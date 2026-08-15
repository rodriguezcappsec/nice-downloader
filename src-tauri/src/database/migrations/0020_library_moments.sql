CREATE TABLE library_moments (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL,
    label TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    start_seconds REAL NOT NULL CHECK(start_seconds >= 0),
    end_seconds REAL CHECK(end_seconds IS NULL OR end_seconds > start_seconds),
    color TEXT NOT NULL DEFAULT 'accent',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(item_id) REFERENCES media_library(id) ON DELETE CASCADE
);

CREATE INDEX library_moments_item_time
    ON library_moments(item_id, start_seconds, created_at, id);
