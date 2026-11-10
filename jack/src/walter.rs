//! Syncing Jack's archive to Walter. Walter has no receive side yet, so the link is a trait;
//! `connect` returns `None` until he does, and records simply wait in jack.db (nothing is lost).
use crate::archive::{Archive, now};

/// Why a send did not get Walter's acknowledgement.
// built by Walter's link once he has a receive side; until then only the tests build them
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// Walter can't be reached: stop for now, try again next run.
    Unreachable(String),
    /// Walter refused this one record: leave it unsynced, carry on with the rest.
    Rejected(String),
}

/// The way to Walter. `send` returns `Ok` only once Walter has acknowledged storing the record.
/// Walter must treat a repeated `id` as already received (retries are safe).
pub trait Walter {
    fn send(&mut self, id: &str, handoff_v1: &str) -> Result<(), SendError>;
}

/// The link to Walter, if there is one. None for now: Walter has no receive side yet.
pub fn connect() -> Option<Box<dyn Walter>> {
    None
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub sent: usize,
    pub rejected: usize,
    pub unreachable: Option<String>,
}

/// Sends every unsynced record, `batch` at a time, marking each synced only after Walter's ack.
pub fn sync(a: &Archive, w: &mut dyn Walter, batch: usize) -> Result<SyncReport, String> {
    let mut r = SyncReport::default();
    let mut after = 0;
    loop {
        let rows = a.unsynced(after, batch.max(1))?;
        let Some(last) = rows.last() else {
            return Ok(r);
        };
        after = last.seq;
        for u in rows {
            match w.send(&u.id, &u.handoff) {
                Ok(()) => {
                    a.mark_synced(u.seq, now())?;
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

/// One line for the run report.
pub fn report(a: &Archive, link: Option<&mut (dyn Walter + '_)>, batch: usize) -> String {
    let synced = match link {
        None => Ok(None),
        Some(w) => sync(a, w, batch).map(Some),
    };
    let (all, left) = a.counts().unwrap_or((0, 0));
    let tail = match synced {
        Err(e) => format!("sync failed ({e})"),
        Ok(None) => "Walter has no receive side yet".into(),
        Ok(Some(r)) => match r.unreachable {
            Some(e) => format!("sent {}; Walter unreachable ({e})", r.sent),
            None if r.rejected > 0 => format!("sent {}; Walter refused {}", r.sent, r.rejected),
            None => format!("sent {}", r.sent),
        },
    };
    format!("archive: {all} record(s) in jack.db, {left} not yet with Walter; {tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::{handoff, open_temp};
    use std::collections::BTreeMap;

    /// Walter in a jar: keeps what he's sent by id, and can be made to sulk.
    #[derive(Default)]
    struct JarWalter {
        got: BTreeMap<String, String>,
        deliveries: usize,
        down: bool,
        refuse: Option<String>,
        fail_after: Option<usize>,
    }

    impl Walter for JarWalter {
        fn send(&mut self, id: &str, handoff_v1: &str) -> Result<(), SendError> {
            if self.down || self.fail_after == Some(self.deliveries) {
                return Err(SendError::Unreachable("connection refused".into()));
            }
            if self.refuse.as_deref() == Some(id) {
                return Err(SendError::Rejected("no thanks".into()));
            }
            self.deliveries += 1;
            self.got.insert(id.into(), handoff_v1.into());
            Ok(())
        }
    }

    #[test]
    fn ack_marks_synced_and_records_stay() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        let (id, _) = a.write(&handoff("A")).unwrap();
        a.write(&handoff("B")).unwrap();
        let mut w = JarWalter::default();
        let r = sync(&a, &mut w, 1).unwrap();
        assert_eq!((r.sent, r.unreachable), (2, None));
        assert!(a.synced_at(&id).unwrap().is_some());
        assert_eq!(
            a.counts().unwrap(),
            (2, 0),
            "Jack keeps his records after syncing"
        );
        let h = windups::handoff::Handoff::read_json(w.got[&id].as_bytes()).unwrap();
        assert_eq!(h, handoff("A"), "Walter gets handoff JSON v1");
    }

    #[test]
    fn failed_send_leaves_unsynced_and_retry_does_not_duplicate() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        for t in ["A", "B", "C"] {
            a.write(&handoff(t)).unwrap();
        }
        let mut w = JarWalter {
            fail_after: Some(1),
            ..Default::default()
        };
        let r = sync(&a, &mut w, 32).unwrap();
        assert_eq!(r.sent, 1);
        assert!(r.unreachable.is_some());
        assert_eq!(a.counts().unwrap(), (3, 2), "unacked records stay unsynced");
        w.fail_after = None;
        assert_eq!(sync(&a, &mut w, 32).unwrap().sent, 2);
        assert_eq!(sync(&a, &mut w, 32).unwrap().sent, 0, "nothing sent twice");
        assert_eq!((w.deliveries, w.got.len()), (3, 3));
        // the same record written again is not a new one
        a.write(&handoff("A")).unwrap();
        assert_eq!(a.counts().unwrap(), (3, 0));
    }

    #[test]
    fn refused_record_stays_unsynced_others_go() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        let (bad, _) = a.write(&handoff("A")).unwrap();
        a.write(&handoff("B")).unwrap();
        let mut w = JarWalter {
            refuse: Some(bad.clone()),
            ..Default::default()
        };
        let r = sync(&a, &mut w, 32).unwrap();
        assert_eq!((r.sent, r.rejected), (1, 1));
        assert_eq!(a.synced_at(&bad), Some(None));
    }

    #[test]
    fn no_walter_yet_keeps_everything() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        a.write(&handoff("A")).unwrap();
        assert!(connect().is_none());
        let line = report(&a, None, 32);
        assert!(
            line.contains("1 record(s) in jack.db, 1 not yet with Walter"),
            "{line}"
        );
        let mut down = JarWalter {
            down: true,
            ..Default::default()
        };
        let line = report(&a, Some(&mut down), 32);
        assert!(line.contains("Walter unreachable"), "{line}");
        assert_eq!(a.counts().unwrap(), (1, 1));
    }
}
