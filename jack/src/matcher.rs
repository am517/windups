//! Filename -> lookup query.
use crate::providers::Query;
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct MatchCfg {
    pub separators: String,
    pub tv_patterns: Vec<String>,
    pub movie_patterns: Vec<String>,
    pub cleanup: Vec<String>,
    pub show_from_parent_dir: bool,
    /// Use the text after SxxExx (minus cleanup junk) as the episode title when no provider has one.
    pub episode_title_from_filename: bool,
    /// Years this far apart still count as agreeing (release dates differ by country). A hint for
    /// matching only: never evidence that two things are the same.
    pub year_tolerance: u32,
    /// A fetched record more than this many years from the user's --year is a different work
    /// and blocks the match. Default 1 (a release can be a year apart in different countries).
    pub user_year_tolerance: u32,
    /// Found sources that must confirm a match; any found source naming a different work
    /// blocks it, and sources that found nothing don't count. Default 1.
    pub min_confirming: usize,
    /// Accept a title only at or above this score (0 to 1). Default 0.90.
    #[serde(default = "default_threshold")]
    pub fuzzy_threshold: f64,
}

fn default_threshold() -> f64 {
    0.90
}

fn re(p: &str) -> Result<Regex, String> {
    Regex::new(p).map_err(|e| format!("[matching] bad regex {p:?}: {e}"))
}

fn tidy(s: &str, cfg: &MatchCfg) -> Result<String, String> {
    let mut s = s.to_string();
    for c in &cfg.cleanup {
        s = re(c)?.replace_all(&s, " ").into_owned();
    }
    Ok(s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| " -([".contains(c))
        .to_string())
}

/// `kind`: Some("tv") skips movie patterns, Some("movie") skips TV patterns.
pub fn parse(path: &Path, cfg: &MatchCfg, kind: Option<&str>) -> Result<Query, String> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = if cfg.separators.is_empty() {
        stem
    } else {
        re(&cfg.separators)?.replace_all(&stem, " ").into_owned()
    };
    let mut q = Query::default();
    let none = vec![];
    for p in if kind == Some("movie") {
        &none
    } else {
        &cfg.tv_patterns
    } {
        if let Some(c) = re(p)?.captures(&stem) {
            q.season = c.name("season").and_then(|m| m.as_str().parse().ok());
            q.episode = c.name("episode").and_then(|m| m.as_str().parse().ok());
            q.episode_title = Some(tidy(&stem[c.get(0).map_or(0, |m| m.end())..], cfg)?)
                .filter(|t| !t.is_empty());
            q.title = tidy(c.name("show").map_or("", |m| m.as_str()), cfg)?;
            if q.title.is_empty() && cfg.show_from_parent_dir {
                let dir = path
                    .parent()
                    .and_then(|d| d.file_name())
                    .map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let dir = if dir.to_lowercase().starts_with("season") {
                    path.parent()
                        .and_then(|d| d.parent())
                        .and_then(|d| d.file_name())
                        .map(|d| d.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    dir
                };
                q.title = tidy(&re(&cfg.separators)?.replace_all(&dir, " "), cfg)?;
            }
            return Ok(q);
        }
    }
    for p in if kind == Some("tv") {
        &none
    } else {
        &cfg.movie_patterns
    } {
        if let Some(c) = re(p)?.captures(&stem) {
            q.title = tidy(c.name("title").map_or("", |m| m.as_str()), cfg)?;
            q.year = c.name("year").map(|m| m.as_str().to_string());
            return Ok(q);
        }
    }
    q.title = tidy(&stem, cfg)?;
    Ok(q)
}

/// "auto"/unset -> None.
pub fn forced(kind: Option<&str>) -> Option<&str> {
    kind.filter(|k| *k != "auto")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> MatchCfg {
        crate::config::load(Some(Path::new("config/jack.toml")))
            .unwrap()
            .matching
    }

    #[test]
    fn movies_and_tv() {
        let q = parse(
            Path::new("The.Matrix.1999.1080p.BluRay.x264.mkv"),
            &cfg(),
            None,
        )
        .unwrap();
        assert_eq!(
            (q.title.as_str(), q.year.as_deref(), q.season),
            ("The Matrix", Some("1999"), None)
        );
        let q = parse(Path::new("Blade Runner 2049 (2017).mp4"), &cfg(), None).unwrap();
        assert_eq!(
            (q.title.as_str(), q.year.as_deref()),
            ("Blade Runner 2049", Some("2017"))
        );
        let q = parse(Path::new("Saiki.K.S01E02.720p.WEB-DL.mkv"), &cfg(), None).unwrap();
        assert_eq!(
            (q.title.as_str(), q.season, q.episode, q.episode_title),
            ("Saiki K", Some(1), Some(2), None)
        );
        let q = parse(Path::new("The.Apothecary.Diaries.S03E01.Locusts.REPACK.1080p.CR.WEB-DL.JPN.AAC2.0.H.264.MSubs-ToonsHub.mp4"), &cfg(), None).unwrap();
        assert_eq!(
            (
                q.title.as_str(),
                q.season,
                q.episode,
                q.episode_title.as_deref()
            ),
            ("The Apothecary Diaries", Some(3), Some(1), Some("Locusts"))
        );
        let q = parse(Path::new("Show/Season 2/2x05.m4v"), &cfg(), None).unwrap();
        assert_eq!(
            (q.title.as_str(), q.season, q.episode),
            ("Show", Some(2), Some(5))
        );
        let q = parse(Path::new("home video.mp4"), &cfg(), None).unwrap();
        assert_eq!((q.title.as_str(), q.year), ("home video", None));
        // forced type skips the other heuristics
        let q = parse(Path::new("Show.S01E02.mkv"), &cfg(), Some("movie")).unwrap();
        assert_eq!((q.title.as_str(), q.season), ("Show S01E02", None));
        let q = parse(Path::new("1917 (2019).mkv"), &cfg(), Some("tv")).unwrap();
        assert_eq!((q.title.as_str(), q.year), ("1917 (2019)", None));
    }
}
