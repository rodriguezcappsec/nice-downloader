CREATE TABLE IF NOT EXISTS transformation_tasks (
    id TEXT PRIMARY KEY NOT NULL,
    input_path TEXT NOT NULL,
    output_path TEXT NOT NULL,
    operation_json TEXT NOT NULL,
    state TEXT NOT NULL,
    progress_percent REAL NOT NULL DEFAULT 0,
    current_operation TEXT NOT NULL,
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_transformation_tasks_created_at
    ON transformation_tasks(created_at DESC);
