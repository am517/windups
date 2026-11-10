//! The append-only event log in a windup's own database. Recent events are kept whole; older
//! ones are packed into compressed batches; very old batches become per-kind summaries, but only
//! once a checked snapshot holds their originals (and that snapshot is then never pruned).
use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};

/// Add to a windup's migration list (with `snapshot::SCHEMA_V1`).
pub const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS events (
    seq    INTEGER PRIMARY KEY AUTOINCREMENT,
    at     INTEGER NOT NULL,
    kind   TEXT NOT NULL,
    detail TEXT NOT NULL DEFAULT ''
);
CREATE TRIGGER IF NOT EXISTS events_append_only BEFORE UPDATE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;
CREATE TABLE IF NOT EXISTS event_batches (
    seq       INTEGER PRIMARY KEY,
    first_seq INTEGER NOT NULL,
    last_seq  INTEGER NOT NULL,
    first_at  INTEGER NOT NULL,
    last_at   INTEGER NOT NULL,
    count     INTEGER NOT NULL,
    data      BLOB NOT NULL
);
CREATE TRIGGER IF NOT EXISTS event_batches_append_only BEFORE UPDATE ON event_batches
BEGIN SELECT RAISE(ABORT, 'event batches are append-only'); END;
CREATE TABLE IF NOT EXISTS event_summaries (
    seq       INTEGER PRIMARY KEY,
    first_seq INTEGER NOT NULL,
    last_seq  INTEGER NOT NULL,
    first_at  INTEGER NOT NULL,
    last_at   INTEGER NOT NULL,
    count     INTEGER NOT NULL,
    kinds     TEXT NOT NULL,
    snapshot  TEXT NOT NULL
);
";

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct EventsCfg {
    /// Newest events kept whole.
    pub keep_full: u64,
    /// Events per compressed batch.
    pub batch_size: u64,
    /// Compressed batches kept before the oldest is summarized.
    pub keep_batches: u64,
    /// Deflate level, 0-9.
    pub level: u32,
}

