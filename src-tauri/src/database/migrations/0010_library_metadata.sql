CREATE TABLE IF NOT EXISTS library_media_metadata (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    file_size_bytes INTEGER,
    modified_at_ms INTEGER,
    width INTEGER,
    height INTEGER,
    fps REAL,
    video_codec TEXT,
    audio_codec TEXT,
    container TEXT,
    dynamic_range TEXT,
    state TEXT NOT NULL DEFAULT 'pending',
    error TEXT,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_library_media_metadata_state
    ON library_media_metadata(state, updated_at);
