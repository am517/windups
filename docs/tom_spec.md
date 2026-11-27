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
- Model: a small local embedding model, swappable behind a module (QUESTIONS.md answer 4, Oct 28; bge-small / all-MiniLM, 384 dims, were the proposal). bge-m3 (BAAI, China) is not the lean, but it is a candidate under the Chinese-software conditions (coding_guidelines §3: free, no phoning home, clearly better).
- **DECIDED (2026-11-27):** the quality tier is a normal computer; Pi support is a bonus.
- **DECIDED (2026-11-27):** the embedding models are tested, then one is picked. Test set: the owner's notes plus the Bible text; score: whether a search retrieves the right passage. **PROPOSED** candidates: multilingual-e5-large, EmbeddingGemma, arctic-embed-l-v2.0, bge-m3 (under coding_guidelines §3).
- **Runtime, DECIDED (2026-11-27):** llama.cpp is the default runtime for all models (LLMs and embeddings). ONNX Runtime and Candle are dropped unless needed.
- **DECIDED in principle (2026-11-27):** vectors from a compressed copy of the chosen model are stored `confidence = low` and queued for regeneration on a capable computer; a different small model never stands in.
- The embedding model "gets only a plugin hook until Sammy exists" (WINDUPS.md, Jack Nov 1).
- Larry (hashing) is his peer windup (direction, Nov 27).
- Every windup works fully in isolation with reduced function: it makes human-like choices and does only its own task, never extra tasks just because it is alone (direction, Nov 27).
- Code today: `tom/src/lib.rs` has an `Embedder` trait (model name + embed) and a `NoEmbedder` stub.

## Vectorizing strategy (PROPOSED, 2026-11-27)

All of this is **PROPOSED** (approved to be recorded as proposed; not built).

- **Original text only.** Vectors come from the original text, never from a Sammy rewrite, so every
  hit points to an exact location (page, paragraph or timestamp).
- **Three levels**, all stored and searched together:
  1. *Passages*, for precise hits: paragraphs for books; 30-60 second overlapping windows for
     subtitles and transcripts.
  2. *Segments*: real structure where it exists (chapters, sections); otherwise topic boundaries
     found where the similarity between adjacent window vectors drops.
  3. *Summaries*: Sammy writes a title and summary for each segment and for the whole work. They
     are vectorized **alongside** the passages, never instead of them. Search matches a summary to
     find the work, then a passage for the exact spot or timestamp.
- **Photos and videos:** a vision model writes a description, which is vectorized. An image-embedding
  model in a shared text-and-image space (candidate: Google SigLIP; Chinese models only under coding_guidelines §3) lets text
  queries find images the description missed. Videos: frames sampled at scene changes are
  described and combined with the transcript.
- **Efficiency:** Larry's hash means a file is vectorized once, and again only when its content or
  the model id/version changes. Low-confidence vectors from weak devices are redone later (decided,
  coding_guidelines §1).
- Missing subtitles come from Olivia, the subtitle windup (`docs/olivia_spec.md`, PROPOSED).

## 3. Open questions
Questions 1 and 2 are open for discussion in review (direction, Nov 27).

1. **Wait for Sammy?** The Nov 1 note says only a plugin hook until Sammy exists. Is Tom blocked on Sammy, or may he run a model himself?
2. **Model and runtime**: answered above (test, then pick; llama.cpp). Open: the exact passage set and the pass mark for "retrieves the right passage".
3. **Where the model file lives**: in Tom's folder (one binary + one DB rule), or elsewhere?
4. **Determinism**: "same text and model give the same vector": exact bytes, or within a tolerance (floating point can differ across CPUs)?
