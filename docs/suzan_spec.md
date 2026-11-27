# Suzan — functional spec (DRAFT for Adam)
Suzan is the write-to-file windup. Drafted Nov 26, 2026 by Coding Bot from existing sources only. **Open questions** are things the sources don't say; Adam decides.

## 1. Purpose
- Name: Suzan · Personality: Persistent (NAMES.md)
- Function: writes pending changes to the file, with at most one open Suzan task per entry (PROFILE.toml; NAMES.md).
- Workspace: her own folder, one binary, one encrypted DB `suzan.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- Writes only what the task says to write (PROFILE success check 2).
- Any task that changes an entry adds a Suzan task; at most one open Suzan task per entry (WINDUPS.md, "Daniel tasks table", Nov 1).
- That tasks table is **planned, do not build yet** (WINDUPS.md, Nov 1; TODO.md).
- Records first, kept after the action; acts only on sure decisions (coding_guidelines §1, §4).

## 3. Responses, flags, commands
Global only (base spec). Her own commands: **I don't know**.

## 4. Open questions for Adam
1. **Tasks table**: her one-open-task rule lives in Daniel's tasks table, which is "do not build yet". Build it now (Daniel), or give Suzan her own task list in `suzan.db` until then?
2. **Lauren vs Suzan** (same as Lauren Q2): what exactly does each write? Tags and art (Lauren) vs "pending changes" (Suzan) overlap.
3. **What is a "pending change"**: tags only, or also renames/moves (Sally organizes files)?
4. **Failure and retry**: a write fails (file busy, read-only disk). Keep the task open for Walter to send her back?
5. **Originals**: back up the file (or its tag block) before writing, or rely on George's snapshots (which cover DBs, not media)?
