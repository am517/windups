# Proposals for discussion (answered 2026-10-27)

These were answered on 2026-10-27. Decisions, the shared schema and the open questions now live in the
main `windups` repo (WINDUPS.md, schema/, docs/, QUESTIONS.md). The text below is kept as history.
Done in Jack: no sidecars, filename_max 255 (chars on macOS, bytes on Linux), longest description wins.

These follow NEXT-SESSION.md. Each section ends with my recommendation and the questions I need answered before coding.

## 1. Shared database

**Options**
- **A. SQLite + sqlite-vec + local embeddings (fastembed) (recommended).**
  - One file, no server, readable from any language.
  - sqlite-vec is a small extension that needs to be loaded. fastembed downloads a model of about 30–130 MB on first use.
- **B. SQLite only; vectors later.**
  - Lean, with no model download.
  - Text search via FTS5 now. Leave a nullable `embedding` column so vectors can be added later.
- **C. JSON files per media (like `.meta.json`) plus an index.**
  - Simple and easy to diff, but cross-linking and querying are painful.

**Sketch of the schema (versioned with `PRAGMA user_version`)**
- `media`: id, kind (movie, show, season, episode), parent_id, title, original_title, year, season, episode, language.
- `external_ids`: media_id, source (tmdb, tvdb, imdb, wikidata, tvmaze, …), value. Unique on (source, value).
- `fetches`: id, media_id (nullable, for wrong or unmatched results), source, url, fetched_at, raw_json, chosen (bool).
  - This keeps everything that was downloaded, including wrong and alternative results.
- `art`: id, media_id, source, kind, language, width, height, url, local_path, sha256, chosen.
- `texts`: media_id, field (title, description, subtitle), language, source, text, embedding (vector).
- `groups` and `media_groups` (the "universe": Marvel, Ghibli, …): name, wikidata_qid, source, plus links.
- `genres` and `media_genres`: normalized name plus links.
- `files`: path, size, mtime, hash, media_id. This lets a renamed file still be recognized.

**Questions**
1. Location: `~/.local/share/windups/media.db` (shared, recommended) or `~/.config/windups/jack/` (as you wrote)? The config folder is usually for settings, not data.
2. Option A now, or B now and vectors later? B is my lean pick to start. A once a search feature needs it.
3. Should raw downloads (JSON and every art image) be kept forever, or with a size or age limit? The art could reach several GB.
4. Should the database be the source of truth, with `.meta.json` and sidecar files becoming exports, or should both stay?

## 2. Services

| Service | Free? | Use | Notes |
|---|---|---|---|
| TVmaze | yes, no key | TV episodes, air dates | Good TheTVDB fallback; rate limit 20 req/10 s |
| Wikidata | yes, no key | universe/franchise, cross-ids (IMDb/TMDB/TVDB) | Best source for grouping |
| Wikipedia | yes | longer descriptions | Use with Wikidata |
| iTunes Search API | yes, no key | Apple artwork (big, clean), Apple genres/ratings | Unofficial limits (~20/min) |
| YouTube Data API | free key, daily quota | trailers | Optional |
| Google Play | no official free API | — | Skip |
| OMDb | free key, 1,000/day; paid beyond | IMDb/RT/Metacritic ratings | Free tier, so keep it? |
| IMDb | scraping, non-free datasets for commercial use | — | Jack never scraped it. It only uses IMDb ids (via TMDB/OMDb); keep those ids |

**Questions**
5. Does "non-free" rule out OMDb, which has a free tier with a key but paid tiers above it? I recommend keeping it, as it is the only source of ratings.
6. Order to add: (1) Wikidata (grouping), (2) TVmaze, (3) iTunes art, (4) YouTube trailers. Agree?
7. Should Wikipedia text be a description fallback only, or a separate `long_description`?

## 3. Sammy / `--smart`

**Options**
- **A. `--smart` sends the candidates (descriptions, titles) to any OpenAI-compatible endpoint (recommended).**
  - Config: `[smart] base_url, model, api_key`. This covers Ollama, LM Studio and OpenAI.
  - It returns a choice plus reasons, which are logged.
  - It is off by default, and the plain merge rules stay as the fallback.
- **B. Sammy as a separate windup.** It takes JSON with candidates on stdin and returns JSON with a choice on stdout. Jack calls it if it is installed.

My recommendation: do A now, using the same JSON contract that B would use. Sammy can then slot in later without changing Jack.

**Questions**
8. Which fields should smart choose? Description only, or also title, episode title and art?
9. Is it OK to send metadata (not files) to a remote API when a user configures one?

## 4. Jack vs Lauren roles

