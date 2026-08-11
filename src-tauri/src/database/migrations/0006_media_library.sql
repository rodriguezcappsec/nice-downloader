CREATE TABLE IF NOT EXISTS media_library (
    id TEXT PRIMARY KEY NOT NULL,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    source_url TEXT,
    thumbnail TEXT,
    duration_seconds REAL,
    site TEXT,
    tags_json TEXT NOT NULL DEFAULT '[]',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    missing INTEGER NOT NULL DEFAULT 0,
    added_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_library_added_at ON media_library(added_at DESC);
