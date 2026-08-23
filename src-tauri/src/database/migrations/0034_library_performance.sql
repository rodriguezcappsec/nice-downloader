CREATE INDEX IF NOT EXISTS idx_media_library_catalog_recent
ON media_library(missing, media_kind, added_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_media_library_source_catalog
ON media_library(source_id, missing, media_kind, added_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_media_library_title_catalog
ON media_library(title COLLATE NOCASE, added_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_media_library_site_catalog
ON media_library(site COLLATE NOCASE, missing, added_at DESC);

CREATE INDEX IF NOT EXISTS idx_library_media_metadata_container
ON library_media_metadata(container COLLATE NOCASE, item_id);

CREATE INDEX IF NOT EXISTS idx_library_media_metadata_resolution
ON library_media_metadata(height, item_id);

CREATE INDEX IF NOT EXISTS idx_media_library_duration
ON media_library(duration_seconds, id);

CREATE INDEX IF NOT EXISTS idx_library_media_metadata_size
ON library_media_metadata(file_size_bytes, item_id);

CREATE INDEX IF NOT EXISTS idx_library_media_metadata_modified
ON library_media_metadata(modified_at_ms, item_id);

CREATE INDEX IF NOT EXISTS idx_library_rich_metadata_creator
ON library_rich_metadata(
  COALESCE(
    NULLIF(json_extract(overrides_json, '$.creator'), ''),
    json_extract(extracted_json, '$.creator')
  ) COLLATE NOCASE,
  item_id
);

CREATE INDEX IF NOT EXISTS idx_library_rich_metadata_series
ON library_rich_metadata(
  COALESCE(
    NULLIF(json_extract(overrides_json, '$.series'), ''),
    json_extract(extracted_json, '$.series')
  ) COLLATE NOCASE,
  item_id
);

CREATE INDEX IF NOT EXISTS idx_library_rich_metadata_upload_date
ON library_rich_metadata(
  COALESCE(
    NULLIF(json_extract(overrides_json, '$.uploadDate'), ''),
    json_extract(extracted_json, '$.uploadDate')
  ),
  item_id
);
