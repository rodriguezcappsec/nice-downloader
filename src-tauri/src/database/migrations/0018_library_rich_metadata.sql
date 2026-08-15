CREATE TABLE library_rich_metadata (
    item_id TEXT PRIMARY KEY NOT NULL,
    extracted_json TEXT NOT NULL DEFAULT '{}',
    overrides_json TEXT NOT NULL DEFAULT '{}',
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (item_id) REFERENCES media_library(id) ON DELETE CASCADE
);

CREATE INDEX library_rich_metadata_updated
    ON library_rich_metadata(updated_at DESC, item_id);
