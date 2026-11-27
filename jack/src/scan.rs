//! Finding media files and building the lookup query for one.
use crate::config::{self, Config, Settings, has_ext};
use crate::matcher;
use crate::providers::Query;
use std::path::{Path, PathBuf};

/// A path that was not a media file, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Skip {
    pub path: PathBuf,
    pub error: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Scan {
    pub files: Vec<PathBuf>,
    pub skipped: Vec<Skip>,
    /// Notes that are not failures (an explicitly named file with an unusual extension).
    pub notes: Vec<String>,
}

pub(crate) fn scan(paths: &[PathBuf], s: &Settings) -> Scan {
    let exts = s.extensions.clone().unwrap_or_default();
    let depth = if s.recursive.unwrap_or(true) {
        usize::MAX
    } else {
        1
    };
    let list = if exts.is_empty() {
        "(none configured)".to_string()
    } else {
        exts.join(", ")
    };
    let mut out = Scan::default();
    for p in paths {
        let p = config::expand(p);
        if p.is_file() {
            // A file the person named is always included, spaces and all. .mp4 is a normal type.
            if !exts.is_empty() && !has_ext(&p, &exts) {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("(none)");
                out.notes.push(format!(
                    "including {} (named explicitly; .{ext} is not in the usual list: {list})",
                    p.display()
                ));
            }
            out.files.push(p);
        } else if p.is_dir() {
            let before = out.files.len();
            for e in walkdir::WalkDir::new(&p)
                .max_depth(depth)
                .sort_by_file_name()
                .into_iter()
                .flatten()
            {
                if e.file_type().is_file() && has_ext(e.path(), &exts) {
                    out.files.push(e.into_path());
                }
            }
            if out.files.len() == before {
                out.skipped.push(Skip {
                    path: p,
                    error: "no media files".into(),
                    detail: list.clone(),
                });
            }
        } else if !p.exists() {
            out.skipped.push(Skip {
                path: p,
                error: "not found".into(),
                detail: String::new(),
            });
        } else {
            out.skipped.push(Skip {
                path: p,
                error: "not a file or directory".into(),
                detail: String::new(),
            });
        }
    }
    out
}

/// Query from the filename plus ids from flags.
pub(crate) fn query(path: &Path, s: &Settings, cfg: &Config) -> Result<Query, String> {
    let kind = matcher::forced(s.kind.as_deref());
    let mut q = matcher::parse(path, &cfg.matching, kind)?;
    q.kind = kind.map(String::from);
    // embedded tags are user input: they fill name, date and ids, and a flag still wins
    let own = if cfg.embedded.enabled {
        crate::filemeta::read(path, &cfg.online.ffprobe, &cfg.embedded.tags)
    } else {
        crate::meta::Meta::new()
    };
    // a user's id outranks the file's own tags: they fill nothing then (they are checked later)
    if !s.has_id() {
        let heard = crate::embedded_input::hear(&own);
        crate::embedded_input::fill(&mut q, &heard);
    }
    crate::embedded_input::flags_win(&mut q, s);
    Ok(q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn name_and_year_override_filename_guess() {
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("Dune.2021.mkv");
        fs::write(&m, "").unwrap();
        let cfg = config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let c = cli(&[
            m.to_str().unwrap(),
            "--name",
            "Dune Part One",
            "--year",
            "2021",
        ]);
        let s = config::resolve(&Settings::from(&c), &cfg.base);
        let q = query(&m, &s, &cfg).unwrap();
        assert_eq!(
            (q.title.as_str(), q.year.as_deref()),
            ("Dune Part One", Some("2021"))
        );
        let q = query(&m, &Settings::default(), &cfg).unwrap();
        assert_eq!(
            (q.title.as_str(), q.year.as_deref()),
            ("Dune", Some("2021"))
        );
    }

    #[test]
    fn embedded_ids_fill_the_query_and_a_flag_wins() {
        let tags: Meta = [
            ("show", "Saiki K"),
            ("date", "2016-01-01"),
            ("imdb_id", "tt5"),
            ("tmdb_id", "42"),
            ("thetvdb_id", "7"),
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .into();
        let heard = crate::embedded_input::hear(&tags);
        let mut q = crate::providers::Query {
            title: "from the filename".into(),
            ..Default::default()
        };
        crate::embedded_input::fill(&mut q, &heard);
        assert_eq!(
            (
                q.title.as_str(),
                q.year.as_deref(),
                q.tmdb.as_deref(),
                q.thetvdb.as_deref(),
                q.imdb.as_deref()
            ),
            ("Saiki K", Some("2016"), Some("42"), Some("7"), Some("tt5"))
        );
        assert!(heard.trusted_id());
        let flags = Settings {
            thetvdb: Some("99".into()),
            ..Default::default()
        };
        crate::embedded_input::flags_win(&mut q, &flags);
        assert_eq!(q.thetvdb.as_deref(), Some("99"));
        assert_eq!(q.imdb.as_deref(), Some("tt5"));
    }

    #[test]
    fn scan_temp_dir_recursive_and_flat() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("sub")).unwrap();
        for f in ["a.mp4", "b.txt", "sub/c.mkv", "a.mp4.config"] {
            fs::write(d.path().join(f), "").unwrap();
        }
        let mut s = Settings {
            extensions: Some(vec!["mp4".into(), "mkv".into()]),
            ..Default::default()
        };
        assert_eq!(scan(&[d.path().into()], &s).files.len(), 2);
        s.recursive = Some(false);
        assert_eq!(
            scan(&[d.path().into()], &s).files,
            vec![d.path().join("a.mp4")]
        );
    }

    #[test]
    fn a_named_mp4_with_spaces_is_a_media_file_and_a_missing_path_is_not_silent() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("Folder With Spaces/sub dir/video.mp4");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "x").unwrap();
        let s = Settings {
            extensions: Some(vec!["mp4".into(), "mkv".into()]),
            ..Default::default()
        };
        let hit = scan(std::slice::from_ref(&file), &s);
        assert_eq!(hit.files, vec![file]);
        assert!(hit.skipped.is_empty(), "{:?}", hit.skipped);
        let missing = d.path().join("Folder With Spaces/nope.mp4");
        let miss = scan(std::slice::from_ref(&missing), &s);
        assert!(miss.files.is_empty());
        assert_eq!(miss.skipped[0].error, "not found");
        assert_eq!(miss.skipped[0].path, missing);
    }
}
