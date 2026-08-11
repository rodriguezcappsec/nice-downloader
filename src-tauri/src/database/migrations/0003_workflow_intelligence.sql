CREATE TABLE IF NOT EXISTS site_preset_rules (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    host_suffix TEXT NOT NULL,
    path_prefix TEXT,
    preset_id TEXT NOT NULL REFERENCES presets(id) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_site_preset_rules_host
    ON site_preset_rules(enabled, host_suffix);

CREATE TABLE IF NOT EXISTS download_archive (
    url TEXT NOT NULL,
    destination TEXT NOT NULL,
    output_path TEXT,
    completed_at INTEGER NOT NULL,
    PRIMARY KEY (url, destination)
);

CREATE INDEX IF NOT EXISTS idx_download_archive_url
    ON download_archive(url, completed_at DESC);

INSERT OR IGNORE INTO download_archive (url, destination, output_path, completed_at)
SELECT url, destination, output_path, COALESCE(completed_at, updated_at)
FROM downloads
WHERE state = 'Completed';
