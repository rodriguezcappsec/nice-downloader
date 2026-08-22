CREATE TABLE library_source_automation (
    source_id TEXT PRIMARY KEY NOT NULL REFERENCES library_sources(id) ON DELETE CASCADE,
    tags_json TEXT NOT NULL DEFAULT '[]',
    collection_id TEXT REFERENCES library_collections(id) ON DELETE SET NULL,
    favorite INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
