CREATE TABLE IF NOT EXISTS library_file_identity (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    device_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_library_file_identity_lookup
    ON library_file_identity(device_id, file_id);

CREATE TABLE IF NOT EXISTS library_changes (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    source_id TEXT REFERENCES library_sources(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('added', 'modified', 'renamed', 'missing', 'restored')),
    previous_path TEXT,
    detected_at INTEGER NOT NULL,
    reviewed_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_library_changes_review
    ON library_changes(reviewed_at, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_library_changes_item
    ON library_changes(item_id, detected_at DESC);
