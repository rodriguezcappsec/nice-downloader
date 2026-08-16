CREATE TABLE IF NOT EXISTS library_private_items (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS library_private_folders (
    source_id TEXT NOT NULL REFERENCES library_sources(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (source_id, relative_path)
);

CREATE INDEX IF NOT EXISTS idx_library_private_folders_source
    ON library_private_folders(source_id, relative_path);
