//! Smoke: versioned handoff JSON into a temp media.db, no duplicates on re-ingest, bad version rejected.
use std::process::Command;
use windups::handoff::{Daniel, Handoff};

fn fields(kv: &[(&str, &str)]) -> windups::handoff::Fields {
    kv.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn fixture_json(media: &std::path::Path) -> String {
    let h = Handoff {
        file: media.to_path_buf(),
        entry: fields(&[
            ("kind", "movie"),
            ("title", "The Matrix"),
            ("year", "1999"),
            ("tmdb_id", "603"),
            ("lifecycle", "unverified"),
        ]),
        candidates: vec![
            (
                "tmdb".into(),
                fields(&[("title", "The Matrix"), ("year", "1999")]),
            ),
            (
                "embedded".into(),
                fields(&[("title", "The.Matrix"), ("year", "1998")]),
            ),
        ],
        conflicts: vec![(
            "year".into(),
            vec![
                ("tmdb".into(), "1999".into()),
                ("embedded".into(), "1998".into()),
            ],
        )],
        from: "jack".into(),
        caller: None,
    };
    let mut buf = Vec::new();
    h.write_json(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

fn counts(c: &rusqlite::Connection) -> (i64, i64, i64, i64) {
    let n = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
    (
        n("SELECT count(*) FROM media WHERE status = 'chosen'"),
        n("SELECT count(*) FROM media WHERE status = 'candidate'"),
        n("SELECT count(*) FROM conflicts WHERE field = 'year' AND status = 'open'"),
        n("SELECT count(*) FROM files"),
    )
}

#[test]
fn ingest_fixture_then_again_does_not_duplicate() {
    let d = tempfile::tempdir().unwrap();
    let media = d.path().join("The.Matrix.1999.mkv");
    std::fs::write(&media, b"matrix").unwrap();
    let json_path = d.path().join("The.Matrix.1999.mkv.jack.json");
    std::fs::write(&json_path, fixture_json(&media)).unwrap();
    let db = d.path().join("media.db");

    let cfg = daniel::StoreCfg {
        enabled: true,
        database: db.to_string_lossy().into(),
        fallback_suffix: ".jack.json".into(),
        busy_timeout_secs: 1,
        ..Default::default()
    };
    let mut dan = daniel::Store::new(&cfg);
    dan.config.walter.enabled = false;
    let h = Handoff::read_json(std::fs::File::open(&json_path).unwrap()).unwrap();
    let r = dan.accept(&h).unwrap();
    assert!(r.starts_with("database: media #"), "{r}");

    let c = windups::db::open(&db).unwrap();
    let first = counts(&c);
    assert_eq!(first, (1, 2, 1, 1));
    assert_eq!(
        c.query_row(
            "SELECT title, year FROM media WHERE status = 'chosen'",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap(),
        ("The Matrix".into(), 1999)
    );
    drop(c);

    let r2 = dan.accept(&h).unwrap();
    assert!(r2.contains("media #"), "{r2}");
    let c = windups::db::open(&db).unwrap();
    assert_eq!(counts(&c), first, "re-accept must not duplicate rows");
}

#[test]
fn ingest_cli_then_rejects_bad_version() {
    let d = tempfile::tempdir().unwrap();
    let media = d.path().join("The.Matrix.1999.mkv");
    std::fs::write(&media, b"matrix").unwrap();
    let json_path = d.path().join("ok.jack.json");
    std::fs::write(&json_path, fixture_json(&media)).unwrap();
    let db = d.path().join("media.db");
    let exe = env!("CARGO_BIN_EXE_daniel");

    let out = Command::new(exe)
        .args([
            "ingest",
            "--db",
            db.to_str().unwrap(),
            json_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr={} stdout={stdout}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("database: media #"), "{stdout}");

    let again = Command::new(exe)
        .args([
            "accept",
            "--db",
            db.to_str().unwrap(),
            json_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    let c = windups::db::open(&db).unwrap();
    assert_eq!(counts(&c), (1, 2, 1, 1));

    let bad = d.path().join("bad.json");
    std::fs::write(
        &bad,
        r#"{"v":99,"file":"/nope","entry":{},"candidates":[],"conflicts":[],"from":"jack","caller":null}"#,
    )
    .unwrap();
    let rej = Command::new(exe)
        .args([
            "ingest",
            "--db",
            db.to_str().unwrap(),
            bad.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!rej.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&rej.stdout),
        String::from_utf8_lossy(&rej.stderr)
    );
    assert!(text.contains("unsupported handoff version 99"), "{text}");
}
