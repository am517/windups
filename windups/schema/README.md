# Shared database (schema v1, draft)

`~/.local/share/windups/media.db`, SQLite + FTS5, with optional vectors via sqlite-vec
(`vectors.sql`; local embeddings, e.g. fastembed-rs). Versioned with `PRAGMA user_version`;
upgrades are numbered migration files added next to `media.sql`.

| Table | Holds |
|---|---|
| media | movies, shows, seasons, episodes (episode -> season -> show via parent_id) |
| entities | people and organizations as full entries (Johnny Depp, Studio Ghibli) |
| media_entities | who did what (actor + character, director, studio, ...) |
| external_ids | ids in online databases, for media and entities |
| genres / media_genres | genres (with sub-genres) and their links |
| groups / media_groups | universes, franchises, collections (Marvel, Ghibli) |
| relations | sequel, remake, based_on, ... (graph search later) |
| texts (+ texts_fts) | titles, descriptions, summaries: every language and source; full-text indexed |
| art | all art, chosen or not |
| ratings | IMDb, Rotten Tomatoes, Metacritic, ... |
| files | files on disk (path, hash), so moved files are still recognized |

Decisions (2026-10-27):
- **No fetches table.** Everything downloaded is a real row: wrong or alternative results are
  `status = 'candidate'` / `'rejected'` with `canonical_id` -> the chosen row; unchosen texts and art
  have `chosen = 0`. Raw responses (`raw_json`) and downloaded files (`art.local_path`) are pruned
  after `raw_max_age_days` (0 = forever).
- **Tables link both ways** through junction tables: `media_genres` answers both "genres of this
  movie" and "movies of this genre".
- **Built for search later** (Steve): indexes on kind/year/names, full-text on texts, sub-genres,
  entities with roles, relations for graph queries, vectors for meaning ("gay vampire movies").
  `examples.sql` shows the queries; `./check.sh` runs them.

Text layer (v9, [`text_layer.sql`](text_layer.sql)): `media_extracts` (derived text with its quality),
`media_chunks` (searchable spans with a locator) and `media_chunks_fts` (word index), and
`media_vectors`. Every vector names its model id **and version** (`embeddings.model_version` too);
a search never mixes them. Daniel's other catalogs (people, records, maps, photos) use the same
template; see [`docs/daniel_spec.md`](../../docs/daniel_spec.md).

The portable handoff file (one entry, written by Jack, ingested by Daniel) is documented in [`handoff_v1.md`](handoff_v1.md).
