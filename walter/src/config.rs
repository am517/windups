//! Walter's settings: built-in defaults (config/walter.toml) overlaid by the user's file.
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_CONFIG: &str = include_str!("../config/walter.toml");

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Retry {
    pub backoff_hours: Vec<i64>,
    pub wait_for: BTreeMap<String, String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Config {
    /// name -> command line
    pub windups: BTreeMap<String, Vec<String>>,
    pub retry: Retry,
    pub audit: Audit,
}

/// `walter audit`: where windup folders live (each `<name>/<name>.db` under a root is audited).
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Audit {
    pub roots: Vec<String>,
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self, String> {
        let mut t: toml::Table = toml::from_str(DEFAULT_CONFIG).map_err(|e| e.to_string())?;
        if let Some(Ok(s)) = path.map(std::fs::read_to_string) {
            let user: toml::Table =
                toml::from_str(&s).map_err(|e| format!("walter config: {e}"))?;
            for (k, v) in user {
                match (t.get_mut(&k), v) {
                    (Some(toml::Value::Table(a)), toml::Value::Table(b)) => a.extend(b),
                    (_, v) => {
                        t.insert(k, v);
                    }
                }
            }
        }
        t.try_into().map_err(|e| format!("walter config: {e}"))
    }
}
