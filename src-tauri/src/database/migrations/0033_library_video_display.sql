CREATE TABLE library_video_display_settings (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_library(id) ON DELETE CASCADE,
    fit TEXT NOT NULL CHECK (fit IN ('fit', 'fill', 'original', 'stretch')),
    zoom REAL NOT NULL CHECK (zoom >= 1.0 AND zoom <= 3.0),
    pan_x REAL NOT NULL CHECK (pan_x >= -1.0 AND pan_x <= 1.0),
    pan_y REAL NOT NULL CHECK (pan_y >= -1.0 AND pan_y <= 1.0),
    rotation INTEGER NOT NULL CHECK (rotation IN (0, 90, 180, 270)),
    flip_horizontal INTEGER NOT NULL CHECK (flip_horizontal IN (0, 1)),
    flip_vertical INTEGER NOT NULL CHECK (flip_vertical IN (0, 1)),
    brightness REAL NOT NULL CHECK (brightness >= -100.0 AND brightness <= 100.0),
    contrast REAL NOT NULL CHECK (contrast >= -100.0 AND contrast <= 100.0),
    saturation REAL NOT NULL CHECK (saturation >= -100.0 AND saturation <= 100.0),
    gamma REAL NOT NULL CHECK (gamma >= -100.0 AND gamma <= 100.0),
    updated_at INTEGER NOT NULL
);
