//! Capabilities: windups that answer `--capabilities`, and the sources their profiles name.
//! Anything not seen before is recorded and wakes what waits for it; a new source also wakes
//! everything waiting for "new_source".
use crate::caller::Caller;
use rusqlite::{Connection, params};

/// (version, sources) from a `--capabilities` answer: JSON from `-w`, or TOML from a stub.
fn profile_fields(profile: &str) -> (String, Vec<String>) {
    let from_json = serde_json::from_str::<serde_json::Value>(profile).ok();
    if let Some(v) = from_json.filter(|v| v.is_object()) {
        return (field_str(&v, "version"), field_list(&v, "sources"));
    }
    if let Ok(t) = toml::from_str::<toml::Value>(profile) {
        return (field_str(&t, "version"), field_list(&t, "sources"));
    }
    (String::new(), Vec::new())
}

fn field_str(v: &impl SourceValue, key: &str) -> String {
    v.string(key).unwrap_or_default()
}

fn field_list(v: &impl SourceValue, key: &str) -> Vec<String> {
    v.list(key)
}

trait SourceValue {
    fn string(&self, key: &str) -> Option<String>;
    fn list(&self, key: &str) -> Vec<String>;
}

impl SourceValue for serde_json::Value {
    fn string(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str().map(str::to_string))
    }
    fn list(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl SourceValue for toml::Value {
    fn string(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str().map(str::to_string))
    }
    fn list(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Registers answering windups; returns the capabilities that woke something.
pub fn discover(
    c: &Connection,
    cfg: &crate::config::Config,
    caller: &dyn Caller,
    extra: &[String],
) -> rusqlite::Result<Vec<String>> {
    for (name, cmd) in &cfg.windups {
        if let Some(p) = caller.capabilities(cmd) {
            let (version, _) = profile_fields(&p);
            windups::registry::register(c, name, &version, &p)?;
        }
    }
    let mut caps: Vec<(String, &str)> = vec![];
    for (name, profile) in windups::registry::all(c)? {
        for s in profile_fields(&profile).1 {
            caps.push((s, "source"));
        }
        caps.push((name, "windup"));
    }
    caps.extend(extra.iter().map(|e| (e.clone(), "named")));
    let mut woke = vec![];
    for (name, kind) in caps {
        let new = kind == "named"
            || c.execute(
                "INSERT OR IGNORE INTO capabilities(name, kind) VALUES (?1, ?2)",
                params![name, kind],
            )? > 0;
        if !new {
            continue;
        }
        let mut n = windups::returns::wake(c, &name)?;
        if kind == "source" {
            n += windups::returns::wake(c, "new_source")?;
        }
        if n > 0 {
            woke.push(name);
        }
    }
    Ok(woke)
}

#[cfg(test)]
mod tests {
    use super::profile_fields;

    #[test]
    fn json_and_toml_capabilities_both_name_sources() {
        let (v, s) = profile_fields(
            r#"{"version":"0.2.0","sources":["tmdb","thetvdb"],"kinds":[{"name":"book","can":false,"why":"later"}]}"#,
        );
        assert_eq!(v, "0.2.0");
        assert_eq!(s, ["tmdb", "thetvdb"]);
        let (v, s) = profile_fields("name = \"jack\"\nversion = \"0.1\"\nsources = [\"omdb\"]\n");
        assert_eq!(
            (v.as_str(), s.as_slice()),
            ("0.1", ["omdb".to_string()].as_slice())
        );
    }
}
