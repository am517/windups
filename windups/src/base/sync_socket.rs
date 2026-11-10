//! A `SyncTarget` over a Unix domain socket using the `wire` protocol: one request per batch, one
//! result per record. Any receiver that speaks `wire` (Daniel today) can sit at the other end.
use super::sync::{SendError, SyncTarget};
use super::wire::{self, Rec, Request, Response};
use std::path::PathBuf;
use std::time::Duration;

pub struct SocketTarget {
    path: PathBuf,
    timeout: Duration,
    max_frame: usize,
    conn: Option<std::os::unix::net::UnixStream>,
}

impl SocketTarget {
    pub fn new(path: PathBuf, timeout: Duration, max_frame: usize) -> Self {
        Self {
            path,
            timeout,
            max_frame,
            conn: None,
        }
    }

    fn exchange(&mut self, req: &[u8]) -> Result<Response, String> {
        if self.conn.is_none() {
            let s = std::os::unix::net::UnixStream::connect(&self.path)
                .map_err(|e| format!("{}: {e}", self.path.display()))?;
            s.set_read_timeout(Some(self.timeout))
                .map_err(|e| e.to_string())?;
            s.set_write_timeout(Some(self.timeout))
                .map_err(|e| e.to_string())?;
            self.conn = Some(s);
        }
        let s = self.conn.as_mut().ok_or("not connected")?;
        let r = wire::write_frame(s, req)
            .and_then(|_| wire::read_frame(s, self.max_frame))
            .map_err(|e| e.to_string());
        match r {
            Ok(Some(body)) => serde_json::from_slice(&body).map_err(|e| format!("bad answer: {e}")),
            Ok(None) => {
                self.conn = None;
                Err("the receiver hung up".into())
            }
            Err(e) => {
                self.conn = None;
                Err(e)
            }
        }
    }
}

impl SyncTarget for SocketTarget {
    fn send(&mut self, id: &str, record: &str) -> Result<(), SendError> {
        self.send_batch(&[(id, record)])
            .pop()
            .unwrap_or(Err(SendError::Unreachable("no answer".into())))
    }

    fn send_batch(&mut self, recs: &[(&str, &str)]) -> Vec<Result<(), SendError>> {
        let all = |e: String| {
            recs.iter()
                .map(|_| Err(SendError::Unreachable(e.clone())))
                .collect()
        };
        let req = Request {
            v: wire::VERSION,
            op: "sync".into(),
            records: recs
                .iter()
                .map(|(i, r)| Rec {
                    id: (*i).into(),
                    record: (*r).into(),
                })
                .collect(),
        };
        let body = match serde_json::to_vec(&req) {
            Ok(b) => b,
            Err(e) => return all(e.to_string()),
        };
        drop(req);
        let resp = match self.exchange(&body) {
            Ok(r) => r,
            Err(e) => return all(e),
        };
        if let Some(e) = resp.error {
            return all(e);
        }
        // match answers by id, in order; anything unanswered counts as not acknowledged
        let mut it = resp.results.into_iter().peekable();
        recs.iter()
            .map(|(id, _)| match it.next_if(|o| o.id == *id) {
                Some(o) if o.ok => Ok(()),
                Some(o) => Err(SendError::Rejected(o.detail)),
                None => Err(SendError::Unreachable(format!("no answer for {id}"))),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn a_batch_round_trips_with_one_result_per_record() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("r.sock");
        let l = UnixListener::bind(&p).unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut h = |recs: &[(&str, &str)]| {
                Ok(recs
                    .iter()
                    .map(|(i, _)| {
                        if *i == "bad" {
                            Err("no".to_string())
                        } else {
                            Ok("kept".into())
                        }
                    })
                    .collect())
            };
            wire::answer(&mut s, 1 << 20, &mut h).unwrap();
        });
        let mut t = SocketTarget::new(p.clone(), Duration::from_secs(5), 1 << 20);
        let r = t.send_batch(&[("a", "{}"), ("bad", "{}")]);
        assert_eq!(r, vec![Ok(()), Err(SendError::Rejected("no".into()))]);
        assert_eq!(t.send("c", "{}"), Ok(()), "the connection is reused");
        drop(t);
        server.join().unwrap();
        let mut gone = SocketTarget::new(p, Duration::from_secs(1), 1 << 20);
        assert!(matches!(
            gone.send("a", "{}"),
            Err(SendError::Unreachable(_))
        ));
    }
}
