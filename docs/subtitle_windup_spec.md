# Subtitle windup — functional spec (PROPOSED stub)
Name: the owner proposed **John**, but John is already the device-group windup (NAMES.md), so the
name is on hold until the owner chooses. Everything here is **PROPOSED** (2026-11-27); nothing is built.

## 1. Purpose
- Makes timestamped subtitles for a video or recording that has none, using speech-to-text.
- Workspace: own folder, one binary, one encrypted DB (coding_guidelines §4).

## 2. Rules
- Candidate engine: whisper.cpp (OpenAI Whisper on ggml; runs on a Mac). It fits the llama.cpp
  runtime direction (coding_guidelines §3). No Chinese models.
- Output feeds Tom's 30-60 second passage windows and Sammy's segment summaries
  (`docs/tom_spec.md`, "Vectorizing strategy").
- Existing subtitles are never overwritten. The output is stored through Daniel as an extract
  (`method = transcript`), never written into the source file or as a sidecar unless the user asks.
- Permanent output follows the quality rule (coding_guidelines §1): a smaller or compressed model's
  transcript is marked `confidence = low` and queued for regeneration.

## 3. Open questions
1. **Name.**
2. **Model size**: which Whisper model by default, and on which computers?
3. **Languages**: detect automatically, or take the language from Jack's metadata?
4. **Who calls it**: Daniel when an extract is missing, Tom, or only a person?
