# Julie — functional spec (DRAFT for Adam)
Julie is the verification windup. Drafted Nov 26, 2026 by Coding Bot from existing sources only. **Open questions** are things the sources don't say; Adam decides.

## 1. Purpose
- Name: Julie · Personality: Skeptical (NAMES.md)
- Function: verifies entries against evidence (ids, the file's own facts, independent sources) and calls Sammy when judgment is needed (PROFILE.toml).
- Workspace: own folder, one binary, one encrypted DB `julie.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- Every verdict cites its evidence (success check 1).
- She never decides between candidates herself; judgment goes to Sammy (success check 2). Verifying and thinking are separate (WINDUPS.md round 7).
- An entry is verified only by evidence (an id), never by name and year alone (success check 3; jack_spec §2: only an id flag is trusted).
- `unverified` entries wait for `julie` (WINDUPS.md round 7).
- Real verification moves Jack's verify steps over (TODO.md).
- Code today: `julie/src/lib.rs` has `Verdict { Verified, Refuted, NeedsJudgment }` and a `Verifier` trait.

## 3. Open questions for Adam
1. **Which of Jack's checks move**: jack_spec §2 has Jack check that all sources agree before something is a fact. Does that stay in Jack, move to Julie, or both (Jack checks, Julie re-checks later)?
2. **Independent sources**: does Julie look things up online herself (her own source code, like Jack's), or only compare evidence already stored?
3. **Without Sammy**: a `NeedsJudgment` verdict when Sammy isn't there: leave the entry `unverified` for Walter, or ask the person (Jack's "see candidates (y/n)?" pattern)?
4. **Where verdicts go**: `julie.db` and synced to Daniel?
