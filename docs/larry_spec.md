# Larry — functional spec (DRAFT)
Larry is the hashing windup. Drafted Nov 26, 2026 from existing sources only; updated Nov 27 with review direction. **Open questions** are things neither says; they stay open until decided in review.

## 1. Purpose
- Name: Larry · Personality: Exacting (NAMES.md)
- Function: fingerprints files: an exact BLAKE3 hash (partial first, full later) to re-find renamed or moved files, and a look-alike fingerprint that only proposes possible duplicates (PROFILE.toml).
- **He is a full hashing windup, a peer of Tom (the vector windup)** (direction, Nov 27): his own folder, `larry.db`, the plain-word commands.
- Today he is a library Daniel calls (`larry::describe`) when storing an entry (larry/README.md; WINDUPS.md round 6).

## 2. Rules (from the sources)
- Same bytes, same hash; different bytes, a different one (tested).
- Partial hashing by default (size + start/middle/end chunks); full on request; files under `full_below_bytes` are hashed whole (`larry/config/larry.toml`; tested).
- Look-alike fingerprints only propose (`possible_same`); they never merge (tested). He never merges entries or decides what a file is (PROFILE `never`).
- Real fingerprint (later): align by audio first, then compare frames at matching times; sampled-frame pHashes via ffmpeg, optional Chromaprint; tune `max_distance` (WINDUPS.md round 6; TODO.md).
- Text hash: `larry::hash::text_hash` (always whole, `b3:`) gives each of Daniel's chunks its content hash, so Tom re-embeds only changed text (#44, #47).
- A full-hash pass over partial-hashed files as resources permit (TODO.md).

## 3. Open questions
1. **Full-hash pass**: who starts it and when ("as resources permit": Walter, a person, idle time)? What counts as enough resources?
2. **Perceptual fingerprint tools**: ffmpeg (external, detected at runtime) and Chromaprint (`fpcalc`) are OK as optional local tools?
3. **Where hashes live**: as a full windup, `larry.db` first (own records first, coding_guidelines §4) then synced to Daniel? And does Daniel keep calling him while storing, or does Larry get files another way?
