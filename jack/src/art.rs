//! Choosing and downloading artwork.
use crate::config::{Config, Settings, suffixed};
use crate::meta::Meta;
use crate::providers::Http;
use std::path::{Path, PathBuf};

/// The art URL for the effective style (flag > [tagger] art_style), with fallbacks.
pub(crate) fn art_url(meta: &Meta, s: &Settings, cfg: &Config) -> Option<String> {
    art_keys(meta, s, cfg)
        .into_iter()
        .find_map(|k| meta.get(k).cloned())
}

/// Art meta keys in preference order for the effective style plus its fallbacks.
pub(crate) fn art_keys(meta: &Meta, s: &Settings, cfg: &Config) -> Vec<&'static str> {
    let t = &cfg.tagger;
    let ep = meta.get("kind").is_some_and(|k| k == "episode");
    let style = s.art_style.clone().unwrap_or_else(|| t.art_style.clone());
    let order = t
        .art_fallback
        .get(&style)
        .cloned()
        .unwrap_or_else(|| vec![style.clone()]);
    let keys = |st: &str| -> Vec<&'static str> {
        match (st, ep) {
            ("landscape", true) => t
                .tv_landscape
                .iter()
                .map(|c| {
                    if c == "still" {
                        "still_url"
                    } else {
                        "backdrop_url"
                    }
                })
                .collect(),
            ("landscape", false) => vec!["backdrop_url"],
            (_, true) => t
                .tv_poster
                .iter()
                .map(|c| {
                    if c == "season" {
                        "season_poster_url"
                    } else {
                        "poster_url"
                    }
                })
                .collect(),
            _ => vec!["poster_url"],
        }
    };
    let mut v: Vec<&'static str> = vec![];
    for k in order.iter().flat_map(|st| keys(st)) {
        if !v.contains(&k) {
            v.push(k);
        }
    }
    v
}

