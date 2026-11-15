//! A whole run over all given paths.
use crate::config::{self, Settings};
use crate::providers::{self, Http};
use crate::{cli::*, print::*, process::*, scan::*, secrets::*, setflags::*};
use std::fmt::Write as _;

/// Returns (report text, --print output, number of failed files).
pub(crate) fn run(cli: &Cli, http: &dyn Http) -> Result<(String, String, usize), String> {
    if let (Some(t), Some(f)) = (
        &cli.kind,
        cli.tv.then_some("tv").or(cli.movie.then_some("movie")),
    ) {
        if t != f {
            return Err(format!("--{f} conflicts with --type {t}"));
        }
    }
    let mut pre = String::new();
    if cli.set {
        pre = save_flags(cli)?;
        if cli.paths.is_empty() {
            return Ok((pre, String::new(), 0));
        }
    } else if cli.paths.is_empty() {
        return Err("no files or directories given (see --help)".into());
    }
    let mut cfg = config::load(cli.config.as_deref())?;
    cfg.emit_json = cli.emit_json.clone();
    cfg.windup = cli.windup;
    cfg.sidecar_json = cli.json;
    // discover relationships (assistant / matchmaker / database / llm); isolation if nobody answers
    let found = windups::base::relationships::resolve(
        cfg.relationships.clone(),
        &windups::base::relationships::NoDirectory,
    );
    cfg.relationships = found.rels;
    if !cli.windup {
        for n in &found.notes {
            if n.contains("no relationships configured") {
                continue;
            }
            pre.push_str(&format!("{n}\n"));
        }
    }
    if !found.changed.is_empty() {
        if let Some(p) = cli.config.as_deref() {
            if let Err(e) = config::save_relationships(p, &found.changed) {
                pre.push_str(&format!("warning: relationships: {e}\n"));
            }
        }
    }
    if let Some(p) = &cli.providers {
        cfg.providers.order = p.clone();
    }
    if let Some(t) = &cli.mp4_tool {
        cfg.tagger.mp4_tool = t.clone();
    }
    if let Some(v) = &cli.file_language {
        cfg.naming.file_language = v.clone();
    }
    if let Some(v) = &cli.folder_language {
        cfg.naming.folder_language = v.clone();
    }
    // credential flags beat config and embedded keys (this run)
    for (_, k, v) in secret_values(cli) {
        let p = &mut cfg.providers;
        *match k {
            "providers.tmdb.read_token" => &mut p.tmdb.read_token,
            "providers.thetvdb.api_key" => &mut p.thetvdb.api_key,
            "providers.thetvdb.pin" => &mut p.thetvdb.pin,
            _ => &mut p.omdb.api_key,
        } = v;
    }
    if let Some(v) = tri(cli.embed_artwork, cli.no_embed_artwork) {
        cfg.tagger.embed_artwork = v;
    }
    if let Some(v) = tri(cli.ask_art, cli.no_ask_art) {
        cfg.tagger.ask_art = v;
    }
    if cli.windup {
        cfg.tagger.ask_art = false;
    }
    let (ib, changed) = providers::normalize_image_base(&cfg.providers.tmdb.image_base);
    if changed && !ib.is_empty() {
        pre += &format!(
            "warning: [providers.tmdb] image_base {:?} -> {ib:?} (sizes come from poster_size etc.; update your config)\n",
            cfg.providers.tmdb.image_base
        );
        cfg.providers.tmdb.image_base = ib;
    }
    let flags = Settings::from(cli);
    let top = flags.clone().over(&cfg.base);
    let files = scan(&cli.paths, &top);
    let dry = Settings::flag(top.dry_run);
    let mut o = pre
        + &format!(
            "jack{}: {} media file(s) found\n",
            if dry { " dry run" } else { "" },
            files.len()
        );
    let (mut failed, mut pr) = (0, Printed::default());
    let ex = crate::explain::Templates(&cfg.explain);
    let mut hk = crate::settle::Hooks {
        esc: &crate::escalate::NoSammy,
        ex: &ex,
        open: vec![],
        done: vec![],
    };
    for p in files {
        let _ = writeln!(o, "{}", p.display());
        let r = process(
            &p,
            &config::resolve(&flags, &cfg.base),
            &cfg,
            http,
            &mut o,
            &mut pr,
            &mut hk,
        );
        if let Err(e) = r {
            failed += 1;
            let _ = writeln!(o, "    error: {e}");
        }
    }
    // Jack's archive: sync what the target (Daniel) doesn't have yet; records stay in jack.db
    let handed = !dry && (Settings::flag(top.download_only) || !Settings::flag(top.print));
    if handed && !hk.done.is_empty() {
        let _ = writeln!(o, "{}", crate::universal::sync_line(&cfg));
    }
    let flagged = crate::review::pass(&cfg.review, &crate::review::NoReview, &hk.done, &mut o);
    hk.open.extend(flagged);
    o += &crate::explain::report(&hk.open, hk.ex);
    // a file Jack could not identify is a failure (exit 1)
    let more = crate::explain::MORE_CONTEXT;
    let mut items = Vec::new();
    for (file, m) in &hk.done {
        let unresolved = m.get("resolution").is_some_and(|r| r == "unresolved");
        if unresolved {
            failed += 1;
        }
        items.push(serde_json::json!({
            "file": file,
            "ok": !unresolved,
            "lifecycle": m.get("lifecycle"),
            "reason": m.get("unresolved_reason"),
            "error": if unresolved { serde_json::Value::String(more.into()) } else { serde_json::Value::Null },
        }));
    }
    if cli.windup {
        let ok = failed == 0;
        pr.text = windups::base::flags::result_json(ok, &items) + "\n";
    }
    // where entries stand
    let mut stages = std::collections::BTreeMap::<&str, usize>::new();
    for (_, m) in &hk.done {
        *stages
            .entry(m.get("lifecycle").map_or("unknown", |s| s.as_str()))
            .or_default() += 1;
    }
    if !cli.windup && !stages.is_empty() {
        let l: Vec<String> = stages.iter().map(|(k, v)| format!("{v} {k}")).collect();
        let _ = writeln!(o, "entries: {}; failed: {failed}", l.join(", "));
    }
    if !cli.windup && !pr.json.is_empty() {
        pr.text += &(serde_json::to_string_pretty(&pr.json).map_err(|e| e.to_string())? + "\n");
    }
    Ok((o, pr.text, failed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn dry_run_prints_tag_command() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n").unwrap();
        let m = d.path().join("m.m4v");
        fs::write(&m, "x").unwrap();
        let h = || {
            providers::fake::FakeHttp::new(vec![
                (
                    "/movie/603",
                    json!({"title":"The Matrix","release_date":"1999-03-30","poster_path":"/p.jpg"}),
                ),
                ("/p.jpg", json!(null)),
            ])
        };
        let base = [
            d.path().to_str().unwrap(),
            "--config",
            cfg.to_str().unwrap(),
            "--dry-run",
            "--tmdb",
            "603",
        ];
        let (out, _, failed) = run(&cli(&base), &h()).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(
            out.contains("1 media file(s) found") && !out.contains("sidecar"),
            "{out}"
        );
        assert!(
            out.contains("tmdb=603") && out.contains("--title 'The Matrix' --year 1999"),
            "{out}"
        );
        assert!(
            !out.contains("move:"),
            "no sorting without -o / [sort] enabled: {out}"
        );
        let (out, _, _) = run(&cli(&[&base[..], &["-o", "/srt"]].concat()), &h()).unwrap();
        assert!(
            out.contains("move: /srt/The Matrix/The Matrix (1999).m4v"),
            "{out}"
        );
        assert!(!Path::new("/srt").exists());
        assert_eq!(fs::read(&m).unwrap(), b"x"); // untouched
        assert_eq!(
            fs::read_dir(d.path()).unwrap().count(),
            2,
            "no files written beside the media"
        );
    }

    #[test]
    fn nothing_dropped_everything_explained() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let db = d.path().join("media.db");
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, format!("[relationships]\ndatabase = \"daniel\"\n[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n[store]\nenabled = true\ndatabase = {:?}\n", db.to_str().unwrap())).unwrap();
        for f in ["The.Office.S01E01.mkv", "Nothing.Like.It.S01E01.mkv"] {
            fs::write(d.path().join(f), "x").unwrap();
        }
        let h = providers::fake::FakeHttp::new(vec![
            (
                "query=The%20Office",
                json!({"results":[{"id":1,"name":"The Office","first_air_date":"2005-03-24"},{"id":2,"name":"The Office","first_air_date":"2001-07-09"}]}),
            ),
            ("query=Nothing", json!({"results":[]})),
            (
                "/tv/1",
                json!({"name":"The Office","first_air_date":"2005-03-24"}),
            ),
        ]);
        let (out, _, failed) = run(
            &cli(&[
                d.path().to_str().unwrap(),
                "--config",
                cfg.to_str().unwrap(),
            ]),
            &h,
        )
        .unwrap();
        assert_eq!(failed, 2, "unresolved counts as failure: {out}");
        assert!(
            out.contains("entries: 1 not_identified, 1 undetermined; failed: 2"),
            "{out}"
        );
        assert!(out.contains("2 item(s) need attention"), "{out}");
        assert!(
            out.contains("couldn't figure it out") && out.contains("--year"),
            "{out}"
        );
        let c = rusqlite::Connection::open(&db).unwrap();
        let n = |q: &str| c.query_row(q, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(n("SELECT count(*) FROM files"), 2);
        assert_eq!(
            n("SELECT count(*) FROM unresolved WHERE status = 'open'"),
            2
        );
        assert_eq!(
            n("SELECT count(*) FROM media WHERE source = 'tmdb:search'"),
            2,
            "the whole search list is kept"
        );
        assert_eq!(
            n("SELECT count(*) FROM media WHERE lifecycle = 'undetermined' AND kind = 'unknown'"),
            1,
            "file purgatory"
        );
        assert_eq!(
            n("SELECT count(*) FROM status_history WHERE actor = 'jack'"),
            2
        );
        let ids = |c: &rusqlite::Connection| -> Vec<i64> {
            c.prepare("SELECT id FROM media WHERE lifecycle IS NOT NULL ORDER BY id")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        let before = ids(&c);
        drop(c);
        run(
            &cli(&[
                d.path().to_str().unwrap(),
                "--config",
                cfg.to_str().unwrap(),
            ]),
            &h,
        )
        .unwrap();
        let c = rusqlite::Connection::open(&db).unwrap();
        assert_eq!(
            ids(&c),
            before,
            "a re-run keeps the same entries (stable ids)"
        );
    }

    #[test]
    fn download_only_fetches_via_providers() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(
            &cfg,
            "[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n",
        )
        .unwrap();
        let m = d.path().join("The Matrix (1999).mp4");
        fs::write(&m, "x").unwrap();
        let h = providers::fake::FakeHttp::new(vec![
            ("/search/movie", json!({"results":[{"id":603}]})),
            (
                "/movie/603",
                json!({"title":"The Matrix","release_date":"1999-03-30","poster_path":"/p.jpg"}),
            ),
            ("/p.jpg", json!(null)),
        ]);
        let c = cli(&[
            m.to_str().unwrap(),
            "--config",
            cfg.to_str().unwrap(),
            "-d",
            "--tmdb",
            "603",
        ]);
        let (out, _, failed) = run(&c, &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        let meta = meta::load(&d.path().join("The Matrix (1999).mp4.meta.json")).unwrap();
        assert_eq!(meta["year"], "1999");
        assert_eq!(
            fs::read(d.path().join("The Matrix (1999).mp4.poster.jpg")).unwrap(),
            b"JPEG"
        );
        assert_eq!(fs::read(&m).unwrap(), b"x");
    }

    #[test]
    fn print_text_json_combined_with_d_and_set() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"SECRETTOK\"\nbase_url = \"http://t\"\n").unwrap();
        let cs = cfg.to_str().unwrap();
        let m = d.path().join("The Matrix (1999).mp4");
        fs::write(&m, "x").unwrap();
        let ms = m.to_str().unwrap();
        let h = || {
            providers::fake::FakeHttp::new(vec![
                ("/search/movie", json!({"results":[{"id":603}]})),
                (
                    "/movie/603",
                    json!({"title":"The Matrix","release_date":"1999-03-30","poster_path":"/p.jpg","imdb_id":"tt0133093"}),
                ),
                ("/p.jpg", json!(null)),
            ])
        };
        let listing = || {
            let mut v: Vec<_> = fs::read_dir(d.path())
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            v.sort();
            v
        };
        let before = listing();
        // text, print only: nothing written, media untouched
        let (_, p, failed) = run(&cli(&["--config", cs, "-p", "--tmdb", "603", ms]), &h()).unwrap();
        assert_eq!(failed, 0);
        assert!(
            p.starts_with(ms)
                && p.contains("  title: The Matrix\n")
                && p.contains("  imdb_id: tt0133093")
                && p.contains("art_url: ")
                && p.contains("/p.jpg"),
            "{p}"
        );
        assert!(!p.contains("SECRETTOK"));
        assert_eq!(listing(), before);
        assert_eq!(fs::read(&m).unwrap(), b"x");
        // json is clean stdout
        let (_, p, _) = run(
            &cli(&[
                "--config",
                cs,
                "--print",
                "--print-format",
                "json",
                "--tmdb",
                "603",
                ms,
            ]),
            &h(),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&p).unwrap();
        assert_eq!(v[0]["file"], ms);
        assert_eq!(v[0]["metadata"]["year"], "1999");
        assert_eq!(listing(), before);
        // -d + --print: both
        let (_, p, _) = run(
            &cli(&["--config", cs, "-d", "-p", "--tmdb", "603", ms]),
            &h(),
        )
        .unwrap();
        assert!(p.contains("title: The Matrix"));
        assert_eq!(
            meta::load(&d.path().join("The Matrix (1999).mp4.meta.json")).unwrap()["title"],
            "The Matrix"
        );
        assert!(d.path().join("The Matrix (1999).mp4.poster.jpg").exists());
        assert_eq!(fs::read(&m).unwrap(), b"x");
        // --set saves print + print_format; config then drives json
        run(
            &cli(&["--set", "--config", cs, "-p", "--print-format", "json"]),
            &h(),
        )
        .unwrap();
        let t = fs::read_to_string(&cfg).unwrap();
        assert!(
            t.contains("print = true") && t.contains("print_format = \"json\""),
            "{t}"
        );
        let (_, p, _) = run(&cli(&["--config", cs, "--tmdb", "603", ms]), &h()).unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&p)
                .unwrap()
                .is_array()
        );
    }

    #[test]
    fn apothecary_real_world_regressions() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[providers]\norder = [\"tmdb\", \"thetvdb\"]\nlanguage = \"en\"\nkeep_original = true\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\nimage_base = \"http://img/t/p/original/\"\n[providers.thetvdb]\napi_key = \"k\"\nbase_url = \"http://v\"\n").unwrap();
        let m = d
            .path()
            .join("The Apothecary Diaries - S03E01 - Locusts.mp4");
        fs::write(&m, "x").unwrap();
        let h = providers::fake::FakeHttp::new(vec![
            ("http://t/search/tv", json!({"results":[{"id":220542}]})),
            ("http://t/tv/220542/episode_groups", json!({"results":[]})),
            (
                "http://t/tv/220542?",
                json!({"name":"The Apothecary Diaries","original_name":"薬屋のひとりごと","first_air_date":"2023-10-22","poster_path":"/p.jpg","genres":[{"name":"Animation"},{"name":"Drama"}],"seasons":[{"season_number":1,"episode_count":24},{"season_number":2,"episode_count":24},{"season_number":3,"episode_count":12}]}),
            ),
            ("/login", json!({"data":{"token":"T"}})),
            (
                "http://v/search",
                json!({"data":[{"tvdb_id":"482073","name":"薬屋のひとりごと (2027)","year":"2027"},{"tvdb_id":"433631","name":"薬屋のひとりごと","year":"2023","translations":{"eng":"The Apothecary Diaries"}}]}),
            ),
            (
                "/series/433631/translations/eng",
                json!({"data":{"name":"The Apothecary Diaries","overview":"Maomao..."}}),
            ),
            (
                "/series/433631/episodes",
                json!({"data":{"episodes":[{"id":777,"name":"Locusts","seasonNumber":3,"number":1}]}}),
            ),
            (
                "/episodes/777/translations/eng",
                json!({"data":{"name":"Locusts"}}),
            ),
            (
                "/episodes/777/translations/jpn",
                json!({"data":{"name":"蝗害"}}),
            ),
            (
                "/series/433631/extended",
                json!({"data":{"genres":[{"name":"animation"},{"name":"Mystery"}]}}),
            ),
            (
                "/series/433631",
                json!({"data":{"id":433631,"name":"薬屋のひとりごと","firstAired":"2023-10-22","year":"2027","originalLanguage":"jpn"}}),
            ),
        ]);
        let c = cli(&[
            m.to_str().unwrap(),
            "--config",
            cfg.to_str().unwrap(),
            "-d",
            "-p",
            "-o",
            "/out",
            "--grouping",
            "Anime",
            "--name",
            "The Apothecary Diaries",
            "--year",
            "2023",
            "--tmdb",
            "220542",
        ]);
        let (out, p, failed) = run(&c, &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        let meta = meta::load(
            &d.path()
                .join("The Apothecary Diaries - S03E01 - Locusts.mp4.meta.json"),
        )
        .unwrap();
        assert_eq!(meta["show"], "The Apothecary Diaries");
        assert_eq!(meta["original_show"], "薬屋のひとりごと");
        assert_eq!(meta["year"], "2023", "firstAired, not the 2027 duplicate");
        assert_eq!(meta["episode_title"], "Locusts");
        assert_eq!(
            meta["original_episode_title"], "蝗害",
            "list name was English; original via translations/jpn"
        );
        assert_eq!(meta["genre"], "animation,drama,mystery");
        assert_eq!(meta["grouping"], "Anime");
        // --name/--year only filtered the search; --tmdb is what settled identity
        assert_eq!(
            meta["lifecycle"],
            "complete",
            "an id flag is proof of identity: {}",
            meta.get("lifecycle").cloned().unwrap_or_default()
        );
        assert!(out.contains("warning: tmdb: no S03E01"), "{out}");
        assert!(
            out.contains("tmdb=220542") && out.contains("433631"),
            "{out}"
        );
        assert_eq!(out.matches("image_base").count(), 1, "{out}");
        assert!(
            out.contains("warning: artwork:"),
            "art failure is a warning: {out}"
        ); // and meta.json was still saved (loaded above)
        assert!(p.contains("original_show: 薬屋のひとりごと"), "{p}");
        let (out, _, _) = run(
            &cli(&[
                m.to_str().unwrap(),
                "--config",
                cfg.to_str().unwrap(),
                "--dry-run",
                "-o",
                "/out",
                "--name",
                "The Apothecary Diaries",
                "--year",
                "2023",
                "--tmdb",
                "220542",
            ]),
            &h,
        )
        .unwrap();
        assert!(
            out.contains(
                "/out/The Apothecary Diaries (2023)/Season 3/薬屋のひとりごと S03E01 - 蝗害.mp4"
            ),
            "{out}"
        );
    }

    #[test]
    fn windup_mode_prints_json_only_and_unresolved_counts_as_failure() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[store]\nenabled = false\n[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n").unwrap();
        let m = d.path().join("The.Office.mkv");
        fs::write(&m, "x").unwrap();
        let h = providers::fake::FakeHttp::new(vec![(
            "/search/tv",
            json!({"results":[
                {"id":1,"name":"The Office","first_air_date":"2005-03-24"},
                {"id":2,"name":"The Office","first_air_date":"2001-07-09"}
            ]}),
        )]);
        let mut c = cli(&[
            m.to_str().unwrap(),
            "--config",
            cfg.to_str().unwrap(),
            "--dry-run",
            "--windup",
        ]);
        let (out, printed, failed) = run(&c, &h).unwrap();
        assert!(failed >= 1, "unresolved is a failure: {out}");
        let v: serde_json::Value = serde_json::from_str(printed.trim()).unwrap();
        assert_eq!(v["v"], 1);
        assert_eq!(v["ok"], false);
        assert!(
            v["items"][0]["error"]
                .as_str()
                .unwrap()
                .contains("couldn't figure it out"),
            "{printed}"
        );
        assert!(
            !printed.contains("media file"),
            "human report is not on the machine stream: {printed}"
        );
        // human report still built (for tests) but --windup main() suppresses it
        assert!(out.contains("media file"), "{out}");

        // usage error stays an Err (exit 2)
        c.paths.clear();
        c.windup = true;
        assert!(run(&c, &h).is_err());
    }

    #[test]
    fn json_sidecar_and_db_first_skip() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfgp = d.path().join("c.toml");
        fs::write(&cfgp, "[store]\nenabled = false\n[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\nimage_base = \"http://img\"\n").unwrap();
        let m = d.path().join("The.Matrix.1999.mkv");
        fs::write(&m, "x").unwrap();
        let h = providers::fake::FakeHttp::new(vec![
            (
                "/movie/603",
                json!({"title":"The Matrix","release_date":"1999-03-30","imdb_id":"tt0133093","poster_path":"/p.jpg"}),
            ),
            ("/p.jpg", json!(null)),
        ]);
        let args = |extra: &[&str]| {
            let mut v = vec![
                m.to_str().unwrap(),
                "--config",
                cfgp.to_str().unwrap(),
                "--tmdb",
                "603",
                "--dry-run",
            ];
            v.extend(extra);
            cli(&v)
        };
        let (out, _, failed) = run(&args(&["--json"]), &h).unwrap();
        // dry-run does not write the sidecar or jack.db
        assert_eq!(failed, 0, "{out}");
        assert!(!d.path().join("The.Matrix.1999.mkv.jack.json").exists());

        let h = providers::fake::FakeHttp::new(vec![
            (
                "/movie/603",
                json!({"title":"The Matrix","release_date":"1999-03-30","imdb_id":"tt0133093","poster_path":"/p.jpg"}),
            ),
            ("/p.jpg", json!(null)),
        ]);
        let (out, _, failed) = run(&args(&["--json", "--no-dry-run", "-d"]), &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        let side = d.path().join("The.Matrix.1999.mkv.jack.json");
        assert!(side.exists(), "sidecar: {out}");
        let js = fs::read_to_string(&side).unwrap();
        assert!(js.contains("The Matrix") && js.contains("\"v\":1"), "{js}");

        // second run: no HTTP needed when the record is complete
        let quiet = providers::fake::FakeHttp::new(vec![]);
        let (out, _, failed) = run(&args(&["--no-dry-run", "-d"]), &quiet).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(
            out.contains("search skipped") || out.contains("already in jack.db"),
            "{out}"
        );
        assert!(
            quiet.calls.borrow().is_empty(),
            "no network: {:?}",
            quiet.calls.borrow()
        );
    }
}
