CREATE TABLE library_playlists (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    description TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX library_playlists_name_unique
ON library_playlists(name COLLATE NOCASE);

CREATE TABLE library_playlist_items (
    playlist_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, item_id),
    FOREIGN KEY (playlist_id) REFERENCES library_playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (item_id) REFERENCES media_library(id) ON DELETE CASCADE
);

CREATE INDEX library_playlist_items_order
ON library_playlist_items(playlist_id, position, added_at);
