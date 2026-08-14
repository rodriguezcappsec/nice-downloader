CREATE TABLE IF NOT EXISTS library_file_operations (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL,
    output_item_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('move', 'copy', 'rename')),
    state TEXT NOT NULL CHECK (state IN ('planned', 'executing', 'completed', 'failed', 'undone')),
    source_path TEXT NOT NULL,
    destination_path TEXT NOT NULL,
    source_size_bytes INTEGER NOT NULL,
    source_modified_at_ms INTEGER NOT NULL,
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_library_file_operations_history
    ON library_file_operations(state, created_at DESC);
