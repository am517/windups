//! Jack's self-description (PROFILE.toml in the repo is the source of truth).
//! Human mode is prose. Windup mode (`-w`) is JSON: reasons sit in their own fields,
//! never inside a value.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

fn doc() -> toml::Value {
    toml::from_str(PROFILE).expect("PROFILE.toml")
}

fn strings(v: &toml::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// (name, can, why). `why` is empty when he can.
fn kinds(v: &toml::Value) -> Vec<(String, bool, String)> {
    let mut out: Vec<_> = strings(v, "kinds")
        .into_iter()
        .map(|n| (n, true, String::new()))
        .collect();
    if let Some(arr) = v.get("cant").and_then(|c| c.as_array()) {
        for c in arr {
            let name = c
                .get("kind")
                .and_then(|k| k.as_str())
                .unwrap_or("")
                .to_string();
            let why = c
                .get("why")
                .and_then(|k| k.as_str())
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                out.push((name, false, why));
            }
        }
    }
    out
}

fn line(items: &[String]) -> String {
    items
        .iter()
        .map(|s| format!("  {s}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Pretty text for a person. Says what he can't do, and what `writes` is for.
pub fn human() -> String {
    let v = doc();
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut kinds_txt = String::new();
    for (name, can, why) in kinds(&v) {
        if can {
            kinds_txt.push_str(&format!("  {name}\n"));
        } else {
            kinds_txt.push_str(&format!("  {name} — can't: {why}\n"));
        }
    }
    format!(
        "\
{name} {version} — {role}
{summary}

Purpose
  {purpose}

Kinds
{kinds}
Sources
{sources}

Reads
{reads}

Writes
  What Jack changes. This is not a list of another windup's tables; Daniel stores the shared catalog.
  jack.db — his own archive
  handoff records — one record per item, synced to Daniel
  file tags — until Lauren takes tagging
  sorted folders — until Sally takes sorting

Never
{never}

Hands off
{hands}

Command
  {command}
",
        name = s("name"),
        version = env!("CARGO_PKG_VERSION"),
        role = s("role"),
        summary = s("summary"),
        purpose = s("purpose"),
        kinds = kinds_txt,
        sources = line(&strings(&v, "sources")),
        reads = line(&strings(&v, "reads")),
        never = line(&strings(&v, "never")),
        hands = line(&strings(&v, "hands_off")),
        command = s("command"),
    )
}

/// Structured capabilities for another windup. No commentary inside the values.
pub fn machine() -> String {
    let v = doc();
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("");
    let kind_vals: Vec<serde_json::Value> = kinds(&v)
        .into_iter()
        .map(|(name, can, why)| {
            if can {
                serde_json::json!({"name": name, "can": true})
            } else {
                serde_json::json!({"name": name, "can": false, "why": why})
            }
        })
        .collect();
    let checks = v
        .get("success_checks")
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .map(|c| {
                    serde_json::json!({
                        "check": c.get("check").and_then(|x| x.as_str()).unwrap_or(""),
                        "test": c.get("test").and_then(|x| x.as_str()).unwrap_or(""),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    serde_json::json!({
        "v": 1,
        "name": s("name"),
        "version": env!("CARGO_PKG_VERSION"),
        "role": s("role"),
        "summary": s("summary"),
        "purpose": s("purpose"),
        "personality": s("personality"),
        "status": s("status"),
        "kinds": kind_vals,
        "sources": strings(&v, "sources"),
        "reads": strings(&v, "reads"),
        "writes": strings(&v, "writes"),
        "never": strings(&v, "never"),
        "hands_off": strings(&v, "hands_off"),
        "command": s("command"),
        "success_checks": checks,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_parses_and_names_jack() {
        let v: toml::Value = toml::from_str(PROFILE).unwrap();
        assert_eq!(v["name"].as_str(), Some("jack"));
        assert!(v.get("later_kinds").is_none());
    }

    #[test]
    fn human_merges_kinds_and_explains_writes() {
        let h = human();
        assert!(h.contains("movie"), "{h}");
        assert!(h.contains("episode"), "{h}");
        assert!(h.contains("online_video"), "{h}");
        assert!(
            h.contains("book — can't: I don't know book metadata yet"),
            "{h}"
        );
        assert!(
            h.contains("document — can't: I don't know document metadata yet"),
            "{h}"
        );
        assert!(
            h.contains("What Jack changes"),
            "writes should say what the section is for: {h}"
        );
        assert!(h.contains("jack.db"), "{h}");
        assert!(
            !h.contains("media.db via sync"),
            "writes is not Daniel's schema: {h}"
        );
        let book = h.find("book").unwrap();
        let movie = h.find("movie").unwrap();
        assert!(movie < book, "can't-do kinds sit in the same list");
    }

    #[test]
    fn machine_is_json_with_reasons_beside_the_values() {
        let raw = machine();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["name"], "jack");
        assert_eq!(v["v"], 1);
        let kinds = v["kinds"].as_array().unwrap();
        assert!(
            kinds
                .iter()
                .any(|k| k["name"] == "movie" && k["can"] == true)
        );
        let book = kinds.iter().find(|k| k["name"] == "book").unwrap();
        assert_eq!(book["can"], false);
        assert_eq!(book["why"], "I don't know book metadata yet");
        assert!(!book["name"].as_str().unwrap().contains("can't"));
        for w in v["writes"].as_array().unwrap() {
            let s = w.as_str().unwrap();
            assert!(!s.contains('(') && !s.contains("Daniel"), "{s}");
        }
        let sources = v["sources"].as_array().unwrap_or_else(|| panic!("{v}"));
        assert!(
            sources.iter().any(|s| s.as_str() == Some("tmdb")),
            "{sources:?}"
        );
        assert!(!raw.contains("later_kinds"));
    }
}
