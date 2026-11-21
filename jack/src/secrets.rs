//! Credential flags: prompting and collecting them.
use crate::cli::*;

/// (flag, config key) for each credential flag.
pub(crate) const SECRETS: [(&str, &str); 4] = [
    ("--tmdb-token", "providers.tmdb.read_token"),
    ("--tvdb-key", "providers.thetvdb.api_key"),
    ("--tvdb-pin", "providers.thetvdb.pin"),
    ("--omdb-key", "providers.omdb.api_key"),
];

pub(crate) fn secret_fields(cli: &mut Cli) -> [&mut Option<Option<String>>; 4] {
    [
        &mut cli.tmdb_token,
        &mut cli.tvdb_key,
        &mut cli.tvdb_pin,
        &mut cli.omdb_key,
    ]
}

/// Fills value-less credential flags via `read` (hidden prompt in `main`).
pub(crate) fn resolve_secrets(
    cli: &mut Cli,
    read: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<(), String> {
    for (f, (flag, _)) in secret_fields(cli).into_iter().zip(SECRETS) {
        if let Some(None) = f {
            let v = read(&format!("{flag}: "))?.trim().to_string();
            if v.is_empty() {
                return Err(format!("{flag}: empty value"));
            }
            *f = Some(Some(v));
        }
    }
    Ok(())
}

/// Hidden prompt on a terminal; otherwise one line from stdin (e.g. `echo $KEY | jack --set --omdb-key`).
pub(crate) fn read_secret(prompt: &str) -> Result<String, String> {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        rpassword::prompt_password(prompt).map_err(|e| format!("reading key: {e}"))
    } else {
        let mut s = String::new();
        std::io::stdin()
            .read_line(&mut s)
            .map_err(|e| format!("reading key from stdin: {e}"))?;
        Ok(s)
    }
}

/// True when a credential flag was given a value (space or `=` form).
pub(crate) fn has_secret(cli: &Cli) -> bool {
    !secret_values(cli).is_empty()
}

