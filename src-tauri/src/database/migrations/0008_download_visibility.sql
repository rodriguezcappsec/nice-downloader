ALTER TABLE downloads ADD COLUMN visible_in_downloads INTEGER NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_downloads_visible_created
    ON downloads(visible_in_downloads, created_at DESC);
