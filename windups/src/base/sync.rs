//! Where a windup's records are copied to. The target is chosen in config (`sync_target`); no
//! target is hard-wired. A target acknowledges each record it has stored; sending the same id
//! twice must be a no-op on its side, so retries are safe.

/// Why a record did not get the target's acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// The target can't be reached: stop for now, try again later.
    Unreachable(String),
    /// The target refused this one record: leave it unsynced, carry on with the rest.
    Rejected(String),
}

pub trait SyncTarget {
    /// Delivers one record (handoff JSON v1) under its stable id; `Ok` = acknowledged.
    fn send(&mut self, id: &str, record: &str) -> Result<(), SendError>;

    /// Delivers a batch; one result per record, in order. A target that acknowledges a whole
    /// batch at once overrides this. The default sends one by one and stops at the first
    /// unreachable, marking the rest unreachable too.
    fn send_batch(&mut self, recs: &[(&str, &str)]) -> Vec<Result<(), SendError>> {
        let mut out = Vec::with_capacity(recs.len());
        for (id, rec) in recs {
            let r = self.send(id, rec);
            let stop = matches!(r, Err(SendError::Unreachable(_)));
            out.push(r);
            if stop {
                break;
            }
        }
        while out.len() < recs.len() {
            let why = match out.last() {
                Some(Err(SendError::Unreachable(e))) => e.clone(),
                _ => "unreachable".into(),
            };
            out.push(Err(SendError::Unreachable(why)));
        }
        out
    }
}

/// The sync targets a windup can be configured with. Only `none` exists today; a real target
/// (and which windup it is) gets added here once its receive side exists.
pub fn connect(target: &str) -> Result<Option<Box<dyn SyncTarget>>, String> {
    match target {
        "" | "none" => Ok(None),
        other => Err(format!(
            "sync_target {other:?} has no receive side yet (known: none)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flaky(usize);
    impl SyncTarget for Flaky {
        fn send(&mut self, id: &str, _: &str) -> Result<(), SendError> {
            if id == "bad" {
                return Err(SendError::Rejected("no".into()));
            }
            if self.0 == 0 {
                return Err(SendError::Unreachable("down".into()));
            }
            self.0 -= 1;
            Ok(())
        }
    }

    #[test]
    fn default_batch_gives_one_result_per_record() {
        let recs = [("a", ""), ("bad", ""), ("b", ""), ("c", ""), ("d", "")];
        let r = Flaky(2).send_batch(&recs);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], Ok(()));
        assert_eq!(r[1], Err(SendError::Rejected("no".into())));
        assert_eq!(r[2], Ok(()));
        assert_eq!(r[3], Err(SendError::Unreachable("down".into())));
        assert_eq!(r[4], Err(SendError::Unreachable("down".into())));
    }

    #[test]
    fn only_none_is_known_until_a_target_has_a_receive_side() {
        assert!(connect("none").unwrap().is_none());
        assert!(connect("").unwrap().is_none());
        assert!(connect("walter").err().unwrap().contains("no receive side"));
    }
}
