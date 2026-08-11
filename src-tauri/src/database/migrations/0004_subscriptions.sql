CREATE TABLE IF NOT EXISTS subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    preset_id TEXT NOT NULL REFERENCES presets(id) ON DELETE RESTRICT,
    destination TEXT NOT NULL,
    check_interval_minutes INTEGER NOT NULL DEFAULT 360,
    max_entries INTEGER NOT NULL DEFAULT 10,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_checked_at INTEGER,
    next_check_at INTEGER NOT NULL,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_subscriptions_due
    ON subscriptions(enabled, next_check_at);