impl Default for EventsCfg {
    fn default() -> Self {
        // generous: 50k whole + 200 batches of 5k = over a million events before any summary
        Self {
            keep_full: 50_000,
            batch_size: 5_000,
            keep_batches: 200,
            level: 6,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub seq: i64,
    pub at: i64,
    pub kind: String,
    pub detail: String,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Unix seconds as `YYYY-MM-DD HH:MM:SS UTC` (no date crate needed).
pub fn fmt_utc(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

pub fn append(c: &Connection, kind: &str, detail: &str) -> Result<i64, String> {
    c.execute(
        "INSERT INTO events(at, kind, detail) VALUES (?1, ?2, ?3)",
        params![now(), kind, detail],
    )
    .map_err(|e| e.to_string())?;
    Ok(c.last_insert_rowid())
}

/// The newest `n` whole events, oldest first.
pub fn recent(c: &Connection, n: usize) -> Result<Vec<Event>, String> {
    let mut st = c
        .prepare_cached(
            "SELECT seq, at, kind, detail FROM
             (SELECT * FROM events ORDER BY seq DESC LIMIT ?1) ORDER BY seq",
        )
        .map_err(|e| e.to_string())?;
    let rows = st
        .query_map([n as i64], |r| {
            Ok(Event {
                seq: r.get(0)?,
                at: r.get(1)?,
                kind: r.get(2)?,
                detail: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

/// Highest event sequence number ever written (whole, batched or summarized).
pub fn last_seq(c: &Connection) -> Result<i64, String> {
    c.query_row(
        "SELECT max(coalesce((SELECT max(seq) FROM events), 0),
                    coalesce((SELECT max(last_seq) FROM event_batches), 0),
                    coalesce((SELECT max(last_seq) FROM event_summaries), 0))",
        [],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}

/// Decodes one compressed batch back to its events.
pub fn read_batch(data: &[u8]) -> Result<Vec<Event>, String> {
    let mut text = String::new();
    DeflateDecoder::new(data)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    text.lines()
        .map(|l| serde_json::from_str(l).map_err(|e| e.to_string()))
        .collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Compacted {
    pub batched: u64,
    pub summarized: u64,
    /// Batches over the limit that wait for a snapshot holding their originals.
    pub waiting_for_backup: u64,
}

fn count(tx: &Transaction, table: &str) -> rusqlite::Result<u64> {
    tx.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
}

/// Applies the limits. Cheap when nothing is over them. One transaction.
pub fn compact(c: &mut Connection, cfg: &EventsCfg) -> Result<Compacted, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let tx = c.transaction().map_err(e)?;
    let mut r = Compacted::default();
    let size = cfg.batch_size.max(1);
    while count(&tx, "events").map_err(e)? >= cfg.keep_full + size {
        pack(&tx, size, cfg.level)?;
        r.batched += 1;
    }
    let mut over = count(&tx, "event_batches")
        .map_err(e)?
        .saturating_sub(cfg.keep_batches);
    while over > 0 {
        if !summarize_oldest(&tx)? {
            r.waiting_for_backup = over;
            break;
        }
        r.summarized += 1;
        over -= 1;
    }
    tx.commit().map_err(e)?;
    Ok(r)
}

fn pack(tx: &Transaction, size: u64, level: u32) -> Result<(), String> {
    let e = |e: rusqlite::Error| e.to_string();
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::new(level.min(9)));
    let (mut first, mut last, mut first_at, mut last_at, mut n) = (0, 0, 0, 0, 0u64);
    {
        let mut st = tx
            .prepare("SELECT seq, at, kind, detail FROM events ORDER BY seq LIMIT ?1")
            .map_err(e)?;
        let mut rows = st.query([size as i64]).map_err(e)?;
        while let Some(row) = rows.next().map_err(e)? {
            let ev = Event {
                seq: row.get(0).map_err(e)?,
                at: row.get(1).map_err(e)?,
                kind: row.get(2).map_err(e)?,
                detail: row.get(3).map_err(e)?,
            };
            if n == 0 {
                (first, first_at) = (ev.seq, ev.at);
            }
            (last, last_at, n) = (ev.seq, ev.at, n + 1);
            serde_json::to_writer(&mut enc, &ev).map_err(|e| e.to_string())?;
            enc.write_all(b"\n").map_err(|e| e.to_string())?;
        }
    }
    let data = enc.finish().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO event_batches(first_seq, last_seq, first_at, last_at, count, data)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![first, last, first_at, last_at, n as i64, data],
    )
    .map_err(e)?;
    tx.execute("DELETE FROM events WHERE seq <= ?1", [last])
        .map_err(e)?;
    Ok(())
}

/// Summarizes the oldest batch if a snapshot already holds it; false if none does yet.
fn summarize_oldest(tx: &Transaction) -> Result<bool, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let Some((seq, first, last, first_at, last_at, n, data)) = tx
        .query_row(
            "SELECT seq, first_seq, last_seq, first_at, last_at, count, data
             FROM event_batches ORDER BY seq LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, Vec<u8>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(e)?
    else {
        return Ok(false);
    };
    let Some(snap) = super::snapshot::holding(tx, last).map_err(e)? else {
        return Ok(false);
    };
    let mut kinds = BTreeMap::<String, u64>::new();
    for ev in read_batch(&data)? {
        *kinds.entry(ev.kind).or_default() += 1;
    }
    let kinds = serde_json::to_string(&kinds).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO event_summaries(first_seq, last_seq, first_at, last_at, count, kinds, snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![first, last, first_at, last_at, n, kinds, snap],
    )
    .map_err(e)?;
    tx.execute("DELETE FROM event_batches WHERE seq = ?1", [seq])
        .map_err(e)?;
    Ok(true)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::base::schema::migrate(&mut c, "t.db", crate::base::CORE_V1).unwrap();
        c
    }

    fn n(c: &Connection, q: &str) -> i64 {
        c.query_row(q, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn utc_formatting() {
        assert_eq!(fmt_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(fmt_utc(1_794_200_400), "2026-11-09 05:00:00 UTC");
        assert_eq!(fmt_utc(951_782_400), "2000-02-29 00:00:00 UTC");
    }

    #[test]
    fn append_only_and_recent_in_order() {
        let c = db();
        for i in 0..5 {
            append(&c, "record", &format!("r{i}")).unwrap();
        }
        let r = recent(&c, 2).unwrap();
        assert_eq!((r[0].detail.as_str(), r[1].detail.as_str()), ("r3", "r4"));
        let e = c.execute("UPDATE events SET kind = 'x'", []).unwrap_err();
        assert!(e.to_string().contains("append-only"), "{e}");
    }

    #[test]
    fn old_events_are_batched_losslessly() {
        let mut c = db();
        for i in 0..25 {
            append(&c, if i % 2 == 0 { "a" } else { "b" }, &format!("d{i}")).unwrap();
        }
        let cfg = EventsCfg {
            keep_full: 10,
            batch_size: 5,
            keep_batches: 100,
            level: 6,
        };
        let r = compact(&mut c, &cfg).unwrap();
        assert_eq!(r.batched, 3);
        assert_eq!(n(&c, "SELECT count(*) FROM events"), 10);
        let data: Vec<u8> = c
            .query_row(
                "SELECT data FROM event_batches ORDER BY seq LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let first = read_batch(&data).unwrap();
        assert_eq!(first.len(), 5);
        assert_eq!((first[0].seq, first[0].detail.as_str()), (1, "d0"));
        assert_eq!(
            compact(&mut c, &cfg).unwrap(),
            Compacted::default(),
            "idempotent"
        );
        assert_eq!(last_seq(&c).unwrap(), 25);
        // sequence numbers never repeat, even after deletes
        assert_eq!(append(&c, "a", "").unwrap(), 26);
    }

    #[test]
    fn summaries_wait_for_a_snapshot_holding_the_originals() {
        let mut c = db();
        for _ in 0..20 {
            append(&c, "record", "").unwrap();
        }
        let cfg = EventsCfg {
            keep_full: 0,
            batch_size: 5,
            keep_batches: 1,
            level: 1,
        };
        let r = compact(&mut c, &cfg).unwrap();
        assert_eq!((r.batched, r.summarized, r.waiting_for_backup), (4, 0, 3));
        c.execute(
            "INSERT INTO snapshots(at, path, bytes, events_through) VALUES (1, '/b/s1.db', 1, 12)",
            [],
        )
        .unwrap();
        let r = compact(&mut c, &cfg).unwrap();
        assert_eq!(
            (r.summarized, r.waiting_for_backup),
            (2, 1),
            "seqs 1-10 are in the snapshot"
        );
        let (kinds, snap): (String, String) = c
            .query_row(
                "SELECT kinds, snapshot FROM event_summaries LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (kinds.as_str(), snap.as_str()),
            ("{\"record\":5}", "/b/s1.db")
        );
        assert_eq!(last_seq(&c).unwrap(), 20);
    }
}
