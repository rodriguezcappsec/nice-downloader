CREATE TABLE IF NOT EXISTS library_sources (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    media_kind TEXT NOT NULL DEFAULT 'both',
    recursive INTEGER NOT NULL DEFAULT 1,
    watch_enabled INTEGER NOT NULL DEFAULT 0,
    private INTEGER NOT NULL DEFAULT 0,
    include_patterns_json TEXT NOT NULL DEFAULT '[]',
    exclude_patterns_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'online',
    last_error TEXT,
    last_scanned_at INTEGER,
    item_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS library_scan_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    source_id TEXT NOT NULL REFERENCES library_sources(id) ON DELETE CASCADE,
    state TEXT NOT NULL,
    discovered_count INTEGER NOT NULL DEFAULT 0,
    indexed_count INTEGER NOT NULL DEFAULT 0,
    skipped_count INTEGER NOT NULL DEFAULT 0,
    warning_count INTEGER NOT NULL DEFAULT 0,
    current_path TEXT,
    warnings_json TEXT NOT NULL DEFAULT '[]',
    started_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER
);

ALTER TABLE media_library ADD COLUMN source_id TEXT REFERENCES library_sources(id) ON DELETE SET NULL;
ALTER TABLE media_library ADD COLUMN relative_path TEXT;
ALTER TABLE media_library ADD COLUMN media_kind TEXT;

CREATE INDEX IF NOT EXISTS idx_library_sources_status ON library_sources(status);
CREATE INDEX IF NOT EXISTS idx_library_scan_jobs_source ON library_scan_jobs(source_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_media_library_source ON media_library(source_id, relative_path);
