# Daniel spec: the record keeper

Daniel stores, updates and serves records. He is the only writer of the catalogs below. Other
windups write their own database first and hand entries to him. He does not hash (Larry), embed
(Tom), judge (Sammy), review a whole entry (Julie) or own rich search (Steve).

Status: decided items are marked **Decided (2026-11-27)**. Anything not decided is listed as a
question in §12. Nothing here is guessed.

## 1. What already exists (kept)

- `daniel ingest` / `daniel accept`: versioned handoff JSON (`"v": 1`,
  [handoff_v1.md](../windups/schema/handoff_v1.md)) into media.db. Re-ingest updates, never
  duplicates. Unchanged by this spec.
- `daniel serve`: synced records over a Unix socket. Unchanged.
- At store time Daniel asks Larry for the hash and fingerprint and Ruth for the work reference,
  then may alert Walter. Unchanged.
- media.db (the shared schema in `windups/schema/`) stays the media/reference catalog. Its
  `entities` table (cast, crew, studios from online sources) stays as it is; see Q3.

## 2. Purpose catalogs

**Decided (2026-11-27):** catalogs are split by **purpose**, not file type. A PDF may be a
reference book or a tax paper; a video may be a lecture or a family clip. When the purpose is
unclear, the row is stored and marked `undecided`. Daniel stays **one** windup.

| Catalog | Holds | Name in config |
|---|---|---|
| Media / reference | Books, films, shows, archived long-form video, articles, encyclopedia and public-domain text, Bible reference works, subtitles and other extracts | `media` (media.db) |
| People / entities | People, places, things, ideas, topics: the shared entity layer, with aliases, relationships and mentions | `people` |
| Personal records | Receipts, medical papers, tax papers and similar private documents | `records` |
| Maps | Enough to draw layers on a globe: coordinates, optional geometry, layer, time, labels, source, style | `maps` |
| Personal photos and videos | Personal stills and home videos | `photos` |

**Decided (2026-11-27):** records and photos are separate catalogs.

**Decided (2026-11-27):** catalogs are linked. Photos and media reference people/entities and
map locations (§4).

### Files

Each catalog's database path is a config value (`[catalogs]` in `daniel/config/daniel.toml`).
An empty value means `<catalog>.db` in Daniel's own folder (media keeps the shared media.db).
Table names carry the catalog's name (`records_items`, `maps_features`, ...) and each catalog
keeps its own schema version in a `catalog_versions` table, so **several catalogs may share one
file** just by giving them the same path. Whether the default should be one file or several is
Q1.

## 3. Database engine

**Decided (2026-11-27):** SQLite is the main database. A graph engine is on the back burner:
"everything connected to X" is answered with entity, mention and relationship tables and
depth-limited recursive CTEs. A graph engine may later be added as an index, never as a second
source of truth, and only if real queries show SQLite is not enough.

## 4. Links between catalogs

SQLite cannot enforce a foreign key across files, so a link to another catalog is a pair
`(catalog, id)`, for example `place_feature_id` in photos points at `maps_features.id`.
Walter's audit reports a link whose target is missing. Daniel never deletes a row because a link
dangles.

- Photos → maps: a photo's place is a map feature (`place_feature_id`) and/or coordinates.
- Photos, media, records → people: through **mentions** (§6). A person in a photo is a mention
  with a region or timestamp as its locator.
- Maps → people: a map feature may name the place entity it draws (`place_entity_id`).

## 5. Record model

The same text layer is in every catalog that holds text (media, records, photos):

```
<c>_extracts   derived text (subtitles, OCR, HTML to text): how made, source file hash, language,
               quality (ok | held_needs_ocr | garbled | empty). Empty or garbled text is a status,
               never indexed as clean.
<c>_chunks     searchable spans: level (1 overview, 2 structure, 3 passage), parent, locator
               (page, heading, verse or timestamp), language, text. No locator = findable, not
               citable.
<c>_chunks_fts FTS5 index over chunk text, kept in sync by triggers.
<c>_vectors    vectors for chunks (§8).
```

`<c>` is the catalog name (`media`, `records`, `photos`).

- **Media / reference:** the existing media.db tables. Books, articles and other reference works
  are `media` rows with their own `kind`; authors are `media_entities` with role `author`. Copies,
  editions and translations are separate rows; licence is kept per copy (Q6).
- **People:** `people_entities` (kind person | place | thing | idea | topic | organization | work,
  canonical name, description, status), `people_aliases` (every other name and spelling, with
  language; never a silent merge), `people_external_ids` (known ids only, never guessed),
  `people_relationships` (entity → entity with type, source, confidence), `people_proposals`
  (possible same, waiting for Julie, Sammy or a person), `people_mentions` (§6).
- **Personal records:** `records_items` (kind receipt | medical | tax | other | undecided, title,
  date, status), `records_files` (path, size, mtime, hash or `hash_pending`), and the text layer.
- **Maps:** `maps_layers` (name, kind, source, style, valid time) and `maps_features` (layer,
  parent feature, kind, WGS84 latitude and longitude, optional GeoJSON geometry, valid time,
  label and a second-language label, source, stable uid, `verified`, optional place entity).
- **Photos:** `photos_items` (kind photo | video, taken at, coordinates, place feature, title,
  status), `photos_files`, and the text layer (captions, OCR, transcripts).

