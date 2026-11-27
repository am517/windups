-- The text layer of a purpose catalog (@c@ = the catalog name: media, records, photos).
-- Extracts are derived text; chunks are made only from extracts whose quality is 'ok', so the
-- word index never holds empty or garbled text. Vectors carry their model id and version.
CREATE TABLE @c@_extracts (
  id            INTEGER PRIMARY KEY,
  record_id     INTEGER NOT NULL,              -- the catalog's own row (media.id, records_items.id, ...)
  file_hash     TEXT,                          -- Larry's hash of the source file (NULL = hash pending)
  method        TEXT NOT NULL,                 -- subtitle | ocr | html | pdf_text | transcript | ...
  language      TEXT,
  quality       TEXT NOT NULL CHECK (quality IN ('ok', 'held_needs_ocr', 'garbled', 'empty')),
  made_by       TEXT NOT NULL,                 -- windup and version that made it
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX @c@_extracts_record ON @c@_extracts(record_id);

CREATE TABLE @c@_chunks (
  id            INTEGER PRIMARY KEY,
  extract_id    INTEGER NOT NULL REFERENCES @c@_extracts(id) ON DELETE CASCADE,
  parent_id     INTEGER REFERENCES @c@_chunks(id),
  level         INTEGER NOT NULL CHECK (level IN (1, 2, 3)), -- overview | structure | passage
  locator       TEXT,                          -- page, heading, verse or timestamp; NULL = not citable
  language      TEXT,
  text          TEXT NOT NULL,
  content_hash  TEXT NOT NULL,
  mentions      TEXT NOT NULL DEFAULT 'pending' CHECK (mentions IN ('pending', 'done')),
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX @c@_chunks_extract ON @c@_chunks(extract_id, level);
CREATE INDEX @c@_chunks_parent ON @c@_chunks(parent_id);

CREATE VIRTUAL TABLE @c@_chunks_fts USING fts5(
  text, content='@c@_chunks', content_rowid='id', tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER @c@_chunks_ai AFTER INSERT ON @c@_chunks BEGIN
  INSERT INTO @c@_chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER @c@_chunks_ad AFTER DELETE ON @c@_chunks BEGIN
  INSERT INTO @c@_chunks_fts(@c@_chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER @c@_chunks_au AFTER UPDATE OF text ON @c@_chunks BEGIN
  INSERT INTO @c@_chunks_fts(@c@_chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO @c@_chunks_fts(rowid, text) VALUES (new.id, new.text);
END;

-- Written by Tom through Daniel. Search never mixes models or versions.
CREATE TABLE @c@_vectors (
  id            INTEGER PRIMARY KEY,
  chunk_id      INTEGER NOT NULL REFERENCES @c@_chunks(id) ON DELETE CASCADE,
  model         TEXT NOT NULL,
  model_version TEXT NOT NULL,
  dim           INTEGER NOT NULL,
  vector        BLOB NOT NULL,                 -- little-endian f32
  content_hash  TEXT NOT NULL,                 -- the chunk's hash when embedded; re-embed when it changes
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (chunk_id, model, model_version)
);
