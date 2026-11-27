# Larry — functional spec (DRAFT for Adam)
Larry is the hashing windup. Drafted Nov 26, 2026 by Coding Bot from existing sources only. **Open questions** are things the sources don't say; Adam decides.

## 1. Purpose
- Name: Larry · Personality: Exacting (NAMES.md)
- Function: fingerprints files: an exact BLAKE3 hash (partial first, full later) to re-find renamed or moved files, and a look-alike fingerprint that only proposes possible duplicates (PROFILE.toml).
- Today he is a library Daniel calls (`larry::describe`) when storing an entry (larry/README.md; WINDUPS.md round 6).

## 2. Rules (from the sources)
- Same bytes, same hash; different bytes, a different one (tested).
- Partial hashing by default (size + start/middle/end chunks); full on request; files under `full_below_bytes` are hashed whole (`larry/config/larry.toml`; tested).
- Look-alike fingerprints only propose (`possible_same`); they never merge (tested). He never merges entries or decides what a file is (PROFILE `never`).
- Real fingerprint (later): align by audio first, then compare frames at matching times; sampled-frame pHashes via ffmpeg, optional Chromaprint; tune `max_distance` (WINDUPS.md round 6; TODO.md).
- A full-hash pass over partial-hashed files as resources permit (TODO.md).

## 3. Open questions for Adam
1. **Library or windup**: stay a library Daniel calls, or become a full windup with his own folder, `larry.db`, the plain-word commands, and Noise?
2. **Full-hash pass**: who starts it and when ("as resources permit": Walter, a person, idle time)? What counts as enough resources?
3. **Perceptual fingerprint tools**: ffmpeg (external, detected at runtime) and Chromaprint (`fpcalc`) are OK as optional local tools?
4. **Where hashes live**: only in Daniel's catalog (today), or in `larry.db` first (own records first, per coding_guidelines §4)?
