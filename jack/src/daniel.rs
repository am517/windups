//! The handoff: Jack only fetches and verifies; one entry per file goes to Daniel, the database
//! windup (his own crate), with the disagreements Jack found between sources. When there is no
//! database, or when asked, Jack also writes a versioned handoff JSON (see schema/handoff_v1.md).
use crate::meta::Meta;
use daniel::{HandoffJson, StoreCfg};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use windups::handoff::{Daniel, Handoff};

static CALLER: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Who called this Jack run (`--caller`), carried in every handoff.
pub fn set_caller(c: Option<String>) {
    let _ = CALLER.set(c);
}

/// Whether this run should hand over (in-process, sidecar, or `--emit-json`).
pub fn needed(store: &StoreCfg, emit_json: Option<&str>) -> bool {
    store.enabled || emit_json.is_some() || store.handoff_json != HandoffJson::Off
}

/// Hands one entry for `path` to Daniel and/or writes the versioned JSON.
pub fn hand_over(
    path: &Path,
    entry: &Meta,
    found: &[(String, Meta)],
    store: &StoreCfg,
    emit_json: Option<&str>,
) -> Result<String, String> {
    let conflicts = if entry.get("resolution").is_some_and(|r| r == "unresolved") {
        vec![]
    } else {
        crate::conflicts::find(entry, found)
    };
    let h = Handoff {
        file: path.to_path_buf(),
        entry: entry.clone(),
        candidates: found.to_vec(),
        conflicts,
        from: "jack".into(),
        caller: CALLER.get().cloned().flatten(),
    };

    let mut notes = Vec::new();
    if let Some(dest) = emit_json {
        notes.push(emit(&h, dest, path)?);
    }

    let sidecar = sidecar_path(path, store);
    let write_now = match store.handoff_json {
        HandoffJson::Always => true,
        HandoffJson::Off => false,
        HandoffJson::Auto => !store.enabled,
    };
    if write_now {
        write_file(&h, &sidecar)?;
        notes.push(format!("saved {}", sidecar.display()));
    }

    if store.enabled {
        match daniel::Store::new(store).accept(&h) {
            Ok(w) => notes.push(w),
            Err(e) => {
                if store.handoff_json == HandoffJson::Auto && !write_now {
                    write_file(&h, &sidecar)?;
                    notes.push(format!("no database ({e}); saved {}", sidecar.display()));
                } else {
                    return Err(e);
                }
            }
        }
    }

    if notes.is_empty() {
        Ok("handoff: nothing written".into())
    } else {
        Ok(notes.join("; "))
    }
}

fn sidecar_path(path: &Path, store: &StoreCfg) -> PathBuf {
    let suffix = if store.fallback_suffix.is_empty() {
        ".jack.json"
    } else {
        store.fallback_suffix.as_str()
    };
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn write_file(h: &Handoff, path: &Path) -> Result<(), String> {
    let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut w = BufWriter::new(f);
    h.write_json(&mut w)?;
    w.write_all(b"\n")
        .map_err(|e| format!("{}: {e}", path.display()))?;
    w.flush().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

/// `--emit-json -` writes one object to stdout; `--emit-json DIR` writes `<name>.jack.json` there.
fn emit(h: &Handoff, dest: &str, media: &Path) -> Result<String, String> {
    if dest == "-" {
        let stdout = std::io::stdout();
        let mut w = BufWriter::new(stdout.lock());
        h.write_json(&mut w)?;
        w.write_all(b"\n").map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        Ok("handoff json: stdout".into())
    } else {
        std::fs::create_dir_all(dest).map_err(|e| format!("{dest}: {e}"))?;
        let mut p = PathBuf::from(dest);
        p.push(
            media
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("handoff")),
        );
        let mut s = p.into_os_string();
        s.push(".jack.json");
        let p = PathBuf::from(s);
        write_file(h, &p)?;
        Ok(format!("handoff json: {}", p.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(kv: &[(&str, &str)]) -> Meta {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn auto_with_no_db_writes_json_that_round_trips() {
        let d = tempfile::tempdir().unwrap();
        let media = d.path().join("The.Matrix.1999.mkv");
        std::fs::write(&media, b"x").unwrap();
        let store = StoreCfg {
            enabled: false,
            database: String::new(),
            fallback_suffix: ".jack.json".into(),
            busy_timeout_secs: 1,
            handoff_json: HandoffJson::Auto,
        };
        let entry = meta(&[
            ("kind", "movie"),
            ("title", "The Matrix"),
            ("year", "1999"),
            ("lifecycle", "unverified"),
        ]);
        let found = vec![("tmdb".into(), meta(&[("title", "The Matrix")]))];
        let r = hand_over(&media, &entry, &found, &store, None).unwrap();
        let side = d.path().join("The.Matrix.1999.mkv.jack.json");
        assert!(r.contains(&side.display().to_string()), "{r}");
        assert!(!r.contains("database:"), "{r}");
        let h = Handoff::read_json(std::fs::File::open(&side).unwrap()).unwrap();
        assert_eq!(h.file, media);
        assert_eq!(h.from, "jack");
        assert_eq!(h.entry.get("title").map(String::as_str), Some("The Matrix"));
        assert_eq!(h.candidates[0].0, "tmdb");
        // also emit into a directory
        let dest = d.path().join("out");
        hand_over(&media, &entry, &found, &store, Some(dest.to_str().unwrap())).unwrap();
        let emitted = dest.join("The.Matrix.1999.mkv.jack.json");
        let h2 = Handoff::read_json(std::fs::File::open(&emitted).unwrap()).unwrap();
        assert_eq!(h, h2);
    }

    #[test]
    fn off_does_not_write_sidecar_when_db_disabled() {
        let d = tempfile::tempdir().unwrap();
        let media = d.path().join("a.mkv");
        std::fs::write(&media, b"x").unwrap();
        let store = StoreCfg {
            enabled: false,
            fallback_suffix: ".jack.json".into(),
            handoff_json: HandoffJson::Off,
            ..Default::default()
        };
        let r = hand_over(&media, &meta(&[("title", "A")]), &[], &store, None).unwrap();
        assert!(r.contains("nothing written"), "{r}");
        assert!(!d.path().join("a.mkv.jack.json").exists());
    }
}
