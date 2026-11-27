//! Content hash to re-find a file after it is renamed or moved (BLAKE3).
//! Default "partial" first: size plus chunks from start, middle and end, a few MiB per file, so a
//! big library is covered quickly. A partial match is near-certain for video but only a hint. "full"
//! (the whole file; an equal hash proves an identical file) follows later as resources permit. The prefix says which: `b3:` whole (proof), `b3p:` partial (hint).
//! Re-encodes have different bytes: the perceptual fingerprint (fingerprint.rs) is for those.
use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct HashCfg {
    pub enabled: bool,
    /// "partial" (default; fast, hint) or "full" (proof).
    pub mode: String,
    /// Bytes read at each sample point (partial mode).
    pub chunk_bytes: u64,
    /// Files up to this size are always hashed whole.
    pub full_below_bytes: u64,
}

impl Default for HashCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: "partial".into(),
            chunk_bytes: 1 << 20,
            full_below_bytes: 16 << 20,
        }
    }
}

/// Hash of a piece of text (a chunk), always whole: `b3:` + BLAKE3 of its UTF-8 bytes.
pub fn text_hash(text: &str) -> String {
    format!("b3:{}", blake3::hash(text.as_bytes()).to_hex())
}

pub fn file_hash(path: &Path, cfg: &HashCfg) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let size = f.metadata()?.len();
    let mut h = blake3::Hasher::new();
    if cfg.mode == "full"
        || size <= cfg.full_below_bytes
        || size <= cfg.chunk_bytes.saturating_mul(3)
    {
        std::io::copy(&mut f, &mut h)?;
        return Ok(format!("b3:{}", h.finalize().to_hex()));
    }
    h.update(&size.to_le_bytes());
    let mut buf = vec![0u8; cfg.chunk_bytes as usize];
    for at in [0, size / 2 - cfg.chunk_bytes / 2, size - cfg.chunk_bytes] {
        f.seek(SeekFrom::Start(at))?;
        f.read_exact(&mut buf)?;
        h.update(&buf);
    }
    Ok(format!("b3p:{}", h.finalize().to_hex()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_text_has_an_equal_whole_hash() {
        assert_eq!(text_hash("a b"), text_hash("a b"));
        assert_ne!(text_hash("a b"), text_hash("a c"));
        assert!(text_hash("").starts_with("b3:"));
    }
    #[test]
    fn whole_for_small_partial_for_big_same_bytes_same_hash() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("moved.mkv"));
        std::fs::write(&a, b"hello").unwrap();
        std::fs::write(&b, b"hello").unwrap();
        let cfg = HashCfg {
            mode: "full".into(),
            ..Default::default()
        };
        assert!(file_hash(&a, &cfg).unwrap().starts_with("b3:"));
        assert_eq!(file_hash(&a, &cfg).unwrap(), file_hash(&b, &cfg).unwrap());
        let small = HashCfg {
            mode: "partial".into(),
            chunk_bytes: 4,
            full_below_bytes: 8,
            ..Default::default()
        };
        let big: Vec<u8> = (0..100u8).collect();
        std::fs::write(&a, &big).unwrap();
        let p = file_hash(&a, &small).unwrap();
        assert!(p.starts_with("b3p:"));
        let mut other = big.clone();
        other[99] = 0; // the end differs
        std::fs::write(&b, &other).unwrap();
        assert_ne!(file_hash(&b, &small).unwrap(), p);
        assert!(
            file_hash(
                &a,
                &HashCfg {
                    mode: "full".into(),
                    ..small
                }
            )
            .unwrap()
            .starts_with("b3:")
        );
    }
}
