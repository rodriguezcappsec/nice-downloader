CREATE TABLE library_visual_signatures (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    signature TEXT NOT NULL,
    file_size_bytes INTEGER,
    modified_at_ms INTEGER,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_library_visual_signatures_fingerprint
    ON library_visual_signatures(file_size_bytes, modified_at_ms);
