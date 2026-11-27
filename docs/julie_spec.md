# Julie — functional spec (DRAFT)
Julie is the verification windup. Drafted Nov 26, 2026 from existing sources only; updated Nov 27 with review direction. **Open questions** are things neither says; they stay open until decided in review.

## 1. Purpose
- Name: Julie · Personality: Skeptical (NAMES.md)
- Function: verifies entries against evidence (ids, the file's own facts, independent sources) and calls Sammy when judgment is needed (PROFILE.toml).
- **Direction (Nov 27):** Jack checks his own work as he goes (algorithms, plus Sammy). Julie roams the database and, whenever an entry is updated, reviews the whole entry for consistency with all the evidence from every windup, which Jack can't see. She may be a peer of Walter.
- Workspace: own folder, one binary, one encrypted DB `julie.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- Every verdict cites its evidence (success check 1).
- She never decides between candidates herself; judgment goes to Sammy (success check 2). Verifying and thinking are separate (WINDUPS.md round 7).
- An entry is verified only by evidence (an id), never by name and year alone (success check 3; jack_spec §2: only an id flag is trusted).
- `unverified` entries wait for `julie` (WINDUPS.md round 7).
- TODO.md says "move Jack's verify steps over". The Nov 27 direction keeps Jack's own checks in Jack; Julie reviews whole entries afterwards.
- Daniel's people catalog opens proposals instead of merging (`people_proposals`, mentions with `needs_review`; #43, #44, #48). Per `docs/daniel_spec.md` §7 she reviews those.
- Code today: `julie/src/lib.rs` has `Verdict { Verified, Refuted, NeedsJudgment }` and a `Verifier` trait.

## 3. Open questions
1. **Peer of Walter**: is she a peer of Walter (both roam the DB read-only and send windups back), and how do their jobs split: Walter for gaps and staleness, Julie for consistency?
2. **Independent sources**: does Julie look things up online herself, or only compare the evidence windups have already stored?
3. **How she learns an entry changed**: does Daniel tell her, or does she find out by roaming?
4. **Without Sammy**: a `NeedsJudgment` verdict when Sammy isn't there: leave the entry `unverified` for Walter, or ask the person (Jack's "see candidates (y/n)?" pattern)?
5. **Where verdicts go**: `julie.db` and synced to Daniel?
6. **Proposals**: may she accept or reject a `possible_same` proposal or a `needs_review` mention herself, when the evidence is an id, or only recommend, leaving the decision to a person or Sammy?
7. **DECIDED (2026-11-27):** Julie is the judge. She runs the best model, can query other windups and sees the whole picture. Daniel still does the writing, and windups still work without her. **DECIDED (2026-11-27):** Walter only checks and reports, by adding items to Julie's task list (kept in the operational database); Julie decides. Q1 is answered in part; Q2-Q6 stay open.
