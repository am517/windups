//! Daniel's receive side: a batch of records from a windup's own archive (handoff JSON v1, each
//! under its stable id), one result per record. A library call today; a socket can wrap the same
//! function later. Storing the same record again changes nothing, so senders may retry.
use crate::Store;
use windups::handoff::{Daniel, Handoff};

/// `Err` for the whole batch: the catalog can't be opened (the sender keeps everything unsynced).
pub fn receive_batch(
    store: &Store,
    recs: &[(&str, &str)],
) -> Result<Vec<Result<String, String>>, String> {
    let db = crate::store::db_path(store.store)?;
    windups::db::open(&db).map_err(|e| format!("no database ({e})"))?;
    Ok(recs
        .iter()
        .map(|(_, json)| Handoff::read_json(json.as_bytes()).and_then(|h| store.accept(&h)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoreCfg;

    #[test]
    fn one_result_per_record_and_a_resend_adds_nothing() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("Heat.1995.mkv");
        std::fs::write(&f, "x").unwrap();
        let cfg = StoreCfg {
            enabled: true,
            database: d.path().join("m.db").to_string_lossy().into(),
            busy_timeout_secs: 1,
            ..Default::default()
        };
        let mut dan = Store::new(&cfg);
        dan.config.walter.enabled = false;
        let h = Handoff {
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
        let mut good = Vec::new();
        h.write_json(&mut good).unwrap();
        let good = String::from_utf8(good).unwrap();
        let r = receive_batch(&dan, &[("jack-1", &good), ("jack-2", "{\"v\":9}")]).unwrap();
        assert!(
            r[0].as_ref().unwrap().starts_with("database: media #1"),
            "{r:?}"
        );
        assert!(
            r[1].as_ref()
                .unwrap_err()
                .starts_with("invalid handoff JSON"),
            "{r:?}"
        );
        receive_batch(&dan, &[("jack-1", &good)]).unwrap();
        let c = windups::db::open(&d.path().join("m.db")).unwrap();
        let n: i64 = c
            .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "the resend stored nothing new");
        let bad = StoreCfg {
            database: d.path().to_string_lossy().into(),
            ..cfg.clone()
        };
        let e = receive_batch(&Store::new(&bad), &[("jack-1", &good)]).unwrap_err();
        assert!(e.starts_with("no database"), "{e}");
    }
}
