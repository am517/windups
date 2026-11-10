//! Checked snapshots of a windup's own database: SQLite's online backup API copies it while it is
//! in use, then the copy must pass `PRAGMA integrity_check` and match the schema version, or it
//! is deleted. Off-device copies and watching are for Walter or a later windup: see `OffDevice`.
use rusqlite::{Connection, OptionalExtension, backup::Backup, params};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS snapshots (
    seq            INTEGER PRIMARY KEY,
    at             INTEGER NOT NULL,
    path           TEXT NOT NULL,
    bytes          INTEGER NOT NULL,
    events_through INTEGER NOT NULL,
    pruned_at      INTEGER
);
";

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct SnapshotCfg {
    /// Folder for snapshots; empty = `~/.local/share/windups/backups/<name>/`.
    pub dir: String,
    /// Snapshots kept (ones holding the originals of summarized events are always kept).
    pub keep: usize,
    /// Pages copied per backup step, and the pause between steps, so a big copy never hogs the DB.
    pub pages_per_step: i32,
    pub pause_ms: u64,
    /// A newest snapshot older than this counts as stale (health, audit).
    pub stale_after_days: u64,
}

impl Default for SnapshotCfg {
    fn default() -> Self {
        Self {
            dir: String::new(),
            keep: 14,
            pages_per_step: 256,
            pause_ms: 0,
            stale_after_days: 7,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub path: PathBuf,
    pub bytes: u64,
    pub events_through: i64,
    pub pruned: Vec<PathBuf>,
}

/// Called after every good snapshot. Walter (or a later windup) copies it off the device.
pub trait OffDevice {
    fn taken(&self, s: &Snapshot) -> Result<(), String>;
}

/// Nobody takes copies off the device yet.
pub struct NoOffDevice;

impl OffDevice for NoOffDevice {
    fn taken(&self, _: &Snapshot) -> Result<(), String> {
        Ok(())
    }
}

/// Path of an unpruned snapshot that holds every event up to `seq`, if any.
pub fn holding(c: &Connection, seq: i64) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT path FROM snapshots WHERE pruned_at IS NULL AND events_through >= ?1
         ORDER BY seq LIMIT 1",
        [seq],
        |r| r.get(0),
    )
    .optional()
}

fn check(copy: &Path, want_version: i64) -> Result<(), String> {
    let c = Connection::open(copy).map_err(|e| e.to_string())?;
    let ok: String = c
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if ok != "ok" {
        return Err(format!("integrity_check: {ok}"));
    }
    let v = super::schema::version(&c)?;
    if v != want_version {
        return Err(format!("copy is schema {v}, database is {want_version}"));
    }
    Ok(())
}

/// Takes, checks, records and prunes. `dir` is the resolved snapshot folder.
pub fn take(
    c: &Connection,
    name: &str,
    dir: &Path,
    cfg: &SnapshotCfg,
    hook: &dyn OffDevice,
) -> Result<Snapshot, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let at = super::events::now();
    let through = super::events::last_seq(c)?;
    let mut path = dir.join(format!("{name}-{at}.db"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{name}-{at}-{n}.db"));
        n += 1;
    }
    let copied = (|| {
        let mut dst = Connection::open(&path).map_err(|e| e.to_string())?;
        Backup::new(c, &mut dst)
            .and_then(|b| {
                b.run_to_completion(
                    cfg.pages_per_step.max(1),
                    std::time::Duration::from_millis(cfg.pause_ms),
                    None,
                )
            })
            .map_err(|e| e.to_string())?;
        drop(dst);
        check(&path, super::schema::version(c)?)
    })();
    if let Err(e) = copied {
        let _ = std::fs::remove_file(&path);
        return Err(format!("snapshot {}: {e}", path.display()));
    }
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let p = path.to_string_lossy();
    c.execute(
        "INSERT INTO snapshots(at, path, bytes, events_through) VALUES (?1, ?2, ?3, ?4)",
        params![at, p, bytes as i64, through],
    )
    .map_err(|e| e.to_string())?;
    let mut s = Snapshot {
        path: path.clone(),
        bytes,
        events_through: through,
        pruned: prune(c, cfg.keep.max(1))?,
    };
    s.pruned.retain(|x| x != &path);
    hook.taken(&s)?;
    Ok(s)
}

