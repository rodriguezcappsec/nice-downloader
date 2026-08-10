ALTER TABLE downloads ADD COLUMN request_json TEXT;
ALTER TABLE downloads ADD COLUMN diagnostics_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE downloads ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 1;
ALTER TABLE downloads ADD COLUMN retry_of_id TEXT;

CREATE INDEX IF NOT EXISTS idx_downloads_state_updated_at
    ON downloads(state, updated_at DESC);
