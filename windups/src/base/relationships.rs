//! Windup relationships, stored only in config. A windup is naive about the others: it
//! uses a name only after it appears here, and it can always proceed alone.
//!
//! ```toml
//! [relationships]
//! assistant = ""      # e.g. flora (written by Flora on install, not hardcoded)
//! matchmaker = ""     # e.g. oliver
//! database = ""       # e.g. daniel
//! llm = ""            # e.g. sammy
//! ```
//!
//! All keys are optional and empty by default. Flora and Oliver do not exist yet; the
//! lookup uses a [`Directory`] that cleanly reports "not reachable", and the windup
//! continues in isolation with its own fallbacks.
use serde::{Deserialize, Serialize};

/// The four relationship roles. Names live in the windup's config, never in code defaults.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(default)]
pub struct Relationships {
    pub assistant: String,
    pub matchmaker: String,
    pub database: String,
    pub llm: String,
}

pub const ROLES: [&str; 4] = ["assistant", "matchmaker", "database", "llm"];

impl Relationships {
    pub fn is_set(s: &str) -> bool {
        !s.trim().is_empty()
    }

    pub fn get(&self, role: &str) -> &str {
        match role {
            "assistant" => self.assistant.as_str(),
            "matchmaker" => self.matchmaker.as_str(),
            "database" => self.database.as_str(),
            "llm" => self.llm.as_str(),
            _ => "",
        }
    }

    pub fn set(&mut self, role: &str, name: String) {
        match role {
            "assistant" => self.assistant = name,
            "matchmaker" => self.matchmaker = name,
            "database" => self.database = name,
            "llm" => self.llm = name,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContactError {
    /// The peer could not be reached (not installed, down, or no transport).
    Unreachable(String),
    UnknownRole(String),
}

impl std::fmt::Display for ContactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContactError::Unreachable(e) => write!(f, "not reachable ({e})"),
            ContactError::UnknownRole(r) => write!(f, "unknown role {r:?}"),
        }
    }
}

/// How a windup asks another windup for a name. Flora and Oliver will implement this.
pub trait Directory {
    /// Ask `peer` who fills `role` (`matchmaker`, `database`, `llm`, ...).
    fn introduce(&self, peer: &str, role: &str) -> Result<String, ContactError>;
}

/// Isolation default: nobody is out there. Reports "not reachable" for every ask.
pub struct NoDirectory;