Status values follow the existing rules: lifecycle and unresolved flags as in media.db; ingested
is not the same as valid.

## 6. Mentions

**Decided (2026-11-27):** mentions cover people, places, things, ideas and topics, and are the
basis for "everything connected to X". Built now.

A mention is a span in a chunk (or a region or timestamp in a photo or video) that refers to an
entity:

```
people_mentions: catalog, record_id, chunk_id, locator, surface text as it appeared,
                 start/end offsets, entity_id (or a proposed name when no entity exists yet),
                 confidence, needs_review, extractor id and version, created_at
```

- Unsure mentions are stored with `needs_review`, not dropped and not treated as certain.
- Daniel never merges entities on a similar name or one mention; that opens a proposal.
- Search shows both the surface text and the canonical name, so a reviewer sees why it linked.

### The extractor interface (for Sammy)

Extraction is not Daniel's job. The interface lives in the shared crate
(`windups::mentions`) so Sammy, or another extractor, can implement it without depending on
Daniel:

- input: the chunk text, its language, catalog, record and chunk ids, locator, and the
  candidate entities whose names or aliases appear (Daniel looks these up);
- output: mentions with offsets, surface text, the chosen entity or a proposed new one,
  confidence and `needs_review`;
- every extractor reports its id, version and **quality**: `final` or `placeholder`.

**Decided (2026-11-27), the quality rule** (also in [coding_guidelines.md](coding_guidelines.md)):
a windup may scale down when it reads or uses data, but when it writes permanent records it must
use the best-quality method. So:

- no rule-based mention extractor is the final extractor;
- Daniel refuses to store mentions from an extractor whose quality is `placeholder`. A
  placeholder may only preview (nothing written);
- until Sammy provides the extractor, no mentions are written.

## 7. Search

**Decided (2026-11-27):** FTS5 word search is built now.

- `daniel find [--catalog NAME]... [--limit N] [-j] WORDS`: a short candidate list (default 5,
  config `[find] limit`). Each candidate shows its catalog, what it is, its locator, and a
  snippet.
- Default catalogs are `[find] catalogs` in config. Personal records are searched only when
  asked for by name (`--catalog records`), never by default.
- Full details for a picked candidate, more results, vectors and fusion belong to Steve (the
  search windup); `find` is Daniel's thin search for when Steve is absent (Q4).
- Japanese and other scripts without spaces: Q5.

## 8. Vectors

Daniel stores vectors; Tom makes them and writes them through Daniel.

**Decided (2026-11-27):** every vector is stored with its **model id and model version**. Search
never mixes models or versions. Re-embedding is incremental, keyed by content hash.

- media.db `embeddings` gains `model_version`.
- `<c>_vectors` in each text catalog: chunk id, model, model version, dimension, vector
  (little-endian f32 BLOB), content hash.

Tom's model: **leaning toward `BAAI/bge-m3`** (1024 dimensions, multilingual, long input). Not
final; see Q7.

## 9. Personal records

**Decided (2026-11-27):** designed now; **nothing is ingested for real** until Daniel is final.

- `[records] ingest = false` in config, and Daniel has no records ingest command yet. A request
  to store a record is refused with `records_ingest_disabled`.
- The source tree is read-only, always: no writes, renames, tags, sidecars, xattrs or tag tools.
  Lauren, Suzan and Sally never touch it.
- Tests use fixtures only, read-only.
- Default search does not include records (§7). Logs, sync payloads and backup listings never
  quote record file names or contents.
- Record text is not sent to a remote model unless the owner asks.

## 10. Migration of notes.db and Bible Bot's reference.db

**Decided (2026-11-27):** both move into Daniel later. Plan only:
[daniel_migration_plan.md](daniel_migration_plan.md). Those databases are not touched.

## 11. Isolation

Alone, Daniel still ingests, stores, runs `find` and the "connected to X" query, and records what
is missing: no hash yet (`hash_pending`), no vectors (`not_embedded`), no review. He does not
embed, guess metadata, extract mentions or merge entities.

## 12. Open questions

1. **One file or several?** The guidelines say one database per windup; the draft recommends a
   file per catalog (privacy of personal records, growth of maps and photos). The code supports
   both through config. Which should be the default?
2. **Who settles an `undecided` purpose?** The owner only, or Sammy with Julie's review and the
   owner told?
3. **media.db `entities` versus the people catalog.** Should cast and crew from online sources
   move into the people catalog (one entity layer), or stay in media.db with a link?
4. **How far does Daniel's `find` go once Steve exists?** Keep it as the isolated fallback only,
   or remove it?
5. **Japanese and similar text in FTS.** Add a trigram index beside the word index, or wait for a
   proper tokenizer?
6. **Licence for very large corpora** (encyclopedia dumps, public-domain libraries): the same rule
   as the Bible reference database (licence per copy, blank if unknown, no silent public-domain
   claim)?
7. **Tom's model.** bge-m3 is the lean. Confirm, or compare first with the 384-dimension models
   the existing databases use?
8. **When SQLite vectors stop being enough:** stay until it hurts, plan shards now, or evaluate an
   embedded vector library?
9. **Map geometry:** is GeoJSON text enough, or is a binary format wanted for large layers?
10. **Who extracts mentions while Sammy is missing?** As decided, nobody writes; confirm that
    mentions simply wait (marked `mentions_pending`) until Sammy's extractor exists.
