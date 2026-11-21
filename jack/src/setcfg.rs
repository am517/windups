//! `--set`: write flag values into the main config as new defaults, keeping comments/formatting.
use std::fmt::Write;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Value};

/// One setting to save: dotted config key, new value, and whether it is a secret (masked in output).
pub struct Change {
    pub key: &'static str,
    pub value: Value,
    pub secret: bool,
}

fn shown(v: &str) -> String {
    let t = v.trim();
    if t.is_empty() || t == "\"\"" {
        "(unset)".into()
    } else {
        t.into()
    }
}

/// Applies `changes` to the config at `path` (created from `template` if missing). Returns a report.
pub fn save(path: &Path, template: &str, changes: &[Change]) -> Result<String, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => template.to_string(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = String::new();
    let mut changed = false;
    for c in changes {
        let parts: Vec<&str> = c.key.split('.').collect();
        let (last, tables) = parts.split_last().unwrap();
        let mut t = doc.as_table_mut();
        for p in tables {
            if !t.contains_key(p) {
                t.insert(p, Item::Table(toml_edit::Table::new()));
            }
            t = t[*p].as_table_mut().ok_or(format!(
                "{}: '{}' is not a table",
                path.display(),
                c.key
            ))?;
        }
        let old = t
            .get(last)
            .and_then(|i| i.as_value())
            .map(|v| v.clone().decorated("", "").to_string());
        let new = c.value.clone().decorated("", "").to_string();
        if old.as_deref() == Some(new.as_str()) {
            let _ = writeln!(out, "{}: unchanged", c.key);
            continue;
        }
        let mut v = c.value.clone();
        match t.get_mut(last) {
            // replace in place: keeps the key's leading comments and the value's trailing comment
            Some(Item::Value(o)) => {
                *v.decor_mut() = o.decor().clone();
                *o = v;
            }
            _ => {
                t.insert(last, Item::Value(v));
            }
        }
        changed = true;
        if c.secret {
            // saved / replaced, never the value
            let word = match &old {
                None => "saved",
                Some(o) if shown(o) == "(unset)" => "saved",
                Some(_) => "replaced",
            };
            let _ = writeln!(out, "{}: {word}", c.key);
        } else {
            let _ = writeln!(
                out,
                "{}: {} -> {}",
                c.key,
                old.map_or_else(|| "(unset)".into(), |o| shown(&o)),
                shown(&new)
            );
        }
    }
    if changed {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(path, doc.to_string()).map_err(|e| format!("{}: {e}", path.display()))?;
        let _ = writeln!(out, "saved {}", path.display());
    } else {
        let _ = writeln!(out, "no changes to {}", path.display());
    }
    Ok(out)
}
