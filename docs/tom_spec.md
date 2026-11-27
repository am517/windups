# Tom — functional spec (DRAFT)
Tom is the vector windup. Drafted Nov 26, 2026 from existing sources only; updated Nov 27 with review direction. **Open questions** are things neither says; they stay open until decided in review.

## 1. Purpose
- Name: Tom · Personality: Analytical (NAMES.md)
- Function: embeds texts (Mary's summary first, else the description) as vectors for meaning search (PROFILE.toml).
- Workspace: own folder, one binary, one encrypted DB `tom.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- Every embedded text has a vector stored with its model name (success check 1; test `windups/src/vectors.rs::store_and_search_by_model`).
- **Decided (Nov 27):** every vector is stored with its model id **and version**; a search never mixes models or versions (`windups::vectors::Model`, `embeddings.model_version`, #43).
- Chunk vectors go through Daniel (`daniel/src/chunk_vectors.rs`, #47). `not_embedded` lists what to embed for a model and version, and `store` refuses a vector made from text that has since changed.
- The same text and model give the same vector (success check 2).
- Vectors go to the shared `vectors` table via Daniel (PROFILE `writes`; `windups/schema/vectors.sql`).
- Model: a small local embedding model, swappable behind a module (QUESTIONS.md answer 4, Oct 28; bge-small / all-MiniLM, 384 dims, were the proposal). bge-m3 was the Nov 27 lean but is **dropped**: it comes from BAAI (China), against the no-Chinese-software rule (coding_guidelines §3).
- **DECIDED (2026-11-27):** the quality tier is a normal computer; Pi support is a bonus.
- **PROPOSED (2026-11-27):** a bake-off on real data of multilingual-e5-large, EmbeddingGemma and arctic-embed-l-v2.0 picks the model.
- **Runtime, DECIDED (2026-11-27):** the most efficient and most universal option with few dependencies. **PROPOSED:** llama.cpp as the default (shared with LLMs) if the chosen model works well there; compare ONNX Runtime and Candle only if needed.
- **DECIDED in principle (2026-11-27):** vectors from a compressed copy of the chosen model are stored `confidence = low` and queued for regeneration on a capable computer; a different small model never stands in.
- The embedding model "gets only a plugin hook until Sammy exists" (WINDUPS.md, Jack Nov 1).
- Larry (hashing) is his peer windup (direction, Nov 27).
- Every windup works fully in isolation with reduced function: it makes human-like choices and does only its own task, never extra tasks just because it is alone (direction, Nov 27).
- Code today: `tom/src/lib.rs` has an `Embedder` trait (model name + embed) and a `NoEmbedder` stub.

## 3. Open questions
Questions 1 and 2 are open for discussion in review (direction, Nov 27).

1. **Wait for Sammy?** The Nov 1 note says only a plugin hook until Sammy exists. Is Tom blocked on Sammy, or may he run a model himself?
2. **Model and runtime**: answered above (bake-off, llama.cpp proposed). Open: the real-data test set for the bake-off, and how results are scored.
3. **Where the model file lives**: in Tom's folder (one binary + one DB rule), or elsewhere?
4. **Determinism**: "same text and model give the same vector": exact bytes, or within a tolerance (floating point can differ across CPUs)?
