# Suzan — functional spec (DRAFT)
Suzan is the write-to-file windup. Drafted Nov 26, 2026 from existing sources only; updated Nov 27 with review direction. **Open questions** are things neither says; they stay open until decided in review.

## 1. Purpose
- Name: Suzan · Personality: Persistent (NAMES.md)
- Function: writes pending changes to the file, with at most one open Suzan task per entry (PROFILE.toml; NAMES.md).
- **She does the filesystem writes: rename, move, organize** (direction, Nov 27). Metadata inside files is Lauren's, so the two are not redundant.
- Workspace: her own folder, one binary, one encrypted DB `suzan.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- Writes only what the task says to write (PROFILE success check 2).
- Any task that changes an entry adds a Suzan task; at most one open Suzan task per entry (WINDUPS.md, "Daniel tasks table", Nov 1).
- That tasks table is **planned, do not build yet** (WINDUPS.md, Nov 1; TODO.md).
- **Read-only trees (Nov 27):** she never renames, moves or writes in a tree marked read-only, such as the personal-records source folder (`docs/daniel_spec.md` §9).
- Records first, kept after the action; acts only on sure decisions (coding_guidelines §1, §4).

## 3. Responses, flags, commands
Global only (base spec). Her own commands: **I don't know**.

## 4. Open questions
1. **Tasks**: **DECIDED (2026-11-27), the flow:** Daniel records the planned action as pending; Suzan acts and logs it; she reports back; Daniel settles it or marks it failed. **PROPOSED (agreed):** tasks live in a separate operational database, apart from the catalogs. `--dry-run` is recorded on the task (base spec §1).
2. **Suzan vs Sally**: NAMES.md and the roster give Sally "organizes files (rename, move, sort)", and she takes over Jack's sorting. With Suzan now doing rename/move/organize, what is left for Sally (for example, deciding *where* things go while Suzan does the writing)?
3. **Failure and retry**: a write fails (file busy, read-only disk). Keep the task open for Walter to send her back?
4. **Originals**: back up the file (or its tag block) before writing, or rely on George's snapshots (which cover DBs, not media)?
