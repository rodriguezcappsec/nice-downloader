CREATE VIRTUAL TABLE library_transcript_search USING fts5(
    item_id UNINDEXED,
    subtitle_name UNINDEXED,
    language UNINDEXED,
    start_seconds UNINDEXED,
    end_seconds UNINDEXED,
    cue_text,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER library_transcript_media_delete
AFTER DELETE ON media_library
BEGIN
    DELETE FROM library_transcript_search WHERE item_id = OLD.id;
END;
