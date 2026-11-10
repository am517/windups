//! Syncing Jack's archive to his sync target (`[archive] sync_target`, default "daniel").
//! Records are sent in batches; each is marked synced only after its own acknowledgement.
use crate::archive::{Archive, now};
use windups::base::sync::{SendError, SyncTarget};

/// The configured target and transport, if any. `[store] enabled = false` also means none.
pub fn connect<'a>(
    a: &crate::archive::ArchiveCfg,
    store: &'a daniel::StoreCfg,
) -> Result<Option<Box<dyn SyncTarget + 'a>>, String> {
    match (a.sync_target.as_str(), a.sync_transport.as_str()) {
        ("daniel", _) if !store.enabled => Ok(None),
        ("daniel", "library" | "") => Ok(Some(Box::new(crate::to_daniel::ToDaniel::new(store)))),
        ("daniel", "socket") => Ok(Some(Box::new(
            windups::base::sync_socket::SocketTarget::new(
                windups::paths::expand(&a.daniel_socket),
                std::time::Duration::from_secs(a.socket_timeout_secs.max(1)),
                a.max_frame_bytes.max(1024),
            ),
        ))),
        ("daniel", t) => Err(format!("sync_transport {t:?} (known: library, socket)")),
        (other, _) => Ok(windups::base::sync::connect(other)?.map(|b| b as Box<dyn SyncTarget>)),
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub sent: usize,
    pub rejected: usize,
    pub unreachable: Option<String>,
}

