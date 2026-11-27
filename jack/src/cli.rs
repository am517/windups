//! Command-line flags and their mapping onto Settings.
use crate::config::Settings;
use clap::Parser;
use std::path::PathBuf;

pub(crate) const ATTRIBUTION: &str = "Metadata provided by TheTVDB: https://thetvdb.com\n\
This product uses the TMDB API but is not endorsed or certified by TMDB: https://www.themoviedb.org\n\
Ratings, rated and awards from OMDb (The Open Movie Database): https://www.omdbapi.com";

/// No Debug: holds API keys.
#[derive(Parser, Default)]
#[command(name = "jack", version, about = "Identify movie and TV files and report what they are", after_help = ATTRIBUTION, disable_help_flag = true)]
pub(crate) struct Cli {
    /// Files or directories to process
    pub(crate) paths: Vec<PathBuf>,
    /// Usage, the commands, and what I can and can't do; with -j, the same as JSON
    #[arg(short = 'h', long = "help")]
    pub(crate) help: bool,
    /// The windup that called this run (a name, e.g. flora); handed through so the database windup doesn't call it back
    #[arg(long, value_name = "NAME")]
    pub(crate) caller: Option<String>,
    /// Called by another windup: same as -j (kept until windups talk over Noise)
    #[arg(short = 'w', long = "windup")]
    pub(crate) windup: bool,
    /// The result as JSON on stdout (with -p, the facts too); reasons in their own fields; no prompts
    #[arg(short = 'j', long = "json")]
    pub(crate) json: bool,
    /// Save the other option flags as defaults in the main config. A key is only saved (no scan). Other flags are saved, then any given paths are run
    #[arg(long)]
    pub(crate) set: bool,
    /// Provider order, comma-separated (e.g. tmdb,thetvdb,omdb)
    #[arg(long, value_delimiter = ',')]
    pub(crate) providers: Option<Vec<String>>,
    /// TMDB v3 key (32 hex) or read-access token (JWT). `--tmdb-key <KEY>`. Alias: `--tmdb-token`.
    #[arg(long = "tmdb-key", visible_alias = "tmdb-token", value_name = "KEY")]
    pub(crate) tmdb_key: Option<String>,
    /// TheTVDB API key (this run; with --set: saved). `--tvdb-key <KEY>`.
    #[arg(long, value_name = "KEY")]
    pub(crate) tvdb_key: Option<String>,
    /// TheTVDB subscriber PIN (this run; with --set: saved). `--tvdb-pin <PIN>`.
    #[arg(long, value_name = "PIN")]
    pub(crate) tvdb_pin: Option<String>,
    /// OMDb API key (this run; with --set: saved). `--omdb-key <KEY>`.
    #[arg(long, value_name = "KEY")]
    pub(crate) omdb_key: Option<String>,
    /// Main config file (default: config.toml in Jack's data folder; see windups/LAYOUT.md)
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
    /// X post id or URL (digits, or an x.com / twitter.com status link)
    #[arg(long)]
    pub(crate) x: Option<String>,
    /// Wikidata item id or URL (Q123 or a wikidata.org link). Trusted alone.
    #[arg(long)]
    pub(crate) wikidata: Option<String>,
    /// Wikipedia article title or URL. Trusted alone.
    #[arg(long)]
    pub(crate) wikipedia: Option<String>,
    /// Title to search for (overrides the filename guess)
    #[arg(long)]
    pub(crate) name: Option<String>,
    /// Release / first-air year (search filter)
    #[arg(long)]
    pub(crate) year: Option<String>,
    /// Media type: tv, movie or auto (forces filename matching and provider search)
    #[arg(long = "type", value_parser = ["tv", "movie", "auto"])]
    pub(crate) kind: Option<String>,
    /// Same as --type tv
    #[arg(long, conflicts_with = "movie")]
    pub(crate) tv: bool,
    /// Same as --type movie
    #[arg(long)]
    pub(crate) movie: bool,
    /// Candidate picks, as numbered in the list Jack shows: at most one per source,
    /// separated by spaces (e.g. `--pick "1 6"`; `--pick 0` picks none). No prompt is shown.
    #[arg(long, value_name = "NUMBERS")]
    pub(crate) pick: Option<String>,
    /// Descend into subdirectories (default)
    #[arg(long, overrides_with = "no_recursive")]
    pub(crate) recursive: bool,
    #[arg(long, overrides_with = "recursive")]
    pub(crate) no_recursive: bool,
    /// Print the results (merged facts). Jack still always says what he did.
    #[arg(short, long, overrides_with = "no_print")]
    pub(crate) print: bool,
    /// Don't print (undoes a saved print = true)
    #[arg(long, overrides_with = "print")]
    pub(crate) no_print: bool,
    /// Show what would happen; change nothing
    #[arg(long, overrides_with = "no_dry_run")]
    pub(crate) dry_run: bool,
    #[arg(long, overrides_with = "dry_run")]
    pub(crate) no_dry_run: bool,
}

