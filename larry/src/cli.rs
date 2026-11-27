//! `larry hash [--full] [-j] FILE...`: print each file's content hash. Reading only: nothing is
//! stored and no file is changed.
use crate::hash::{HashCfg, file_hash};
use std::fmt::Write;
use std::path::PathBuf;

/// The command line, without the "usage:" label (help adds its own).
pub const COMMAND_LINE: &str = "larry hash [--full] [-j] FILE... | larry -h [-j]";

pub fn usage() -> String {
    format!("usage: {COMMAND_LINE}")
}

#[derive(Debug, PartialEq)]
pub struct Request {
    pub full: bool,
    pub json: bool,
    pub files: Vec<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Request, String> {
    let mut r = Request {
        full: false,
        json: false,
        files: Vec::new(),
    };
    for a in args {
        match a.as_str() {
            "--full" => r.full = true,
            "-j" | "--json" => r.json = true,
            s if s.starts_with('-') => return Err(format!("unknown argument: {s}\n{}", usage())),
            s => r.files.push(PathBuf::from(s)),
        }
    }
    if r.files.is_empty() {
        return Err(format!("no file given\n{}", usage()));
    }
    Ok(r)
}

/// One result per file, in order: its hash, or why it couldn't be read.
pub fn hash_all(r: &Request, cfg: &HashCfg) -> Vec<(PathBuf, Result<String, String>)> {
    let mut cfg = cfg.clone();
    if r.full {
        cfg.mode = "full".into();
    }
    r.files
        .iter()
        .map(|f| {
            let h = file_hash(f, &cfg).map_err(|e| e.to_string());
            (f.clone(), h)
        })
        .collect()
}

/// Human lines say which kind each hash is: `b3:` proves identical bytes, `b3p:` is a strong hint.
pub fn human(results: &[(PathBuf, Result<String, String>)]) -> String {
    let mut out = String::new();
    for (f, r) in results {
        let _ = match r {
            Ok(h) if h.starts_with("b3p:") => writeln!(
                out,
                "{h}  {} (partial: a strong hint, not proof)",
                f.display()
            ),
            Ok(h) => writeln!(out, "{h}  {} (whole file)", f.display()),
            Err(e) => writeln!(out, "not hashed  {}: {e}", f.display()),
        };
    }
    out
}

pub fn json(results: &[(PathBuf, Result<String, String>)]) -> String {
    let items: Vec<serde_json::Value> = results
        .iter()
        .map(|(f, r)| match r {
            Ok(h) => serde_json::json!({
                "file": f.to_string_lossy(),
                "ok": true,
                "hash": h,
                "whole": h.starts_with("b3:"),
            }),
            Err(e) => serde_json::json!({
                "file": f.to_string_lossy(),
                "ok": false,
                "reason": "unreadable",
                "error": e,
            }),
        })
        .collect();
    let ok = results.iter().all(|(_, r)| r.is_ok());
    serde_json::json!({"v": 1, "ok": ok, "items": items}).to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn files_and_flags_are_parsed_and_bad_input_is_explained() {
        let r = parse(&args(&["--full", "-j", "a", "b"])).unwrap();
        assert!(r.full && r.json && r.files.len() == 2);
        assert!(parse(&args(&[])).unwrap_err().starts_with("no file given"));
        assert!(
            parse(&args(&["--fast", "a"]))
                .unwrap_err()
                .starts_with("unknown argument")
        );
    }

    #[test]
    fn full_hashes_a_big_file_whole_and_a_missing_file_is_reported_not_fatal() {
        let d = tempfile::tempdir().unwrap();
        let big = d.path().join("big.bin");
        std::fs::write(&big, vec![7u8; 64]).unwrap();
        let before = std::fs::metadata(&big).unwrap().modified().unwrap();
        let cfg = HashCfg {
            chunk_bytes: 8,
            full_below_bytes: 16,
            ..HashCfg::default()
        };
        let missing = d.path().join("gone.bin");
        let partial = Request {
            full: false,
            json: false,
            files: vec![big.clone(), missing],
        };
        let r = hash_all(&partial, &cfg);
        assert!(r[0].1.as_ref().unwrap().starts_with("b3p:"));
        assert!(r[1].1.is_err());
        let h = human(&r);
        assert!(h.contains("(partial: a strong hint, not proof)"), "{h}");
        assert!(h.contains("not hashed"), "{h}");
        let j: serde_json::Value = serde_json::from_str(&json(&r)).unwrap();
        assert_eq!(j["ok"], false);
        assert_eq!(j["items"][1]["reason"], "unreadable");
        let full = Request {
            full: true,
            ..partial
        };
        assert!(
            hash_all(&full, &cfg)[0]
                .1
                .as_ref()
                .unwrap()
                .starts_with("b3:")
        );
        assert_eq!(
            std::fs::metadata(&big).unwrap().modified().unwrap(),
            before,
            "only read"
        );
    }
}