/// Sends every unsynced record, `batch` at a time, marking each synced only after its ack.
pub fn sync(a: &Archive, t: &mut dyn SyncTarget, batch: usize) -> Result<SyncReport, String> {
    let mut r = SyncReport::default();
    let mut after = 0;
    loop {
        let rows = a.unsynced(after, batch.max(1))?;
        let Some(last) = rows.last() else {
            return Ok(r);
        };
        after = last.seq;
        let recs: Vec<(&str, &str)> = rows
            .iter()
            .map(|u| (u.id.as_str(), u.handoff.as_str()))
            .collect();
        let results = t.send_batch(&recs);
        for (u, res) in rows
            .iter()
            .zip(results.into_iter().chain(std::iter::repeat_with(|| {
                Err(SendError::Unreachable("no answer for this record".into()))
            })))
        {
            match res {
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

/// One line for the run report (and the `sync` command).
pub fn report(a: &Archive, target: Option<&mut (dyn SyncTarget + '_)>, batch: usize) -> String {
    let synced = match target {
        None => Ok(None),
        Some(t) => sync(a, t, batch).map(Some),
    };
    let (all, left) = a.counts().unwrap_or((0, 0));
    let tail = match synced {
        Err(e) => format!("sync failed ({e})"),
        Ok(None) => "no sync target configured".into(),
        Ok(Some(r)) => match r.unreachable {
            Some(e) => format!("synced {}; target unreachable ({e})", r.sent),
            None if r.rejected > 0 => format!("synced {}; target refused {}", r.sent, r.rejected),
            None => format!("synced {}", r.sent),
        },
    };
    format!("archive: {all} record(s) in jack.db, {left} not yet synced; {tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::{handoff, open_temp};
    use std::collections::BTreeMap;

    /// A target in a jar: keeps what it's sent by id, and can be made to sulk.
    #[derive(Default)]
    struct Jar {
        got: BTreeMap<String, String>,
        deliveries: usize,
        batches: usize,
        down: bool,
        refuse: Option<String>,
        fail_after: Option<usize>,
    }

    impl SyncTarget for Jar {
        fn send(&mut self, id: &str, rec: &str) -> Result<(), SendError> {
            if self.down || self.fail_after == Some(self.deliveries) {
                return Err(SendError::Unreachable("connection refused".into()));
            }
            if self.refuse.as_deref() == Some(id) {
                return Err(SendError::Rejected("no thanks".into()));
            }
            self.deliveries += 1;
            self.got.insert(id.into(), rec.into());
            Ok(())
        }
    }

    /// Acks a whole batch at once, with a result per record.
    #[derive(Default)]
    struct BatchJar(Jar);

    impl SyncTarget for BatchJar {
        fn send(&mut self, id: &str, rec: &str) -> Result<(), SendError> {
            self.0.send(id, rec)
        }
        fn send_batch(&mut self, recs: &[(&str, &str)]) -> Vec<Result<(), SendError>> {
            self.0.batches += 1;
            recs.iter().map(|(i, r)| self.0.send(i, r)).collect()
        }
    }

    #[test]
    fn ack_marks_synced_and_records_stay() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        let (id, _) = a.write(&handoff("A")).unwrap();
        a.write(&handoff("B")).unwrap();
        let mut t = Jar::default();
        let r = sync(&a, &mut t, 1).unwrap();
        assert_eq!((r.sent, r.unreachable), (2, None));
        assert!(a.synced_at(&id).unwrap().is_some());
        assert_eq!(
            a.counts().unwrap(),
            (2, 0),
            "Jack keeps his records after syncing"
        );
        let h = windups::handoff::Handoff::read_json(t.got[&id].as_bytes()).unwrap();
        assert_eq!(h, handoff("A"), "the target gets handoff JSON v1");
    }

    #[test]
    fn failed_send_leaves_unsynced_and_retry_does_not_duplicate() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        for t in ["A", "B", "C"] {
            a.write(&handoff(t)).unwrap();
        }
        let mut t = Jar {
            fail_after: Some(1),
            ..Default::default()
        };
        let r = sync(&a, &mut t, 32).unwrap();
        assert_eq!(r.sent, 1);
        assert!(r.unreachable.is_some());
        assert_eq!(a.counts().unwrap(), (3, 2), "unacked records stay unsynced");
        t.fail_after = None;
        assert_eq!(sync(&a, &mut t, 32).unwrap().sent, 2);
        assert_eq!(sync(&a, &mut t, 32).unwrap().sent, 0, "nothing sent twice");
        assert_eq!((t.deliveries, t.got.len()), (3, 3));
        a.write(&handoff("A")).unwrap();
        assert_eq!(a.counts().unwrap(), (3, 0));
    }

    #[test]
    fn batch_acks_carry_a_result_per_record() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        let (bad, _) = a.write(&handoff("A")).unwrap();
        for t in ["B", "C", "D"] {
            a.write(&handoff(t)).unwrap();
        }
        let mut t = BatchJar(Jar {
            refuse: Some(bad.clone()),
            ..Default::default()
        });
        let r = sync(&a, &mut t, 2).unwrap();
        assert_eq!((r.sent, r.rejected, t.0.batches), (3, 1, 2));
        assert_eq!(a.synced_at(&bad), Some(None), "the refused one waits");
    }

    #[test]
    fn no_target_keeps_everything() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        a.write(&handoff("A")).unwrap();
        let off = daniel::StoreCfg::default();
        let cfg = |t: &str, tr: &str| crate::archive::ArchiveCfg {
            sync_target: t.into(),
            sync_transport: tr.into(),
            ..Default::default()
        };
        assert!(
            connect(&cfg("daniel", "library"), &off).unwrap().is_none(),
            "[store] enabled = false"
        );
        assert!(connect(&cfg("none", ""), &off).unwrap().is_none());
        assert!(connect(&cfg("carrier-pigeon", ""), &off).is_err());
        let on = daniel::StoreCfg {
            enabled: true,
            ..Default::default()
        };
        assert!(connect(&cfg("daniel", "socket"), &on).unwrap().is_some());
        assert!(connect(&cfg("daniel", "smoke signals"), &on).is_err());
        let line = report(&a, None, 32);
        assert!(
            line.contains("1 record(s) in jack.db, 1 not yet synced"),
            "{line}"
        );
        let mut down = Jar {
            down: true,
            ..Default::default()
        };
        let line = report(&a, Some(&mut down), 32);
        assert!(line.contains("target unreachable"), "{line}");
        assert_eq!(a.counts().unwrap(), (1, 1));
    }

    #[test]
    fn syncs_into_daniels_catalog_by_library_call() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        let media = d.path().join("Heat.1995.mkv");
        std::fs::write(&media, b"x").unwrap();
        let mut h = handoff("Heat");
        h.file = media;
        a.write(&h).unwrap();
        let store = daniel::StoreCfg {
            enabled: true,
            database: d.path().join("media.db").to_string_lossy().into_owned(),
            busy_timeout_secs: 1,
            ..Default::default()
        };
        let mut t = crate::to_daniel::ToDaniel::quiet(&store);
        assert_eq!(sync(&a, &mut t, 32).unwrap().sent, 1);
        let c = windups::db::open(&d.path().join("media.db")).unwrap();
        let n: i64 = c
            .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let gone = daniel::StoreCfg {
            database: d.path().to_string_lossy().into_owned(),
            ..store.clone()
        };
        a.write(&handoff("Other")).unwrap();
        let r = sync(&a, &mut crate::to_daniel::ToDaniel::quiet(&gone), 32).unwrap();
        assert!(r.unreachable.unwrap().starts_with("no database"));
        assert_eq!(a.counts().unwrap(), (2, 1));
    }

    #[test]
    fn syncs_into_daniels_catalog_over_the_socket_and_resends_safely() {
        let d = tempfile::tempdir().unwrap();
        let a = open_temp(&d);
        let media = d.path().join("Heat.1995.mkv");
        std::fs::write(&media, b"x").unwrap();
        let mut h = handoff("Heat");
        h.file = media;
        let (id, _) = a.write(&h).unwrap();
        let sock = d.path().join("daniel.sock");
        let l = daniel::serve::bind(&sock).unwrap();
        let db = d.path().join("media.db").to_string_lossy().into_owned();
        let server = std::thread::spawn(move || {
            let cfg = daniel::StoreCfg {
                enabled: true,
                database: db,
                busy_timeout_secs: 1,
                ..Default::default()
            };
            let mut s = daniel::Store::new(&cfg);
            s.config.walter.enabled = false;
            daniel::serve::serve(&l, &s, 1 << 20, Some(2)).unwrap();
        });
        let acfg = crate::archive::ArchiveCfg {
            sync_transport: "socket".into(),
            daniel_socket: sock.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let on = daniel::StoreCfg {
            enabled: true,
            ..Default::default()
        };
        let mut t = connect(&acfg, &on).unwrap().unwrap();
        assert_eq!(sync(&a, t.as_mut(), 32).unwrap().sent, 1);
        drop(t);
        // a resend of the same record (as after a lost ack) is acknowledged and adds nothing
        let rec = a.page(0, 1).unwrap().remove(0).handoff;
        let mut again = connect(&acfg, &on).unwrap().unwrap();
        assert_eq!(again.send(&id, &rec), Ok(()));
        drop(again);
        server.join().unwrap();
        let c = windups::db::open(&d.path().join("media.db")).unwrap();
        let n: i64 = c
            .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(a.counts().unwrap(), (1, 0));
    }
}