**Options**
- **A. Strict split (already approved in principle).**
  - Jack fetches and outputs versioned JSON.
  - Lauren tags, embeds art, renames and sorts.
  - `jack` keeps a one-step convenience mode that calls `lauren`.
- **B. Jack as a meta windup.** It orchestrates Sammy and Lauren and owns no file I/O itself.
- **C. Status quo.** Jack does everything.

My recommendation: A. Move the code in steps:
1. Define `windups-media.schema.json` v1, based on today's `.meta.json`.
2. Add `jack --json` as the canonical output.
3. Create Lauren from `tagger.rs` and `sorter.rs`.
4. Make `jack` (default) call `lauren` when it is on PATH and fall back to the built-in tagging, then deprecate the built-in tagging later.

**Questions**
10. Should Lauren be a separate repo and formula (`windups-lauren`) or a second binary in this repo for now? Separate is cleaner. One repo is faster while the schema settles.
11. Schema ownership: should there be a shared `windups-schema` repo, versioned with semver, that all windups depend on?
12. Should the shared rules (one job per windup, JSON in and out, `--json` / `--version` / `--config`, config under `~/.config/windups/<name>/`, exit codes) be written up as a `WINDUPS.md` standard now?

## 4b. Separate Lauren (tags/art) and Sally (rename/move/sort)?

**My honest view: yes, split them.** This is the one split that clearly pays off.

**Why it pays off**
- Tagging and moving are independent jobs. People rename files they can't or won't tag (AVI, or files tagged elsewhere), and tag files they keep in place (the default today).
- Sally becomes useful for any file type, beyond media (music, photos, documents), from any metadata JSON.
- The risks differ:
  - Tagging rewrites file contents but keeps the path.
  - Moving changes paths, and its failure modes are collisions, cross-disk copies and permissions.
  - Each can have focused safety rules and tests.

**Costs**
- Three binaries to install and keep in step on the schema.
- The path changes partway through the pipeline, so every stage must pass the updated record on.
- Users need a one-step mode. `jack` can run the pipe for them.

**Pipe sketch: newline-delimited JSON, one record per media file**

```
jack scan ~/Downloads --json | lauren --json | sally --json > log.jsonl
```

Record (schema `windups.media/1`):

```json
{"schema":"windups.media/1",
 "path":"/path/to/video.mp4",
 "kind":"movie",
 "ids":{"tmdb":"1234","imdb":"tt0251075"},
 "meta":{"title":"Evolution","year":"2001","genre":"comedy,sci-fi","original_title":"…"},
 "art":{"chosen":"/…/video.mp4.poster.jpg","candidates":[{"provider":"tmdb","kind":"poster","url":"…","lang":"en","w":2000,"h":3000}]},
 "status":[{"stage":"jack","ok":true}],
 "dry_run":false}
```

**How each stage behaves**
- **jack:** fetches and emits records. It never touches files.
- **lauren:** reads records and writes tags and art. It re-emits the record with `status += {"stage":"lauren","ok":true,"changed":["title","covr"]}` and does not change the path.
- **sally:**
  - Renders the templates from `meta`.
  - Moves or copies the file and its sidecars (`.config`, `.meta.json`, `.poster.jpg`).
  - Re-emits the record with `"path":"<new>"`, `"previous_path":"<old>"` and a status entry.
  - Is last in the pipe, so no later stage needs the old path.
  - If the database exists, it updates `files.path` there.

**Dry run**
- `dry_run: true` in a record, or `--dry-run` on any stage, passes down the pipe.
- In a dry run, Lauren reports the tag commands and Sally reports `"target":"…"` without acting. Every stage still emits records, so the end of the pipe shows the full plan.

**Failure handling**
- A stage that fails on a record does not drop it. It emits the record with `{"stage":"lauren","ok":false,"error":"…"}`.
- Later stages skip records that have an earlier failure, unless run with `--force`. A file that failed tagging is not filed away as if it were done.
- Exit codes: 0 when all records are OK, 1 when some records failed, 2 for usage or config errors.
- Sally never overwrites. On a collision it fails the record (or, with `on_conflict = "suffix"`, adds " (2)").
- A cross-disk move is copy, verify size (and optionally hash), then delete.

**Undo**
- Sally's output log (`previous_path` → `path`) works as an undo file: `sally --undo log.jsonl`.

**Questions**
14. Should the split be Lauren plus Sally as above, or a single Lauren with a `--sort` mode? I recommend separate.
15. Should Sally move the sidecars and the art file along with the media? I recommend yes.
16. Should the one-step `jack` (default) keep working without Lauren and Sally installed, using a built-in fallback, or require them? I recommend keeping the fallback during the transition, then requiring them.

## 5. Small items

17. `filename_max` defaults to 120 characters (extension kept). Is that the right default, or should it be 0 (off)?
