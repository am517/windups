# Tom — functional spec (DRAFT for Adam)
Tom is the vector windup. Drafted Nov 26, 2026 by Coding Bot from existing sources only. **Open questions** are things the sources don't say; Adam decides.

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
- Code today: `tom/src/lib.rs` has an `Embedder` trait (model name + embed) and a `NoEmbedder` stub.

## 3. Open questions for Adam
1. **Wait for Sammy?** The Nov 1 note says only a plugin hook until Sammy exists. Is Tom blocked on Sammy, or may he run a model himself?
2. **Which model and runtime**: bge-small or all-MiniLM, run how (ONNX Runtime, candle, llama.cpp embeddings)? Each is a large dependency; the model is a ~30-130 MB download.
3. **Where the model file lives**: in Tom's folder (one binary + one DB rule), or elsewhere?
4. **Determinism**: "same text and model give the same vector": exact bytes, or within a tolerance (floating point can differ across CPUs)?
