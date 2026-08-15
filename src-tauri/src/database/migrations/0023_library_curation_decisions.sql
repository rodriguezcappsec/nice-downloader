CREATE TABLE library_curation_decisions (
    item_id TEXT PRIMARY KEY NOT NULL,
    decision TEXT NOT NULL CHECK(decision IN ('keep', 'reviewLater')),
    reviewed_at INTEGER NOT NULL,
    FOREIGN KEY(item_id) REFERENCES media_library(id) ON DELETE CASCADE
);

CREATE INDEX library_curation_decisions_reviewed
    ON library_curation_decisions(decision, reviewed_at DESC);
