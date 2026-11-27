-- Personal records catalog v1 (receipts, medical and tax papers, ...). Design only: nothing is
-- ingested for real yet ([records] ingest = false). The source tree is always read-only.
CREATE TABLE records_items (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL DEFAULT 'undecided' CHECK (kind IN ('receipt', 'medical', 'tax', 'other', 'undecided')),
  title         TEXT,
  date          TEXT,                          -- ISO 8601, as far as known
  status        TEXT NOT NULL DEFAULT 'unverified',
  created_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX records_items_kind_date ON records_items(kind, date);

CREATE TABLE records_files (
  id            INTEGER PRIMARY KEY,
  item_id       INTEGER REFERENCES records_items(id) ON DELETE SET NULL,
  path          TEXT NOT NULL UNIQUE,
  size          INTEGER,
  mtime         TEXT,
  hash          TEXT                           -- Larry's hash; NULL = hash pending
);
CREATE INDEX records_files_hash ON records_files(hash);
