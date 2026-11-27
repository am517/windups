# Lauren — functional spec (DRAFT for Adam)
Lauren is the tagging windup. Drafted Nov 26, 2026 by Coding Bot from the repo's existing sources only; nothing here is new behavior. Every line cites where it comes from. **Open questions** are things the sources don't say: I don't know them, and Adam decides.

## 1. Purpose
- Name: Lauren · Personality: Artistic (NAMES.md, set by Adam Nov 10, work in progress)
- Function: writes tags and embeds art into media files from what the database already knows (PROFILE.toml `purpose`; NAMES.md).
- She takes over Jack's file-editing code (roster; coding_guidelines §4 "file editing moves to Lauren later", Nov 20).
- Workspace: her own folder with one binary and one encrypted DB (`lauren.db`), like every windup (coding_guidelines §4).

## 2. Rules (from the sources)
- What she writes comes from the database, never a fresh guess (PROFILE success check 1).
- Embedded art is JPEG (QUESTIONS.md answer 3, Oct 28: MP4 `covr` only allows JPEG/PNG); Kate makes the JPEG (Kate's profile).
- She acts only on sure decisions and keeps her complete action history (coding_guidelines §1); her own record is written before she touches a file (§4).
- She uses free, locally installed tools when present and says clearly when one is missing (§3). Jack's old tagger used AtomicParsley or exiftool for MP4 and mkvpropedit/mkvmerge for MKV (`jack/src/tagger.rs` on `main`).

## 3. Responses
Global rules only (windups_base_spec §3-4): say what was written, what was skipped and why.

## 4. Flags and commands
Global flags and plain-word commands only (base spec §1-2). Her own commands: **I don't know** (none are specified).

## 5. Modules (proposal, not decided)
Reuse Jack's `tagger.rs` / `art.rs` from `main` as Lauren's starting point, one job per file: `plan` (which tags from which DB fields), `mp4`, `mkv`, `art`.

## 6. Open questions for Adam
1. **Which database does she read**: Daniel's media.db, Jack's jack.db, or records sent to her over Noise? ("from the database" doesn't say which.)
2. **Lauren vs Suzan**: Suzan "writes pending changes to the file". Does Lauren decide *what* to write and Suzan do the writing, or does Lauren write tags/art herself?
3. **Jack's fallback**: her profile says "Jack keeps a fallback" (success check 2), but PR #25 removes all tagging from Jack. Drop that check?
4. **Which tags**: Jack's old set (title, year, ids, content rating, cover) or a new list?
5. **External tools vs a Rust crate**: keep calling AtomicParsley/exiftool/mkvpropedit, or write tags in Rust (e.g. `mp4ameta`)? Guidelines prefer few deps and Rust first.
6. **Formats**: MP4 and MKV only to start?
7. **Who calls her, and when**: after Daniel stores an entry, by Tammy, or only on a person's request?
