-- People / entities catalog v1: people, places, things, ideas, topics; their other names,
-- relationships and mentions. Never a silent merge: possible duplicates are proposals.
CREATE TABLE people_entities (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('person', 'place', 'thing', 'idea', 'topic', 'organization', 'work')),
  name          TEXT NOT NULL,                 -- canonical name
  description   TEXT,
  status        TEXT NOT NULL DEFAULT 'chosen' CHECK (status IN ('chosen', 'candidate', 'rejected')),
  canonical_id  INTEGER REFERENCES people_entities(id), -- candidate/rejected -> the chosen entity
  source        TEXT,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX people_entities_name ON people_entities(name COLLATE NOCASE);
CREATE INDEX people_entities_kind ON people_entities(kind);

-- Other names and spellings, in any language. An alias never creates a second entity.
CREATE TABLE people_aliases (
  id            INTEGER PRIMARY KEY,
  entity_id     INTEGER NOT NULL REFERENCES people_entities(id) ON DELETE CASCADE,
  name          TEXT NOT NULL,
  language      TEXT NOT NULL DEFAULT '',
  source        TEXT,
  UNIQUE (entity_id, name, language)
);
CREATE INDEX people_aliases_name ON people_aliases(name COLLATE NOCASE);

-- Ids in other systems, only when known (never guessed). Strong evidence, not a merge.
CREATE TABLE people_external_ids (
  entity_id     INTEGER NOT NULL REFERENCES people_entities(id) ON DELETE CASCADE,
  source        TEXT NOT NULL,
  scope         TEXT NOT NULL DEFAULT '',
  value         TEXT NOT NULL,
  UNIQUE (source, scope, value)
);
CREATE INDEX people_external_ids_entity ON people_external_ids(entity_id);

-- entity -[rel]-> entity ("lives in", "discussed in", "appears with"), for "connected to X".
CREATE TABLE people_relationships (
  from_id       INTEGER NOT NULL REFERENCES people_entities(id) ON DELETE CASCADE,
  to_id         INTEGER NOT NULL REFERENCES people_entities(id) ON DELETE CASCADE,
  rel           TEXT NOT NULL,
  source        TEXT NOT NULL,
  confidence    REAL,
  PRIMARY KEY (from_id, to_id, rel, source)
);
CREATE INDEX people_relationships_to ON people_relationships(to_id, rel);

-- Possible duplicates, waiting for Julie, Sammy or a person; never acted on automatically.
CREATE TABLE people_proposals (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL,                 -- possible_same
  entity_id     INTEGER NOT NULL REFERENCES people_entities(id),
  other_id      INTEGER NOT NULL REFERENCES people_entities(id),
  score         REAL,
  source        TEXT NOT NULL,
  status        TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'accepted', 'rejected')),
  decided_by    TEXT,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (kind, entity_id, other_id)
);

-- A span in a chunk (or a region or time in a photo or video) that refers to an entity.
-- record_id and chunk_id belong to `catalog` (another file, so not a foreign key).
-- Written only from an extractor whose quality is final (see docs/daniel_spec.md §6).
CREATE TABLE people_mentions (
  id            INTEGER PRIMARY KEY,
  catalog       TEXT NOT NULL CHECK (catalog IN ('media', 'people', 'records', 'maps', 'photos')),
  record_id     INTEGER NOT NULL,
  chunk_id      INTEGER,
  locator       TEXT,                          -- page, verse, timestamp or image region
  surface       TEXT NOT NULL,                 -- the name as it appeared
  start_offset  INTEGER,
  end_offset    INTEGER,
  entity_id     INTEGER REFERENCES people_entities(id) ON DELETE CASCADE,
  proposed_name TEXT,                          -- no entity yet: a proposal, not a link
  confidence    REAL NOT NULL CHECK (confidence BETWEEN 0 AND 1),
  needs_review  INTEGER NOT NULL DEFAULT 0,
  extractor     TEXT NOT NULL,
  extractor_version TEXT NOT NULL,
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CHECK ((entity_id IS NULL) != (proposed_name IS NULL))
);
CREATE UNIQUE INDEX people_mentions_once ON people_mentions(
  catalog, record_id, IFNULL(chunk_id, -1), IFNULL(start_offset, -1), IFNULL(end_offset, -1),
  IFNULL(entity_id, -1), IFNULL(proposed_name, ''), extractor
);
CREATE INDEX people_mentions_entity ON people_mentions(entity_id);
CREATE INDEX people_mentions_record ON people_mentions(catalog, record_id);

-- Word index over every name (canonical and aliases), kept in sync by triggers.
CREATE VIRTUAL TABLE people_names_fts USING fts5(
  name, entity_id UNINDEXED, tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER people_entities_ai AFTER INSERT ON people_entities BEGIN
  INSERT INTO people_names_fts(name, entity_id) VALUES (new.name, new.id);
END;
CREATE TRIGGER people_entities_ad AFTER DELETE ON people_entities BEGIN
  DELETE FROM people_names_fts WHERE entity_id = old.id;
END;
CREATE TRIGGER people_entities_au AFTER UPDATE OF name ON people_entities BEGIN
  DELETE FROM people_names_fts WHERE entity_id = old.id AND name = old.name;
  INSERT INTO people_names_fts(name, entity_id) VALUES (new.name, new.id);
END;
CREATE TRIGGER people_aliases_ai AFTER INSERT ON people_aliases BEGIN
  INSERT INTO people_names_fts(name, entity_id) VALUES (new.name, new.entity_id);
END;
CREATE TRIGGER people_aliases_ad AFTER DELETE ON people_aliases BEGIN
  DELETE FROM people_names_fts WHERE rowid IN (
    SELECT rowid FROM people_names_fts WHERE entity_id = old.entity_id AND name = old.name LIMIT 1
  );
END;
