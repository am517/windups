//! Sends a windup's unsynced records to its sync target, a batch at a time, and marks each one
//! synced only after its own acknowledgement. The windup only shapes the body it sends.
use super::records::{Records, now};
use super::sync::{SendError, SyncTarget};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub sent: usize,
    pub rejected: usize,
    pub unreachable: Option<String>,
}

/// Sends every unsynced record, `batch` at a time. `body` turns a stored record into what the
/// target gets (for example, facts only). Unreachable stops the run; a refusal skips one record.
pub fn sync(
    records: &Records,
    target: &mut dyn SyncTarget,
    batch: usize,
    body: &dyn Fn(&str) -> String,
) -> Result<SyncReport, String> {
    let mut r = SyncReport::default();
    let mut after = 0;
    loop {
        let rows = records.unsynced(after, batch.max(1))?;
        let Some(last) = rows.last() else {
            return Ok(r);
        };
        after = last.seq;
        let payloads: Vec<(String, String)> = rows
            .iter()
            .map(|u| (u.id.clone(), body(&u.handoff)))
            .collect();
        let recs: Vec<(&str, &str)> = payloads
            .iter()
            .map(|(id, h)| (id.as_str(), h.as_str()))
            .collect();
        let results = target.send_batch(&recs);
        for (u, res) in rows
            .iter()
            .zip(results.into_iter().chain(std::iter::repeat_with(|| {
                Err(SendError::Unreachable("no answer for this record".into()))
            })))
        {
            match res {
                Ok(()) => {
                    records.mark_synced(u.seq, now())?;
                    r.sent += 1;
                }
                Err(SendError::Rejected(_)) => r.rejected += 1,
                Err(SendError::Unreachable(e)) => {
                    r.unreachable = Some(e);
                    return Ok(r);
                }
            }
        }
    }
}

/// One line for a run report and the `sync` command, e.g.
/// `archive: 3 record(s) in jack.db, 0 not yet synced; synced 3`.
pub fn report(
    records: &Records,
    db_name: &str,
    target: Option<&mut (dyn SyncTarget + '_)>,
    batch: usize,
    body: &dyn Fn(&str) -> String,
) -> String {
    let synced = match target {
        None => Ok(None),
        Some(t) => sync(records, t, batch, body).map(Some),
    };
    let (all, left) = records.counts().unwrap_or((0, 0));
    let tail = match synced {
        Err(e) => format!("sync failed ({e})"),
        Ok(None) => "no sync target configured".into(),
        Ok(Some(r)) => match r.unreachable {
            Some(e) => format!("synced {}; target unreachable ({e})", r.sent),
            None if r.rejected > 0 => format!("synced {}; target refused {}", r.sent, r.rejected),
            None => format!("synced {}", r.sent),
        },
    };
    format!("archive: {all} record(s) in {db_name}, {left} not yet synced; {tail}")
}
