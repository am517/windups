//! The handoff: Jack only fetches and verifies; one entry per file goes into his own archive
//! (jack.db) and on to Daniel, the database windup (his own crate), with the disagreements Jack
//! found between sources. `--emit-json` also writes the versioned JSON (see schema/handoff_v1.md).
use crate::archive::{Archive, ArchiveCfg};
use crate::meta::Meta;
use daniel::StoreCfg;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use windups::handoff::{Daniel, Handoff};

static CALLER: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Who called this Jack run (`--caller`), carried in every handoff.
pub fn set_caller(c: Option<String>) {
    let _ = CALLER.set(c);
}

/// Hands one entry for `path` over: first into Jack's own archive (jack.db, always), then to
/// Daniel in-process when `[store] enabled`, and as JSON when `--emit-json` asks. With no database
/// for Daniel, nothing is lost and nothing is jotted beside the media: the record waits in jack.db.
pub fn hand_over(
    path: &Path,
    entry: &Meta,
    found: &[(String, Meta)],
    archive: &ArchiveCfg,
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

    // the archive comes first: if Jack can't keep his own record, that is a real failure
    let (id, new) = Archive::open(archive)?.write(&h)?;
    let mut notes = vec![format!(
        "jack.db: {id}{}",
        if new { "" } else { " (already kept)" }
    )];
    if let Some(dest) = emit_json {
        notes.push(emit(&h, dest, path)?);
    }
    if store.enabled {
        match daniel::Store::new(store).accept(&h) {
            Ok(w) => notes.push(w),
            Err(e) => notes.push(format!("{e}; kept in jack.db")),
        }
    }
    Ok(notes.join("; "))
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

    fn archive(d: &tempfile::TempDir) -> ArchiveCfg {
        ArchiveCfg {
            database: d.path().join("jack.db").to_string_lossy().into_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn no_database_keeps_the_record_in_jack_db_and_writes_no_json() {
        let d = tempfile::tempdir().unwrap();
        let media = d.path().join("The.Matrix.1999.mkv");
        std::fs::write(&media, b"x").unwrap();
        let store = StoreCfg {
            enabled: false,
            ..Default::default()
        };
        let entry = meta(&[
            ("kind", "movie"),
            ("title", "The Matrix"),
            ("year", "1999"),
            ("lifecycle", "unverified"),
        ]);
        let found = vec![("tmdb".into(), meta(&[("title", "The Matrix")]))];
        let r = hand_over(&media, &entry, &found, &archive(&d), &store, None).unwrap();
        assert!(r.starts_with("jack.db: jack-"), "{r}");
        let mut left: Vec<_> = std::fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["The.Matrix.1999.mkv", "jack.db"],
            "no sidecar, no loose JSON"
        );
        let a = Archive::open(&archive(&d)).unwrap();
        let u = a.unsynced(0, 10).unwrap();
        let h = Handoff::read_json(u[0].handoff.as_bytes()).unwrap();
        assert_eq!(h.file, media);
        assert_eq!(h.from, "jack");
        assert_eq!(h.entry.get("title").map(String::as_str), Some("The Matrix"));
        assert_eq!(h.candidates[0].0, "tmdb");
        // asked for explicitly, --emit-json DIR still writes the same v1 object
        let dest = d.path().join("out");
        let r = hand_over(
            &media,
            &entry,
            &found,
            &archive(&d),
            &store,
            Some(dest.to_str().unwrap()),
        )
        .unwrap();
        assert!(r.contains("(already kept)"), "{r}");
        let emitted = dest.join("The.Matrix.1999.mkv.jack.json");
        let h2 = Handoff::read_json(std::fs::File::open(&emitted).unwrap()).unwrap();
        assert_eq!(h, h2);
        assert_eq!(a.counts().unwrap(), (1, 1), "the re-run did not duplicate");
    }

    #[test]
    fn unreachable_daniel_is_a_note_not_a_loss() {
        let d = tempfile::tempdir().unwrap();
        let media = d.path().join("a.mkv");
        std::fs::write(&media, b"x").unwrap();
        let store = StoreCfg {
            enabled: true,
            database: d.path().to_string_lossy().into_owned(), // a folder, not a database
            busy_timeout_secs: 1,
            ..Default::default()
        };
        let r = hand_over(
            &media,
            &meta(&[("title", "A")]),
            &[],
            &archive(&d),
            &store,
            None,
        )
        .unwrap();
        assert!(r.contains("kept in jack.db"), "{r}");
        assert!(!d.path().join("a.mkv.jack.json").exists());
        assert_eq!(
            Archive::open(&archive(&d)).unwrap().counts().unwrap(),
            (1, 1)
        );
    }
}
