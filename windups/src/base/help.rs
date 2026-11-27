//! `-h/--help` for every windup: usage, my commands, and what I can and can't do (it absorbs the
//! old `--capabilities`). `-h -j` gives the same as JSON (windups_base_spec §1).

/// `None` when `args` is not a help request; `Some(json)` when it is (`-j`/`--json`, or `-w`).
pub fn requested(args: &[String]) -> Option<bool> {
    let has = |names: &[&str]| args.iter().any(|a| names.contains(&a.as_str()));
    has(&["-h", "--help"]).then(|| has(&["-j", "--json", "-w", "--windup"]))
}

/// The help as JSON: the profile's fields, plus `v`, `version` and `commands` (name, does).
pub fn json(profile: &str, version: &str, commands: &[(&str, &str)]) -> String {
    let mut v = toml::from_str::<toml::Table>(profile)
        .ok()
        .and_then(|t| serde_json::to_value(t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    v["v"] = 1.into();
    v["version"] = version.into();
    v["commands"] = commands
        .iter()
        .map(|(name, does)| serde_json::json!({"name": name, "does": does}))
        .collect();
    v.to_string()
}

/// The help for a person: who I am, how to call me, my commands, what I can and can't do.
pub fn human(profile: &str, version: &str, usage: &str, commands: &[(&str, &str)]) -> String {
    let t = toml::from_str::<toml::Table>(profile).unwrap_or_default();
    let s = |k: &str| t.get(k).and_then(|v| v.as_str()).unwrap_or("");
    let list = |k: &str| -> Vec<&str> {
        t.get(k)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_default()
    };
    let mut o = format!(
        "{} {version}: {}\n\nUsage: {usage}\n",
        s("name"),
        s("summary")
    );
    if commands.is_empty() {
        o.push_str("\nCommands: none yet\n");
    } else {
        o.push_str("\nCommands:\n");
        let width = commands.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
        for (name, does) in commands {
            o.push_str(&format!("  {name:width$}  {does}\n"));
        }
    }
    o.push_str(&format!("\nI can: {}\n", s("purpose")));
    if s("status") == "stub" {
        o.push_str("I can't yet: anything beyond this help; I'm a stub\n");
    }
    for (key, label) in [
        ("reads", "I read"),
        ("writes", "I write"),
        ("never", "I never"),
    ] {
        let items = list(key);
        if !items.is_empty() {
            o.push_str(&format!("{label}: {}\n", items.join("; ")));
        }
    }
    o
}

/// The whole `main` of a stub windup: `-h` (with `-j`), `-V`, or a plain refusal (exit 2).
pub fn stub_main(version: &str, profile: &str, args: &[String]) -> i32 {
    let name = toml::from_str::<toml::Table>(profile)
        .ok()
        .and_then(|t| t.get("name").and_then(|v| v.as_str()).map(str::to_string))
        .unwrap_or_default();
    if let Some(as_json) = requested(args) {
        if as_json {
            println!("{}", json(profile, version, &[]));
        } else {
            print!(
                "{}",
                human(profile, version, &format!("{name} -h [-j] | -V"), &[])
            );
        }
        return 0;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("{name} {version}");
        return 0;
    }
    eprintln!("{name}: I'm a stub, so I only answer -h/--help (add -j for JSON) and -V/--version");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "name = \"beth\"\nsummary = \"Beth trains LoRAs.\"\npurpose = \"Train LoRAs.\"\nstatus = \"stub\"\nsources = [\"phrases\"]\n";

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn help_is_requested_with_or_without_json() {
        assert_eq!(requested(&args(&["-h"])), Some(false));
        assert_eq!(requested(&args(&["--help", "-j"])), Some(true));
        assert_eq!(requested(&args(&["-w", "-h"])), Some(true));
        assert_eq!(requested(&args(&["status"])), None);
    }

    #[test]
    fn help_json_holds_the_profile_version_and_commands() {
        let v: serde_json::Value =
            serde_json::from_str(&json(PROFILE, "0.1.0", &[("status", "how I am")])).unwrap();
        assert_eq!(v["v"], 1);
        assert_eq!(v["name"], "beth");
        assert_eq!(v["version"], "0.1.0");
        assert_eq!(v["sources"][0], "phrases");
        assert_eq!(v["commands"][0]["name"], "status");
    }

    #[test]
    fn human_help_says_what_a_stub_can_and_cannot_do() {
        let h = human(PROFILE, "0.1.0", "beth -h [-j] | -V", &[]);
        assert!(h.starts_with("beth 0.1.0: Beth trains LoRAs."), "{h}");
        assert!(h.contains("Usage: beth -h [-j] | -V"), "{h}");
        assert!(h.contains("Commands: none yet"), "{h}");
        assert!(h.contains("I can: Train LoRAs."), "{h}");
        assert!(h.contains("I can't yet:"), "{h}");
    }

    #[test]
    fn a_stub_refuses_anything_else_with_exit_2() {
        assert_eq!(stub_main("0.1.0", PROFILE, &args(&["--capabilities"])), 2);
        assert_eq!(stub_main("0.1.0", PROFILE, &args(&["-h"])), 0);
    }
}
