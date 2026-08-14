ALTER TABLE media_library ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0;
ALTER TABLE media_library ADD COLUMN rating INTEGER CHECK (rating IS NULL OR (rating >= 1 AND rating <= 5));

CREATE TABLE IF NOT EXISTS library_collections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS library_collection_items (
    collection_id TEXT NOT NULL REFERENCES library_collections(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (collection_id, item_id)
);

CREATE INDEX IF NOT EXISTS idx_media_library_favorite ON media_library(favorite, added_at DESC);
CREATE INDEX IF NOT EXISTS idx_media_library_rating ON media_library(rating, added_at DESC);
CREATE INDEX IF NOT EXISTS idx_library_collection_items_item ON library_collection_items(item_id);
