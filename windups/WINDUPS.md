# The Windups standard (draft 1, 2026-10-27)

## Origin

The windups come from Adam's view of the mind as a hive of many small selves, each with its own
identity and drive, negotiating influence and woven together into one.

A windup is a small program that does one job well. Windups call each other; a "super" windup
orchestrates others. This file is the contract. Anything shared lives in this repo.

## Cast list

- **Jack**: fetches and verifies metadata for a file, keeps it in his own `jack.db` and syncs it to Daniel.
- **Daniel**: the record keeper; owns the shared catalog (media.db), its schema and all writes to it. Receives synced records (`daniel::receive`, or `daniel ingest`), asks Larry and Ruth, stores.
- **Lauren**: writes tags and embeds art from the database (takes over from Jack; Jack keeps a fallback).
- **Suzan**: write-to-file; at most one open Suzan task per entry.
- **Mary**: summaries (before Tom only; never blocks Larry).
- **Kate**: image conversion (JPEG to embed, JPEG XL to archive).
- **Tom**: vectors/embeddings for meaning search.
- **Larry**: hashing: exact BLAKE3 (partial first, full later) and the perceptual fingerprint.
- **Julie**: verification against evidence; calls Sammy when judgment is needed.
- **John**: device group (pair, list and remove, keys, trust); comes just before Walter.
- **Walter**: the auditor; reads every windup's DB (read-only), finds gaps, unsynced records, stale backups and unfinished tasks, and sends the right windup back to fix them; keeps no full duplicate archive. Called by windups, then exits.
- **Sammy**: the shared LLM thinking service (local or remote models); judgment when evidence alone can't decide.
- **Steve**: search the database.
- **Sally**: organizes files (rename, move, sort).
- **Tammy**: super windup; ingests new media by orchestrating the others.
- **Margaret**: directory of windups and their capabilities (PATH + `--capabilities` is enough until she is needed).
- **Oliver**: matchmaker; breaks requests down and teams windups up; when a windup is missing, asks Ben to fetch and install it.
- **Flora**: the user-facing talker (terminal, GUI app, OS assistant) and the receptionist. She understands via Sammy, asks Margaret who can help, hands off via Oliver, and reports back. A personality LoRA matters for her. She comes later in the build order, after the file-pipeline windups and after Sammy.
- **Ruth**: builds the APA 7 work reference and work key.
- **Ben**: rebuilds the windups for new platforms; fetches and installs a missing windup when Oliver asks.
- **Rover**: refines other windups, bound by each one's purpose and success checks; proposes and tests, never applies unreviewed.
- **Beth**: trains LoRAs from each windup's training phrases and parameters (later; until then LoRAs are made manually).

## Roster

Every named windup has a crate. The name registry is [`NAMES.md`](NAMES.md).

| Windup | Job | Status | Position |
|---|---|---|---|
| Jack | fetches and verifies metadata for a file, keeps it in jack.db and syncs it to Daniel | building | 1 |
| Daniel | record keeper: owns the shared catalog, its schema and all writes; receives synced records (library call, or `daniel ingest`) | own crate (real store + ingest CLI) | 2 |
| Lauren | writes tags and embeds art from the database (takes over from Jack; Jack keeps a fallback) | stub crate | 3 |
| Suzan | write-to-file; at most one open Suzan task per entry | stub crate | 4 |
| Mary | summaries; runs before Tom only, never blocks Larry | stub crate | 5 |
| Kate | image conversion: JPEG to embed, JPEG XL to archive | stub crate | 6 |
| Tom | vectors/embeddings for meaning search | stub crate | 7 |
| Larry | hashing: exact (BLAKE3, partial first, full later) and perceptual | stub crate (hashing works, perceptual placeholder) | 8 |
| Julie | verification: checks entries against evidence, calls Sammy when judgment is needed | stub crate | 9 |
| John | device group: pair, list and remove devices, keys, trust checks | stub crate | 10 |
| Walter | auditor: reads every windup's DB, finds what is missing or stale, sends the right windup back | working | 11 |
| Sammy | shared LLM thinking service (local or remote models); judgment when evidence alone can't decide | stub crate | 12 |
| Steve | search the database (genres, people, dates, full text, meaning; graph later) | stub crate | 13 |
| Sally | organizes files (rename, move, sort); takes over Jack's built-in sorting (Jack keeps a fallback) | stub crate | 14 |
| Tammy | super windup: ingests new media by orchestrating the others | stub crate | 15 |
| Margaret | directory: knows every windup and its capabilities, points callers to the right one | stub crate | 16 |
| Oliver | matchmaker: breaks down messy requests, teams windups up; when a windup is missing, asks Ben to fetch and install it | stub crate | 17 |
| Flora | user-facing talker (terminal, GUI app, OS assistant) and receptionist: understands via Sammy, asks Margaret who can help, hands off via Oliver, reports back; a personality LoRA matters | stub crate | 18 |
| Ruth | references: APA 7 work reference and key from gathered facts | library (real code) | — |
| Ben | builds: rebuilds the windups for new platforms; fetches and installs a missing windup when Oliver asks | stub crate | — |
| Rover | refining: proposes and tests improvements to other windups, within their profiles; never applies unreviewed | stub crate | — |
| Beth | trains LoRAs from each windup's training phrases and parameters | stub crate | — |

