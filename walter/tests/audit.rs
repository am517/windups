//! Walter audits other windups read-only and names the fix; he never writes to them.
use rusqlite::Connection;

fn windup_db(root: &std::path::Path, name: &str, unsynced: usize) -> std::path::PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.db"));
    let mut c = Connection::open(&p).unwrap();
    let mut m = vec!["CREATE TABLE records(seq INTEGER PRIMARY KEY, id TEXT, synced_at INTEGER);"];
    m.extend_from_slice(windups::base::CORE_V1);
    windups::base::schema::migrate(&mut c, "t", &m).unwrap();
    for i in 0..unsynced {
        c.execute("INSERT INTO records(id) VALUES (?1)", [format!("r{i}")])
            .unwrap();
    }
    p
}

fn bytes(p: &std::path::Path) -> Vec<u8> {
    std::fs::read(p).unwrap()
}

#[test]
fn audit_reports_findings_and_fixes_without_writing() {
    let d = tempfile::tempdir().unwrap();
    let jack = windup_db(d.path(), "jack", 2);
    let larry = windup_db(d.path(), "larry", 0);
    Connection::open(&larry)
        .unwrap()
        .execute(
            "INSERT INTO snapshots(at, path, bytes, events_through) VALUES (?1, 'x', 1, 0)",
            [windups::base::events::now()],
        )
        .unwrap();
    std::fs::create_dir_all(d.path().join("notawindup")).unwrap();
    let before = (bytes(&jack), bytes(&larry));
    let out = walter::audit::report(&[d.path().to_path_buf(), d.path().to_path_buf()], 7);
    assert!(
        out.starts_with("audited 2 windup(s): 2 finding(s)"),
        "{out}"
    );
    assert!(
        out.contains("  jack: 2 record(s) not yet synced -> run: jack sync"),
        "{out}"
    );
    assert!(
        out.contains("  jack: no backup yet -> run: jack backup"),
        "{out}"
    );
    assert!(out.contains("  all good: larry"), "{out}");
    assert_eq!((bytes(&jack), bytes(&larry)), before, "read-only");
}

#[test]
fn walter_answers_universal_commands_and_audits_by_talk() {
    use windups::base::commands::{Command, Hooks, dispatch};
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("windups");
    windup_db(&root, "jack", 1);
    let cfg = d.path().join("walter/config.toml");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    std::fs::write(
        &cfg,
        format!("[audit]\nroots = [{:?}]\n", root.to_str().unwrap()),
    )
    .unwrap();
    let mut w = walter::universal::Walter::open(Some(&cfg)).unwrap();
    assert_eq!(
        w.layout.db,
        d.path().join("walter/walter.db"),
        "his own folder"
    );
    let h = Hooks {
        helper: &windups::base::talk::NoHelper,
        off_device: &windups::base::snapshot::NoOffDevice,
    };
    let r = dispatch(&mut w, Command::Talk("check on everyone".into()), &h).unwrap();
    assert_eq!(r.ran, "audit");
    assert!(
        r.text
            .contains("jack: 1 record(s) not yet synced -> run: jack sync"),
        "{}",
        r.text
    );
    let v = dispatch(&mut w, Command::Version, &h).unwrap();
    assert!(v.text.starts_with("walter 0."), "{}", v.text);
    let log = dispatch(&mut w, Command::Log(5), &h).unwrap().text;
    assert!(log.contains(" action audited 1 windup(s)"), "{log}");
}
