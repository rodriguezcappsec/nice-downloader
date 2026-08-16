CREATE TABLE post_download_recipes (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    steps_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE post_download_recipe_runs (
    id TEXT PRIMARY KEY NOT NULL,
    recipe_id TEXT NOT NULL,
    download_id TEXT NOT NULL,
    state TEXT NOT NULL,
    current_step INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER,
    UNIQUE(recipe_id, download_id),
    FOREIGN KEY (recipe_id) REFERENCES post_download_recipes(id) ON DELETE CASCADE,
    FOREIGN KEY (download_id) REFERENCES downloads(id) ON DELETE CASCADE
);

CREATE INDEX idx_post_download_recipe_runs_updated
    ON post_download_recipe_runs(updated_at DESC);
