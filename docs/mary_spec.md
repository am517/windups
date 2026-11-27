# Mary — functional spec (DRAFT)
Mary is the summary windup. Drafted Nov 26, 2026 from existing sources only; updated Nov 27 with review direction. **Open questions** are things neither says; they stay open until decided in review.

## 1. Purpose
- Name: Mary · Personality: Articulate (NAMES.md)
- Function: a short, faithful summary of every entry from its stored texts (descriptions, plots) (PROFILE.toml).
- Workspace: own folder, one binary, one encrypted DB `mary.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- A summary only states what the stored texts say (PROFILE success check 1).
- Runs before Tom (he embeds her summary); never blocks Larry (PROFILE; WINDUPS.md round 7).
- Writes the summary via Daniel (PROFILE `writes`).
- Summaries are "truths", not facts (jack_spec §1: subjective text such as summaries). Jack keeps truths but doesn't sync them to Daniel.
- Classical algorithms first, Sammy if needed (coding_guidelines §1). Jack without Sammy: "longest summary wins" (WINDUPS.md, helpers rule).
- Every windup works fully in isolation with reduced function: it makes human-like choices and does only its own task, never extra tasks just because it is alone (direction, Nov 27).
- Code today: `mary/src/lib.rs` has a `Summarizer` trait and a `NoSummary` stub.

## 3. Open questions
1. **How she summarizes**: with Sammy (an LLM) only, or also without Sammy (for example extractive: first sentence(s) of the longest stored text)? Without Sammy, does she do nothing?
2. **Length**: what counts as "short" (characters, sentences)? A config value with what default?
3. **Truth vs fact**: jack_spec says truths aren't synced to Daniel, but Mary's profile says she writes the summary via Daniel. Which wins for Mary?
4. **Language**: summarize in the source text's language, or always English?
5. **Where the texts come from**: Daniel's catalog, or Jack's truths?
