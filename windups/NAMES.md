# Windup names

This file is the name registry: the source of truth for which windups exist.
The cast table in [`WINDUPS.md`](WINDUPS.md) should show the same names, roles
and build positions. `tests/names.rs` fails if a crate folder and this registry
disagree.

`Sampy`/`Dory` is a display model, not a windup, and does not belong here.
Unnamed ideas (a refusal monitor, a profile-managing windup) stay out until
Adam gives them a name and a role.

Every windup follows the society rules in [`CULTURE.md`](CULTURE.md), whatever its personality.

## Rules

1. One row per name, with its role, personality, status (`stub`, `active` or `done`), build
   position and the date it was added.
2. A new name lands in this registry and gets a stub crate in the same commit.
3. Never reuse or rename a name without a note below.
4. Build position is the numbered sequence in `WINDUPS.md` / `TODO.md`. A dash
   (`—`) means the windup sits outside that sequence (already working, or later
   support such as Beth training LoRAs).

5. Personality is one trait (a word or short phrase, Inside Out style) that shapes how the windup
   does its job: style and priorities, never an exception to [`CULTURE.md`](CULTURE.md). It also
   lives in the windup's `PROFILE.toml` (`personality`, `personality_status = "set"|"set (work in progress)"|"proposed"`);
   `[core] personality` in its config may override it. The prompt line is built by
   `windups::base::personality::prompt_line`: "Your name is X and you are <Trait>. Your job is Y."
   **(proposed)** marks a draft for Adam to choose. All 22 traits were set by Adam on 2026-11-10
   with `personality_status = "set (work in progress)"` (Daniel's Meticulous and Walter's Data Fiend
   were his first picks). Adam would eventually like Inside Out-style emotion names, so these may
   change.

## Roles decided (Adam, 2026-11-10)

- **Daniel is the record keeper.** He owns the shared catalog (media.db), its schema and all writes
  to it. Windups keep their own records in their own DB and sync them to Daniel (a library call
  today, a socket later); Daniel answers each batch with a result per record.
- **Walter is the auditor.** He reads every windup's DB (read-only, `windups::base::audit`), finds
  gaps, unsynced records, stale backups and unfinished tasks, and sends the right windup back to fix
  them. He keeps no full duplicate archive.
- **Jack syncs to Daniel, not Walter**: one route (no separate in-process hand-off).

## Notes (renames and reuse)

None yet.

## Registry

| Name | Role | Personality | Status | Position | Added |
| --- | --- | --- | --- | --- | --- |
| Jack | fetches and verifies metadata for a file, keeps it in jack.db and syncs it to Daniel | Curiosity | active | 1 | 2026-10-28 |
| Daniel | record keeper: owns the shared catalog (media.db), its schema and all writes to it; windups sync their records to him | Meticulous | active | 2 | 2026-10-28 |
| Lauren | writes tags and embeds art from the database | Artistic | stub | 3 | 2026-11-05 |
| Suzan | write-to-file; at most one open task per entry | Persistent | stub | 4 | 2026-11-05 |
| Mary | summaries; runs before Tom only, never blocks Larry | Articulate | stub | 5 | 2026-10-28 |
| Kate | image conversion: JPEG to embed, JPEG XL to archive | Utilitarian | stub | 6 | 2026-10-28 |
| Tom | vectors/embeddings for meaning search | Analytical | stub | 7 | 2026-10-28 |
| Larry | hashing: exact BLAKE3 (partial first, full later) and perceptual | Exacting | active | 8 | 2026-10-28 |
| Julie | verification against evidence; calls Sammy when judgment is needed | Skeptical | stub | 9 | 2026-10-28 |
| John | device group: pair, list and remove devices, keys, trust | Guarded | stub | 10 | 2026-11-05 |
| Walter | auditor: reads every windup's DB (read-only), finds gaps, unsynced records, stale backups and unfinished tasks, and sends the right windup back to fix them; keeps no full duplicate archive | Data Fiend | active | 11 | 2026-10-28 |
| Sammy | shared LLM thinking service (local or remote models) | Smart | stub | 12 | 2026-10-28 |
| Steve | search the database | Eager | stub | 13 | 2026-11-05 |
| Sally | organizes files (rename, move, sort) | Orderly | stub | 14 | 2026-11-05 |
| Tammy | super windup: ingests new media by orchestrating the others | Media Buff | stub | 15 | 2026-11-05 |
| Margaret | directory of windups and their capabilities | Matchmaker | stub | 16 | 2026-10-28 |
| Oliver | matchmaker; asks Ben to fetch and install a missing windup | Friendly | stub | 17 | 2026-10-28 |
| Flora | user-facing talker and receptionist | Kindness | stub | 18 | 2026-11-05 |
| Ruth | APA 7 work reference and work key | Academic | active | — | 2026-10-28 |
| Ben | rebuilds windups for new platforms; fetches and installs when Oliver asks | Handy | stub | — | 2026-10-28 |
| Rover | refines other windups within their profiles; never applies unreviewed | Restless | stub | — | 2026-10-28 |
| Beth | trains LoRAs from each windup's training phrases and parameters | Empathetic | stub | — | 2026-11-05 |
