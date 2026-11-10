//! The handoff: Jack only fetches and verifies; one entry per file goes into his own archive
//! (jack.db), with the disagreements Jack found between sources. Daniel, the record keeper, gets
//! it from there by sync (see sync.rs): one route. `--emit-json` also writes the versioned JSON
//! (see schema/handoff_v1.md).
use crate::archive::{Archive, ArchiveCfg};
use crate::meta::Meta;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use windups::handoff::Handoff;

static CALLER: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Who called this Jack run (`--caller`), carried in every handoff.
pub fn set_caller(c: Option<String>) {
    let _ = CALLER.set(c);
}

/// Keeps one entry for `path` in Jack's own archive (jack.db), and writes it as JSON when
/// `--emit-json` asks. Nothing is jotted beside the media; the sync at the end of the run takes it
/// to Daniel, and if he can't be reached the record just waits in jack.db.
pub fn hand_over(
    path: &Path,
    entry: &Meta,
    found: &[(String, Meta)],
    archive: &ArchiveCfg,
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
    fn the_record_is_kept_in_jack_db_and_no_json_is_written_beside_the_media() {
        let d = tempfile::tempdir().unwrap();
        let media = d.path().join("The.Matrix.1999.mkv");
        std::fs::write(&media, b"x").unwrap();
        let entry = meta(&[
            ("kind", "movie"),
            ("title", "The Matrix"),
            ("year", "1999"),
            ("lifecycle", "unverified"),
        ]);
        let found = vec![("tmdb".into(), meta(&[("title", "The Matrix")]))];
        let r = hand_over(&media, &entry, &found, &archive(&d), None).unwrap();
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
            Some(dest.to_str().unwrap()),
        )
        .unwrap();
        assert!(r.contains("(already kept)"), "{r}");
        let emitted = dest.join("The.Matrix.1999.mkv.jack.json");
        let h2 = Handoff::read_json(std::fs::File::open(&emitted).unwrap()).unwrap();
        assert_eq!(h, h2);
        assert_eq!(a.counts().unwrap(), (1, 1), "the re-run did not duplicate");
    }
}
