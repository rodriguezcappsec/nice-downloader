CREATE TABLE library_montages (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    notes TEXT NOT NULL DEFAULT '',
    fade_seconds REAL NOT NULL DEFAULT 0,
    segment_count INTEGER NOT NULL CHECK (segment_count BETWEEN 0 AND 50),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE library_montage_segments (
    montage_id TEXT NOT NULL REFERENCES library_montages(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    item_id TEXT NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    start_seconds REAL NOT NULL,
    end_seconds REAL NOT NULL,
    PRIMARY KEY (montage_id, position)
);

CREATE INDEX idx_library_montage_segments_item
    ON library_montage_segments(item_id);
