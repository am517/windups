//! Larry, the hashing windup. Given a file: its exact hash (re-finds renamed or moved files) and,
//! when enabled, its look-alike fingerprint (a hint that opens proposals, never merges).
//! Daniel calls Larry when it stores an entry; Larry never decides what a file is.
pub mod cli;
pub mod fingerprint;
pub mod hash;

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

pub const PROFILE: &str = include_str!("../PROFILE.toml");
pub const DEFAULT_CONFIG: &str = include_str!("../config/larry.toml");

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub hash: hash::HashCfg,
    pub fingerprint: fingerprint::FingerprintCfg,
}

impl Config {
    /// Built-in defaults, overlaid by `path` if it exists.
    pub fn load(path: Option<&Path>) -> Result<Self, String> {
        let mut t: toml::Table = toml::from_str(DEFAULT_CONFIG).map_err(|e| e.to_string())?;
        if let Some(Ok(s)) = path.map(std::fs::read_to_string) {
            let user: toml::Table = toml::from_str(&s).map_err(|e| format!("larry config: {e}"))?;
            for (k, v) in user {
                match (t.get_mut(&k), v) {
                    (Some(toml::Value::Table(a)), toml::Value::Table(b)) => a.extend(b),
                    (_, v) => {
                        t.insert(k, v);
                    }
                }
            }
        }
        t.try_into().map_err(|e| format!("larry config: {e}"))
    }
}

/// What Larry adds to an entry for a file: `file_hash` and, if enabled, `file_fingerprint`.
pub fn describe(
    path: &Path,
    cfg: &Config,
    f: &dyn fingerprint::Fingerprinter,
) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let hash = cfg
        .hash
        .enabled
        .then(|| hash::file_hash(path, &cfg.hash).ok());
    if let Some(h) = hash.flatten() {
        m.insert("file_hash".into(), h);
    }
    let fp = cfg
        .fingerprint
        .enabled
        .then(|| f.fingerprint(path, &cfg.fingerprint));
    if let Some(fp) = fp.flatten() {
        m.insert("file_fingerprint".into(), fp);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_partial_first_and_describe() {
        let cfg = Config::load(None).unwrap();
        assert_eq!(cfg.hash.mode, "partial");
        assert!(!cfg.fingerprint.enabled);
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.mkv");
        std::fs::write(&f, b"x").unwrap();
        let m = describe(&f, &cfg, &fingerprint::NoFingerprint);
        assert!(
            m["file_hash"].starts_with("b3:"),
            "small files are hashed whole"
        );
        assert!(!m.contains_key("file_fingerprint"));
    }
}
