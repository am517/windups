# Olivia — functional spec (PROPOSED stub)
Olivia is the subtitle windup (name chosen 2026-11-27; not to be confused with Oliver, the matchmaker). Everything here is **PROPOSED** (2026-11-27); nothing is built.

## 1. Purpose
- Makes timestamped subtitles for a video or recording that has none, using speech-to-text.
- Workspace: own folder, one binary, one encrypted DB (coding_guidelines §4).

## 2. Rules
- Candidate engine: whisper.cpp (OpenAI Whisper on ggml; runs on a Mac). It fits the llama.cpp
  runtime direction (coding_guidelines §3). Chinese models (for example a WeSpeaker-based
  pyannote speaker model for diarization) are OK under coding_guidelines §3: free, no phoning home,
  clearly better.
- Output feeds Tom's 30-60 second passage windows and Sammy's segment summaries
  (`docs/tom_spec.md`, "Vectorizing strategy").
- Existing subtitles are never overwritten. The output is stored through Daniel as an extract
  (`method = transcript`), never written into the source file or as a sidecar unless the user asks.
- Permanent output follows the quality rule (coding_guidelines §1): a smaller or compressed model's
  transcript is marked `confidence = low` and queued for regeneration.

- **Trigger (PROPOSED):** when Daniel creates a new entry he queues its new-entry task set
  (daniel_spec §8a), which includes Olivia's task. If subtitles already exist, she only verifies
  that they exist and are usable; she does not recreate them.
- **Language (PROPOSED):** indicators first (the video's origin, its metadata, title, and so on),
  then auto-detection, which is the way for multilingual files.
- **Resource courtesy:** she never slows the device to a crawl (base spec §5c).

## 3. Open questions
1. **Default model (OPEN, research deferred to Dec 3):** pending research on the smallest-but-best options, ideally with
   speaker diarization, and mindful of resource use since many windups share a device.