pub(crate) fn secret_values(cli: &Cli) -> Vec<(&'static str, &'static str, String)> {
    [&cli.tmdb_token, &cli.tvdb_key, &cli.tvdb_pin, &cli.omdb_key]
        .into_iter()
        .zip(SECRETS)
        .filter_map(|(v, (f, k))| v.clone().flatten().map(|v| (f, k, v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn credential_flags_run_only_set_masked_and_prompted() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        fs::write(&c, "# my keys\n[providers]\norder = [\"omdb\"]\n[providers.omdb]\napi_key = \"CFGKEY1234\" # keep me\nbase_url = \"http://o/\"\n").unwrap();
        let m = d.path().join("Dune (1984).mp4");
        fs::write(&m, "").unwrap();
        let (cs, ms) = (c.to_str().unwrap(), m.to_str().unwrap());
        let h = providers::fake::FakeHttp::new(vec![(
            "http://o/",
            json!({"Title":"Dune","Year":"1984"}),
        )]);
        // run-only: flag beats config; config untouched; key never printed
        let (o, _, _) = run(
            &cli(&["--config", cs, "--dry-run", "--omdb-key=FLAGKEY5678", ms]),
            &h,
        )
        .unwrap();
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|u| u.contains("apikey=FLAGKEY5678"))
        );
        assert!(
            !o.contains("FLAGKEY5678") && !o.contains("CFGKEY1234"),
            "{o}"
        );
        assert!(fs::read_to_string(&c).unwrap().contains("CFGKEY1234"));
        // value-less flag prompts through the reader; --set saves masked
        let mut a = cli(&["--set", "--config", cs, "--omdb-key", "--tvdb-pin=9999"]);
        let mut asked = vec![];
        resolve_secrets(&mut a, &mut |p: &str| {
            asked.push(p.to_string());
            Ok("NEWKEY0001\n".into())
        })
        .unwrap();
        assert_eq!(asked, ["--omdb-key: "]);
        let (o, _, _) = run(&a, &h).unwrap();
        assert!(
            o.contains("providers.omdb.api_key: replaced")
                && o.contains("providers.thetvdb.pin: saved"),
            "{o}"
        );
        assert!(
            !o.contains("NEWKEY0001") && !o.contains("CFGKEY1234") && !o.contains("9999"),
            "{o}"
        );
        let t = fs::read_to_string(&c).unwrap();
        assert!(
            t.contains("api_key = \"NEWKEY0001\" # keep me")
                && t.starts_with("# my keys")
                && t.contains("pin = \"9999\""),
            "{t}"
        );
        let mut e = cli(&["--omdb-key"]);
        assert!(resolve_secrets(&mut e, &mut |_: &str| Ok(" \n".into())).is_err());
    }

    #[test]
    fn space_separated_keys_save_without_a_prompt_a_scan_or_an_echo() {
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        fs::write(&c, "[providers]\norder = [\"omdb\"]\n").unwrap();
        let cs = c.to_str().unwrap();
        let http = providers::fake::FakeHttp::new(vec![]);
        let mut asked = false;
        let mut a = cli(&[
            "--set",
            "--config",
            cs,
            "--tvdb-key",
            "SECRETVALUE1",
            "--tmdb-token",
            "TOKVALUE1",
            "--tvdb-pin",
            "1234",
            "--omdb-key",
            "OMDBVALUE1",
        ]);
        assert!(
            a.paths.is_empty(),
            "the values are not paths: {:?}",
            a.paths
        );
        resolve_secrets(&mut a, &mut |_| {
            asked = true;
            Ok("should-not-prompt".into())
        })
        .unwrap();
        assert!(!asked, "a value after a space is not a prompt");
        let (o, _, failed) = run(&a, &http).unwrap();
        assert_eq!(failed, 0, "{o}");
        assert!(
            !o.contains("media file"),
            "setting a key does not scan: {o}"
        );
        for secret in ["SECRETVALUE1", "TOKVALUE1", "1234", "OMDBVALUE1"] {
            assert!(!o.contains(secret), "echoed {secret} in {o}");
        }
        assert!(o.contains("providers.thetvdb.api_key: saved"), "{o}");
        assert!(o.contains("providers.tmdb.read_token: saved"), "{o}");
        assert!(o.contains("providers.thetvdb.pin: saved"), "{o}");
        assert!(o.contains("providers.omdb.api_key: saved"), "{o}");
        let saved = fs::read_to_string(&c).unwrap();
        assert!(saved.contains("SECRETVALUE1"), "{saved}");

        // equals form still works, and the same value is unchanged
        let mut b = cli(&["--set", "--config", cs, "--tvdb-key=SECRETVALUE1"]);
        resolve_secrets(&mut b, &mut |_| {
            asked = true;
            Ok("no".into())
        })
        .unwrap();
        let (o, _, _) = run(&b, &http).unwrap();
        assert!(o.contains("providers.thetvdb.api_key: unchanged"), "{o}");
        assert!(!o.contains("SECRETVALUE1"), "{o}");
        assert!(!o.contains("media file"), "{o}");

        // a different value is replaced, still without the value
        let mut r = cli(&["--set", "--config", cs, "--tvdb-key", "SECRETVALUE2"]);
        resolve_secrets(&mut r, &mut |_| Ok("no".into())).unwrap();
        let (o, _, _) = run(&r, &http).unwrap();
        assert!(o.contains("providers.thetvdb.api_key: replaced"), "{o}");
        assert!(
            !o.contains("SECRETVALUE1") && !o.contains("SECRETVALUE2"),
            "{o}"
        );

        // other value flags take a space, and a key plus a path still does not scan
        let other = cli(&[
            "--name",
            "Dune",
            "--year",
            "1984",
            "--thetvdb",
            "27498",
            "--output",
            "/tmp/out",
            "--set",
            "--config",
            cs,
            "--tvdb-key",
            "SECRETVALUE2",
            "Bear Scratch/Mountain.mp4",
        ]);
        assert_eq!(other.name.as_deref(), Some("Dune"));
        assert_eq!(other.year.as_deref(), Some("1984"));
        assert_eq!(other.thetvdb.as_deref(), Some("27498"));
        assert_eq!(other.paths.len(), 1);
        let (o, _, _) = run(&other, &http).unwrap();
        assert!(o.contains("not scanning"), "{o}");
        assert!(!o.contains("media file"), "{o}");
        assert!(!o.contains("SECRETVALUE2"), "{o}");
    }
}
