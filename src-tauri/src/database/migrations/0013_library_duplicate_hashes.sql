CREATE TABLE IF NOT EXISTS library_content_hashes (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    file_size_bytes INTEGER NOT NULL,
    modified_at_ms INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_library_content_hashes_digest
    ON library_content_hashes(sha256, file_size_bytes);
