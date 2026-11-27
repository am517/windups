# Kate — functional spec (DRAFT for Adam)
Kate is the image windup. Drafted Nov 26, 2026 by Coding Bot from existing sources only. **Open questions** are things the sources don't say; Adam decides.

## 1. Purpose
- Name: Kate · Personality: Utilitarian (NAMES.md)
- Function: converts artwork: JPEG for embedding in media files, JPEG XL for the archive (PROFILE.toml; QUESTIONS.md answer 3, Oct 28).
- Workspace: own folder, one binary, one encrypted DB `kate.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- Embedded art is JPEG that common players read (PROFILE success check 1).
- Archived art is JPEG XL, lossless from JPEG sources (success check 2; QUESTIONS.md answer 3: "lossless JPEG recompression").
- The original image is never altered or lost (success check 3).
- She converts Jack's art (TODO.md).
- Code today: `kate/src/lib.rs` has `Target { EmbedJpeg, ArchiveJxl }`, a `Converter` trait and a refusing stub.

## 3. Open questions for Adam
1. **Encoder**: lossless JPEG → JPEG XL recompression is only in libjxl (C++; `cjxl`, or the `jpegxl-rs` crate binding it). The pure-Rust crates I know of (`jxl-oxide` decodes; `zune-jpegxl` encodes pixels only) do not do lossless JPEG recompression (not re-checked today). Use the `cjxl` tool when installed (like exiftool/yt-dlp, detected at runtime), or link libjxl? Guidelines allow another language only when there is no choice.
2. **Non-JPEG sources** (PNG, WebP): JPEG XL lossless from pixels, or lossy at what quality?
3. **Embed JPEG**: what maximum size/dimensions and quality? PNG sources re-encoded to JPEG, or embedded as PNG (MP4 allows PNG)?
4. **Where the archive lives**: in Kate's folder, or handed to someone (Daniel's raw_dir, George)? Windups never write outside their own folder.
5. **Who calls her**: Jack after fetching art, Lauren before embedding, or both?
