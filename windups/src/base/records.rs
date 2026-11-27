//! A windup's own records: the `records` table in its own DB, written first and kept after sync
//! (the copy at the record keeper is on purpose: either side can rebuild the other). Each record
//! is handoff JSON v1 under a stable id; sync state is one nullable column, `synced_at`. Every
//! windup uses this; a windup only decides what goes into its record (coding_guidelines §5).
use crate::handoff::Handoff;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use std::path::Path;

/// The `records` table (schema v1, as Jack first shipped it).
pub const SCHEMA_V1: &str = "
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

/// Where and how to open a windup's records.
pub struct Options<'a> {
    /// The windup's name: the id prefix (`<name>-…`) and the DB label in messages.
    pub name: &'static str,
    /// Path of the windup's DB file.
    pub path: &'a str,
    pub busy_timeout_secs: u64,
    pub id: IdStrategy,
    /// The windup's migrations, in order (`SCHEMA_V1` among them).
    pub migrations: &'a [&'a str],
}

/// One stored record (from `unsynced` or `page`).
pub struct StoredRecord {
    pub seq: i64,
    pub id: String,
    pub handoff: String,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Stable id from the record's own bytes (FNV-1a, 128-bit), plus a salt for `per_run`: the same
/// record always gets the same id, so writing it twice or the target receiving it twice changes nothing.
pub fn record_id(name: &str, json: &[u8], salt: &[u8]) -> String {
    const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013b;
    let h = salt
        .iter()
        .chain(json)
        .fold(OFFSET, |h, b| (h ^ u128::from(*b)).wrapping_mul(PRIME));
    format!("{name}-{h:032x}")
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

pub struct Records {
    c: Connection,
    name: &'static str,
    salt: &'static [u8],
}

impl Records {
    /// Opens (or creates) the DB and runs its forward migrations.
    pub fn open(o: &Options) -> Result<Self, String> {
        let p = o.path;
        if let Some(dir) = Path::new(p).parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut c = Connection::open(p).map_err(|e| format!("{p}: {e}"))?;
        c.busy_timeout(std::time::Duration::from_secs(o.busy_timeout_secs))
            .map_err(|e| format!("{p}: {e}"))?;
        let label = Path::new(p)
            .file_name()
            .map_or_else(|| p.to_string(), |f| f.to_string_lossy().into_owned());
        crate::base::schema::migrate(&mut c, &label, o.migrations)?;
        let salt = match o.id {
            IdStrategy::Content => &[][..],
            IdStrategy::PerRun => run_salt(),
        };
        Ok(Self {
            c,
            name: o.name,
            salt,
        })
    }

    pub fn conn(&mut self) -> &mut Connection {
        &mut self.c
    }

    /// A fixed salt instead of the configured one (for tests and tools).
    pub fn with_salt(mut self, salt: &'static [u8]) -> Self {
        self.salt = salt;
        self
    }

    /// Writes one record (as handoff JSON v1). Returns its id and whether it was new.
    pub fn write(&self, h: &Handoff) -> Result<(String, bool), String> {
        let mut json = Vec::with_capacity(512);
        h.write_json(&mut json)?;
        let id = record_id(self.name, &json, self.salt);
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
            crate::base::events::append(&self.c, "record", &id)?;
        }
        Ok((id, n == 1))
    }

    /// Up to `limit` unsynced records after `after_seq`, oldest first.
    pub fn unsynced(&self, after_seq: i64, limit: usize) -> Result<Vec<StoredRecord>, String> {
        let mut st = self
            .c
            .prepare_cached(
                "SELECT seq, id, handoff FROM records
                 WHERE synced_at IS NULL AND seq > ?1 ORDER BY seq LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![after_seq, limit as i64], |r| {
                Ok(StoredRecord {
                    seq: r.get(0)?,
                    id: r.get(1)?,
                    handoff: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    /// Up to `limit` records after `after_seq`, synced or not, oldest first (for export).
    pub fn page(&self, after_seq: i64, limit: usize) -> Result<Vec<StoredRecord>, String> {
        let mut st = self
            .c
            .prepare_cached(
                "SELECT seq, id, handoff FROM records WHERE seq > ?1 ORDER BY seq LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![after_seq, limit as i64], |r| {
                Ok(StoredRecord {
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

    /// The newest record whose `file` column equals one of `paths` (raw or canonical).
    pub fn by_file(&self, paths: &[String]) -> Result<Option<(String, Handoff)>, String> {
        if paths.is_empty() {
            return Ok(None);
        }
        // newest first; a re-run of the same file should win
        let mut st = self
            .c
            .prepare_cached(
                "SELECT id, handoff FROM records WHERE file = ?1 ORDER BY seq DESC LIMIT 1",
            )
            .map_err(|e| e.to_string())?;
        for p in paths {
            let row = st
                .query_row([p], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .optional()
                .map_err(|e| e.to_string())?;
            if let Some((id, json)) = row {
                let h = Handoff::read_json(json.as_bytes())?;
                return Ok(Some((id, h)));
            }
        }
        Ok(None)
    }

    /// The newest record whose entry or candidates carry any of these ids (`(key, value)`).
    pub fn by_ids(&self, ids: &[(String, String)]) -> Result<Option<(String, Handoff)>, String> {
        if ids.is_empty() {
            return Ok(None);
        }
        let mut after = 0;
        let mut best: Option<(i64, String, Handoff)> = None;
        loop {
            let page = self.page(after, 64)?;
            let Some(last) = page.last() else { break };
            after = last.seq;
            for u in page {
                let Ok(h) = Handoff::read_json(u.handoff.as_bytes()) else {
                    continue;
                };
                let hit = ids.iter().any(|(k, v)| {
                    h.entry.get(k).is_some_and(|x| x == v)
                        || h.candidates
                            .iter()
                            .any(|(_, m)| m.get(k).is_some_and(|x| x == v))
                });
                if hit {
                    best = Some((u.seq, u.id, h));
                }
            }
        }
        Ok(best.map(|(_, id, h)| (id, h)))
    }

    /// When the record with this id was synced: `None` if there is no such record.
    pub fn synced_at(&self, id: &str) -> Result<Option<Option<i64>>, String> {
        self.c
            .query_row("SELECT synced_at FROM records WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handoff(title: &str) -> Handoff {
        Handoff {
            file: format!("/media/{title}.mkv").into(),
            entry: [("title".to_string(), title.to_string())]
                .into_iter()
                .collect(),
            candidates: vec![],
            conflicts: vec![],
            from: "kate".into(),
            caller: None,
        }
    }

    fn open(d: &tempfile::TempDir) -> Records {
        let path = d.path().join("kate.db");
        Records::open(&Options {
            name: "kate",
            path: path.to_str().unwrap(),
            busy_timeout_secs: 1,
            id: IdStrategy::Content,
            migrations: &[SCHEMA_V1, crate::base::events::SCHEMA_V1],
        })
        .unwrap()
    }

    #[test]
    fn any_windup_keeps_its_records_under_its_own_name_until_acknowledged() {
        let d = tempfile::tempdir().unwrap();
        let r = open(&d);
        let (id, new) = r.write(&handoff("Heat")).unwrap();
        assert!(new && id.starts_with("kate-"), "{id}");
        assert!(
            !r.write(&handoff("Heat")).unwrap().1,
            "same record, same id"
        );
        let u = r.unsynced(0, 10).unwrap();
        assert_eq!((u.len(), u[0].id.as_str()), (1, id.as_str()));
        r.mark_synced(u[0].seq, 7).unwrap();
        assert_eq!(r.synced_at(&id).unwrap(), Some(Some(7)));
        assert_eq!(r.counts().unwrap(), (1, 0), "kept after sync");
    }
}