impl Cli {
    /// Parses the flags; `-j` and `-w` give the same JSON-only result (`-w` goes once windups use Noise).
    pub(crate) fn read<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let mut c = Self::try_parse_from(args)?;
        c.windup |= c.json;
        Ok(c)
    }
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
            dry_run: tri(c.dry_run, c.no_dry_run),
            print: tri(c.print, c.no_print),
            recursive: tri(c.recursive, c.no_recursive),
            imdb: c.imdb.clone(),
            thetvdb: c.thetvdb.clone(),
            tmdb: c.tmdb.clone(),
            youtube: c.youtube.clone(),
            x: c.x.clone(),
            wikidata: c.wikidata.clone(),
            wikipedia: c.wikipedia.clone(),
            name: c.name.clone(),
            year: c.year.clone(),
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
        fs::write(&cfg, "tmdb = \"main\"\nimdb = \"main\"\nrecursive = true\n").unwrap();
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
        let s = config::resolve(&Settings::from(&c), &main.base);
        assert_eq!(s.imdb.as_deref(), Some("flag"));
        assert_eq!(s.tmdb.as_deref(), Some("main"));
        assert_eq!(s.recursive, Some(true));
        assert!(s.extensions.unwrap().contains(&"mkv".to_string()));
    }

    #[test]
    fn help_shows_attribution() {
        use clap::CommandFactory;
        let h = Cli::command().render_long_help().to_string();
        assert!(h.contains("Metadata provided by TheTVDB: https://thetvdb.com"));
        assert!(h.contains("not endorsed or certified by TMDB") && h.contains("omdbapi.com"));
        assert!(h.contains("--tvdb-key <KEY>"), "{h}");
        assert!(h.contains("--tmdb-key <KEY>"), "{h}");
        assert!(h.contains("--tmdb-token"), "{h}");
    }

    #[test]
    fn type_flags_conflicts_and_set() {
        assert!(Cli::try_parse_from(["jack", "--tv", "--movie", "x"]).is_err());
        let c = Cli::read(["jack", "--caller", "flora", "-j", "x"]).unwrap();
        assert_eq!(
            c.caller.as_deref(),
            Some("flora"),
            "loop guard, a name only"
        );
        assert!(c.windup && c.json);
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
        assert_eq!((f.print, f.dry_run), (Some(false), Some(true)));
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        let cs = c.to_str().unwrap();
        fs::write(&c, "[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n").unwrap();
        let none =
            || providers::fake::FakeHttp::new(vec![("/movie/1", serde_json::json!({"title":"T"}))]);
        run(&cli(&["--set", "--config", cs, "-p"]), &none()).unwrap();
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
        let (o, p, failed) = run(&args(&[]), &none()).unwrap();
        assert!(
            failed >= 1 && o.contains("nothing processed") && p.is_empty(),
            "{o}"
        );
        let (o, p, _) = run(&args(&["--recursive", "--no-print"]), &none()).unwrap();
        assert!(
            o.contains("1 media file(s)") && o.contains("identified: ") && p.is_empty(),
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
                os: "linux".into(),
                ..Default::default()
            },
        );
        assert_eq!(
            l.config,
            PathBuf::from("/h/.local/share/windups/jack/config.toml")
        );
        assert_eq!(l.db, PathBuf::from("/h/.local/share/windups/jack/jack.db"));
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
