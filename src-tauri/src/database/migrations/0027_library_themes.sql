CREATE TABLE IF NOT EXISTS library_theme_analyses (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    mode TEXT NOT NULL,
    summary TEXT NOT NULL,
    overall_confidence REAL NOT NULL,
    file_size_bytes INTEGER,
    modified_at_ms INTEGER,
    analyzed_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS library_theme_assignments (
    item_id TEXT NOT NULL REFERENCES library_theme_analyses(item_id) ON DELETE CASCADE,
    theme_key TEXT NOT NULL,
    label TEXT NOT NULL,
    confidence REAL NOT NULL,
    evidence_json TEXT NOT NULL,
    PRIMARY KEY (item_id, theme_key)
);

CREATE INDEX IF NOT EXISTS idx_library_theme_assignments_theme
    ON library_theme_assignments(theme_key, confidence DESC, item_id);

CREATE INDEX IF NOT EXISTS idx_library_theme_analyses_fingerprint
    ON library_theme_analyses(model, mode, file_size_bytes, modified_at_ms);
