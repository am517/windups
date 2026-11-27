# Kate — functional spec (DRAFT)
Kate is the image windup. Drafted Nov 26, 2026 from existing sources only; updated Nov 27 with review direction. **Open questions** are things neither says; they stay open until decided in review.

## 1. Purpose
- Name: Kate · Personality: Utilitarian (NAMES.md)
- Function: converts artwork: JPEG for embedding in media files, JPEG XL for the archive (PROFILE.toml; QUESTIONS.md answer 3, Oct 28).
- Workspace: own folder, one binary, one encrypted DB `kate.db` (coding_guidelines §4).

## 2. Rules (from the sources)
- Embedded art is JPEG that common players read (PROFILE success check 1).
- Archived art is JPEG XL, lossless from JPEG sources (success check 2; QUESTIONS.md answer 3: "lossless JPEG recompression").
- The original image is never altered or lost (success check 3).
- She converts Jack's art (TODO.md).
- **She may link libjxl (C++)** for lossless JPEG → JPEG XL (direction, Nov 27). C/C++ libraries are fine as dependencies when no Rust version exists; all new code is Rust (coding_guidelines §3).
- Code today: `kate/src/lib.rs` has `Target { EmbedJpeg, ArchiveJxl }`, a `Converter` trait and a refusing stub.

## 3. Open questions
1. **Non-JPEG sources** (PNG, WebP): JPEG XL lossless from pixels, or lossy at what quality?
2. **Embed JPEG**: what maximum size/dimensions and quality? PNG sources re-encoded to JPEG, or embedded as PNG (MP4 allows PNG)?
3. **Where the archive lives**: in Kate's folder, or handed to someone (Daniel's raw_dir, George)? Windups never write outside their own folder.
4. **Who calls her**: Jack after fetching art, Lauren before embedding, or both?
