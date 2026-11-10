//! `--set`: saving flags into the main config.
use crate::config::{self, Settings};
use crate::setcfg;
use crate::{cli::*, secrets::*};
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
    if let Some(v) = &cli.output {
        ch.push(s("output", &v.to_string_lossy()));
    }
    if let Some(v) = &cli.art {
        ch.push(s("tagger.art_style", v));
    }
    if let Some(v) = &cli.mp4_tool {
        ch.push(s("tagger.mp4_tool", v));
    }
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
    for (k, v) in [
        ("naming.file_language", &cli.file_language),
        ("naming.folder_language", &cli.folder_language),
    ] {
        if let Some(v) = v {
            ch.push(s(k, v));
        }
    }
    if let Some(v) = &cli.grouping {
        let _ = writeln!(
            o,
            "warning: --grouping is usually per-file; saved as the default for all files"
        );
        ch.push(s("grouping", v));
    }
    if let Some(v) = &cli.print_format {
        ch.push(s("print_format", v));
    }
    let f = Settings::from(cli);
    for (k, v) in [
        ("dry_run", f.dry_run),
        ("download_only", f.download_only),
        ("print", f.print),
        ("recursive", f.recursive),
        (
            "tagger.embed_artwork",
            tri(cli.embed_artwork, cli.no_embed_artwork),
        ),
        ("tagger.ask_art", tri(cli.ask_art, cli.no_ask_art)),
    ] {
        if let Some(v) = v {
            ch.push(Change {
                key: k,
                value: Value::from(v),
                secret: false,
            });
        }
    }
    for (_, k, v) in secret_values(cli) {
        ch.push(Change {
            key: k,
            value: Value::from(v),
            secret: true,
        });
    }
    if ch.is_empty() {
        let _ = writeln!(o, "--set: no option flags to save");
        return Ok(o);
    }
    o += &setcfg::save(&main_config_path(cli)?, config::DEFAULT_CONFIG, &ch)?;
    Ok(o)
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
                "--art",
                "landscape",
                "-o",
                "/out",
                "--providers",
                "tmdb,omdb",
                "--mp4-tool",
                "exiftool",
                "--imdb",
                "tt1",
                "--name",
                "X",
            ]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap();
        assert!(
            o.contains("warning: --imdb is per-file; not saved")
                && o.contains("warning: --name is per-file"),
            "{o}"
        );
        assert!(
            o.contains("tagger.art_style: \"poster\" -> \"landscape\"")
                && o.contains("output: (unset) -> \"/out\""),
            "{o}"
        );
        assert!(!o.contains("media file"), "save-only must not run: {o}");
        let t = fs::read_to_string(&c).unwrap();
        assert!(
            t.contains("# \"poster\" or \"landscape\".") && t.contains("art_style = \"landscape\"")
        );
        assert!(t.contains("order = [\"tmdb\", \"omdb\"]") && !t.contains("tt1"));
        let cfg = config::load(Some(&c)).unwrap();
        assert_eq!(
            (cfg.tagger.mp4_tool.as_str(), cfg.base.output.as_deref()),
            ("exiftool", Some(Path::new("/out")))
        );
        // idempotent
        let (o, _, _) = run(
            &cli(&["--set", "--config", cs, "--art", "landscape"]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap();
        assert!(
            o.contains("tagger.art_style: unchanged") && o.contains("no changes"),
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
}