impl Directory for NoDirectory {
    fn introduce(&self, peer: &str, role: &str) -> Result<String, ContactError> {
        Err(ContactError::Unreachable(format!(
            "{peer} has no introduce for {role} (not installed)"
        )))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolution {
    pub rels: Relationships,
    /// Role keys that changed and should be written back to config.
    pub changed: Vec<(String, String)>,
    pub notes: Vec<String>,
}

fn ask(dir: &dyn Directory, peer: &str, role: &str, notes: &mut Vec<String>) -> Option<String> {
    if !Relationships::is_set(peer) {
        return None;
    }
    match dir.introduce(peer, role) {
        Ok(name) if Relationships::is_set(&name) => Some(name),
        Ok(_) => {
            notes.push(format!("{peer} had no {role}"));
            None
        }
        Err(e) => {
            notes.push(format!("could not reach {peer} for {role}: {e}"));
            None
        }
    }
}

fn fill(rels: &mut Relationships, role: &str, name: String, changed: &mut Vec<(String, String)>) {
    if rels.get(role) != name {
        rels.set(role, name.clone());
        changed.push((role.into(), name));
    }
}

/// Discover missing relationships.
///
/// 1. If matchmaker is unknown, ask the assistant for one.
/// 2. If database or llm is unknown, ask the matchmaker, then the assistant.
/// 3. If both are unreachable, leave the slot empty and proceed in isolation.
pub fn resolve(mut rels: Relationships, dir: &dyn Directory) -> Resolution {
    let mut notes = Vec::new();
    let mut changed = Vec::new();

    if !Relationships::is_set(&rels.matchmaker)
        && let Some(m) = ask(dir, &rels.assistant.clone(), "matchmaker", &mut notes)
    {
        fill(&mut rels, "matchmaker", m, &mut changed);
    }

    for role in ["database", "llm"] {
        if Relationships::is_set(rels.get(role)) {
            continue;
        }
        let name = ask(dir, &rels.matchmaker.clone(), role, &mut notes)
            .or_else(|| ask(dir, &rels.assistant.clone(), role, &mut notes));
        if let Some(n) = name {
            fill(&mut rels, role, n, &mut changed);
        }
    }

    if !Relationships::is_set(&rels.matchmaker)
        && !Relationships::is_set(&rels.assistant)
        && !Relationships::is_set(&rels.database)
        && !Relationships::is_set(&rels.llm)
    {
        notes.push("no relationships configured; proceeding in isolation".into());
    }

    Resolution {
        rels,
        changed,
        notes,
    }
}

/// After a contact failure mid-run: ask the matchmaker, then the assistant, for `role`.
/// Isolation if both fail (the slot is left as it was unless a name comes back).
pub fn retry(rels: &mut Relationships, dir: &dyn Directory, role: &str) -> Resolution {
    let mut notes = Vec::new();
    let mut changed = Vec::new();
    let name = ask(dir, &rels.matchmaker.clone(), role, &mut notes)
        .or_else(|| ask(dir, &rels.assistant.clone(), role, &mut notes));
    match name {
        Some(n) => fill(rels, role, n, &mut changed),
        None => notes.push(format!(
            "could not reach anyone for {role}; proceeding in isolation"
        )),
    }
    Resolution {
        rels: rels.clone(),
        changed,
        notes,
    }
}

/// An old `[archive] sync_target` name becomes `relationships.database` when that slot is
/// still empty. `"none"` and empty are not names.
pub fn migrate_database(rels: &mut Relationships, sync_target: &str) -> Option<String> {
    if Relationships::is_set(&rels.database) {
        return None;
    }
    let t = sync_target.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return None;
    }
    rels.database = t.to_string();
    Some(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct MapDir(BTreeMap<(String, String), Result<String, ContactError>>);

    impl Directory for MapDir {
        fn introduce(&self, peer: &str, role: &str) -> Result<String, ContactError> {
            self.0
                .get(&(peer.into(), role.into()))
                .cloned()
                .unwrap_or_else(|| {
                    Err(ContactError::Unreachable(format!("{peer} down for {role}")))
                })
        }
    }

    fn put(d: &mut MapDir, peer: &str, role: &str, v: Result<String, ContactError>) {
        d.0.insert((peer.into(), role.into()), v);
    }

    #[test]
    fn no_peers_proceeds_in_isolation() {
        let r = resolve(Relationships::default(), &NoDirectory);
        assert_eq!(r.rels, Relationships::default());
        assert!(r.changed.is_empty());
        assert!(
            r.notes.iter().any(|n| n.contains("isolation")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn assistant_then_matchmaker_are_saved() {
        let mut d = MapDir(BTreeMap::new());
        put(&mut d, "flora", "matchmaker", Ok("oliver".into()));
        put(&mut d, "oliver", "database", Ok("daniel".into()));
        put(&mut d, "oliver", "llm", Ok("sammy".into()));
        let start = Relationships {
            assistant: "flora".into(),
            ..Default::default()
        };
        let r = resolve(start, &d);
        assert_eq!(r.rels.matchmaker, "oliver");
        assert_eq!(r.rels.database, "daniel");
        assert_eq!(r.rels.llm, "sammy");
        assert!(r.changed.contains(&("matchmaker".into(), "oliver".into())));
        assert!(r.changed.contains(&("database".into(), "daniel".into())));
        assert!(r.changed.contains(&("llm".into(), "sammy".into())));
    }

    #[test]
    fn matchmaker_failure_falls_back_to_assistant_then_isolation() {
        let mut d = MapDir(BTreeMap::new());
        put(
            &mut d,
            "oliver",
            "database",
            Err(ContactError::Unreachable("down".into())),
        );
        put(&mut d, "flora", "database", Ok("daniel".into()));
        let start = Relationships {
            assistant: "flora".into(),
            matchmaker: "oliver".into(),
            ..Default::default()
        };
        let r = resolve(start, &d);
        assert_eq!(r.rels.database, "daniel");
        assert!(
            r.notes.iter().any(|n| n.contains("oliver")),
            "{:?}",
            r.notes
        );

        let r = resolve(
            Relationships {
                assistant: "flora".into(),
                matchmaker: "oliver".into(),
                ..Default::default()
            },
            &NoDirectory,
        );
        assert!(r.rels.database.is_empty() && r.rels.llm.is_empty());
        assert!(
            r.notes.iter().any(|n| n.contains("not reachable")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn retry_asks_matchmaker_then_assistant() {
        let mut rels = Relationships {
            assistant: "flora".into(),
            matchmaker: "oliver".into(),
            database: "stale".into(),
            ..Default::default()
        };
        let mut d = MapDir(BTreeMap::new());
        put(
            &mut d,
            "oliver",
            "database",
            Err(ContactError::Unreachable("down".into())),
        );
        put(&mut d, "flora", "database", Ok("daniel".into()));
        let r = retry(&mut rels, &d, "database");
        assert_eq!(rels.database, "daniel");
        assert_eq!(r.changed, vec![("database".into(), "daniel".into())]);
    }

    #[test]
    fn old_sync_target_fills_empty_database_only() {
        let mut rels = Relationships::default();
        assert_eq!(
            migrate_database(&mut rels, "daniel").as_deref(),
            Some("daniel")
        );
        assert_eq!(rels.database, "daniel");
        assert_eq!(migrate_database(&mut rels, "other"), None);
        assert_eq!(rels.database, "daniel");
        let mut empty = Relationships::default();
        assert_eq!(migrate_database(&mut empty, "none"), None);
        assert_eq!(migrate_database(&mut empty, "  "), None);
        assert!(empty.database.is_empty());
    }
}
