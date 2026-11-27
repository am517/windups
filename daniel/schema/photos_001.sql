-- Personal photos and videos catalog v1. People in a photo are mentions (people catalog);
-- its place is a map feature and/or coordinates.
CREATE TABLE photos_items (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('photo', 'video')),
  taken_at      TEXT,
  lat           REAL CHECK (lat BETWEEN -90 AND 90),
  lon           REAL CHECK (lon BETWEEN -180 AND 180),
  place_feature_id INTEGER,                    -- maps_features.id (another catalog)
  title         TEXT,
  status        TEXT NOT NULL DEFAULT 'unverified',
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CHECK ((lat IS NULL) = (lon IS NULL))
);
CREATE INDEX photos_items_taken ON photos_items(taken_at);
CREATE INDEX photos_items_place ON photos_items(place_feature_id);

CREATE TABLE photos_files (
  id            INTEGER PRIMARY KEY,
  item_id       INTEGER REFERENCES photos_items(id) ON DELETE SET NULL,
  path          TEXT NOT NULL UNIQUE,
  size          INTEGER,
  mtime         TEXT,
  hash          TEXT                           -- Larry's hash; NULL = hash pending
);
CREATE INDEX photos_files_hash ON photos_files(hash);
