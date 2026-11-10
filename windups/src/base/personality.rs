//! Personalities, Inside Out style: each windup has one trait (a word or short phrase) that shapes
//! how it does its job: style and priorities, never an exception to the rules in CULTURE.md.
//! The trait lives in the windup's PROFILE.toml (`personality`); `[core] personality` in its
//! config may override it.

/// The prompt line, exactly: "Your name is X and you are <Trait>. Your job is Y."
pub fn prompt_line(name: &str, personality: &str, job: &str) -> String {
    let job = job.trim().trim_end_matches('.');
    format!("Your name is {name} and you are {personality}. Your job is {job}.")
}

/// The trait to use: the config override if set, else the profile's.
pub fn choose<'a>(config_override: &'a str, profile: &'a str) -> &'a str {
    let o = config_override.trim();
    if o.is_empty() { profile.trim() } else { o }
}

/// (personality, personality_status) from a PROFILE.toml's text.
pub fn from_profile(profile: &str) -> Option<(String, String)> {
    let v: toml::Table = toml::from_str(profile).ok()?;
    let p = v.get("personality")?.as_str()?.trim().to_string();
    let s = v
        .get("personality_status")
        .and_then(|s| s.as_str())
        .unwrap_or("proposed")
        .to_string();
    (!p.is_empty()).then_some((p, s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_is_exact() {
        assert_eq!(
            prompt_line("Daniel", "Meticulous", "keeping the shared catalog."),
            "Your name is Daniel and you are Meticulous. Your job is keeping the shared catalog."
        );
        assert_eq!(
            prompt_line("Walter", "Data Fiend", "auditing every windup's records"),
            "Your name is Walter and you are Data Fiend. Your job is auditing every windup's records."
        );
    }

    #[test]
    fn config_overrides_the_profile() {
        assert_eq!(choose("", "Curious"), "Curious");
        assert_eq!(choose(" Bold ", "Curious"), "Bold");
        let p = "name = \"x\"\npersonality = \"Curious\"\npersonality_status = \"proposed\"\n";
        assert_eq!(from_profile(p), Some(("Curious".into(), "proposed".into())));
        assert_eq!(from_profile("name = \"x\"\n"), None);
    }
}
