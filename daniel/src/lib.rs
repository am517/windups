//! Daniel, the database windup. Takes one entry per file (`windups::handoff::Handoff`), asks Larry
//! for the file's hash and fingerprint and Ruth for the work reference, stores it all, then tells
//! Walter. Windups hand entries to Daniel; only Daniel writes them.
pub mod alert;
pub mod config;
pub mod ingest;
pub mod store;
pub mod walter;

pub use store::{HandoffJson, StoreCfg};
use windups::handoff::{Daniel, Handoff};

pub const PROFILE: &str = include_str!("../PROFILE.toml");

pub struct Store<'a> {
    pub store: &'a StoreCfg,
    pub larry: larry::Config,
    pub config: config::Config,
}

impl<'a> Store<'a> {
    /// With Larry's and Daniel's user configs from ~/.config/windups/<name>/config.toml.
    pub fn new(store: &'a StoreCfg) -> Self {
        let user = |n: &str| {
            std::env::var_os("HOME")
                .map(|h| std::path::Path::new(&h).join(format!(".config/windups/{n}/config.toml")))
        };
        Store {
            store,
            larry: larry::Config::load(user("larry").as_deref()).unwrap_or_default(),
            config: config::Config::load(user("daniel").as_deref()).unwrap_or_default(),
        }
    }
}

/// What the file's entry was before this store: None if Daniel hadn't seen the file.
fn previous(db: &std::path::Path, file: &std::path::Path) -> Option<Option<String>> {
    let c = windups::db::open(db).ok()?;
    c.query_row(
        "SELECT m.lifecycle FROM files f JOIN media m ON m.id = f.media_id WHERE f.path = ?1",
        [file.to_string_lossy()],
        |r| r.get(0),
    )
    .ok()
}

impl Store<'_> {
    /// Counts the change and alerts Walter if it's time, unless Walter started this run.
    fn after_store(&self, h: &Handoff, before: Option<&Option<String>>) -> Option<String> {
        let db = store::db_path(self.store).ok()?;
        let c = windups::db::open(&db).ok()?;
        let change = match before {
            None => alert::Change::NewEntry,
            Some(old) if old.as_ref() != h.entry.get("lifecycle") => alert::Change::StageChange,
            Some(_) => alert::Change::Update,
        };
        alert::count(&c, change, &self.config.importance).ok()?;
        let w = &self.config.walter;
        if !w.enabled || h.caller.as_deref() == Some("walter") {
            return None; // the loop guard: Walter's own work doesn't call him back
        }
        let why = alert::due(&c, &self.config).ok()??;
        let r = walter::notify(&w.command, &db)?;
        alert::reset(&c).ok()?;
        Some(format!("{r} ({why})"))
    }
}

impl Daniel for Store<'_> {
    fn accept(&self, h: &Handoff) -> Result<String, String> {
        let db = store::db_path(self.store).ok();
        let before = db.as_deref().and_then(|p| previous(p, &h.file));
        let mut entry = h.entry.clone();
        let fp = larry::fingerprint::NoFingerprint;
        entry.extend(larry::describe(&h.file, &self.larry, &fp));
        let mut out = store::save(
            &h.file,
            &entry,
            &h.candidates,
            &h.conflicts,
            self.store,
            &fp,
            &self.larry.fingerprint,
        )?;
        if out.starts_with("database:") {
            if let Some(n) = self.after_store(h, before.as_ref()) {
                out += &format!("; {n}");
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_handoff_with_larrys_hash_and_ruths_reference() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.mkv");
        std::fs::write(&f, "content").unwrap();
        let cfg = StoreCfg {
            enabled: true,
            database: d.path().join("m.db").to_string_lossy().into(),
            fallback_suffix: ".json".into(),
            busy_timeout_secs: 1,
            ..Default::default()
        };
        let mut dan = Store::new(&cfg);
        dan.config.walter.enabled = false;
        let entry: windups::handoff::Fields = [
            ("kind", "movie"),
            ("title", "Heat"),
            ("year", "1995"),
            ("lifecycle", "unverified"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let r = dan
            .accept(&Handoff {
                file: f,
                entry,
                candidates: vec![],
                conflicts: vec![],
                from: "jack".into(),
                caller: None,
            })
            .unwrap();
        assert!(r.starts_with("database: media #1"), "{r}");
        let c = windups::db::open(&d.path().join("m.db")).unwrap();
        let (hash, prov): (String, bool) = c
            .query_row(
                "SELECT f.hash, w.provisional FROM files f JOIN media m ON m.id = f.media_id JOIN works w ON w.id = m.work_id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(hash.starts_with("b3"), "{hash}");
        assert!(prov, "no original title or language yet");
        assert!(PROFILE.contains("name = \"daniel\""));
    }

    #[test]
    fn alerts_walter_at_the_threshold_but_never_for_walters_own_work() {
        let d = tempfile::tempdir().unwrap();
        let mark = d.path().join("walter-was-called");
        let fake = d.path().join("walter");
        std::fs::write(
            &fake,
            format!("#!/bin/sh\necho \"$@\" >> {}\n", mark.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cfg = StoreCfg {
            enabled: true,
            database: d.path().join("m.db").to_string_lossy().into(),
            fallback_suffix: ".json".into(),
            busy_timeout_secs: 1,
            ..Default::default()
        };
        let mut dan = Store::new(&cfg);
        dan.config.walter.command = vec![fake.to_string_lossy().into(), "patrol".into()];
        dan.config.walter.max_changes = 2;
        dan.config.walter.importance = 100;
        dan.config.walter.max_hours = 1000.0;
        let hand = |name: &str, caller: Option<&str>| {
            let f = d.path().join(name);
            std::fs::write(&f, name).unwrap();
            dan.accept(&Handoff {
                file: f,
                entry: [("kind", "movie"), ("title", name), ("year", "2000")]
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                candidates: vec![],
                conflicts: vec![],
                from: "jack".into(),
                caller: caller.map(String::from),
            })
            .unwrap()
        };
        let calls = || std::fs::read_to_string(&mark).map_or(0, |s| s.lines().count());
        hand("a", None);
        assert_eq!(calls(), 0, "below the threshold");
        hand("b", Some("walter"));
        assert_eq!(calls(), 0, "threshold reached, but Walter started this run");
        let r = hand("c", None);
        assert_eq!(calls(), 1, "{r}");
        assert!(r.contains("walter patrolled (3 changes)"), "{r}");
        assert!(
            std::fs::read_to_string(&mark)
                .unwrap()
                .contains("patrol --db")
        );
        hand("d", None);
        assert_eq!(calls(), 1, "counting afresh after the call");
    }
}
