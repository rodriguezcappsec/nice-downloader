CREATE TABLE library_media_stacks (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    notes TEXT NOT NULL DEFAULT '',
    cover_item_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(cover_item_id) REFERENCES media_library(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX library_media_stacks_name
    ON library_media_stacks(name COLLATE NOCASE);

CREATE TABLE library_media_stack_items (
    stack_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK(position >= 0),
    role TEXT NOT NULL CHECK(role IN ('original', 'alternate', 'edit', 'generated', 'other')),
    PRIMARY KEY(stack_id, item_id),
    FOREIGN KEY(stack_id) REFERENCES library_media_stacks(id) ON DELETE CASCADE,
    FOREIGN KEY(item_id) REFERENCES media_library(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX library_media_stack_position
    ON library_media_stack_items(stack_id, position);

CREATE UNIQUE INDEX library_media_stack_single_membership
    ON library_media_stack_items(item_id);

CREATE UNIQUE INDEX library_media_stack_original
    ON library_media_stack_items(stack_id)
    WHERE role = 'original';

CREATE TRIGGER library_media_stack_remove_incomplete
AFTER DELETE ON library_media_stack_items
WHEN (SELECT COUNT(*) FROM library_media_stack_items WHERE stack_id = OLD.stack_id) < 2
BEGIN
    DELETE FROM library_media_stacks WHERE id = OLD.stack_id;
END;
