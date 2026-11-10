//! Command-line flags and their mapping onto Settings.
use crate::config::Settings;
use clap::Parser;
use std::path::PathBuf;

pub(crate) const ATTRIBUTION: &str = "Metadata provided by TheTVDB: https://thetvdb.com\n\
This product uses the TMDB API but is not endorsed or certified by TMDB: https://www.themoviedb.org\n\
Ratings, rated and awards from OMDb (The Open Movie Database): https://www.omdbapi.com";

/// No Debug: holds API keys.
#[derive(Parser, Default)]
#[command(name = "jack", version, about = "Tag and sort movie and TV files", after_help = ATTRIBUTION)]
pub(crate) struct Cli {
    /// Files or directories to process
    pub(crate) paths: Vec<PathBuf>,
    /// Print Jack's profile (what he does, reads, writes) and exit
    #[arg(long)]
    pub(crate) capabilities: bool,
    /// The windup that called this run (e.g. walter); handed to Daniel so he doesn't call it back
    #[arg(long, value_name = "WINDUP")]
    pub(crate) caller: Option<String>,
    /// Save the other option flags as defaults in the main config (then run, if paths are given)
    #[arg(long)]
    pub(crate) set: bool,
    /// Provider order, comma-separated (e.g. tmdb,thetvdb,omdb)
    #[arg(long, value_delimiter = ',')]
    pub(crate) providers: Option<Vec<String>>,
    /// TMDB read access token (this run; with --set: saved). No value: prompt (hidden)
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "TOKEN")]
    pub(crate) tmdb_token: Option<Option<String>>,
    /// TheTVDB API key (this run; with --set: saved). No value: prompt (hidden)
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "KEY")]
    pub(crate) tvdb_key: Option<Option<String>>,
    /// TheTVDB subscriber PIN (this run; with --set: saved). No value: prompt (hidden)
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "PIN")]
    pub(crate) tvdb_pin: Option<Option<String>>,
    /// OMDb API key (this run; with --set: saved). No value: prompt (hidden)
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "KEY")]
    pub(crate) omdb_key: Option<Option<String>>,
    /// MP4 tag writer: atomicparsley or exiftool
    #[arg(long, value_parser = ["atomicparsley", "exiftool"])]
    pub(crate) mp4_tool: Option<String>,
    /// Main config file (default: ~/.config/windups/jack/config.toml)
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    #[arg(long)]
    pub(crate) imdb: Option<String>,
    #[arg(long, alias = "tvdb")]
    pub(crate) thetvdb: Option<String>,
    #[arg(long)]
    pub(crate) tmdb: Option<String>,
    /// YouTube video id or URL (an online video; combine with --tmdb/--thetvdb to name its show)
    #[arg(long)]
    pub(crate) youtube: Option<String>,
    /// X post id or URL
    #[arg(long)]
    pub(crate) x: Option<String>,
    /// Title to search for (overrides the filename guess)
    #[arg(long)]
    pub(crate) name: Option<String>,
    /// Release / first-air year (search filter)
    #[arg(long)]
    pub(crate) year: Option<String>,
    /// Art orientation: poster or landscape
    #[arg(long, value_parser = ["poster", "landscape"])]
    pub(crate) art: Option<String>,
    /// iTunes Grouping (MP4 ©grp, MKV GROUPING)
    #[arg(long)]
    pub(crate) grouping: Option<String>,
    /// File-name language: original or a language code (see [naming])
    #[arg(long)]
    pub(crate) file_language: Option<String>,
    /// Folder-name language: original or a language code
    #[arg(long)]
    pub(crate) folder_language: Option<String>,
    /// Media type: tv, movie or auto (forces filename matching and provider search)
    #[arg(long = "type", value_parser = ["tv", "movie", "auto"])]
    pub(crate) kind: Option<String>,
    /// Same as --type tv
    #[arg(long, conflicts_with = "movie")]
    pub(crate) tv: bool,
    /// Same as --type movie
    #[arg(long)]
    pub(crate) movie: bool,
    /// Fetch metadata/artwork only (saved beside the file); don't modify media
    #[arg(short, long, overrides_with = "no_download_only")]
    pub(crate) download_only: bool,
    #[arg(long, overrides_with = "download_only")]
    pub(crate) no_download_only: bool,
    /// Descend into subdirectories (default)
    #[arg(long, overrides_with = "no_recursive")]
    pub(crate) recursive: bool,
    #[arg(long, overrides_with = "recursive")]
    pub(crate) no_recursive: bool,
    /// Embed artwork into media (default)
    /// Download all art candidates, list them, and ask which to embed
    #[arg(long, overrides_with = "no_ask_art")]
    pub(crate) ask_art: bool,
    #[arg(long, overrides_with = "ask_art")]
    pub(crate) no_ask_art: bool,
    #[arg(long, overrides_with = "no_embed_artwork")]
    pub(crate) embed_artwork: bool,
    #[arg(long, overrides_with = "embed_artwork")]
    pub(crate) no_embed_artwork: bool,
    /// Print composite metadata to stdout; without -d, modify and write nothing
    #[arg(short, long, overrides_with = "no_print")]
    pub(crate) print: bool,
    /// Don't print (undoes a saved print = true)
    #[arg(long, overrides_with = "print")]
    pub(crate) no_print: bool,
    /// --print format: text or json
    #[arg(long, value_parser = ["text", "json"])]
    pub(crate) print_format: Option<String>,
    /// Destination root for sorted files
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,
    /// Show what would change; change nothing
    #[arg(long, overrides_with = "no_dry_run")]
    pub(crate) dry_run: bool,
    #[arg(long, overrides_with = "dry_run")]
    pub(crate) no_dry_run: bool,
    /// Write one versioned handoff JSON per entry to stdout (`-`) or to a directory
    #[arg(long, value_name = "DIR|-")]
    pub(crate) emit_json: Option<String>,
}

