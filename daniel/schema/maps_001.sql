-- Maps catalog v1: enough to draw layers on a globe. WGS84 coordinates, optional GeoJSON geometry.
CREATE TABLE maps_layers (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL UNIQUE,
  kind          TEXT,                          -- places | routes | borders | photos | ...
  source        TEXT,
  style         TEXT,                          -- JSON: colour, icon, line width, ...
  valid_from    TEXT,
  valid_to      TEXT,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE maps_features (
  id            INTEGER PRIMARY KEY,
  layer_id      INTEGER NOT NULL REFERENCES maps_layers(id) ON DELETE CASCADE,
  parent_id     INTEGER REFERENCES maps_features(id), -- a playground inside a park
  kind          TEXT,
  lat           REAL CHECK (lat BETWEEN -90 AND 90),
  lon           REAL CHECK (lon BETWEEN -180 AND 180),
  geometry      TEXT,                          -- GeoJSON geometry, when more than a point
  valid_from    TEXT,
  valid_to      TEXT,
  label         TEXT NOT NULL,
  label_alt     TEXT,                          -- the label in a second language
  label_alt_language TEXT,
  source        TEXT,
  uid           TEXT UNIQUE,                   -- stable id from the source, when it has one
  verified      INTEGER NOT NULL DEFAULT 0,
  place_entity_id INTEGER,                     -- people_entities.id of the place (another catalog)
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CHECK ((lat IS NULL) = (lon IS NULL)),
  CHECK (lat IS NOT NULL OR geometry IS NOT NULL)
);
CREATE INDEX maps_features_layer ON maps_features(layer_id);
CREATE INDEX maps_features_point ON maps_features(lat, lon);
CREATE INDEX maps_features_parent ON maps_features(parent_id);

-- Word index over labels in both languages.
CREATE VIRTUAL TABLE maps_labels_fts USING fts5(
  label, label_alt, content='maps_features', content_rowid='id', tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER maps_features_ai AFTER INSERT ON maps_features BEGIN
  INSERT INTO maps_labels_fts(rowid, label, label_alt) VALUES (new.id, new.label, new.label_alt);
END;
CREATE TRIGGER maps_features_ad AFTER DELETE ON maps_features BEGIN
  INSERT INTO maps_labels_fts(maps_labels_fts, rowid, label, label_alt) VALUES ('delete', old.id, old.label, old.label_alt);
END;
CREATE TRIGGER maps_features_au AFTER UPDATE OF label, label_alt ON maps_features BEGIN
  INSERT INTO maps_labels_fts(maps_labels_fts, rowid, label, label_alt) VALUES ('delete', old.id, old.label, old.label_alt);
  INSERT INTO maps_labels_fts(rowid, label, label_alt) VALUES (new.id, new.label, new.label_alt);
END;
