# Windup names

This file is the name registry: the source of truth for which windups exist.
The cast table in [`WINDUPS.md`](WINDUPS.md) should show the same names, roles
and build positions. `tests/names.rs` fails if a crate folder and this registry
disagree.

`Sampy`/`Dory` is a display model, not a windup, and does not belong here.
Unnamed ideas (a refusal monitor, a profile-managing windup) stay out until
Adam gives them a name and a role.

## Rules

1. One row per name, with its role, status (`stub`, `active` or `done`), build
   position and the date it was added.
2. A new name lands in this registry and gets a stub crate in the same commit.
3. Never reuse or rename a name without a note below.
4. Build position is the numbered sequence in `WINDUPS.md` / `TODO.md`. A dash
   (`—`) means the windup sits outside that sequence (already working, or later
   support such as Beth training LoRAs).

## Notes (renames and reuse)

None yet.

## Registry

| Name | Role | Status | Position | Added |
| --- | --- | --- | --- | --- |
| Jack | fetches and verifies metadata for a file, hands one complete entry to Daniel | active | 1 | 2026-10-28 |
| Daniel | database manager: takes handoffs and keeps everything in media.db | active | 2 | 2026-10-28 |
| Lauren | writes tags and embeds art from the database | stub | 3 | 2026-11-05 |
| Suzan | write-to-file; at most one open task per entry | stub | 4 | 2026-11-05 |
| Mary | summaries; runs before Tom only, never blocks Larry | stub | 5 | 2026-10-28 |
| Kate | image conversion: JPEG to embed, JPEG XL to archive | stub | 6 | 2026-10-28 |
| Tom | vectors/embeddings for meaning search | stub | 7 | 2026-10-28 |
| Larry | hashing: exact BLAKE3 (partial first, full later) and perceptual | active | 8 | 2026-10-28 |
| Julie | verification against evidence; calls Sammy when judgment is needed | stub | 9 | 2026-10-28 |
| John | device group: pair, list and remove devices, keys, trust | stub | 10 | 2026-11-05 |
| Walter | patrol: watches unfinished work and calls windups back | active | 11 | 2026-10-28 |
| Sammy | shared LLM thinking service (local or remote models) | stub | 12 | 2026-10-28 |
| Steve | search the database | stub | 13 | 2026-11-05 |
| Sally | organizes files (rename, move, sort) | stub | 14 | 2026-11-05 |
| Tammy | super windup: ingests new media by orchestrating the others | stub | 15 | 2026-11-05 |
| Margaret | directory of windups and their capabilities | stub | 16 | 2026-10-28 |
| Oliver | matchmaker; asks Ben to fetch and install a missing windup | stub | 17 | 2026-10-28 |
| Flora | user-facing talker and receptionist | stub | 18 | 2026-11-05 |
| Ruth | APA 7 work reference and work key | active | — | 2026-10-28 |
| Ben | rebuilds windups for new platforms; fetches and installs when Oliver asks | stub | — | 2026-10-28 |
| Rover | refines other windups within their profiles; never applies unreviewed | stub | — | 2026-10-28 |
| Beth | trains LoRAs from each windup's training phrases and parameters | stub | — | 2026-11-05 |
