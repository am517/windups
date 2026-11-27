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
A link whose target is missing is meant to be reported by Walter's audit (not built; Q12). Daniel never deletes a row because a link
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

### Tags (PROPOSED, 2026-11-27)
No mass automatic tags: in an earlier reference database, about 30% of rows were noisy auto-tags.
Only a small set of chosen tags: people, places and a few topics. Vector levels and summaries:
`docs/tom_spec.md`, "Vectorizing strategy".

## 6. Mentions

**Decided (2026-11-27):** mentions cover people, places, things, ideas and topics, and are the
basis for "everything connected to X". Built now.

A mention is a span in a chunk (or a region or timestamp in a photo or video) that refers to an
entity:

```
people_mentions: catalog, record_id, chunk_id, locator, surface text as it appeared,
                 start/end offsets, entity_id (or a proposed name and kind when no entity exists yet),
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
- `daniel connected [--id N] [--depth N] [--catalog NAME]... [--limit N] [-j] NAME`: "everything
  connected to X". The name (or an alias) is resolved to entities; when several match, they are
  listed for a pick (`--id`) and none is guessed. Relationships are walked both ways up to
  `[connected] depth` hops (default 2) by a recursive CTE; mentions come from `[connected] catalogs`
  (records only when named), sure ones first.
- Japanese and other scripts without spaces: Q5.

## 8. Vectors

Daniel stores vectors; Tom makes them and writes them through Daniel.

**Decided (2026-11-27):** every vector is stored with its **model id and model version**. Search
never mixes models or versions. Re-embedding is incremental, keyed by content hash.

- media.db `embeddings` gains `model_version`.
- `<c>_vectors` in each text catalog: chunk id, model, model version, dimension, vector
  (little-endian f32 BLOB), content hash.

`daniel/src/chunk_vectors.rs` is Tom's path through Daniel. `not_embedded` lists the chunks with no
vector for a model and version, or whose text changed since. `store` refuses a vector without a
model id and version, or one made from text that has since changed (`chunk_changed`). `search`
ranks within one model and version.

Tom's model: not chosen. bge-m3 is **no longer the lean**: it comes from BAAI (China), which
conflicts with the no-Chinese-software rule (coding_guidelines §3). See `docs/tom_spec.md` for the
proposed bake-off.

Low-confidence vectors (coding_guidelines §1): a vector made by a compressed copy of the chosen
model is stored with `confidence = low` and queued for regeneration on a capable computer. A
different, smaller model never stands in.

## 8a. Works not obtained (PROPOSED, 2026-11-27)

All of this is **PROPOSED**; nothing is built.

- **Mention flow (PROPOSED):** a mention goes to Jack, who only fetches metadata and neither knows
  nor cares whether the item is owned. Jack returns the results to Daniel, and Daniel activates the
  other windups for their refinements. For a work that is `not_obtained` (or `paywalled`), Larry
  (hashing) and Tom (full-text vectors) are blocked; metadata-level low-confidence vectors (below)
  are fine.
- Obtaining is Charles's job (`docs/charles_spec.md`, PROPOSED), on demand only.
- Daniel may catalog a work the owner doesn't have, found anywhere (archive.org, Amazon, the
  Library of Congress, ...), for example a book mentioned in a video. The work record has no copy
  or file, is marked `not_obtained`, and keeps its source URL(s) and the date each was retrieved.
- Whatever the source offers (metadata, description, table of contents, free preview text) is
  indexed and vectorized as `confidence = low`. The mention links to the work.
- When the item is obtained, Larry hashes it, Tom vectorizes the full text, and the record is
  upgraded in place. Nothing is downloaded until it is needed.
- Walter may flag dead or changed source links by adding items to Julie's task list.
- Metadata only, and each source's terms are respected. Amazon still needs the API check first
  (jack_spec).
- Status of a work: `obtained`, `not_obtained` or `paywalled` (PROPOSED), so the edge of what can be
  obtained is visible.
- First sources (PROPOSED, a suggestion the owner may change): archive.org and the Library of
  Congress. Who adds works: answered by the mention flow above.

### Reference trees for papers (PROPOSED, 2026-11-27)
- A paper's references become mentions. Jack resolves them (DOI first); Charles fetches the open
  copies.
- Free citation data (OpenAlex, Crossref) maps the tree before anything is downloaded.
- Depth cap in config, default 2 to 3. Open: the exact default.
- Each paper is `obtained`, `not_obtained` or `paywalled`, so the tree's edge is visible.

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

## 12. Answers (2026-11-27) and open questions

Answers are marked DECIDED or PROPOSED as given; a PROPOSED answer is not built until confirmed.

1. **Storage layout.** **OPEN, current direction (2026-11-27, awaiting confirmation; replaces the
   two-file proposal):** **one encrypted database** in which every record has a **sensitivity
   level**, government-style (for example a diary at the highest level, a photo at the lowest).
   Each windup gets a **clearance** for its role, and Daniel enforces it whenever he serves a read.
   All windups are trusted as a group; the levels limit the damage if one is compromised. Nothing
   leaves the windups except to the user, at any level. Keys live in the OS keychain
   ([identity_and_security.md](identity_and_security.md)); the file lives in the windups folder in
   the platform's app-data location (not the home folder). Daniel is the only writer. Open
   sub-questions: the level names and how many there are; the default clearance of each windup;
   who assigns a record's level. Today's code supports any layout through `[catalogs]`.
2. **Who settles an `undecided` purpose?** Open. (Julie is the judge, see Q12; does that cover
   purpose?)
3. **Cast and crew.** **DECIDED (2026-11-27):** cast and crew are **links from media to people, with a role**.
   Person details live only in the people catalog. Open: how today's media.db `entities` rows
   move over (a Dec 3 task).
4. **How far does Daniel's `find` go once Steve exists?** Open.
5. **Japanese and similar text in FTS.** Open.
6. **Licence for very large corpora.** Open.
7. **Tom's model.** See `docs/tom_spec.md`: bge-m3 is dropped (no-Chinese-software rule). **PROPOSED (2026-11-27):**
   candidates multilingual-e5-large, EmbeddingGemma and arctic-embed-l-v2.0. **DECIDED:** test, then
   pick (see `docs/tom_spec.md`).
8. **When SQLite vectors stop being enough.** Open.
9. **Map geometry.** Open.
10. **Mentions while Sammy is missing.** Open: confirm that chunks simply stay `mentions = 'pending'`.
11. **Who reads photo EXIF.** **PROPOSED (2026-11-27):** (agreed) **Jack reads** photo EXIF and other in-file
    metadata; **Lauren only writes** it.
12. **Links between catalogs.** **DECIDED (2026-11-27):** Walter checks cross-catalog links,
    including dangling ones and possible new ones, and only reports: he adds items to Julie's task
    list. **Julie decides.** How Walter reaches the catalogs follows from Q1.
13. **Julie.** **DECIDED (2026-11-27):** Julie is the judge: she runs the best model, can query other windups and
    sees the whole picture. Daniel still does all writing, and windups still work without her.
14. **Tasks.** **DECIDED (2026-11-27):** tasks (planned actions such as Suzan's, and Julie's task
    list) live in their **own separate operational database**, apart from the catalogs. See
    `docs/suzan_spec.md` for the flow.
