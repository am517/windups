//! Read-only audit of any windup's database, for Walter (the auditor): finds unsynced records,
//! stale or missing backups, damage and old schemas, and says which windup command fixes each.
//! It never writes. Windup-specific checks (e.g. unfinished tasks) plug in as `Check`s.
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub windup: String,
    pub what: String,
    /// The command that sends the windup back to fix it, e.g. `jack sync`.
    pub fix: String,
}

pub trait Check {
    fn check(&self, windup: &str, c: &Connection) -> Result<Vec<Finding>, String>;
}

fn has_table(c: &Connection, t: &str) -> bool {
    c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [t],
        |r| r.get::<_, i64>(0),
    )
    .is_ok_and(|n| n > 0)
}

fn finding(w: &str, what: String, fix: &str) -> Finding {
    Finding {
        windup: w.into(),
        what,
        fix: format!("{w} {fix}"),
    }
}

/// The checks every windup gets: integrity, core tables present, unsynced records, backup age.
pub struct Standard {
    pub stale_after_days: u64,
    pub now: i64,
}

impl Check for Standard {
    fn check(&self, w: &str, c: &Connection) -> Result<Vec<Finding>, String> {
        let e = |e: rusqlite::Error| e.to_string();
        let mut out = Vec::new();
        let ok: String = c
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(e)?;
        if ok != "ok" {
            out.push(finding(w, format!("database damaged: {ok}"), "health"));
        }
        if !has_table(c, "events") || !has_table(c, "snapshots") {
            out.push(finding(
                w,
                "schema predates the shared core".into(),
                "status",
            ));
            return Ok(out);
        }
        if has_table(c, "records") {
            let n: i64 = c
                .query_row(
                    "SELECT count(*) FROM records WHERE synced_at IS NULL",
                    [],
                    |r| r.get(0),
                )
                .map_err(e)?;
            if n > 0 {
                out.push(finding(w, format!("{n} record(s) not yet synced"), "sync"));
            }
        }
        let last: Option<i64> = c
            .query_row(
                "SELECT max(at) FROM snapshots WHERE pruned_at IS NULL",
                [],
                |r| r.get(0),
            )
            .map_err(e)?;
        match last {
            None => out.push(finding(w, "no backup yet".into(), "backup")),
            Some(at) if self.now - at > (self.stale_after_days * 86_400) as i64 => {
                out.push(finding(
                    w,
                    format!("last backup {} is stale", super::events::fmt_utc(at)),
                    "backup",
                ))
            }
            Some(_) => {}
        }
        Ok(out)
    }
}

/// Opens `db` read-only and runs the checks.
pub fn audit(windup: &str, db: &Path, checks: &[&dyn Check]) -> Result<Vec<Finding>, String> {
    let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("{}: {e}", db.display()))?;
    let mut out = Vec::new();
    for ch in checks {
        out.extend(ch.check(windup, &c)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{events, schema};

    #[test]
    fn finds_gaps_read_only_and_names_the_fix() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("toy.db");
        let mut c = Connection::open(&p).unwrap();
        let mut m = vec!["CREATE TABLE records(id TEXT, synced_at INTEGER);"];
        m.extend_from_slice(crate::base::CORE_V1);
        schema::migrate(&mut c, "toy.db", &m).unwrap();
        c.execute("INSERT INTO records VALUES ('a', NULL), ('b', 5)", [])
            .unwrap();
        let std = Standard {
            stale_after_days: 7,
            now: events::now(),
        };
        let f = audit("toy", &p, &[&std]).unwrap();
        let fixes: Vec<_> = f.iter().map(|x| x.fix.as_str()).collect();
        assert_eq!(fixes, ["toy sync", "toy backup"]);
        assert_eq!(f[0].what, "1 record(s) not yet synced");
        c.execute(
            "INSERT INTO snapshots(at, path, bytes, events_through) VALUES (?1, 'x', 1, 0)",
            [events::now()],
        )
        .unwrap();
        c.execute("UPDATE records SET synced_at = 1", []).unwrap();
        assert!(audit("toy", &p, &[&std]).unwrap().is_empty());
        drop(c);
        // read-only: the audit can't change what it reads
        let ro = Connection::open_with_flags(&p, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert!(ro.execute("DELETE FROM records", []).is_err());
    }

    #[test]
    fn an_old_schema_is_a_finding_not_a_crash() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("old.db");
        Connection::open(&p)
            .unwrap()
            .execute_batch("CREATE TABLE records(id TEXT);")
            .unwrap();
        let std = Standard {
            stale_after_days: 7,
            now: 0,
        };
        let f = audit("old", &p, &[&std]).unwrap();
        assert_eq!(f[0].what, "schema predates the shared core");
    }
}
