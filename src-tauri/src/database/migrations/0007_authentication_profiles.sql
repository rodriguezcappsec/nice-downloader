CREATE TABLE IF NOT EXISTS authentication_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    host_suffix TEXT NOT NULL,
    browser_cookie_source TEXT NOT NULL,
    browser_profile TEXT,
    proxy_url TEXT,
    managed_cookie_path TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_authentication_profiles_host
    ON authentication_profiles(host_suffix, enabled);