/// Where downloaded art goes: beside the media with -d, else a temp file (no sidecars).
pub(crate) fn art_path(path: &Path, s: &Settings, dl: bool) -> PathBuf {
    let suf = s.poster_suffix.as_deref().unwrap_or(".poster.jpg");
    if dl {
        return suffixed(path, suf);
    }
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = format!(
        "jack-{}-{n}-{}{suf}",
        std::process::id(),
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    std::env::temp_dir().join(name)
}

pub(crate) fn save_poster(
    pp: &Path,
    s: &Settings,
    cfg: &Config,
    meta: &mut Meta,
    http: &dyn Http,
) -> Result<(), String> {
    let url = art_url(meta, s, cfg);
    if let Some(url) = url {
        std::fs::write(pp, http.bytes(&url)?).map_err(|e| format!("{}: {e}", pp.display()))?;
        meta.insert("poster_file".into(), pp.to_string_lossy().into_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    /// Normal (non -d) runs embed art by default, TV picks the season poster, re-runs don't duplicate.
    #[test]
    fn integration_art_embedded_by_default() {
        use serde_json::json;
        use std::process::Command;
        let ok = |t: &str| {
            Command::new("which")
                .arg(t)
                .output()
                .is_ok_and(|o| o.status.success())
        };
        if ![
            "ffmpeg",
            "AtomicParsley",
            "exiftool",
            "mkvpropedit",
            "mkvmerge",
            "mkvextract",
        ]
        .iter()
        .all(|t| ok(t))
        {
            return eprintln!("skip art: tools missing");
        }
        let d = tempfile::tempdir().unwrap();
        let jpg = d.path().join("art.jpg");
        assert!(
            Command::new("ffmpeg")
                .args([
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=red:s=8x8",
                    "-frames:v",
                    "1"
                ])
                .arg(&jpg)
                .status()
                .unwrap()
                .success()
        );
        let at = format!("@{}", jpg.display());
        for (name, writer) in [
            ("Saiki.K.S01E02.mp4", "atomicparsley"),
            ("Saiki.K.S01E02.m4v", "exiftool"),
            ("Saiki.K.S01E02.mkv", "atomicparsley"),
        ] {
            let cfgp = d.path().join("c.toml");
            fs::write(&cfgp, format!("[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\nimage_base = \"http://img\"\n[tagger]\nmp4_tool = \"{writer}\"\n")).unwrap();
            let m = d.path().join(name);
            assert!(
                Command::new("ffmpeg")
                    .args([
                        "-y",
                        "-loglevel",
                        "error",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=black:s=16x16:d=0.2",
                        "-c:v",
                        "mpeg4"
                    ])
                    .arg(&m)
                    .status()
                    .unwrap()
                    .success()
            );
            let h = providers::fake::FakeHttp::new(vec![
                ("/search/tv", json!({"results":[{"id":42}]})),
                ("/tv/42/season/1/episode/2", json!({"id":9,"name":"Ep"})),
                ("/tv/42/season/1?", json!({"poster_path":"/season.jpg"})),
                (
                    "/tv/42?",
                    json!({"name":"Saiki K","poster_path":"/series.jpg"}),
                ),
                ("/season.jpg", json!(at.clone())),
            ]);
            let c = cli(&[
                m.to_str().unwrap(),
                "--config",
                cfgp.to_str().unwrap(),
                "--name",
                "Saiki K",
                "--year",
                "2016",
                "--tmdb",
                "42",
            ]);
            for _ in 0..2 {
                let (out, _, failed) = run(&c, &h).unwrap();
                assert_eq!(failed, 0, "{out}");
            }
            assert!(!h.calls.borrow().iter().any(|c| c.contains("/series.jpg")));
            let f = m.to_str().unwrap();
            let o = |a: &[&str]| {
                String::from_utf8_lossy(&Command::new(a[0]).args(&a[1..]).output().unwrap().stdout)
                    .into_owned()
            };
            if name.ends_with("mkv") {
                let j: serde_json::Value =
                    serde_json::from_str(&o(&["mkvmerge", "-J", f])).unwrap();
                let a = j["attachments"].as_array().unwrap();
                assert_eq!(a.len(), 1, "{j}");
                assert_eq!(
                    (a[0]["file_name"].as_str(), a[0]["content_type"].as_str()),
                    (Some("cover.jpg"), Some("image/jpeg"))
                );
            } else {
                assert!(
                    o(&["AtomicParsley", f, "-t"])
                        .contains("Atom \"covr\" contains: 1 piece of artwork"),
                    "{name}"
                );
                assert_eq!(
                    Command::new("exiftool")
                        .args(["-b", "-CoverArt", f])
                        .output()
                        .unwrap()
                        .stdout,
                    fs::read(&jpg).unwrap()
                );
            }
        }
    }

    #[test]
    fn integration_landscape_art_embedded() {
        use serde_json::json;
        use std::process::Command;
        let ok = |t: &str| {
            Command::new("which")
                .arg(t)
                .output()
                .is_ok_and(|o| o.status.success())
        };
        if !["ffmpeg", "AtomicParsley", "exiftool"]
            .iter()
            .all(|t| ok(t))
        {
            return eprintln!("skip art: tools missing");
        }
        let d = tempfile::tempdir().unwrap();
        let jpg = d.path().join("still.jpg");
        assert!(
            Command::new("ffmpeg")
                .args([
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=blue:s=16x9",
                    "-frames:v",
                    "1"
                ])
                .arg(&jpg)
                .status()
                .unwrap()
                .success()
        );
        let cfgp = d.path().join("c.toml");
        fs::write(&cfgp, "[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\nimage_base = \"http://img\"\nstill_size = \"w300\"\n").unwrap();
        let m = d.path().join("Saiki.K.S01E02.mp4");
        assert!(
            Command::new("ffmpeg")
                .args([
                    "-y",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=black:s=16x16:d=0.2",
                    "-c:v",
                    "mpeg4"
                ])
                .arg(&m)
                .status()
                .unwrap()
                .success()
        );
        let h = providers::fake::FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":42}]})),
            (
                "/tv/42/season/1/episode/2",
                json!({"id":9,"name":"Ep","still_path":"/still.jpg"}),
            ),
            ("/tv/42/season/1?", json!({"poster_path":"/season.jpg"})),
            (
                "/tv/42?",
                json!({"name":"Saiki K","poster_path":"/series.jpg","backdrop_path":"/bd.jpg"}),
            ),
            ("/w300/still.jpg", json!(format!("@{}", jpg.display()))),
        ]);
        let (out, _, failed) = run(
            &cli(&[
                m.to_str().unwrap(),
                "--config",
                cfgp.to_str().unwrap(),
                "--art",
                "landscape",
                "--name",
                "Saiki K",
                "--year",
                "2016",
                "--tmdb",
                "42",
            ]),
            &h,
        )
        .unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(
            !h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("season.jpg") || c.contains("bd.jpg"))
        );
        assert_eq!(
            Command::new("exiftool")
                .args(["-b", "-CoverArt"])
                .arg(&m)
                .output()
                .unwrap()
                .stdout,
            fs::read(&jpg).unwrap()
        );
    }

    #[test]
    fn art_style_choice_and_fallbacks() {
        let cfg = config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let mk = |kv: &[(&str, &str)]| -> Meta {
            kv.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        let movie = mk(&[
            ("kind", "movie"),
            ("poster_url", "P"),
            ("backdrop_url", "B"),
        ]);
        let land = Settings {
            art_style: Some("landscape".into()),
            ..Default::default()
        };
        assert_eq!(
            art_url(&movie, &Settings::default(), &cfg).as_deref(),
            Some("P")
        ); // poster default
        assert_eq!(art_url(&movie, &land, &cfg).as_deref(), Some("B"));
        let ep = mk(&[
            ("kind", "episode"),
            ("poster_url", "SP"),
            ("season_poster_url", "SEA"),
            ("backdrop_url", "SB"),
            ("still_url", "ST"),
        ]);
        assert_eq!(art_url(&ep, &land, &cfg).as_deref(), Some("ST"));
        let mut e2 = ep.clone();
        e2.remove("still_url");
        assert_eq!(art_url(&e2, &land, &cfg).as_deref(), Some("SB"));
        e2.remove("backdrop_url");
        assert_eq!(art_url(&e2, &land, &cfg).as_deref(), Some("SEA")); // no landscape -> poster
        assert_eq!(
            art_url(
                &mk(&[("kind", "movie"), ("backdrop_url", "B")]),
                &Settings::default(),
                &cfg
            )
            .as_deref(),
            Some("B")
        );
        // flag > config
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("a.mp4");
        fs::write(&m, "").unwrap();
        let c = d.path().join("c.toml");
        fs::write(&c, "[tagger]\nart_style = \"landscape\"\n").unwrap();
        let cfg = config::load(Some(&c)).unwrap();
        let eff = |args: &[&str]| {
            let s = config::resolve(
                &Settings::from(&cli(&[&[m.to_str().unwrap()], args].concat())),
                &cfg.base,
            );
            art_url(&movie, &s, &cfg)
        };
        assert_eq!(eff(&[]).as_deref(), Some("B"));
        assert_eq!(eff(&["--art", "poster"]).as_deref(), Some("P"));
        assert_eq!(
            eff(&["--art", "landscape", "--name", "Saiki K", "--year", "2016"]).as_deref(),
            Some("B")
        );
    }

    #[test]
    fn art_is_a_temp_file_unless_download_only() {
        let m = Path::new("/media/a.mp4");
        assert_eq!(
            art_path(m, &Settings::default(), true),
            PathBuf::from("/media/a.mp4.poster.jpg")
        );
        let t = art_path(m, &Settings::default(), false);
        assert!(
            t.starts_with(std::env::temp_dir())
                && t.to_string_lossy().ends_with("a.mp4.poster.jpg"),
            "{t:?}"
        );
    }

    fn ui(input: &str, tty: bool) {
        askart::UI.with(|u| {
            *u.borrow_mut() = Some(askart::Ui {
                input: Box::new(std::io::Cursor::new(input.to_string())),
                tty,
                log: Some(String::new()),
            })
        });
    }
    fn ui_log() -> String {
        askart::UI.with(|u| u.borrow().as_ref().unwrap().log.clone().unwrap())
    }

    #[test]
    fn ask_art_flow() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[providers]\norder = [\"tmdb\", \"omdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n[providers.omdb]\napi_key = \"k\"\nbase_url = \"http://o/\"\n[tagger]\natomicparsley = \"true\"\n").unwrap();
        let cs = cfg.to_str().unwrap();
        let h = || {
            providers::fake::FakeHttp::new(vec![
                ("/search/movie", json!({"results":[{"id":603}]})),
                (
                    "/movie/603",
                    json!({"title":"The Matrix","release_date":"1999-03-30","poster_path":"/p.jpg","backdrop_path":"/b.png"}),
                ),
                (
                    "http://o/",
                    json!({"Title":"The Matrix","Year":"1999","Poster":"http://x/op.webp"}),
                ),
                ("/p.jpg", json!(null)),
                ("op.webp", json!(null)),
                ("/b.png", json!(null)),
            ])
        };
        let m = d.path().join("The.Matrix.1999.mp4");
        fs::write(&m, "x").unwrap();
        let ms = m.to_str().unwrap();
        let folder = d.path().join("The.Matrix.1999");
        // without --ask-art: unchanged (no prompt, no folder)
        ui("", true);
        let plain = h();
        let (out, _, _) = run(&cli(&["--config", cs, ms]), &plain).unwrap();
        assert!(
            !out.contains("ask-art") && ui_log().is_empty() && !folder.exists(),
            "{out}"
        );
        assert!(
            !plain.calls.borrow().iter().any(|c| c.contains("/images")),
            "no image-list requests without --ask-art"
        );
        // Enter = #1; listed in preference order (posters by provider, then fallback backdrop); folder removed; embedded
        ui("\n", true);
        let (out, _, failed) = run(&cli(&["--config", cs, "--ask-art", ms]), &h()).unwrap();
        let log = ui_log();
        assert_eq!(failed, 0, "{out}");
        let (a, b, c) = (
            log.find("01.jpg").unwrap(),
            log.find("02.webp").unwrap(),
            log.find("03.png").unwrap(),
        );
        assert!(
            a < b
                && b < c
                && log.contains("1. tmdb     poster")
                && log.contains("2. omdb     poster")
                && log.contains("3. tmdb     backdrop"),
            "{log}"
        );
        assert!(
            out.contains("chose #1 (tmdb poster)")
                && out.contains("--artwork")
                && out.contains("The.Matrix.1999.mp4.poster.jpg"),
            "{out}"
        );
        assert!(!folder.exists());
        // invalid then valid
        ui("x\n9\n2\n", true);
        let (out, _, _) = run(&cli(&["--config", cs, "--ask-art", ms]), &h()).unwrap();
        let log = ui_log();
        assert!(
            log.contains("invalid choice 'x'")
                && log.contains("invalid choice '9'")
                && out.contains("chose #2 (omdb poster)"),
            "{log}{out}"
        );
        // s = skip: nothing embedded
        assert!(
            !d.path().join("The.Matrix.1999.mp4.poster.jpg").exists()
                && !fs::read_dir(std::env::temp_dir())
                    .unwrap()
                    .flatten()
                    .any(|e| e
                        .file_name()
                        .to_string_lossy()
                        .ends_with("-The.Matrix.1999.mp4.poster.jpg")),
            "temp art removed after tagging"
        );
        ui("s\n", true);
        let (out, _, _) = run(&cli(&["--config", cs, "--ask-art", ms]), &h()).unwrap();
        assert!(
            out.contains("ask-art: skipped") && !out.contains("--artwork"),
            "{out}"
        );
        // non-TTY: no prompt, default #1 with a warning
        ui("3\n", false);
        let (out, _, _) = run(&cli(&["--config", cs, "--ask-art", ms]), &h()).unwrap();
        assert!(
            ui_log().contains("not a terminal; using #1")
                && !ui_log().contains("Choose")
                && out.contains("chose #1"),
            "{out}"
        );
        // -d keeps the folder; the chosen one is also the .poster.jpg
        ui("3\n", true);
        run(&cli(&["--config", cs, "--ask-art", "-d", ms]), &h()).unwrap();
        assert!(
            folder.join("01.jpg").exists()
                && folder.join("02.webp").exists()
                && folder.join("03.png").exists()
        );
        assert!(d.path().join("The.Matrix.1999.mp4.poster.jpg").exists());
        // --no-ask-art beats a saved ask_art; --set saves it
        run(&cli(&["--set", "--config", cs, "--ask-art"]), &h()).unwrap();
        assert!(fs::read_to_string(&cfg).unwrap().contains("ask_art = true"));
        ui("", true);
        let (out, _, _) = run(&cli(&["--config", cs, "--no-ask-art", ms]), &h()).unwrap();
        assert!(!out.contains("ask-art"), "{out}");
        // directory run: asked per file
        let dir = d.path().join("dir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("a.mp4"), "x").unwrap();
        fs::write(dir.join("b.mp4"), "x").unwrap();
        ui("1\n2\n", true);
        let (out, _, _) = run(&cli(&["--config", cs, dir.to_str().unwrap()]), &h()).unwrap();
        assert_eq!(ui_log().matches("Choose 1-3").count(), 2);
        assert!(
            out.contains("chose #1") && out.contains("chose #2"),
            "{out}"
        );
    }
}
