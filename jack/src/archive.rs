//! Jack's own database (`jack.db`): his archive and source of truth. The records table, ids and
//! sync state come from the shared core (`windups::base::records`); here is only where jack.db
//! lives, how it is configured, and its migrations (records, event log, snapshots).
use serde::Deserialize;
use std::path::{Path, PathBuf};
pub use windups::base::records::{IdStrategy, Records as Archive, StoredRecord as Unsynced, now};

/// File name of Jack's database inside his folder.
pub const DB_NAME: &str = "jack.db";

/// jack.db migrations in order; index + 1 is `PRAGMA user_version`. Append only.
pub const MIGRATIONS: &[&str] = &[
    windups::base::records::SCHEMA_V1,
    windups::base::events::SCHEMA_V1,
    windups::base::snapshot::SCHEMA_V1,
];

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct ArchiveCfg {
    /// Path of jack.db; empty = `jack.db` in Jack's folder (beside his config file).
    pub database: String,
    pub busy_timeout_secs: u64,
    /// Records sent per batch (bounds memory while syncing).
    pub sync_batch: usize,
    /// Legacy: who gets copies. Prefer `[relationships] database`. Empty or "none" = no target.
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
            sync_target: String::new(),
            sync_transport: "library".into(),
            daniel_socket: String::new(),
            socket_timeout_secs: 30,
            max_frame_bytes: 16 << 20,
            id: IdStrategy::Content,
        }
    }
}

/// Where jack.db lives when `[archive] database` is empty: beside the config file.
pub fn default_path(config_file: &Path) -> PathBuf {
    config_file
        .parent()
        .map_or_else(|| PathBuf::from(DB_NAME), |d| d.join(DB_NAME))
}

/// Opens jack.db as configured in `[archive]`.
pub fn open(cfg: &ArchiveCfg) -> Result<Archive, String> {
    Archive::open(&windups::base::records::Options {
        name: "jack",
        path: &cfg.database,
        busy_timeout_secs: cfg.busy_timeout_secs,
        id: cfg.id,
        migrations: MIGRATIONS,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use rusqlite::Connection;
    use windups::handoff::Handoff;

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
        open(&ArchiveCfg {
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
            c.execute_batch(windups::base::records::SCHEMA_V1).unwrap();
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
            windups::base::records::record_id(
                "jack",
                a.unsynced(2, 1).unwrap()[0].handoff.as_bytes(),
                b""
            )
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
