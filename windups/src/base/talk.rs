//! `talk`: plain words to a real command. A small deterministic keyword matcher; whatever it
//! can't place goes to a `Helper` (later Flora, or a local model on small devices).
use super::commands::{Action, Command, UNIVERSAL};
use std::collections::BTreeMap;

/// Words that point at each universal command (more come from `[core.talk] synonyms`).
const WORDS: &[(&str, &[&str])] = &[
    (
        "status",
        &["status", "doing", "up to", "state", "how are you"],
    ),
    (
        "health",
        &[
            "health",
            "healthy",
            "ok",
            "okay",
            "integrity",
            "broken",
            "corrupt",
        ],
    ),
    ("sync", &["sync", "unsynced", "catch up", "send"]),
    ("backup", &["backup", "back up", "snapshot", "save a copy"]),
    ("export", &["export", "dump", "json"]),
    ("version", &["version"]),
    ("config", &["config", "configuration", "settings", "setup"]),
    ("log", &["log", "history", "events", "happened", "recent"]),
    ("help", &["help", "commands", "what can you do"]),
    (
        "migrate",
        &["migrate", "move house", "move folder", "move your folder"],
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Run(Command),
    Ambiguous(Vec<String>),
    Unknown,
}

/// Someone who understands more than keywords. Returns a command line (e.g. "backup") or None.
pub trait Helper {
    fn place(&self, text: &str, choices: &[String]) -> Option<String>;
}

/// No helper yet (no Flora, no local model).
pub struct NoHelper;

impl Helper for NoHelper {
    fn place(&self, _: &str, _: &[String]) -> Option<String> {
        None
    }
}

fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(' ');
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    if !out.ends_with(' ') {
        out.push(' ');
    }
    out
}

fn hits(text: &str, words: impl IntoIterator<Item = impl AsRef<str>>) -> usize {
    words
        .into_iter()
        .filter(|w| text.contains(&norm(w.as_ref())))
        .count()
}

pub fn interpret(
    text: &str,
    actions: &[Action],
    synonyms: &BTreeMap<String, Vec<String>>,
) -> Intent {
    let t = norm(text);
    let extra = |name: &str| synonyms.get(name).map_or(&[][..], Vec::as_slice);
    let mut scored: Vec<(usize, String)> = Vec::new();
    for (name, words) in WORDS {
        scored.push((hits(&t, *words) + hits(&t, extra(name)), (*name).into()));
    }
    for a in actions {
        let n = hits(&t, a.keywords) + hits(&t, extra(a.name)) + 2 * hits(&t, [a.name]);
        scored.push((n, a.name.into()));
    }
    let best = scored.iter().map(|s| s.0).max().unwrap_or(0);
    if best == 0 {
        return Intent::Unknown;
    }
    let top: Vec<String> = scored
        .into_iter()
        .filter(|s| s.0 == best)
        .map(|s| s.1)
        .collect();
    if top.len() > 1 {
        return Intent::Ambiguous(top);
    }
    let name = &top[0];
    if !UNIVERSAL.iter().any(|(n, _)| n == name) {
        return Intent::Run(Command::Action(name.clone(), String::new()));
    }
    let number = t.split(' ').find_map(|w| w.parse::<usize>().ok());
    Intent::Run(match name.as_str() {
        "log" => Command::Log(number.unwrap_or(20)),
        // plain words only preview a migration; `migrate` itself does it
        "migrate" => Command::Migrate { dry_run: true },
        other => super::commands::parse(&[other.to_string()]).unwrap_or(Command::Help),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIONS: &[Action] = &[Action {
        name: "tag",
        keywords: &["tag", "label", "identify"],
        help: "",
    }];

    fn i(t: &str) -> Intent {
        interpret(t, ACTIONS, &BTreeMap::new())
    }

    #[test]
    fn plain_words_map_to_real_commands() {
        assert_eq!(
            i("please back up your database"),
            Intent::Run(Command::Backup)
        );
        assert_eq!(i("Are you OK?"), Intent::Run(Command::Health));
        assert_eq!(
            i("what happened lately, last 5"),
            Intent::Run(Command::Log(5))
        );
        assert_eq!(i("which version are you"), Intent::Run(Command::Version));
        assert_eq!(
            i("identify this movie"),
            Intent::Run(Command::Action("tag".into(), String::new()))
        );
        assert_eq!(i("sing me a song"), Intent::Unknown);
        assert_eq!(
            i("backup the log"),
            Intent::Ambiguous(vec!["backup".into(), "log".into()])
        );
    }

    #[test]
    fn synonyms_come_from_config() {
        let mut s = BTreeMap::new();
        s.insert("backup".to_string(), vec!["stash".to_string()]);
        assert_eq!(interpret("stash it", &[], &s), Intent::Run(Command::Backup));
        assert_eq!(
            interpret("stash it", &[], &BTreeMap::new()),
            Intent::Unknown
        );
    }
}
