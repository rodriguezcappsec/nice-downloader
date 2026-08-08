CREATE TABLE IF NOT EXISTS downloads (
    id TEXT PRIMARY KEY NOT NULL,
    url TEXT NOT NULL,
    title TEXT NOT NULL,
    thumbnail TEXT,
    state TEXT NOT NULL,
    progress_percent REAL NOT NULL DEFAULT 0,
    downloaded_bytes INTEGER,
    total_bytes INTEGER,
    speed_bytes_per_second REAL,
    eta_seconds INTEGER,
    selected_quality TEXT NOT NULL,
    output_format TEXT NOT NULL,
    destination TEXT NOT NULL,
    output_path TEXT,
    current_operation TEXT NOT NULL,
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER,
    options_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_downloads_completed_at
    ON downloads(completed_at DESC);

CREATE TABLE IF NOT EXISTS presets (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    options_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO settings (key, value_json, updated_at)
VALUES ('queue.concurrentDownloads', '2', unixepoch() * 1000);
