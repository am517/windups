//! The `[core]` table every windup's config.toml may carry, and showing a config safely.
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct BaseCfg {
    /// Overrides the PROFILE.toml personality; empty = the profile's.
    pub personality: String,
    pub events: super::events::EventsCfg,
    pub snapshots: super::snapshot::SnapshotCfg,
    pub talk: TalkCfg,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct TalkCfg {
    /// Extra words per command or action, e.g. `backup = ["stash"]`.
    pub synonyms: BTreeMap<String, Vec<String>>,
}

impl BaseCfg {
    /// Reads `[core]` from a whole config file's text (missing table = defaults).
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let v: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        match v.get("core") {
            Some(c) => c.clone().try_into().map_err(|e| format!("[core]: {e}")),
            None => Ok(Self::default()),
        }
    }
}

/// Key names whose values are never shown.
const SECRET_WORDS: [&str; 6] = ["key", "token", "pin", "secret", "password", "pass"];

/// The config text with secret values replaced, for the `config` command.
pub fn redacted(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let t = line.trim_start();
        let hidden = match t.split_once('=') {
            Some((k, v)) if !t.starts_with('#') => {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim();
                SECRET_WORDS.iter().any(|w| k.contains(w)) && v != "\"\"" && v != "''"
            }
            _ => false,
        };
        if hidden {
            let k = t.split_once('=').map_or("", |(k, _)| k.trim_end());
            out.push_str(&line[..line.len() - t.len()]);
            out.push_str(k);
            out.push_str(" = \"<hidden>\"");
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_table_loads_with_generous_defaults() {
        let c = BaseCfg::from_toml("x = 1\n").unwrap();
        assert_eq!(c, BaseCfg::default());
        assert!(c.events.keep_full >= 10_000 && c.snapshots.keep >= 7);
        let c = BaseCfg::from_toml(
            "[core.events]\nkeep_full = 5\n[core.talk.synonyms]\nbackup = [\"stash\"]\n",
        )
        .unwrap();
        assert_eq!(c.events.keep_full, 5);
        assert_eq!(
            c.events.batch_size,
            super::super::events::EventsCfg::default().batch_size
        );
        assert_eq!(c.talk.synonyms["backup"], ["stash"]);
    }

    #[test]
    fn secrets_are_hidden_everything_else_shown() {
        let t = "[providers.tmdb]\nread_token = \"abc123\"\napi_key = \"\"\n  pin = 'p'\n# api_key = \"doc\"\nbase_url = \"http://t\"\n";
        let r = redacted(t);
        assert!(!r.contains("abc123") && !r.contains("'p'"), "{r}");
        assert!(
            r.contains("read_token = \"<hidden>\"") && r.contains("  pin = \"<hidden>\""),
            "{r}"
        );
        assert!(
            r.contains("api_key = \"\"") && r.contains("base_url = \"http://t\""),
            "{r}"
        );
        assert!(r.contains("# api_key = \"doc\""), "comments stay");
    }
}