## Build order

One sequence. The simpler file-pipeline windups (for example Lauren, Suzan, Larry's hashing, and Kate's image conversion) are easy wins that help replace iVI4 Pro sooner; Sammy and Flora are more complex and come after.

1. Jack (done-ish)
2. Daniel (current)
3. Lauren
4. Suzan
5. Mary
6. Kate
7. Tom
8. Larry
9. Julie
10. John (just before Walter)
11. Walter (already working)
12. Sammy
13. Steve
14. Sally
15. Tammy
16. Margaret
17. Oliver
18. Flora

Ruth, Ben, Rover and Beth sit outside this numbered sequence (Ruth and the stubs already exist; Beth trains LoRAs later). See `TODO.md` for the same list with notes, and `NAMES.md` for the name registry.

## Rules

1. **One job per windup.** If a task has two jobs, it is two windups.
2. **Every windup is its own repo** (`am517/<name>`), Homebrew formula `windups-<name>`, MIT.
3. **One windup at a time.** Jack and this repo first; a new windup starts only when the current one is complete.
4. **Standalone.** Every windup works with no other windup installed, using a sane default when a
   helper can't be found (Jack without Sammy: longest summary wins; Tammy: "I'm sorry, I can't figure
   this out right now"). Helpers make results better, never required.
5. **All windups can call all windups.** A windup knows who to talk to without being told (see
   docs/communication.md). No user pipes (`a | b | c`) are needed or expected.
