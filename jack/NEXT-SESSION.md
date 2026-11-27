# Note for Coding Bot: next Jack session (from the owner, 2026-10-25)

Read this before starting work. Design items marked "discuss" should be iterated with the owner before implementing.

## Config file
- name, year, grouping (and other per-file values) are per-file only; remove from main config, or default them to blank if they must exist.
- Clean up config comments: short, simple, concise, for the user only. No internal dev notes.
- Proper spacing/formatting between variables for easy reading.
- Add `filename_max` alongside `short_description_max` (TV episode names can run on).

## Shared database (discuss design first)
- Location: ~/.config/windups/jack/
- Store everything downloaded, even auxiliary (unkept files, multiple results, wrong results).
- One entry per media; metadata includes online DB ids.
- Cross-linking vector table (subtitle, title, description) - do we need special libraries?
- All art.
- Cross-linking grouping table ("grouping" = "universe": Marvel, Ghibli, Batman, etc.).
- Cross-linking genres table.
- Will be shared with other windups, so design it more complete than is readily apparent.

## Services
- Add if possible: TVmaze, Wikipedia, YouTube, Apple store, Google Play.
- Remove non-free: IMDb, and any others that aren't free.

## Sammy (future windup, optional)
- Tiny LLM or decision model to choose the best metadata from sources, especially the description.
- Small but useful to future windups. Maybe `--smart` to hook into Sammy or a user-provided LLM (OpenAI-compatible API). Not required now.

## Windup architecture (think about it)
- Windups call other windups, so keep them open, standard, universal. A GUI over windups may come later.
- Should Jack only fetch metadata, with Lauren owning file manipulation (writing tags, organizing files)?
- Or is Jack a meta windup that calls Sammy and Lauren?
- How focused should each windup be? With potentially 1000 windups in a community, define roles thoughtfully now.

## Grok Bot's suggestions (approved 2026-10-25)
- Split roles: Jack only fetches metadata and outputs standard, versioned JSON. Lauren consumes that JSON and does file work (tags, art embedding, renaming, sorting). Jack may offer a convenience command that calls Lauren to keep the one-step workflow.
- Windup rule of thumb: one job per windup, JSON in/out, versioned schema, composable via pipes. Enables a future GUI and reuse.
- Database: SQLite + `sqlite-vec` extension for vectors (no server). Local embeddings via a small model (e.g. fastembed from Rust), no API needed. Since it's shared across windups, place it at something like `~/.local/share/windups/media.db` rather than under jack's config dir. Design and version the schema before coding.
- Services: Wikidata (structured Wikipedia) is ideal for grouping/universe (franchise links). TVmaze is free. Apple via the free iTunes Search API. Skip Google Play (no free official API). YouTube Data API is free with key and daily quota.
- IMDb: drop scraping, but keep IMDb IDs as the universal cross-reference (supplied free by TMDB and OMDb).
- Sammy: build `--smart` first as a plug-in point for any OpenAI-compatible API (covers local servers like Ollama). Sammy can later be a small local model behind the same interface.
- Config cleanup and `filename_max`: straightforward, do them.
