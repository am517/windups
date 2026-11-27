# Tom — functional spec (DRAFT)
Tom is the vector windup. Drafted Nov 26, 2026 from existing sources only; updated Nov 27 with review direction. **Open questions** are things neither says; they stay open until decided in review.

## 1. Purpose
- Name: Tom · Personality: Analytical (NAMES.md)
- Function: embeds texts (Mary's summary first, else the description) as vectors for meaning search (PROFILE.toml).
- Workspace: own folder, one binary, one encrypted DB `tom.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- Every embedded text has a vector stored with its model name (success check 1; test `windups/src/vectors.rs::store_and_search_by_model`).
- The same text and model give the same vector (success check 2).
- Vectors go to the shared `vectors` table via Daniel (PROFILE `writes`; `windups/schema/vectors.sql`).
- Model: a small local embedding model, swappable behind a module (QUESTIONS.md answer 4, Oct 28; bge-small / all-MiniLM, 384 dims, were the proposal).
- The embedding model "gets only a plugin hook until Sammy exists" (WINDUPS.md, Jack Nov 1).
- Larry (hashing) is his peer windup (direction, Nov 27).
- Every windup works fully in isolation with reduced function: it makes human-like choices and does only its own task, never extra tasks just because it is alone (direction, Nov 27).
- Code today: `tom/src/lib.rs` has an `Embedder` trait (model name + embed) and a `NoEmbedder` stub.

## 3. Open questions
Questions 1 and 2 are open for discussion in review (direction, Nov 27).

1. **Wait for Sammy?** The Nov 1 note says only a plugin hook until Sammy exists. Is Tom blocked on Sammy, or may he run a model himself?
2. **Which model and runtime**: bge-small or all-MiniLM, run how (ONNX Runtime, candle, llama.cpp embeddings)? Each is a large dependency; the model is a ~30-130 MB download.
3. **Where the model file lives**: in Tom's folder (one binary + one DB rule), or elsewhere?
4. **Determinism**: "same text and model give the same vector": exact bytes, or within a tolerance (floating point can differ across CPUs)?
