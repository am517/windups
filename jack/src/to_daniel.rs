//! Jack's sync target: Daniel, the record keeper, by library call (`daniel::receive`). A socket
//! transport can replace this later behind the same `SyncTarget` trait.
use daniel::{Store, StoreCfg};
use windups::base::sync::{SendError, SyncTarget};

pub struct ToDaniel<'a> {
    store: Store<'a>,
}

impl<'a> ToDaniel<'a> {
    pub fn new(cfg: &'a StoreCfg) -> Self {
        Self {
            store: Store::new(cfg),
        }
    }

    #[cfg(test)]
    pub fn quiet(cfg: &'a StoreCfg) -> Self {
        let mut s = Self::new(cfg);
        s.store.config.walter.enabled = false;
        s
    }
}

impl SyncTarget for ToDaniel<'_> {
    fn send(&mut self, id: &str, record: &str) -> Result<(), SendError> {
        self.send_batch(&[(id, record)])
            .pop()
            .unwrap_or(Err(SendError::Unreachable("no answer".into())))
    }

    /// Daniel answers per batch, with one result per record.
    fn send_batch(&mut self, recs: &[(&str, &str)]) -> Vec<Result<(), SendError>> {
        match daniel::receive::receive_batch(&self.store, recs) {
            Err(e) => recs
                .iter()
                .map(|_| Err(SendError::Unreachable(e.clone())))
                .collect(),
            Ok(rs) => rs
                .into_iter()
                .map(|r| r.map(|_| ()).map_err(SendError::Rejected))
                .collect(),
        }
    }
}
