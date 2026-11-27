//! The handoff: Jack only fetches and verifies; one entry per file goes into his own archive
//! (jack.db), with the disagreements Jack found between sources. Daniel, the record keeper, gets
//! it from there by sync (see sync.rs): one route. `jack export` writes the kept records as
//! versioned JSON (see schema/handoff_v1.md).
use crate::archive::ArchiveCfg;
use crate::meta::Meta;
use std::path::Path;
use windups::handoff::Handoff;

static CALLER: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Who called this Jack run (`--caller`), carried in every handoff.
pub fn set_caller(c: Option<String>) {
    let _ = CALLER.set(c);
}

/// Keeps one entry for `path` in Jack's own archive (jack.db). Nothing is jotted beside the media;
/// the sync at the end of the run takes it to Daniel, and if he can't be reached the record just
/// waits in jack.db.
pub fn hand_over(
    path: &Path,
    entry: &Meta,
    found: &[(String, Meta)],
    archive: &ArchiveCfg,
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
    let (id, new) = crate::archive::open(archive)?.write(&h)?;
    Ok(format!(
        "jack.db: {id}{}",
        if new { "" } else { " (already kept)" }
    ))
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
        let r = hand_over(&media, &entry, &found, &archive(&d)).unwrap();
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
        let a = crate::archive::open(&archive(&d)).unwrap();
        let u = a.unsynced(0, 10).unwrap();
        let h = Handoff::read_json(u[0].handoff.as_bytes()).unwrap();
        assert_eq!(h.file, media);
        assert_eq!(h.from, "jack");
        assert_eq!(h.entry.get("title").map(String::as_str), Some("The Matrix"));
        assert_eq!(h.candidates[0].0, "tmdb");
        let r = hand_over(&media, &entry, &found, &archive(&d)).unwrap();
        assert!(r.contains("(already kept)"), "{r}");
        assert_eq!(a.counts().unwrap(), (1, 1), "the re-run did not duplicate");
    }
}
