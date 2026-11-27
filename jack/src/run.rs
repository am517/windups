//! A whole run over all given paths.
use crate::config::{self, Settings};
use crate::credentials;
use crate::providers::{self, Http};
use crate::source_id;
use crate::{cli::*, print::*, process::*, scan::*, secrets::*, setflags::*};
use std::fmt::Write as _;

/// Returns (report text, --print output, number of failed files).
pub(crate) fn run(cli: &Cli, http: &dyn Http) -> Result<(String, String, usize), String> {
    if let Some(value) = &cli.x {
        source_id::x_status(value)?;
    }
    if let Some(value) = &cli.wikidata {
        source_id::require_wikidata(value)?;
    }
    if let Some(value) = &cli.wikipedia {
        source_id::require_wikipedia(value)?;
    }
    let key_failures = rejected_keys(cli);
    if let (Some(t), Some(f)) = (
        &cli.kind,
        cli.tv.then_some("tv").or(cli.movie.then_some("movie")),
    ) && t != f
    {
        return Err(format!("--{f} conflicts with --type {t}"));
    }
    let mut pre = String::new();
    if cli.set {
        pre = save_flags(cli)?;
        if cli.paths.is_empty() || has_secret(cli) {
            if has_secret(cli) && !cli.paths.is_empty() {
                pre.push_str(
                    "not scanning: setting a key does not process files (run again without --set)\n",
                );
            }
            return Ok((pre, String::new(), key_failures));
        }
    } else if cli.paths.is_empty() {
        return Err("no files or directories given (see --help)".into());
    }
    let mut cfg = config::load(cli.config.as_deref())?;
    cfg.windup = cli.windup;
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
    if !found.changed.is_empty()
        && let Some(p) = cli.config.as_deref()
        && let Err(e) = config::save_relationships(p, &found.changed)
    {
        pre.push_str(&format!("warning: relationships: {e}\n"));
    }
    if let Some(p) = &cli.providers {
        cfg.providers.order = p.clone();
    }
    // credential flags beat config and embedded keys (this run)
    for key in credentials::check(cli).0 {
        let providers = &mut cfg.providers;
        match key.config_key {
            "providers.tmdb.api_key" => {
                providers.tmdb.api_key = key.value;
                providers.tmdb.read_token.clear();
            }
            "providers.tmdb.read_token" => {
                providers.tmdb.read_token = key.value;
                providers.tmdb.api_key.clear();
            }
            "providers.thetvdb.api_key" => providers.thetvdb.api_key = key.value,
            "providers.thetvdb.pin" => providers.thetvdb.pin = key.value,
            _ => providers.omdb.api_key = key.value,
        }
    }
    if key_failures > 0 && !cli.set {
        pre.push_str(&credentials::sentence(
            &credentials::check(cli)
                .0
                .iter()
                .map(|key| key.label)
                .collect::<Vec<_>>(),
            &credentials::check(cli).1,
        ));
        pre.push('\n');
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
    let found = scan(&cli.paths, &top);
    let dry = Settings::flag(top.dry_run);
    if found.files.is_empty() {
        return Ok(nothing_processed(cli, &pre, &found));
    }
    let mut o = pre
        + &format!(
            "jack{}: {} media file(s) found\n",
            if dry { " dry run" } else { "" },
            found.files.len()
        );
    for n in &found.notes {
        let _ = writeln!(o, "{n}");
    }
    for s in &found.skipped {
        let _ = writeln!(o, "skipped {}: {}", s.path.display(), s.error);
    }
    let mut pr = Printed::default();
    let ex = crate::explain::Templates(&cfg.explain);
    let mut hk = crate::settle::Hooks {
        esc: &crate::escalate::NoSammy,
        ex: &ex,
        open: vec![],
        done: vec![],
    };
    let files = found.files;
    let mut failed = found.skipped.len();
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
    let handed = !dry && !Settings::flag(top.print);
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
            "metadata": printed_facts(&pr.json, file),
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
    if dry && !cli.windup {
        let _ = writeln!(o, "dry run: changed nothing");
    }
    Ok((o, pr.text, failed + key_failures))
}

/// With `-p -j`, the facts printed for `file`; otherwise null.
fn printed_facts(printed: &[serde_json::Value], file: &str) -> serde_json::Value {
    printed
        .iter()
        .find(|p| p["file"].as_str() == Some(file))
        .map_or(serde_json::Value::Null, |p| p["metadata"].clone())
}

/// No media was processed. Human mode says why; windup mode returns a structured failure.
fn nothing_processed(cli: &Cli, pre: &str, found: &Scan) -> (String, String, usize) {
    let mut human = String::new();
    human.push_str(pre);
    human.push_str("jack: nothing processed\n");
    if found.skipped.is_empty() && found.notes.is_empty() {
        human.push_str("no media files were given\n");
    }
    for n in &found.notes {
        human.push_str(n);
        human.push('\n');
    }
    for s in &found.skipped {
        if s.detail.is_empty() {
            let _ = writeln!(human, "skipped {}: {}", s.path.display(), s.error);
        } else {
            let _ = writeln!(
                human,
                "skipped {}: {} ({})",
                s.path.display(),
                s.error,
                s.detail
            );
        }
    }
    if cli.windup {
        let items: Vec<serde_json::Value> = found
            .skipped
            .iter()
            .map(|s| {
                serde_json::json!({
                    "file": s.path.display().to_string(),
                    "ok": false,
                    "error": s.error,
                    "detail": if s.detail.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(s.detail.clone()) },
                })
            })
            .collect();
        let printed = serde_json::json!({
            "v": 1,
            "ok": false,
            "error": "nothing processed",
            "items": items,
        })
        .to_string()
            + "\n";
        (String::new(), printed, 1)
    } else {
        (human, String::new(), 1)
    }
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
            out.contains("tmdb=603") && out.contains("identified: The Matrix (1999)"),
            "{out}"
        );
        assert!(!out.contains("move:"), "Jack does not sort files: {out}");
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
            "-p",
            "--tmdb",
            "603",
        ]);
        let (out, printed, failed) = run(&c, &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(printed.contains("year: 1999"), "{printed}");
        assert!(!d.path().join("The Matrix (1999).mp4.poster.jpg").exists());
        assert!(!d.path().join("The Matrix (1999).mp4.meta.json").exists());
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
                && p.contains("poster_url: ")
                && p.contains("/p.jpg"),
            "{p}"
        );
        assert!(!p.contains("SECRETTOK"));
        assert_eq!(listing(), before);
        assert_eq!(fs::read(&m).unwrap(), b"x");
        // -p -j: one JSON result on stdout, the facts in their own field
        let (_, p, _) = run(
            &cli(&["--config", cs, "-p", "-j", "--tmdb", "603", ms]),
            &h(),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&p).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["items"][0]["file"], ms);
        assert_eq!(v["items"][0]["metadata"]["year"], "1999");
        assert_eq!(listing(), before);
        // -j without -p: the status only, no facts
        let (_, p, _) = run(
            &cli(&["--config", cs, "-j", "--dry-run", "--tmdb", "603", ms]),
            &h(),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&p).unwrap();
        assert!(v["items"][0]["metadata"].is_null(), "{v}");
        let (_, p, _) = run(&cli(&["--config", cs, "-p", "--tmdb", "603", ms]), &h()).unwrap();
        assert!(p.contains("title: The Matrix"));
        assert!(!d.path().join("The Matrix (1999).mp4.meta.json").exists());
        assert!(!d.path().join("The Matrix (1999).mp4.poster.jpg").exists());
        assert_eq!(fs::read(&m).unwrap(), b"x");
        // the old --print-format flag is gone
        assert!(Cli::read(["jack", "--print-format", "json", ms]).is_err());
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
            "-p",
            "--name",
            "The Apothecary Diaries",
            "--year",
            "2023",
            "--tmdb",
            "220542",
        ]);
        let (out, p, failed) = run(&c, &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(p.contains("show: The Apothecary Diaries"), "{p}");
        assert!(p.contains("original_show: 薬屋のひとりごと"), "{p}");
        assert!(
            p.contains("year: 2023"),
            "firstAired, not the 2027 duplicate: {p}"
        );
        assert!(p.contains("episode_title: Locusts"), "{p}");
        assert!(
            p.contains("original_episode_title: 蝗害"),
            "list name was English; original via translations/jpn: {p}"
        );
        assert!(p.contains("genre: animation,drama,mystery"), "{p}");
        assert!(
            p.contains("lifecycle: complete"),
            "an id flag is proof of identity: {p}"
        );
        assert!(out.contains("warning: tmdb: no S03E01"), "{out}");
        assert!(
            out.contains("tmdb=220542") && out.contains("433631"),
            "{out}"
        );
        assert_eq!(out.matches("image_base").count(), 1, "{out}");
        assert!(
            !out.contains("tag:") && !out.contains("move:"),
            "Jack does not edit the file: {out}"
        );
        let (out, _, _) = run(
            &cli(&[
                m.to_str().unwrap(),
                "--config",
                cfg.to_str().unwrap(),
                "--dry-run",
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
            out.contains("identified: The Apothecary Diaries (2023)"),
            "{out}"
        );
        assert!(!out.contains("move:"), "{out}");
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
    fn json_flag_writes_nothing_beside_the_media_and_db_first_skip() {
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
        // dry-run writes nothing
        assert_eq!(failed, 0, "{out}");
        assert!(!d.path().join("The.Matrix.1999.mkv.jack.json").exists());

        let h = providers::fake::FakeHttp::new(vec![
            (
                "/movie/603",
                json!({"title":"The Matrix","release_date":"1999-03-30","imdb_id":"tt0133093","poster_path":"/p.jpg"}),
            ),
            ("/p.jpg", json!(null)),
        ]);
        let (out, _, failed) = run(&args(&["--json", "--no-dry-run"]), &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(
            !d.path().join("The.Matrix.1999.mkv.jack.json").exists(),
            "no sidecar: {out}"
        );

        // second run: no HTTP needed when the record is complete
        let quiet = providers::fake::FakeHttp::new(vec![]);
        let (out, _, failed) = run(&args(&["--no-dry-run"]), &quiet).unwrap();
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

    #[test]
    fn a_single_file_with_spaces_is_reported_and_an_id_still_checks_other_sources() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(
            &cfg,
            "[store]\nenabled = false\n[archive]\nsync_target = \"none\"\n\
             [providers]\norder = [\"tmdb\", \"thetvdb\", \"tvmaze\"]\n\
             [providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://tmdb\"\n\
             [providers.thetvdb]\napi_key = \"k\"\nbase_url = \"http://tvdb\"\n\
             [providers.tvmaze]\nbase_url = \"http://maze\"\n",
        )
        .unwrap();
        let file = d.path().join("Bear Scratch/iVI4/Mountain/Mountain.mp4");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "x").unwrap();
        let path = file.to_str().unwrap();
        let cs = cfg.to_str().unwrap();
        let http = || {
            providers::fake::FakeHttp::new(vec![
                (
                    "/find/27498",
                    json!({"tv_results":[{"id":55,"name":"Mountain"}],"movie_results":[]}),
                ),
                (
                    "/tv/55",
                    json!({"name":"Mountain","first_air_date":"2019-04-01","overview":"A show"}),
                ),
                ("/login", json!({"data":{"token":"T"}})),
                (
                    "/series/27498",
                    json!({"data":{"id":27498,"name":"Mountain","firstAired":"2019-04-01","overview":"A show","remoteIds":[{"sourceName":"IMDB","id":"tt9"}]}}),
                ),
                (
                    "thetvdb=27498",
                    json!({"id":3,"name":"Mountain","premiered":"2019-04-01","summary":"A show","externals":{"thetvdb":27498,"imdb":"tt9"}}),
                ),
            ])
        };
        let run_at = |extra: &[&str]| {
            let mut args = vec![path, "--config", cs];
            args.extend(extra);
            run(&cli(&args), &http())
        };
        for extra in [
            &["--dry-run"][..],
            &["-p"][..],
            &["-p"][..],
            &["--thetvdb", "27498"][..],
            &["-p", "--thetvdb", "27498"][..],
        ] {
            let (out, printed, _failed) = run_at(extra).unwrap();
            assert!(
                !out.trim().is_empty() || !printed.trim().is_empty(),
                "silent for {extra:?}: out={out:?} printed={printed:?}"
            );
            let both = format!("{out}\n{printed}");
            assert!(
                both.contains("Bear Scratch") && both.contains("Mountain.mp4"),
                "path with spaces missing for {extra:?}: {both}"
            );
            assert!(
                both.contains("1 media file"),
                "mp4 was not counted for {extra:?}: {both}"
            );
        }
        let calls_dir = tempfile::tempdir().unwrap();
        let calls_cfg = calls_dir.path().join("c.toml");
        fs::write(&calls_cfg, fs::read_to_string(&cfg).unwrap()).unwrap();
        let calls_file = calls_dir
            .path()
            .join("Bear Scratch/iVI4/Mountain/Mountain.mp4");
        fs::create_dir_all(calls_file.parent().unwrap()).unwrap();
        fs::write(&calls_file, "x").unwrap();
        let h = http();
        let (out, _, failed) = run(
            &cli(&[
                calls_file.to_str().unwrap(),
                "--config",
                calls_cfg.to_str().unwrap(),
                "--dry-run",
                "--thetvdb",
                "27498",
            ]),
            &h,
        )
        .unwrap();
        assert_eq!(failed, 0, "a trusted id is enough: {out}");
        assert!(out.contains("dry run: changed nothing"), "{out}");
        let calls = h.calls.borrow().clone();
        assert!(
            calls.iter().any(|c| c.contains("/find/27498")),
            "tmdb was not asked about the tvdb id: {calls:?}"
        );
        assert!(
            calls.iter().any(|c| c.contains("/series/27498")),
            "thetvdb was not fetched: {calls:?}"
        );
        assert!(
            calls
                .iter()
                .any(|c| c.contains("thetvdb=27498") || c.contains("imdb=tt9")),
            "another source was not asked about this work: {calls:?}"
        );

        // --name/--year still need sources to agree
        let only = providers::fake::FakeHttp::new(vec![
            (
                "/search/movie",
                json!({"results":[{"id":55,"title":"Mountain","release_date":"2019-04-01"}]}),
            ),
            (
                "/movie/55",
                json!({"title":"Mountain","release_date":"2019-04-01","overview":"A film"}),
            ),
        ]);
        let named = d.path().join("Bear Scratch/Other/Mountain.mp4");
        fs::create_dir_all(named.parent().unwrap()).unwrap();
        fs::write(&named, "x").unwrap();
        let named_cfg = d.path().join("name.toml");
        fs::write(
            &named_cfg,
            "[store]\nenabled = false\n[providers]\norder = [\"tmdb\"]\n\
             [providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://tmdb\"\n",
        )
        .unwrap();
        let (out, _, failed) = run(
            &cli(&[
                named.to_str().unwrap(),
                "--config",
                named_cfg.to_str().unwrap(),
                "--dry-run",
                "--name",
                "Mountain",
                "--year",
                "2019",
            ]),
            &only,
        )
        .unwrap();
        assert_eq!(
            failed, 0,
            "one result at or above the threshold is enough; --name/--year only filtered: {out}"
        );
        assert!(out.contains("identified: Mountain (2019)"), "{out}");
        assert!(out.contains("score"), "{out}");

        let missing = d.path().join("Bear Scratch/missing.mp4");
        let (out, _, failed) = run(
            &cli(&[missing.to_str().unwrap(), "--config", cs, "--dry-run"]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap();
        assert!(failed >= 1, "{out}");
        assert!(out.contains("nothing processed"), "{out}");
        assert!(out.contains("not found"), "{out}");
        assert!(out.contains("Bear Scratch"), "{out}");
        assert!(!out.trim().is_empty());
    }

    #[test]
    fn a_bad_x_flag_is_a_usage_error_and_a_status_link_is_not() {
        let err = run(
            &cli(&["--x", "not-an-id", "clip.mp4"]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap_err();
        assert!(err.contains("status id"), "{err}");
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[store]\nenabled = false\n[providers]\norder = []\n").unwrap();
        let file = d.path().join("clip.mp4");
        fs::write(&file, "x").unwrap();
        let (out, _, failed) = run(
            &cli(&[
                "--config",
                cfg.to_str().unwrap(),
                "--dry-run",
                "--x",
                "https://x.com/nasa/status/42",
                file.to_str().unwrap(),
            ]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap();
        assert!(
            out.contains("source: x 42") || out.contains("yt-dlp is not installed"),
            "{out}"
        );
        assert!(
            failed == 0 || failed == 1,
            "a missing tool is not a usage error: {out}"
        );
        assert!(!out.contains("usage"), "{out}");
    }
}
