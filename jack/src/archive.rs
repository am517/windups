//! Jack's own database (`jack.db`): his archive and source of truth. Every record he creates goes
//! here first, as handoff JSON v1, and stays here after the sync target (Daniel) has it (the
//! duplication is on purpose: either side can rebuild the other). Sync state is one nullable
//! column, `synced_at`. The shared core's tables (event log, snapshots) come in by migration.
use rusqlite::{Connection, params};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use windups::handoff::Handoff;

/// File name of Jack's database inside his folder.
pub const DB_NAME: &str = "jack.db";

/// Jack's own tables (schema v1, as first shipped).
const RECORDS_V1: &str = "
CREATE TABLE IF NOT EXISTS records (
    seq        INTEGER PRIMARY KEY,
    id         TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    file       TEXT NOT NULL,
    handoff    TEXT NOT NULL,
    synced_at  INTEGER
);
CREATE INDEX IF NOT EXISTS records_unsynced ON records(seq) WHERE synced_at IS NULL;
";

/// jack.db migrations in order; index + 1 is `PRAGMA user_version`. Append only.
pub const MIGRATIONS: &[&str] = &[
    RECORDS_V1,
    windups::base::events::SCHEMA_V1,
    windups::base::snapshot::SCHEMA_V1,
];

/// How a record's id is made.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum IdStrategy {
    /// From the record's own bytes: an identical re-run is the same record (default).
    #[default]
    Content,
    /// Each run's records are new ones, even when identical.
    PerRun,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct ArchiveCfg {
    /// Path of jack.db; empty = `jack.db` in Jack's folder (beside his config file).
    pub database: String,
    pub busy_timeout_secs: u64,
    /// Records sent per batch (bounds memory while syncing).
    pub sync_batch: usize,
    /// Who gets copies: "daniel" or "none".
    pub sync_target: String,
    /// How Daniel is reached: "library" (in-process call) or "socket" (`daniel serve`).
    pub sync_transport: String,
    /// Daniel's socket for `sync_transport = "socket"`.
    pub daniel_socket: String,
    pub socket_timeout_secs: u64,
    pub max_frame_bytes: usize,
    pub id: IdStrategy,
}

impl Default for ArchiveCfg {
    fn default() -> Self {
        Self {
            database: String::new(),
            busy_timeout_secs: 5,
            sync_batch: 32,
            sync_target: "daniel".into(),
            sync_transport: "library".into(),
            daniel_socket: "~/windups/daniel/daniel.sock".into(),
            socket_timeout_secs: 30,
            max_frame_bytes: 16 << 20,
            id: IdStrategy::Content,
        }
    }
}

/// One stored record (from `unsynced` or `page`).
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

/// Stable id from the record's own bytes (FNV-1a, 128-bit), plus a salt for `per_run`: the same
/// record always gets the same id, so writing it twice or the target receiving it twice changes nothing.
pub fn record_id(json: &[u8], salt: &[u8]) -> String {
    const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013b;
    let h = salt
        .iter()
        .chain(json)
        .fold(OFFSET, |h, b| (h ^ u128::from(*b)).wrapping_mul(PRIME));
    format!("jack-{h:032x}")
}

/// One value per process run (time + pid), for `id = "per_run"`.
fn run_salt() -> &'static [u8] {
    static S: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("run:{t}:{}|", std::process::id()).into_bytes()
    })
}

pub struct Archive {
    c: Connection,
    salt: &'static [u8],
}

impl Archive {
    pub fn open(cfg: &ArchiveCfg) -> Result<Self, String> {
        let p = &cfg.database;
        if let Some(dir) = Path::new(p).parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut c = Connection::open(p).map_err(|e| format!("{p}: {e}"))?;
        c.busy_timeout(std::time::Duration::from_secs(cfg.busy_timeout_secs))
            .map_err(|e| format!("{p}: {e}"))?;
        windups::base::schema::migrate(&mut c, DB_NAME, MIGRATIONS)?;
        let salt = match cfg.id {
            IdStrategy::Content => &[][..],
            IdStrategy::PerRun => run_salt(),
        };
        Ok(Self { c, salt })
    }

    pub fn conn(&mut self) -> &mut Connection {
        &mut self.c
    }

    #[cfg(test)]
    pub fn with_salt(mut self, salt: &'static [u8]) -> Self {
        self.salt = salt;
        self
    }

    /// Writes one record (as handoff JSON v1). Returns its id and whether it was new.
    pub fn write(&self, h: &Handoff) -> Result<(String, bool), String> {
        let mut json = Vec::with_capacity(512);
        h.write_json(&mut json)?;
        let id = record_id(&json, self.salt);
        let json = String::from_utf8(json).map_err(|e| e.to_string())?;
        let n = self
            .c
            .execute(
                "INSERT INTO records(id, created_at, file, handoff) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO NOTHING",
                params![id, now(), h.file.to_string_lossy(), json],
            )
            .map_err(|e| e.to_string())?;
        if n == 1 {
            windups::base::events::append(&self.c, "record", &id)?;
        }
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

    /// Up to `limit` records after `after_seq`, synced or not, oldest first (for export).
    pub fn page(&self, after_seq: i64, limit: usize) -> Result<Vec<Unsynced>, String> {
        let mut st = self
            .c
            .prepare_cached(
                "SELECT seq, id, handoff FROM records WHERE seq > ?1 ORDER BY seq LIMIT ?2",
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

    /// Marks one record synced (only call after the target acknowledged it). Keeps the record.
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
    fn a_v1_jack_db_migrates_forward_keeping_its_records() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(DB_NAME);
        {
            // exactly what the first jack.db looked like
            let c = Connection::open(&p).unwrap();
            c.execute_batch(RECORDS_V1).unwrap();
            c.execute_batch("PRAGMA user_version = 1;").unwrap();
            c.execute(
                "INSERT INTO records(id, created_at, file, handoff) VALUES ('jack-old', 1, '/a', '{}')",
                [],
            )
            .unwrap();
        }
        let mut a = open_temp(&d);
        let v = windups::base::schema::version(a.conn()).unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);
        assert_eq!(a.counts().unwrap(), (1, 1));
        a.write(&handoff("New")).unwrap();
        let ev = windups::base::events::recent(a.conn(), 5).unwrap();
        assert_eq!(
            (ev.len(), ev[0].kind.as_str()),
            (1, "record"),
            "writes are logged"
        );
    }

    #[test]
    fn per_run_ids_make_identical_runs_separate_records() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d).with_salt(b"run1|");
        let (one, _) = a.write(&handoff("A")).unwrap();
        let a = a.with_salt(b"run2|");
        let (two, new) = a.write(&handoff("A")).unwrap();
        assert!(new && one != two);
        assert_eq!(a.counts().unwrap(), (2, 2));
        let a = a.with_salt(b"");
        let (plain, _) = a.write(&handoff("A")).unwrap();
        assert_eq!(
            plain,
            record_id(a.unsynced(2, 1).unwrap()[0].handoff.as_bytes(), b"")
        );
    }

    #[test]
    fn default_path_is_beside_the_config() {
        assert_eq!(
            default_path(Path::new("/home/u/.config/windups/jack/config.toml")),
            PathBuf::from("/home/u/.config/windups/jack/jack.db")
        );
    }
}