/// Removes all but the newest `keep`, never one that holds summarized originals.
fn prune(c: &Connection, keep: usize) -> Result<Vec<PathBuf>, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let mut st = c
        .prepare(
            "SELECT seq, path FROM snapshots WHERE pruned_at IS NULL
             AND path NOT IN (SELECT snapshot FROM event_summaries)
             ORDER BY seq DESC LIMIT -1 OFFSET ?1",
        )
        .map_err(e)?;
    let old: Vec<(i64, String)> = st
        .query_map([keep as i64], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(e)?
        .collect::<Result<_, _>>()
        .map_err(e)?;
    let mut gone = Vec::with_capacity(old.len());
    for (seq, p) in old {
        match std::fs::remove_file(&p) {
            Ok(()) => {}
            Err(x) if x.kind() == std::io::ErrorKind::NotFound => {}
            Err(x) => return Err(format!("{p}: {x}")),
        }
        c.execute(
            "UPDATE snapshots SET pruned_at = ?2 WHERE seq = ?1",
            params![seq, super::events::now()],
        )
        .map_err(e)?;
        gone.push(PathBuf::from(p));
    }
    Ok(gone)
}

/// The newest good snapshot: (when, path).
pub fn latest(c: &Connection) -> Result<Option<(i64, String)>, String> {
    c.query_row(
        "SELECT at, path FROM snapshots WHERE pruned_at IS NULL ORDER BY seq DESC LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::events;
    use std::cell::RefCell;

    struct Spy(RefCell<Vec<PathBuf>>);
    impl OffDevice for Spy {
        fn taken(&self, s: &Snapshot) -> Result<(), String> {
            self.0.borrow_mut().push(s.path.clone());
            Ok(())
        }
    }

    #[test]
    fn checked_copy_is_recorded_handed_to_the_hook_and_pruned() {
        let d = tempfile::tempdir().unwrap();
        let c = events::tests::db();
        events::append(&c, "record", "one").unwrap();
        let cfg = SnapshotCfg {
            keep: 2,
            ..Default::default()
        };
        let spy = Spy(RefCell::new(vec![]));
        let s = take(&c, "jack", d.path(), &cfg, &spy).unwrap();
        assert_eq!(s.events_through, 1);
        assert_eq!(spy.0.borrow().as_slice(), [s.path.clone()]);
        let copy = Connection::open(&s.path).unwrap();
        let detail: String = copy
            .query_row("SELECT detail FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(detail, "one", "the copy holds the data");
        assert_eq!(latest(&c).unwrap().unwrap().1, s.path.to_string_lossy());
        let s2 = take(&c, "jack", d.path(), &cfg, &NoOffDevice).unwrap();
        let s3 = take(&c, "jack", d.path(), &cfg, &NoOffDevice).unwrap();
        assert_eq!(s3.pruned, [s.path.clone()]);
        assert!(!s.path.exists() && s2.path.exists() && s3.path.exists());
    }

    #[test]
    fn a_snapshot_holding_summarized_originals_is_never_pruned() {
        let d = tempfile::tempdir().unwrap();
        let mut c = events::tests::db();
        for _ in 0..10 {
            events::append(&c, "record", "").unwrap();
        }
        let cfg = SnapshotCfg {
            keep: 1,
            ..Default::default()
        };
        let first = take(&c, "jack", d.path(), &cfg, &NoOffDevice).unwrap();
        let ev = events::EventsCfg {
            keep_full: 0,
            batch_size: 5,
            keep_batches: 0,
            level: 1,
        };
        assert_eq!(events::compact(&mut c, &ev).unwrap().summarized, 2);
        take(&c, "jack", d.path(), &cfg, &NoOffDevice).unwrap();
        take(&c, "jack", d.path(), &cfg, &NoOffDevice).unwrap();
        assert!(first.path.exists(), "it holds the only originals");
    }

    #[test]
    fn a_bad_copy_is_removed_and_reported() {
        let d = tempfile::tempdir().unwrap();
        let c = events::tests::db();
        // the check refuses a copy whose schema version differs from the live database
        let bad = d.path().join("x.db");
        Connection::open(&bad)
            .unwrap()
            .execute_batch("CREATE TABLE t(x);")
            .unwrap();
        let e = check(&bad, 2).unwrap_err();
        assert!(e.contains("schema 0"), "{e}");
        // an unwritable folder fails cleanly and records nothing
        let file = d.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        assert!(take(&c, "jack", &file, &SnapshotCfg::default(), &NoOffDevice).is_err());
        assert!(latest(&c).unwrap().is_none());
    }
}