/// `--x` / `--no-x` (last wins) -> Some(true) / Some(false) / None.
pub(crate) fn tri(on: bool, off: bool) -> Option<bool> {
    if on {
        Some(true)
    } else if off {
        Some(false)
    } else {
        None
    }
}

impl From<&Cli> for Settings {
    fn from(c: &Cli) -> Self {
        Settings {
            output: c.output.clone(),
            dry_run: tri(c.dry_run, c.no_dry_run),
            download_only: tri(c.download_only, c.no_download_only),
            print: tri(c.print, c.no_print),
            recursive: tri(c.recursive, c.no_recursive),
            print_format: c.print_format.clone(),
            imdb: c.imdb.clone(),
            thetvdb: c.thetvdb.clone(),
            tmdb: c.tmdb.clone(),
            youtube: c.youtube.clone(),
            x: c.x.clone(),
            name: c.name.clone(),
            year: c.year.clone(),
            art_style: c.art.clone(),
            grouping: c.grouping.clone(),
            kind: c
                .tv
                .then(|| "tv".into())
                .or(c.movie.then(|| "movie".into()))
                .or(c.kind.clone()),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn precedence_flags_main_defaults() {
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(
            &cfg,
            "output = \"/main\"\ntmdb = \"main\"\nimdb = \"main\"\n[tagger]\nexiftool = \"/x\"\n",
        )
        .unwrap();
        let m = d.path().join("a.mp4");
        fs::write(&m, "").unwrap();
        fs::write(d.path().join("a.mp4.config"), "tmdb = \"side\"\n").unwrap(); // sidecars are ignored
        let c = cli(&[
            m.to_str().unwrap(),
            "--config",
            cfg.to_str().unwrap(),
            "--imdb",
            "flag",
        ]);
        let main = config::load(c.config.as_deref()).unwrap();
        assert_eq!(main.tagger.exiftool, "/x");
        assert_eq!(main.tagger.mkvpropedit, "mkvpropedit"); // deep-merged default
        let s = config::resolve(&Settings::from(&c), &main.base);
        assert_eq!(s.imdb.as_deref(), Some("flag"));
        assert_eq!(s.tmdb.as_deref(), Some("main"));
        assert_eq!(s.output, Some(PathBuf::from("/main")));
        assert_eq!(s.recursive, Some(true));
        assert!(s.extensions.unwrap().contains(&"mkv".to_string()));
    }

    #[test]
    fn help_shows_attribution() {
        use clap::CommandFactory;
        let h = Cli::command().render_long_help().to_string();
        assert!(h.contains("Metadata provided by TheTVDB: https://thetvdb.com"));
        assert!(h.contains("not endorsed or certified by TMDB") && h.contains("omdbapi.com"));
    }

    #[test]
    fn type_flags_conflicts_and_set() {
        assert!(Cli::try_parse_from(["jack", "--tv", "--movie", "x"]).is_err());
        let c = Cli::try_parse_from(["jack", "--caller", "walter", "x"]).unwrap();
        assert_eq!(
            c.caller.as_deref(),
            Some("walter"),
            "carried to Daniel (loop guard)"
        );
        assert!(
            run(
                &cli(&["--type", "movie", "--tv", "x"]),
                &providers::fake::FakeHttp::new(vec![])
            )
            .unwrap_err()
            .contains("conflicts")
        );
        assert_eq!(
            Settings::from(&cli(&["--movie", "x"])).kind.as_deref(),
            Some("movie")
        );
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        run(
            &cli(&["--set", "--config", c.to_str().unwrap(), "--tv"]),
            &providers::fake::FakeHttp::new(vec![]),
        )
        .unwrap();
        assert!(fs::read_to_string(&c).unwrap().contains("type = \"tv\""));
        assert_eq!(
            config::load(Some(&c)).unwrap().base.kind.as_deref(),
            Some("tv")
        );
    }

    #[test]
    fn no_flags_last_wins_and_set_saves_false() {
        let f = Settings::from(&cli(&[
            "--print",
            "--no-print",
            "--no-dry-run",
            "--dry-run",
            "x",
        ]));
        assert_eq!(
            (f.print, f.dry_run, f.download_only),
            (Some(false), Some(true), None)
        );
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        let cs = c.to_str().unwrap();
        fs::write(&c, "[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n").unwrap();
        let none =
            || providers::fake::FakeHttp::new(vec![("/movie/1", serde_json::json!({"title":"T"}))]);
        run(
            &cli(&["--set", "--config", cs, "-p", "--no-embed-artwork"]),
            &none(),
        )
        .unwrap();
        let (o, _, _) = run(
            &cli(&[
                "--set",
                "--config",
                cs,
                "--no-print",
                "--no-recursive",
                "--no-dry-run",
            ]),
            &none(),
        )
        .unwrap();
        assert!(o.contains("print: true -> false"), "{o}");
        let t = fs::read_to_string(&c).unwrap();
        assert!(
            t.contains("print = false")
                && t.contains("recursive = false")
                && t.contains("embed_artwork = false")
                && t.contains("dry_run = false"),
            "{t}"
        );
        // saved print = true is undone per run by --no-print; --recursive beats saved false
        run(&cli(&["--set", "--config", cs, "-p"]), &none()).unwrap();
        let sub = d.path().join("a/b");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("m.mkv"), "x").unwrap();
        let dir = d.path().join("a");
        let args = |extra: &[&str]| {
            let mut v = vec![
                dir.to_str().unwrap(),
                "--config",
                cs,
                "--dry-run",
                "--tmdb",
                "1",
            ];
            v.extend(extra);
            cli(&v)
        };
        let (o, p, _) = run(&args(&[]), &none()).unwrap();
        assert!(o.contains("0 media file(s)") && p.is_empty(), "{o}");
        let (o, p, _) = run(&args(&["--recursive", "--no-print"]), &none()).unwrap();
        assert!(
            o.contains("1 media file(s)") && o.contains("tag: ") && p.is_empty(),
            "{o}"
        );
        let (_, p, _) = run(&args(&["--recursive"]), &none()).unwrap();
        assert!(p.contains("title: T"));
    }

    #[test]
    fn main_config_under_windups() {
        use windups::base::layout::{Probe, resolve};
        let l = resolve(
            "jack",
            &Probe {
                home: Some("/h".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            l.config,
            PathBuf::from("/h/.config/windups/jack/config.toml")
        );
        assert_eq!(l.db, PathBuf::from("/h/.config/windups/jack/jack.db"));
    }

    #[test]
    fn tilde_expands() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(
            config::expand(Path::new("~/x")),
            PathBuf::from(format!("{home}/x"))
        );
        assert_eq!(config::expand(Path::new("~bob/x")), PathBuf::from("~bob/x"));
    }

    #[test]
    fn extension_filter_case_insensitive() {
        let e = vec!["mp4".to_string(), ".mkv".to_string()];
        assert!(has_ext(Path::new("x/A.MP4"), &e));
        assert!(has_ext(Path::new("b.mkv"), &e));
        assert!(!has_ext(Path::new("b.mp4.config"), &e));
        assert!(!has_ext(Path::new("noext"), &e));
    }

    #[test]
    fn missing_explicit_config_errors() {
        assert!(config::load(Some(Path::new("/nonexistent/jack.toml"))).is_err());
    }
}
