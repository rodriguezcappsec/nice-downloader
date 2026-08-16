CREATE TABLE library_local_transcript_cues (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT NOT NULL,
    subtitle_name TEXT NOT NULL,
    language TEXT,
    start_seconds REAL NOT NULL,
    end_seconds REAL NOT NULL,
    cue_text TEXT NOT NULL,
    FOREIGN KEY (item_id) REFERENCES media_library(id) ON DELETE CASCADE
);

CREATE INDEX idx_library_local_transcript_cues_item_time
    ON library_local_transcript_cues(item_id, start_seconds, id);

CREATE TABLE library_transcript_embeddings (
    item_id TEXT PRIMARY KEY NOT NULL,
    model TEXT NOT NULL,
    vector_json TEXT NOT NULL,
    transcript_fingerprint TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (item_id) REFERENCES media_library(id) ON DELETE CASCADE
);

CREATE INDEX idx_library_transcript_embeddings_model
    ON library_transcript_embeddings(model, updated_at DESC);