6. **The database is the source of truth.** All metadata enters `media.db`, then gets written to the
   file. **No sidecars**: a windup never reads `<file>.*` side files (an explicit `--download-only`
   export is the user's business; nothing reads it back).
7. **Efficiency above all**, especially in inter-windup communication.
8. **Never err on the side of simple; it's OK to err on the side of later.** When we build a windup we
   build it as well as we can. Design so later features (e.g. complex search) aren't blocked.
9. **Privacy: nothing leaves the machine unless the user opts in** (`allow_remote_metadata`).
   Fetching from public databases is a lookup, not sharing; sending our metadata to an API (e.g. a
   remote LLM) is sharing.
10. **Free services only**, where "free" includes a free tier (OMDb is fine).
11. **Secrets** never in git, logs, dry runs, errors or Debug output; masked when shown.

## Files and settings

| What | Where |
|---|---|
| Windup config | `~/.config/windups/<name>/config.toml` (`--config` overrides) |
| Shared settings | `~/.config/windups/windups.toml` (template: config/windups.toml) |
| Shared database | `~/.local/share/windups/media.db` (schema/) |
| A windup's own database | one SQLite file in its folder, beside its config (Jack: `jack.db`) |
| Raw downloads (art, API responses) | `~/.local/share/windups/raw/`, pruned per `raw_max_age_days` |

- Every tweakable is a config value, never hard-coded. Config comments are short and for users.
- Precedence: flags > windup config > shared settings > built-in defaults.
- Every on/off flag has a `--no-X` form; `--set` saves flags into the config.

## Command line

- `--help`, `--version`, `--config <path>`, `--dry-run` (changes nothing, shows everything), `--set`.
- `--print` / `--print-format json` for humans and scripts (stdout = data, stderr = report).
- Exit codes: 0 all OK, 1 some items failed, 2 usage or config error.

## Quality bar

- Rust 2024, `cargo test` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Tests use fakes for the network; integration tests skip when external tools are missing.
- macOS first, Linux supported, Windows not a goal. File/folder names max 255 (characters on macOS, bytes on Linux).

## Design rules (Adam, 2026-10-28)
- **Treat each windup like a person.** When the right behavior is unclear, ask what a person would do. Example: like a person keeping their own notebook, Jack writes every record into his own `jack.db` first and keeps it after giving Daniel a copy; with no database reachable nothing is lost and nothing is jotted beside the media.
- **Modular.** One file does one thing (within reason); small focused modules.

Naming: traditional American English names for now (Adam, 2026-10-28).

## Core rule: no guessing (Adam, 2026-10-28)
Windups never best-guess or try to be "smart". A windup acts only on sure decisions, when it has enough information
(e.g. name + year, an id like IMDb/TMDB/TheTVDB, a source URL or yt-dlp info.json, or an unambiguous exact match).
Otherwise it plainly says "not possible with the available information", keeps everything (all candidates and raw
data) as unresolved, and changes nothing. Ambiguous cases are for a future LLM windup, through a resolver hook.

## Profiles (Adam, 2026-10-28)
- Every windup has `PROFILE.toml` in its repo root: the source of truth for who it is and what it does
  (name, role, summary, kinds, sources, reads, writes, never, hands_off, command).
- `windups-<name> --capabilities` prints exactly that file (compiled in with `include_str!`).
- It is copied into media.db's `windups` table and embedded (table `embeddings`, owner `windup`)
  so Margaret and other windups can find the right windup by meaning.

## Unresolved, names and conflicts (schema v2, 2026-10-28)
- Unresolved items live in `unresolved` with a reason from `unresolved_reasons`. Time-based reasons
  (no_match, ambiguous, source_error) are retried after `unresolved_retry_days` (default 7) by Walter
  (`pending` / `patrol`); there is no separate tidy-up windup. Resource-based reasons (conflict, needs_llm_windup) wait for the windup that can settle them.
- Every source's name is kept untouched in `source_names`; `display_name` is the user's own name
  (auto-filled from a source name, `display_name_set_by` = `auto:<source>` or `user`).
- When sources about the same work disagree, a `conflicts` row records all values. Nothing is settled by assumption.
- A file's own embedded metadata is read first and always kept as source `embedded`.
  It is the truth for facts, not wording. A source's value replaces it only when it says the same
  thing written differently (case, spacing, `.`/`_`/`-`), or when evidence confirms the same
  entity. Only an id is proof (one shared with the file, or one the user gave: --imdb/--tmdb/--thetvdb).
  The user's own word (--name with --year, or their channel mapping) settles names but not identity.
  A matching name or year is a strong hint for checking, never confirmation. Similar wording
  is never proof ("Joe Rogan" vs "The Joe Rogan Experience"). Without evidence the file's value stays,
  the file is still tagged with it, a conflict is recorded and the item stays open (`needs_flags`, or
  `needs_llm_windup` for exact facts). Exact facts (year, date, season, episode) are never replaced.
  Originals are always kept.
- Social videos with a matched show get the show as their own parent media row.

## The guiding test

Act like a smart intern from a foreign country: capable with resources, but lacking shared cultural
knowledge. Make no assumptions. When the information is not enough, skip the step or mark it for
research; never guess.

## Search: one call, the whole list

One API call per source. A search returns a list in that one call; the whole list is stored as
candidates (source `<provider>:search`), not just the top hit. A title without a year is a hit only when
exactly one normalized-exact match exists across all returned lists and the sources agree (same year);
otherwise the item is unresolved: `ambiguous` (one list has several exact matches) or `needs_flags`
(sources disagree, or the details fetched are for a different hit), with a hint such as `--year`/`--imdb`.

## Keep everything

Every item is stored, even when incomplete: failed lookups, unreadable names and unresolved matches get
an entry marked incomplete (an open `unresolved` row with its reason) and all candidates. A windup never
silently drops an item. Each one is reported to the user as it happens and again at the end of the run,
with the reason and what would help. Without an LLM, the text comes from static templates in config
(`[explain]` in jack.toml) behind a swappable `Explainer` (one file), so Sammy can later explain better.

## Escalation at every step

At every step (parse, lookup, match, refine, conflict, ...) a windup 1) tries a deterministic,
unambiguous answer; 2) if still ambiguous, hands everything it has to an `Escalation`: step kind plus
full context (reason, detail, merged metadata, every source answer and search hit, the file's own tags)
in; a decision (values plus a note) or a deferral out. Decisions are recorded with provenance in the
`decisions` table (source = `sammy`, step, timestamp, values, note). Until Sammy exists the stub
`NoSammy` always defers, so the fallback is: keep the item as incomplete and explain it with the static
template. Reason `needs_flags` (schema v3) is for items only the user can settle with a flag.

## Review pass (optional)

After a run, if `[review] enabled` and a reviewer is available, each configured field is reviewed
across all items in chunks, with minimal context (e.g. title + year beside a description). The reviewer
(Sammy, `Reviewer` trait; stub `NoReview` is never available) returns flags, not rewrites. Flags are
logged with provenance (source, step=review, time) and reported as open items (`review_flag`); any
change still goes through the sure-or-log rules above.

## Lifecycle: identification is a process, not a goal (schema v4)

Every file gets an entry, even when nothing could be identified, and entries walk stages over time:

| stage | meaning |
|---|---|
| `undetermined` | file purgatory: only the file's and the user's own values (lookup failed or not done) |
| `not_identified` | looked up, but no answer could be settled on (the open `unresolved` row says why) |
| `unverified` | matched by strong hints (name, year, user's --name/--year, channel mapping), not by an id |
| `incomplete` | identified by an id; something is still open (a conflict, a needed flag) |
| `complete` | identified by an id; nothing open |
| `merged` | folded into another entry (`merged_into`); final |

- Stages and allowed moves are rows (`lifecycle_states`, `lifecycle_transitions`): add a stage without
  code. Any working stage may move to any other (an id can turn out wrong), or be merged; merged is final.
- Every move is kept in `status_history` (from, to, actor, reason, time).
- Entries are never deleted (a trigger refuses it) and ids are never reused. An entry is identified in
  place (a re-run of the same file updates the same id) or merged into another: files and ids move over,
  the old entry stays with `merged_into` set, and `merges` records who, why and when. Lookups follow
  `merged_into` to the entry that stands now.
- `unresolved` stays the list of open questions (why, and what would help); the stage says where the
  entry is. Migration v4 put existing entries on `not_identified` (open unresolved) or `unverified`.
- A run counts entries by stage. Incomplete entries are kept, not failures; a failure is something a
  windup could not do at all (e.g. could not write a tag).
- Year tolerance (`[matching] year_tolerance`, default 1) lets sources a year apart agree while
  matching (release dates differ by country). It is a hint only, never evidence.

## Completeness, the user's word, and finding the same thing again (schema v5)

**Complete** means: id-verified, nothing open, and every *required* field for its media kind present.
The lists live in config (`[completeness]` in jack.toml; defaults: description required everywhere it
applies, cast optional). Otherwise the entry is *incomplete* and `missing_fields` lists what's absent.

**The user's word.** `--name`/`--year` and a channel mapping are used, but the entry stays *unverified*
(only an id proves identity) and a decision with source `user_asserted` records it, for Sammy or the
user to review later.

**Same file, same work, maybe the same thing.** Three different questions, three answers:

| Layer | Stored in | Means | Acts on its own? |
|---|---|---|---|
| Exact hash (BLAKE3) | `files.hash` (`b3:` whole file) | identical bytes: **proof** | yes: a file whose old path is gone is re-found under its new path, same entry (also undetermined ones) |
| Perceptual fingerprint | `files.fingerprint` | same picture/sound after re-encode, resize, crop, rotation, aspect change: **hint** | no: opens a `possible_same` row in `proposals`; never merges |
| Work key | `works`, `media.work_key` | different versions/encodes of the same work | groups only |

- *Hash mode.* `mode = "full"` (default) hashes the whole file; BLAKE3 runs at disk speed, so the cost is
  one read per new or changed file. `mode = "partial"` (`b3p:`: size + start/middle/end chunks) reads a
  few MiB instead — near-certain for video, but a hint, not proof. A copy whose original still exists is
  its own file, not a re-find.
- *Fingerprint.* Behind the `Fingerprinter` trait (jack `fingerprint.rs`); thresholds in `[fingerprint]`
  (`frames`, `audio`, `max_distance`). The shipped `NoFingerprint` stub produces nothing; off by default.
  Proposals stay `open` until the user or Sammy sets `accepted`/`rejected`; a merge is a separate act.
- *Work key.* `wk1:` + BLAKE3 of an APA 7-style reference, lower-cased with punctuation and spacing
  normalized: film `Title. (Year). [Film].`; episode `Show (Season s, Episode e) [TV series episode]. (Year).`;
  series `Show. (Year). [TV series].`; online video `Channel. (Date). Title [Video]. Site.`
  Known ids (IMDb, TMDB, TheTVDB, TVmaze, Wikidata, YouTube, X) are stored beside the key in `works.ids`,
  not inside it, so a newly learned id doesn't change the key. Only identified entries get one.

## Finishing well: nothing incomplete is forgotten (schema v6)

Windups care about finishing. They may pause, or accept a partial answer for now, but they always
come back until the entry is truly complete. Every entry that isn't `complete` has a row in `returns`:

- **Time-based:** `next_retry_at` (UTC), from `[retry] backoff_hours` by attempt (default 1 h, 1 day,
  1 week, then monthly). With neither a time nor a resource configured it still returns in a day.
- **Resource-based:** `waiting_for` names a capability (`[retry.wait_for]` per stage; default
  `not_identified = "sammy"`; a provider name or `new_source` also work). When it appears, matching
  items become due at once (`returns::wake`). Walter wakes them from what's present now:
  registered windups (the `windups` table, filled from `--capabilities` profiles) and configured
  providers; `--wake CAP` names one by hand.
- Whichever comes first. Reaching `complete` (or being merged away) clears the return.

CLI: now Walter's, see below (`walter pending`, `walter patrol`). Jack no longer schedules.

## Round 6 (Adam, 2026-10-28): ids, references, and who does what (schema v7)

**Ids.** Every entry has one permanent local id (`media.id`), never changed or reused. Every work has
one permanent `works.id`; its key and reference may improve, the id never changes. Service ids (imdb,
tmdb, tvdb, youtube, …) sit beside them: `external_ids` per entry, `work_ids` per work. A shared
service id finds the same work; two works sharing ids become a `same_work` proposal, never a merge.

**Work reference.** Built from the most original information: original-language title and original
release, not translations; includes the episode title when known (`Title (Season s, Episode e) [TV
series episode]. In Show. (Year).`). Original title and language are stored when sources give them;
without an original language the reference is marked `provisional` and is upgraded later (never
downgraded). Online videos are never provisional.

**Architecture.** Jack ONLY fetches and verifies metadata for a file and hands one complete entry
(`windups::handoff::Handoff`) to Daniel (`windups::handoff::Daniel`). Processing lives elsewhere:
- **Daniel** stores it (own crate since round 7). The portable form is a versioned JSON file
  (`schema/handoff_v1.md`); Jack keeps every record in his own `jack.db` (and can
  `--emit-json`), and `daniel ingest` loads such files into media.db. The in-process `accept`
  path stays. Long term, Jack does not own database writes.
- **Larry** hashes: exact BLAKE3, `partial` by default (head + tail + size), full later as resources
  permit; and the perceptual fingerprint (still a placeholder). Daniel calls Larry before storing.
- **Mary** summarises, before Tom only. **Tom** embeds. **Kate** converts
  images (JPEG embedded, JPEG XL archive). All stubs with a trait and a test.
- **Walter** patrols (below).

**Look-alike fingerprint (note).** Releases differ in intros, jingles and cuts, so align videos
first, by audio preferably (cross-correlate audio fingerprints to find the offset), then compare
frames at matching times. Still a hint only: it proposes, never merges.

**Times** are UTC everywhere.

**No timers, only windups call windups.** No routines, cron or system timers. Walter examines entry
state (stage, latest reason, `last_checked`, its return) and decides when to call Jack again:
- `walter pending` only lists unfinished work (read-only; see Round 9): `#id path [stage] reason — next: <time> UTC or when X appears (attempts n)`.
- `walter patrol` discovers capabilities (windup profiles via `--capabilities`; their `sources`), wakes
  items waiting for anything new (a new source also wakes `new_source`), schedules returns, then calls
  Jack with the files of what is due (`--all` for everything pending; `--wake CAP` by hand).
- A return reschedules (with the next back-off) whenever the entry's `last_checked` differs from the
  one it was scheduled against; Jack stamps `last_checked` on every save, Walter after each call.
- Jack's `pending`/`retry` commands are gone; that logic lives in Walter.

## Round 7 (Adam, 2026-10-28): who calls whom

- **No daemon, no timers, portable.** Windups call Walter; he runs on the call, then exits.
- **Daniel is his own crate** (store code moved from Jack). Jack calls him only through
  `windups::handoff::Daniel`, passing the disagreements he found between sources (`conflicts`).
- **Ruth builds references** (code moved from Jack); Jack only gathers facts. A reference is
  provisional whenever it can't be fully built: a missing original title (or original show/episode
  title), original language, or episode title; `missing` lists what.
- **Verifying and thinking are separate.** Julie verifies against evidence; Sammy judges, and is a
  resource like any other. Waits name the windup waited on: `unverified` waits for `julie`,
  `not_identified` for `sammy`, `undetermined`/`incomplete` for `new_source`.
- **Mary** precedes Tom only; she never blocks Larry.
- Stubs (profile, README, `--capabilities`): Sammy, Julie, Margaret, Oliver, Ben; and Tom, Mary, Kate.

## Round 8 (Adam, 2026-10-28): Walter the data freak, Daniel the gatekeeper

- **Walter patrols the whole database whenever he is called, for any reason**: `patrol`, `note` (the
  same). (Round 9: `pending` no longer patrols.) Each patrol is stamped as `last_patrol` in
  `windup_state` (schema v8). Anyone may call him.
- **Daniel alerts Walter by default, only when it matters.** He counts changes since Walter's last
  patrol and their importance (`[importance]` weights: new entry 3, stage change 2, update 1) and
  calls `walter patrol --db DB` when one threshold in `[walter]` is reached: `max_changes` (25),
  `importance` (10) or `max_hours` (24) since the last patrol. A patrol by anyone restarts the count.
- **Loop guard.** Walter runs Jack as `jack --caller walter`; Jack carries `caller` in the handoff and
  Daniel counts that work but never alerts Walter about it.
- English film references stay provisional until a source gives the original title (confirmed).

## Round 9 (Adam, 2026-10-28): profiles with purpose, Rover, one repo

- **Every PROFILE.toml has a `purpose`** (one sentence: why the windup exists) and **`success_checks`**
  (a short list of concrete, testable statements, e.g. Jack: every file gets an entry; no guesses; only
  ids confirm identity). A windup is judged by its own checks, and **every check is a must-pass test**: each
  `[[success_checks]]` entry names its test (`test = "crate/src/file.rs::test_fn"`) or, for stubs only,
  is marked `pending`. `windups/tests/profiles.rs` fails the build when a check names no existing test.
- **Rover** (stub) refines other windups, strictly bound by each windup's purpose and success checks:
  he proposes changes, tests them against those checks, and never applies an unreviewed change.
  His universal rules: *leave it as you found it or better*; *better, faster, stronger* (never regress
  a windup's purpose, its tests or the rules).
- **`walter pending` is read-only**: it lists scheduled returns and nothing else (no patrol, no
  capability discovery, no Jack). Walter patrols only when called to patrol (`patrol` or `note`).
- **One repo**: all windups live in one Cargo workspace (github.com/am517/windups), one folder each.

## Efficiency rule (Adam, 2026-11-01)

Windups will run on phones and small devices. Aim for the most efficient solution at every step:
tight, fast, low-memory Rust with minimal deps and small binaries (release profile: LTO, opt-level,
strip, `panic = abort` where sensible). Avoid needless allocations and clones, stream files rather
than loading them whole, and benchmark hot paths.

## Sammy as a shared thinking service (Adam, 2026-11-01)

Sammy is a shared LLM thinking service (local or remote models). Each windup thinks for itself with
its own LoRA, keeping one LoRA per base model it has seen (keyed by model+version), and falls back
to the base model plus a system prompt until it has one. A list of available models is future work.
**Beth** trains LoRAs from each windup's own training phrases and parameters. Beth comes later;
LoRAs are made manually at first. **Flora**'s personality LoRA matters for how she talks with the user.

## Packaging: one binary, one portable store (Adam, 2026-11-01)

One shared executable with no self-modifying binaries, plus one portable SQLite file per windup
(e.g. `jack.windup`) holding config, LoRA blobs, training phrases and memory. Every windup gets the
universal commands `export` (to a readable TOML/JSON folder), `import`, and `show config`.

## A windup is an identity, not a file (Adam, 2026-11-01)

Copies on paired devices sync their `.windup` data in the style of Syncthing and share a job list.
Windups move only within the approved private device group and never spread on their own. **John**
manages the device group (pair, list and remove devices, keys, trust checks). John comes just before
Walter.

## Jack (Adam, 2026-11-01; supersedes the never-best-guess rule for Jack)

Before going online, Jack checks the DB. On an exact match (by id, or by name plus year), he updates
only what's needed. For a related item (same show, channel or podcast), he reuses its shared details
and records which entry he copied them from. He produces one versioned handoff JSON (`schema/handoff_v1.md`, `"v": 1`) per entry with his best
guess plus flags (`unresolved`, `ambiguous`) and passes it to Daniel (`daniel ingest`, or in-process
`accept`). Every record is kept first in his own `jack.db` (no beside-media file any more; superseded
2026-11-09); `--emit-json -` or `--emit-json DIR` writes the same
JSON to stdout or a directory. He saves good and bad hits and identified entities as-is, uncompleted, and completes only the
main search. Sources become plugins, and new sources are on hold. Cast/entities wait, and so does the
embedding model, which gets only a plugin hook until Sammy exists. User corrections and preferences
belong to other windups.

(The global "no guessing" rule still applies to every other windup. Jack alone may emit a best-guess
JSON with explicit flags for Daniel.)

## Daniel tasks table (planned; do not build yet) (Adam, 2026-11-01)

Each row: `entry`, `task_type`, `assigned` windup, `status`, `depends_on`, `confidence` (stays open
until 100%), `resources_used` (don't retry unless new resources exist), `claimed_by`, `claimed_at`
and a lease timeout. A claim atomically sets `status=active`. When Jack submits an entry, Daniel adds
all the tasks it needs and tracks which windup to call next. An entry is complete when it has no open
tasks, and it reopens when a new task type is added or a correction is made. Any task that changes an
entry adds a **Suzan** (write-to-file) task, with at most one open Suzan task per entry. User-tunable
config with sane defaults: `max_windups` (global), `max_instances` (per windup, overridable),
`priority` (nice level) and lease timeout. Daniel counts active rows before calling a windup.

## Daniel schema: canonical entities (Adam, 2026-11-01)

Canonical tables for entities (people and orgs), genres and groups, each with an alias table mapping
alias → canonical entry. Files store only canonical ids. Ambiguous aliases are flagged for the user,
never mapped automatically. Composite labels are data rules, for example
`anime = Animation AND Japanese`.

## Shared core, roles and culture (Adam, 2026-11-10)

- **Roles.** Daniel is the record keeper (owns the shared catalog, its schema and all writes).
  Walter is the auditor (reads every windup's DB read-only, finds gaps, unsynced records, stale
  backups and unfinished tasks, sends the right windup back; no full duplicate archive). Jack syncs
  to Daniel, not Walter: a library call now, a socket later; Daniel answers per batch with one result
  per record. This supersedes the in-process `accept` hand-off from Jack (one route now). The older
  "Walter the data freak / patrol" notes above describe his current code, not his role.
- **Shared core** (`windups::base`): universal commands and `talk` ([COMMANDS.md](COMMANDS.md)),
  append-only event log, checked snapshots, forward migrations, the folder layout
  ([LAYOUT.md](LAYOUT.md)), the sync-target trait and the read-only audit.
- **Culture.** Every windup follows [CULTURE.md](CULTURE.md); its personality
  ([NAMES.md](NAMES.md)) is style and priorities, never an exception.


## Answers to the core design questions (Adam, 2026-11-10, 12:21 AM ET)

1. **Personalities** stay proposals (except Daniel and Walter, set by Adam).
2. **Packaging:** per-user `~/windups/<name>/` with `~/.local/bin/<name>` a symlink. A .deb can't
   write into home folders, so it ships the binary and the first run (or `jack setup`) makes the
   folder and links ([LAYOUT.md](LAYOUT.md)). `~/windups/<name>/` is the default ahead of the old
   `~/.config/windups/<name>/` fallback; `--config` and `<NAME>_HOME` still win.
3. **Own folder only.** Snapshots go in `~/windups/<name>/backups/`. The off-device hook stays
   unassigned (a future backup windup); Walter only flags stale or missing copies.
4. **Daniel keeps alerting Walter** after storing records: as Data Fiend he takes every chance to
   look around. Walter's regular schedule is decided later.
5. **Built:** Daniel's socket transport (`daniel serve`, length-prefixed JSON,
   [COMMANDS.md](COMMANDS.md)) behind the sync trait, chosen by Jack's `[archive] sync_transport`
   (the library call stays), and `walter audit` on the audit hook (read-only, universal commands).
