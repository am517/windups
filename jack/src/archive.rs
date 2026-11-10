//! Jack's own database (`jack.db`): his archive and source of truth. Every record he creates goes
//! here first, as handoff JSON v1, and stays here after Walter has a copy (the duplication is on
//! purpose: either side can rebuild the other). Sync state is one nullable column, `synced_at`.
use rusqlite::{Connection, params};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use windups::handoff::Handoff;

/// File name of Jack's database inside his folder.
pub const DB_NAME: &str = "jack.db";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS records (
    seq        INTEGER PRIMARY KEY,
    id         TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    file       TEXT NOT NULL,
    handoff    TEXT NOT NULL,
    synced_at  INTEGER
);
CREATE INDEX IF NOT EXISTS records_unsynced ON records(seq) WHERE synced_at IS NULL;
PRAGMA user_version = 1;
";

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct ArchiveCfg {
    /// Path of jack.db; empty = `jack.db` in Jack's folder (beside his config file).
    pub database: String,
    pub busy_timeout_secs: u64,
    /// Records sent to Walter per batch (bounds memory while syncing).
    pub sync_batch: usize,
}

impl Default for ArchiveCfg {
    fn default() -> Self {
        Self {
            database: String::new(),
            busy_timeout_secs: 5,
            sync_batch: 32,
        }
    }
}

/// One record not yet acknowledged by Walter.
pub struct Unsynced {
    pub seq: i64,
    pub id: String,
    pub handoff: String,
}

/// Where jack.db lives when `[archive] database` is empty: beside the config file.
pub fn default_path(config_file: &Path) -> PathBuf {
    config_file
        .parent()
        .map_or_else(|| PathBuf::from(DB_NAME), |d| d.join(DB_NAME))
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Stable id from the record's own bytes (FNV-1a, 128-bit): the same record always gets the same
/// id, so writing it twice or Walter receiving it twice changes nothing.
pub fn record_id(json: &[u8]) -> String {
    const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013b;
    let h = json
        .iter()
        .fold(OFFSET, |h, b| (h ^ u128::from(*b)).wrapping_mul(PRIME));
    format!("jack-{h:032x}")
}

pub struct Archive {
    c: Connection,
}

impl Archive {
    pub fn open(cfg: &ArchiveCfg) -> Result<Self, String> {
        let p = &cfg.database;
        if let Some(dir) = Path::new(p).parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let c = Connection::open(p).map_err(|e| format!("{p}: {e}"))?;
        c.busy_timeout(std::time::Duration::from_secs(cfg.busy_timeout_secs))
            .and_then(|_| c.execute_batch(SCHEMA))
            .map_err(|e| format!("{p}: {e}"))?;
        Ok(Self { c })
    }

    /// Writes one record (as handoff JSON v1). Returns its id and whether it was new.
    pub fn write(&self, h: &Handoff) -> Result<(String, bool), String> {
        let mut json = Vec::with_capacity(512);
        h.write_json(&mut json)?;
        let id = record_id(&json);
        let json = String::from_utf8(json).map_err(|e| e.to_string())?;
        let n = self
            .c
            .execute(
                "INSERT INTO records(id, created_at, file, handoff) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO NOTHING",
                params![id, now(), h.file.to_string_lossy(), json],
            )
            .map_err(|e| e.to_string())?;
        Ok((id, n == 1))
    }

    /// Up to `limit` unsynced records after `after_seq`, oldest first.
    pub fn unsynced(&self, after_seq: i64, limit: usize) -> Result<Vec<Unsynced>, String> {
        let mut st = self
            .c
            .prepare_cached(
                "SELECT seq, id, handoff FROM records
                 WHERE synced_at IS NULL AND seq > ?1 ORDER BY seq LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![after_seq, limit as i64], |r| {
                Ok(Unsynced {
                    seq: r.get(0)?,
                    id: r.get(1)?,
                    handoff: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    /// Marks one record synced (only call after Walter acknowledged it). Keeps the record.
    pub fn mark_synced(&self, seq: i64, at: i64) -> Result<(), String> {
        self.c
            .execute(
                "UPDATE records SET synced_at = ?2 WHERE seq = ?1 AND synced_at IS NULL",
                params![seq, at],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// (all records, not yet synced).
    pub fn counts(&self) -> Result<(i64, i64), String> {
        self.c
            .query_row(
                "SELECT count(*), count(*) - count(synced_at) FROM records",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())
    }

    /// When the record with this id was synced, if it exists.
    #[cfg(test)]
    pub fn synced_at(&self, id: &str) -> Option<Option<i64>> {
        use rusqlite::OptionalExtension;
        self.c
            .query_row("SELECT synced_at FROM records WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .unwrap()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn handoff(title: &str) -> Handoff {
        Handoff {
            file: PathBuf::from(format!("/media/{title}.mkv")),
            entry: [("title", title), ("lifecycle", "unverified")]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            candidates: vec![],
            conflicts: vec![],
            from: "jack".into(),
            caller: None,
        }
    }

    pub fn open_temp(d: &tempfile::TempDir) -> Archive {
        Archive::open(&ArchiveCfg {
            database: d.path().join(DB_NAME).to_string_lossy().into_owned(),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn writes_v1_records_with_stable_ids() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        let (id, new) = a.write(&handoff("The Matrix")).unwrap();
        assert!(new && id.starts_with("jack-") && id.len() == 37, "{id}");
        let (again, new) = a.write(&handoff("The Matrix")).unwrap();
        assert_eq!(
            (again.as_str(), new),
            (id.as_str(), false),
            "same record, same id"
        );
        assert_eq!(a.counts().unwrap(), (1, 1));
        let u = a.unsynced(0, 10).unwrap();
        let h = Handoff::read_json(u[0].handoff.as_bytes()).unwrap();
        assert_eq!(h, handoff("The Matrix"), "stored as handoff JSON v1");
        assert!(u[0].handoff.starts_with("{\"v\":1,"), "{}", u[0].handoff);
        drop(a);
        // survives reopening (it is the archive)
        assert_eq!(open_temp(&d).counts().unwrap(), (1, 1));
    }

    #[test]
    fn selects_only_unsynced_in_order_and_in_batches() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        for t in ["A", "B", "C"] {
            a.write(&handoff(t)).unwrap();
        }
        let first = a.unsynced(0, 2).unwrap();
        assert_eq!(first.len(), 2);
        a.mark_synced(first[0].seq, 7).unwrap();
        let left: Vec<_> = a
            .unsynced(0, 10)
            .unwrap()
            .into_iter()
            .map(|u| u.seq)
            .collect();
        assert_eq!(left, vec![first[1].seq, first[1].seq + 1]);
        assert_eq!(a.unsynced(first[1].seq, 10).unwrap().len(), 1);
        assert_eq!(a.counts().unwrap(), (3, 2));
    }

    #[test]
    fn default_path_is_beside_the_config() {
        assert_eq!(
            default_path(Path::new("/home/u/.config/windups/jack/config.toml")),
            PathBuf::from("/home/u/.config/windups/jack/jack.db")
        );
    }
}
