# Lauren — functional spec (DRAFT)
Lauren is the tagging windup. Drafted Nov 26, 2026 from the repo's existing sources only; updated Nov 27 with review direction. Every line cites where it comes from. **Open questions** are things neither says: I don't know them, and they stay open until decided in review.

## 1. Purpose
- Name: Lauren · Personality: Artistic (NAMES.md, set Nov 10, work in progress)
- Function: writes tags and embeds art into media files from what the database already knows (PROFILE.toml `purpose`; NAMES.md).
- **She writes metadata inside files, as a front end to exiftool** (direction, Nov 27). Filesystem writes (rename, move, organize) are Suzan's, so the two are not redundant.
- She takes over Jack's file-editing code (roster; coding_guidelines §4 "file editing moves to Lauren later", Nov 20).
- Workspace: her own folder with one binary and one encrypted DB (`lauren.db`), like every windup (coding_guidelines §4).

## 2. Rules (from the sources)
- What she writes comes from the database, never a fresh guess (PROFILE success check 1).
- Embedded art is JPEG (QUESTIONS.md answer 3, Oct 28: MP4 `covr` only allows JPEG/PNG); Kate makes the JPEG (Kate's profile).
- She acts only on sure decisions and keeps her complete action history (coding_guidelines §1); her own record is written before she touches a file (§4).
- **Hard gate (Nov 27, after a wrong write):** she never writes to a file whose entry is unresolved or needs judgment. Only a resolved identity is written, and a regression test must enforce it. Repairing a file an older Jack wrote wrongly: `docs/repair_wrong_tags.md`.
- **Read-only trees (Nov 27):** she never writes in a tree marked read-only, such as the personal-records source folder (`docs/daniel_spec.md` §9). If a write would be needed there, she skips and says why.
- **Quality rule (Nov 27, coding_guidelines §1):** tags written into a file are permanent, so she writes with the best method available, never a placeholder.
- She uses free, locally installed tools when present and says clearly when one is missing (§3). Jack's old tagger used AtomicParsley or exiftool for MP4 and mkvpropedit/mkvmerge for MKV (`jack/src/tagger.rs`, in git history before #25).

## 3. Responses
Global rules only (windups_base_spec §3-4): say what was written, what was skipped and why.

## 4. Flags and commands
Global flags and plain-word commands only (base spec §1-2). Her own commands: **I don't know** (none are specified).

## 5. Modules (proposal, not decided)
Start from Jack's old `tagger.rs` / `art.rs` (in git history), one job per file: `plan` (which tags from which DB fields), `exiftool` (the front end), `art`.

## 6. Open questions
1. **Which database does she read**: Daniel's media.db, Jack's jack.db, or records sent to her over Noise? ("from the database" doesn't say which.)
2. **Jack's fallback**: her profile says "Jack keeps a fallback" (success check 2), but PR #25 removes all tagging from Jack. Drop that check?
3. **Which tags**: Jack's old set (title, year, ids, content rating, cover) or a new list?
4. **Other tools**: exiftool is her front end. Does she still use mkvpropedit for MKV (exiftool can't write Matroska), and AtomicParsley at all?
5. **Formats**: MP4 and MKV only to start?
6. **Who calls her, and when**: after Daniel stores an entry, by Tammy, or only on a person's request?
7. **Undo**: restoring a wrong write is wanted when the pre-write state exists (Nov 27). Does she save a file's existing tags (and art) before each write, and where: her own DB, a sidecar, or Daniel? Without that, the only repair is to clear the tags and re-tag from the identity.
8. **Read-only trees in config**: how is a tree marked read-only? A list of paths in her config, or a flag Daniel keeps per catalog?
