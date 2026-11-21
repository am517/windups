//! `--set`: saving flags into the main config.
use crate::cli::*;
use crate::config::{self, Settings};
use crate::credentials::{self, Accepted};
use crate::setcfg;
use std::fmt::Write as _;
use std::path::PathBuf;

pub(crate) fn main_config_path(cli: &Cli) -> Result<PathBuf, String> {
    match &cli.config {
        Some(p) => Ok(config::expand(p)),
        None if std::env::var_os("HOME").is_none() => Err("HOME not set".into()),
        None => Ok(config::layout(None).config),
    }
}

/// `--set`: save option flags into the main config; per-file identifiers are skipped with a warning.
pub(crate) fn save_flags(cli: &Cli) -> Result<String, String> {
    use setcfg::Change;
    use toml_edit::Value;
    let mut o = String::new();
    for (f, set) in [
        ("--imdb", cli.imdb.is_some()),
        ("--tmdb", cli.tmdb.is_some()),
        ("--thetvdb", cli.thetvdb.is_some()),
        ("--youtube", cli.youtube.is_some()),
        ("--x", cli.x.is_some()),
        ("--wikidata", cli.wikidata.is_some()),
        ("--wikipedia", cli.wikipedia.is_some()),
        ("--name", cli.name.is_some()),
        ("--year", cli.year.is_some()),
    ] {
        if set {
            let _ = writeln!(o, "warning: {f} is per-file; not saved");
        }
    }
    let s = |k, v: &str| Change {
        key: k,
        value: Value::from(v),
        secret: false,
    };
    let mut ch = vec![];
    if let Some(v) = &cli.providers {
        ch.push(Change {
            key: "providers.order",
            value: Value::Array(v.iter().collect()),
            secret: false,
        });
    }
    if let Some(v) = &Settings::from(cli).kind {
        ch.push(s("type", v));
    }
    if let Some(v) = &cli.print_format {
        ch.push(s("print_format", v));
    }
    let f = Settings::from(cli);
    for (k, v) in [
        ("dry_run", f.dry_run),
        ("print", f.print),
        ("recursive", f.recursive),
    ] {
        if let Some(v) = v {
            ch.push(Change {
                key: k,
                value: Value::from(v),
                secret: false,
            });
        }
    }
    let (accepted, rejected) = credentials::check(cli);
    let saved_labels: Vec<&str> = accepted.iter().map(|key| key.label).collect();
    let said = credentials::sentence(&saved_labels, &rejected);
    if !said.is_empty() {
        let _ = writeln!(o, "{said}");
    }
    for key in &accepted {
        ch.push(secret_change(key));
        if let Some(clear) = key.clear_key {
            ch.push(Change {
                key: clear,
                value: Value::from(""),
                secret: true,
            });
        }
    }
    if ch.is_empty() {
        let _ = writeln!(o, "--set: no option flags to save");
        return Ok(o);
    }
    o += &setcfg::save(&main_config_path(cli)?, config::DEFAULT_CONFIG, &ch)?;
    Ok(o)
}

fn secret_change(key: &Accepted) -> setcfg::Change {
    setcfg::Change {
        key: key.config_key,
        value: toml_edit::Value::from(key.value.as_str()),
        secret: true,
    }
}

/// How many key flags were rejected. The good ones are still saved.
pub(crate) fn rejected_keys(cli: &Cli) -> usize {
    credentials::check(cli).1.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn set_saves_flags_keeps_comments_and_is_idempotent() {
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("sub/c.toml");
        let cs = c.to_str().unwrap();
        // missing file: created from the template; identifiers skipped with a warning
        let (o, _, _) = run(
            &cli(&[
                "--set",
                "--config",
                cs,
                "--providers",
                "tmdb,omdb",
                "--imdb",
                "tt1",
                "--name",
                "X",
                "--tv",
            ]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap();
        assert!(
            o.contains("warning: --imdb is per-file; not saved")
                && o.contains("warning: --name is per-file"),
            "{o}"
        );
        assert!(o.contains("providers.order:"), "{o}");
        assert!(!o.contains("media file"), "save-only must not run: {o}");
        let t = fs::read_to_string(&c).unwrap();
        assert!(
            t.contains("order = [\"tmdb\", \"omdb\"]") && !t.contains("tt1"),
            "{t}"
        );
        let cfg = config::load(Some(&c)).unwrap();
        assert_eq!(cfg.base.kind.as_deref(), Some("tv"));
        // idempotent
        let (o, _, _) = run(
            &cli(&["--set", "--config", cs, "--tv"]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap();
        assert!(
            o.contains("type: unchanged") && o.contains("no changes"),
            "{o}"
        );
        assert_eq!(fs::read_to_string(&c).unwrap(), t);
        // save + run
        let m = d.path().join("a.mp4");
        fs::write(&m, "").unwrap();
        let (o, _, _) = run(
            &cli(&["--set", "--config", cs, "--dry-run", m.to_str().unwrap()]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap();
        assert!(
            o.contains("dry_run: false -> true") && o.contains("1 media file(s) found"),
            "{o}"
        );
    }

    #[test]
    fn a_bad_key_is_named_and_the_good_keys_are_saved() {
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        fs::write(&c, "[providers]\norder = [\"omdb\"]\n").unwrap();
        let cs = c.to_str().unwrap();
        let http = providers::fake::FakeHttp::new(vec![]);
        let (out, _, failed) = run(
            &cli(&[
                "--set",
                "--config",
                cs,
                "--tvdb-key",
                "SECRETVALUE1",
                "--tmdb-key",
                "not-a-key",
                "--omdb-key",
                "OMDBVALUE1",
            ]),
            &http,
        )
        .unwrap();
        assert_eq!(failed, 1, "{out}");
        assert!(
            out.contains(
                "I saved the TheTVDB key and the OMDb key, but the TMDB key didn't work because"
            ),
            "{out}"
        );
        for secret in ["SECRETVALUE1", "OMDBVALUE1", "not-a-key"] {
            assert!(!out.contains(secret), "echoed {secret} in {out}");
        }
        let saved = fs::read_to_string(&c).unwrap();
        assert!(saved.contains("SECRETVALUE1"), "{saved}");
        assert!(saved.contains("OMDBVALUE1"), "{saved}");
        assert!(!saved.contains("not-a-key"), "{saved}");

        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIn0.signature";
        let (out, _, failed) =
            run(&cli(&["--set", "--config", cs, "--tmdb-token", jwt]), &http).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(out.contains("I saved the TMDB key."), "{out}");
        assert!(!out.contains(jwt), "{out}");
        let saved = fs::read_to_string(&c).unwrap();
        assert!(saved.contains("read_token"), "{saved}");
        assert!(saved.contains(jwt), "{saved}");

        let v3 = "0123456789abcdef0123456789abcdef";
        let (out, _, failed) =
            run(&cli(&["--set", "--config", cs, "--tmdb-key", v3]), &http).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(!out.contains(v3), "{out}");
        let saved = fs::read_to_string(&c).unwrap();
        assert!(saved.contains(&format!("api_key = \"{v3}\"")), "{saved}");
        assert!(
            saved.contains("read_token = \"\""),
            "the other TMDB credential is cleared: {saved}"
        );
    }
}
