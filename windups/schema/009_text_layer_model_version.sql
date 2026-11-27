-- v9: the media/reference text layer (from text_layer.sql with @c@ = media), and the model
-- version beside every vector.
-- Extracts are derived text; chunks are made only from extracts whose quality is 'ok', so the
-- word index never holds empty or garbled text. Vectors carry their model id and version.
CREATE TABLE media_extracts (
  id            INTEGER PRIMARY KEY,
  record_id     INTEGER NOT NULL,              -- the catalog's own row (media.id, records_items.id, ...)
  file_hash     TEXT,                          -- Larry's hash of the source file (NULL = hash pending)
  method        TEXT NOT NULL,                 -- subtitle | ocr | html | pdf_text | transcript | ...
  language      TEXT,
  quality       TEXT NOT NULL CHECK (quality IN ('ok', 'held_needs_ocr', 'garbled', 'empty')),
  made_by       TEXT NOT NULL,                 -- windup and version that made it
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX media_extracts_record ON media_extracts(record_id);

CREATE TABLE media_chunks (
  id            INTEGER PRIMARY KEY,
  extract_id    INTEGER NOT NULL REFERENCES media_extracts(id) ON DELETE CASCADE,
  parent_id     INTEGER REFERENCES media_chunks(id),
  level         INTEGER NOT NULL CHECK (level IN (1, 2, 3)), -- overview | structure | passage
  locator       TEXT,                          -- page, heading, verse or timestamp; NULL = not citable
  language      TEXT,
  text          TEXT NOT NULL,
  content_hash  TEXT NOT NULL,
  mentions      TEXT NOT NULL DEFAULT 'pending' CHECK (mentions IN ('pending', 'done')),
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX media_chunks_extract ON media_chunks(extract_id, level);
CREATE INDEX media_chunks_parent ON media_chunks(parent_id);

CREATE VIRTUAL TABLE media_chunks_fts USING fts5(
  text, content='media_chunks', content_rowid='id', tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER media_chunks_ai AFTER INSERT ON media_chunks BEGIN
  INSERT INTO media_chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER media_chunks_ad AFTER DELETE ON media_chunks BEGIN
  INSERT INTO media_chunks_fts(media_chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER media_chunks_au AFTER UPDATE OF text ON media_chunks BEGIN
  INSERT INTO media_chunks_fts(media_chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO media_chunks_fts(rowid, text) VALUES (new.id, new.text);
END;

-- Written by Tom through Daniel. Search never mixes models or versions.
CREATE TABLE media_vectors (
  id            INTEGER PRIMARY KEY,
  chunk_id      INTEGER NOT NULL REFERENCES media_chunks(id) ON DELETE CASCADE,
  model         TEXT NOT NULL,
  model_version TEXT NOT NULL,
  dim           INTEGER NOT NULL,
  vector        BLOB NOT NULL,                 -- little-endian f32
  content_hash  TEXT NOT NULL,                 -- the chunk's hash when embedded; re-embed when it changes
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (chunk_id, model, model_version)
);

-- Every vector names its model and version; search never mixes them.
ALTER TABLE embeddings ADD COLUMN model_version TEXT NOT NULL DEFAULT '';
