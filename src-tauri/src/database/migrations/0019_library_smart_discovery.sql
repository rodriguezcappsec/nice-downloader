CREATE VIRTUAL TABLE library_search USING fts5(
    item_id UNINDEXED,
    title,
    creator,
    series,
    description,
    site,
    tags,
    path,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO library_search (item_id, title, creator, series, description, site, tags, path)
SELECT
    media_library.id,
    COALESCE(media_library.title, ''),
    COALESCE(
        NULLIF(json_extract(rich.overrides_json, '$.creator'), ''),
        json_extract(rich.extracted_json, '$.creator'),
        ''
    ),
    COALESCE(
        NULLIF(json_extract(rich.overrides_json, '$.series'), ''),
        json_extract(rich.extracted_json, '$.series'),
        ''
    ),
    COALESCE(
        NULLIF(json_extract(rich.overrides_json, '$.description'), ''),
        json_extract(rich.extracted_json, '$.description'),
        ''
    ),
    COALESCE(media_library.site, ''),
    COALESCE((SELECT group_concat(value, ' ') FROM json_each(media_library.tags_json)), ''),
    COALESCE(media_library.relative_path, media_library.path, '')
FROM media_library
LEFT JOIN library_rich_metadata rich ON rich.item_id = media_library.id;

CREATE TRIGGER library_search_media_insert
AFTER INSERT ON media_library
BEGIN
    INSERT INTO library_search (item_id, title, creator, series, description, site, tags, path)
    VALUES (
        NEW.id,
        COALESCE(NEW.title, ''),
        '',
        '',
        '',
        COALESCE(NEW.site, ''),
        COALESCE((SELECT group_concat(value, ' ') FROM json_each(NEW.tags_json)), ''),
        COALESCE(NEW.relative_path, NEW.path, '')
    );
END;

CREATE TRIGGER library_search_media_update
AFTER UPDATE OF title, site, tags_json, path, relative_path ON media_library
BEGIN
    DELETE FROM library_search WHERE item_id = NEW.id;
    INSERT INTO library_search (item_id, title, creator, series, description, site, tags, path)
    SELECT
        NEW.id,
        COALESCE(NEW.title, ''),
        COALESCE(
            NULLIF(json_extract(rich.overrides_json, '$.creator'), ''),
            json_extract(rich.extracted_json, '$.creator'),
            ''
        ),
        COALESCE(
            NULLIF(json_extract(rich.overrides_json, '$.series'), ''),
            json_extract(rich.extracted_json, '$.series'),
            ''
        ),
        COALESCE(
            NULLIF(json_extract(rich.overrides_json, '$.description'), ''),
            json_extract(rich.extracted_json, '$.description'),
            ''
        ),
        COALESCE(NEW.site, ''),
        COALESCE((SELECT group_concat(value, ' ') FROM json_each(NEW.tags_json)), ''),
        COALESCE(NEW.relative_path, NEW.path, '')
    FROM (SELECT 1)
    LEFT JOIN library_rich_metadata rich ON rich.item_id = NEW.id;
END;

CREATE TRIGGER library_search_media_delete
AFTER DELETE ON media_library
BEGIN
    DELETE FROM library_search WHERE item_id = OLD.id;
END;

CREATE TRIGGER library_search_rich_insert
AFTER INSERT ON library_rich_metadata
BEGIN
    DELETE FROM library_search WHERE item_id = NEW.item_id;
    INSERT INTO library_search (item_id, title, creator, series, description, site, tags, path)
    SELECT
        media_library.id,
        COALESCE(media_library.title, ''),
        COALESCE(NULLIF(json_extract(NEW.overrides_json, '$.creator'), ''), json_extract(NEW.extracted_json, '$.creator'), ''),
        COALESCE(NULLIF(json_extract(NEW.overrides_json, '$.series'), ''), json_extract(NEW.extracted_json, '$.series'), ''),
        COALESCE(NULLIF(json_extract(NEW.overrides_json, '$.description'), ''), json_extract(NEW.extracted_json, '$.description'), ''),
        COALESCE(media_library.site, ''),
        COALESCE((SELECT group_concat(value, ' ') FROM json_each(media_library.tags_json)), ''),
        COALESCE(media_library.relative_path, media_library.path, '')
    FROM media_library
    WHERE media_library.id = NEW.item_id;
END;

CREATE TRIGGER library_search_rich_update
AFTER UPDATE OF extracted_json, overrides_json ON library_rich_metadata
BEGIN
    DELETE FROM library_search WHERE item_id = NEW.item_id;
    INSERT INTO library_search (item_id, title, creator, series, description, site, tags, path)
    SELECT
        media_library.id,
        COALESCE(media_library.title, ''),
        COALESCE(NULLIF(json_extract(NEW.overrides_json, '$.creator'), ''), json_extract(NEW.extracted_json, '$.creator'), ''),
        COALESCE(NULLIF(json_extract(NEW.overrides_json, '$.series'), ''), json_extract(NEW.extracted_json, '$.series'), ''),
        COALESCE(NULLIF(json_extract(NEW.overrides_json, '$.description'), ''), json_extract(NEW.extracted_json, '$.description'), ''),
        COALESCE(media_library.site, ''),
        COALESCE((SELECT group_concat(value, ' ') FROM json_each(media_library.tags_json)), ''),
        COALESCE(media_library.relative_path, media_library.path, '')
    FROM media_library
    WHERE media_library.id = NEW.item_id;
END;

CREATE TRIGGER library_search_rich_delete
AFTER DELETE ON library_rich_metadata
BEGIN
    DELETE FROM library_search WHERE item_id = OLD.item_id;
    INSERT INTO library_search (item_id, title, creator, series, description, site, tags, path)
    SELECT
        media_library.id,
        COALESCE(media_library.title, ''),
        '',
        '',
        '',
        COALESCE(media_library.site, ''),
        COALESCE((SELECT group_concat(value, ' ') FROM json_each(media_library.tags_json)), ''),
        COALESCE(media_library.relative_path, media_library.path, '')
    FROM media_library
    WHERE media_library.id = OLD.item_id;
END;

CREATE TABLE library_smart_views (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    query_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX library_smart_views_updated ON library_smart_views(updated_at DESC, id);
