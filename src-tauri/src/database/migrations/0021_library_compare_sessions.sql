CREATE TABLE library_compare_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    notes TEXT NOT NULL DEFAULT '',
    active_audio_item_id TEXT,
    multiple_audio INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(active_audio_item_id) REFERENCES media_library(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX library_compare_sessions_name
    ON library_compare_sessions(name COLLATE NOCASE);

CREATE TABLE library_compare_session_items (
    session_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    playback_position_seconds REAL NOT NULL DEFAULT 0 CHECK(playback_position_seconds >= 0),
    playback_rate REAL NOT NULL DEFAULT 1 CHECK(playback_rate >= 0.25 AND playback_rate <= 4),
    PRIMARY KEY(session_id, item_id),
    FOREIGN KEY(session_id) REFERENCES library_compare_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(item_id) REFERENCES media_library(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX library_compare_session_position
    ON library_compare_session_items(session_id, position);
