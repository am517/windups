# Plan: moving the notes database and the Bible reference database into Daniel

Plan only. Neither database is opened, changed or moved by this work. They stay where they are and
stay authoritative for their bots until the owner confirms a cutover.

Start only after Daniel can ingest, store, search and hold entities and mentions
([daniel_spec.md](daniel_spec.md)).

## Steps

1. **Freeze and copy.** Back up both databases. Work only from the copies. Keep the originals
   until the owner confirms.
2. **Map purposes.**
   - notes contacts → people (alternate names become aliases).
   - notes places → maps (coordinates, types, uid, verified, parent and bilingual names already
     fit the maps catalog).
   - notes links, ideas and reports, and news items → the catalog whose purpose fits (often
     media/reference). A private family note may belong in personal records. When unsure, import
     as `undecided`; never guess.
   - Bible works, copies, sources, sections, chunks and source status → media/reference, keeping
     the three chunk levels, locators, licence per copy and status.
   - Bible article passages and detected verse references → mentions of passage or work entities.
3. **Keep identifiers.** Record a mapping from every old id (note ids, source and chunk ids,
   place uids) to the new id, so existing links, reports and bots keep working.
4. **Import idempotently**, into Daniel's folder only. Running the import again updates the same
   rows; it never duplicates them.
5. **Vectors.** The two databases use two different 384-dimension models. Either keep the vectors
   labelled with their model and version for interim search, or mark imported chunks
   `not_embedded` and let Tom re-embed with the chosen model. Never mix models in one search.
6. **Parity check.** A fixed set of known queries from each bot (including Japanese place queries
   and vector cases) must return the same documents with locators. A difference is a bug or a
   documented gap, never a silent improvement.
7. **Cut over** each bot only after its parity check passes. Keep the old files read-only for a
   time. George snapshots Daniel's catalogs right after the cutover.

Files on disk (articles, scans) stay where they are; only the catalog rows move.

## Open questions

1. How long do the old files stay as read-only backups after cutover?
2. Should the notes vectors be kept labelled by model (step 5) or re-embedded?
3. Who signs off each bot's parity check: the owner, or Julie with the owner told?
