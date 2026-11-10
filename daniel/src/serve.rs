//! `daniel serve`: Daniel's socket receive side. Listens on a Unix domain socket and answers each
//! batch (windups::base::wire) with `receive::receive_batch`: one result per record. Connections are
//! handled one at a time (low memory; senders queue). Runs until stopped.
use crate::{Store, StoreCfg};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

/// Binds `path`, replacing a stale socket file nobody is listening on.
pub fn bind(path: &Path) -> Result<UnixListener, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    if path.exists() {
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return Err(format!(
                "{}: another Daniel is already serving",
                path.display()
            ));
        }
        std::fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    UnixListener::bind(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Serves `connections` connections (None = forever).
pub fn serve(
    l: &UnixListener,
    store: &Store,
    max_frame: usize,
    connections: Option<usize>,
) -> Result<(), String> {
    let mut left = connections;
    while left != Some(0) {
        let (mut s, _) = l.accept().map_err(|e| e.to_string())?;
        let mut h = |recs: &[(&str, &str)]| crate::receive::receive_batch(store, recs);
        if let Err(e) = windups::base::wire::answer(&mut s, max_frame, &mut h) {
            eprintln!("daniel: connection: {e}");
        }
        left = left.map(|n| n - 1);
    }
    Ok(())
}

/// `daniel serve [--socket PATH] [--db PATH]`.
pub fn run(args: &[String]) -> Result<(), String> {
    let (mut socket, mut db) = (None, String::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--socket" => socket = Some(PathBuf::from(val()?)),
            "--db" => db = val()?,
            o => {
                return Err(format!(
                    "usage: daniel serve [--socket PATH] [--db PATH] (unknown {o})"
                ));
            }
        }
    }
    let cfg = StoreCfg {
        enabled: true,
        database: db,
        busy_timeout_secs: 5,
        ..Default::default()
    };
    let store = Store::new(&cfg);
    let serve_cfg = store.config.serve.clone();
    let path = socket.unwrap_or_else(|| windups::paths::expand(&serve_cfg.socket));
    let l = bind(&path)?;
    eprintln!("daniel: serving on {}", path.display());
    serve(&l, &store, serve_cfg.max_frame_bytes.max(1024), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use windups::base::sync::SyncTarget;
    use windups::base::sync_socket::SocketTarget;

    #[test]
    fn socket_round_trip_and_an_idempotent_resend() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("Heat.1995.mkv");
        std::fs::write(&f, "x").unwrap();
        let db = d.path().join("m.db");
        let sock = d.path().join("daniel.sock");
        let l = bind(&sock).unwrap();
        let dbs = db.to_string_lossy().into_owned();
        let server = std::thread::spawn(move || {
            let cfg = StoreCfg {
                enabled: true,
                database: dbs,
                busy_timeout_secs: 1,
                ..Default::default()
            };
            let mut store = Store::new(&cfg);
            store.config.walter.enabled = false;
            serve(&l, &store, 1 << 20, Some(2)).unwrap();
        });
        let h = windups::handoff::Handoff {
            file: f,
            entry: [("kind", "movie"), ("title", "Heat"), ("year", "1995")]
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            candidates: vec![],
            conflicts: vec![],
            from: "jack".into(),
            caller: None,
        };
        let mut json = Vec::new();
        h.write_json(&mut json).unwrap();
        let json = String::from_utf8(json).unwrap();
        let mut t = SocketTarget::new(sock.clone(), Duration::from_secs(10), 1 << 20);
        let r = t.send_batch(&[("jack-1", &json), ("jack-2", "not json")]);
        assert_eq!(r[0], Ok(()));
        assert!(
            matches!(&r[1], Err(windups::base::sync::SendError::Rejected(_))),
            "{r:?}"
        );
        drop(t);
        let mut again = SocketTarget::new(sock.clone(), Duration::from_secs(10), 1 << 20);
        assert_eq!(
            again.send("jack-1", &json),
            Ok(()),
            "a resend is acknowledged"
        );
        drop(again);
        server.join().unwrap();
        let c = windups::db::open(&db).unwrap();
        let n: i64 = c
            .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "and stores nothing new");
        assert!(bind(&sock).is_ok(), "a stale socket file is replaced");
    }
}
