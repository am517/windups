//! Online videos (YouTube, X): read the label first, the way a person would, then ask the site.
//! Kind + source traits leave room for books and documents later (another kind, other readers).
pub mod crossover;
pub mod embedded;
pub mod exif;
pub mod filename;
pub mod info_json;
pub mod urls;
pub mod youtube_video;
pub mod ytdlp;

use crate::config::Config;
use crate::meta::Meta;
use crate::providers::Http;
use crate::resolver::{Choice, Resolver};
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::Path;

pub const KIND: &str = "online_video";

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct OnlineCfg {
    pub enabled: bool,
    /// Label readers, tried in order: info_json, embedded, filename.
    pub readers: Vec<String>,
    /// Sidecar written by yt-dlp --write-info-json, replacing the media extension.
    pub info_json_suffix: String,
    pub ffprobe: String,
    /// Embedded tags that may hold the source URL.
    pub embedded_tags: Vec<String>,
    /// First capture group = YouTube id (yt-dlp's default "Title [id].ext").
    pub filename_id_pattern: String,
    /// Providers asked whether the channel is also a show (merged only on an exact, unique name match).
    pub crossover_providers: Vec<String>,
    /// Keys copied from the matched show.
    pub crossover_keys: Vec<String>,
}

impl Default for OnlineCfg {
    fn default() -> Self {
        OnlineCfg {
            enabled: true,
            readers: ["info_json", "embedded", "filename"]
                .map(String::from)
                .into(),
            info_json_suffix: ".info.json".into(),
            ffprobe: "ffprobe".into(),
            embedded_tags: ["comment", "purl", "description", "synopsis", "url"]
                .map(String::from)
                .into(),
            filename_id_pattern: r"\[([A-Za-z0-9_-]{11})\]\.[A-Za-z0-9]+$".into(),
            crossover_providers: vec!["tmdb".into()],
            crossover_keys: [
                "show",
                "tmdb_series_id",
                "thetvdb_series_id",
                "imdb_id",
                "genre",
                "network",
            ]
            .map(String::from)
            .into(),
        }
    }
}

/// Where a file came from, as read off its label.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Label {
    /// youtube | x
    pub site: String,
    pub id: String,
    /// What the label itself already says (yt-dlp info), if anything.
    pub info: Meta,
    /// Which reader found it.
    pub reader: String,
}

/// First reader (in config order) that recognizes the file.
pub fn detect(path: &Path, cfg: &OnlineCfg) -> Option<Label> {
    cfg.readers.iter().find_map(|r| match r.as_str() {
        "info_json" => info_json::read(path, cfg),
        "embedded" => embedded::read(path, cfg),
        "filename" => filename::read(path, cfg),
        _ => None,
    })
}

/// A label given on the command line (--youtube / --x, id or URL).
pub fn from_flags(youtube: Option<&str>, x: Option<&str>) -> Option<Label> {
    let l = |site: &str, v: &str| {
        let id = urls::parse(v)
            .filter(|(s, _)| s == site)
            .map(|(_, i)| i)
            .unwrap_or_else(|| v.to_string());
        Label {
            site: site.into(),
            id,
            reader: "flag".into(),
            ..Default::default()
        }
    };
    youtube
        .map(|v| l("youtube", v))
        .or_else(|| x.map(|v| l("x", v)))
}

pub struct Outcome {
    pub meta: Meta,
    pub found: Vec<(String, Meta)>,
    /// True when Jack couldn't decide; nothing should be tagged from a guess.
    pub unresolved: bool,
}

/// Site data for the label, then a crossover check against show databases.
pub fn fetch(
    label: &Label,
    forced: &Meta,
    cfg: &Config,
    http: &dyn Http,
    res: &dyn Resolver,
    out: &mut String,
) -> Result<Outcome, String> {
    let _ = writeln!(
        out,
        "    source: {} {} (from {})",
        label.site, label.id, label.reader
    );
    let mut found = vec![];
    if label.site == "youtube" {
        let yc = &cfg.providers.youtube;
        match youtube_video::lookup(&label.id, yc, http) {
            Ok(Some(m)) => found.push(("youtube".to_string(), m)),
            Ok(None) => {}
            Err(e) => {
                let _ = writeln!(out, "    warning: youtube: {e}");
            }
        }
    }
    if !label.info.is_empty() {
        found.push(("yt-dlp".to_string(), label.info.clone()));
    }
    // Official API first (YouTube, when a key is set). X has no free read API.
    // yt-dlp is a full source when it is installed and the API did not already answer.
    let api_answered = found.iter().any(|(name, _)| name == "youtube");
    if !api_answered && label.info.is_empty() && (label.site == "x" || label.site == "youtube") {
        let url = match label.site.as_str() {
            "x" => format!("https://x.com/i/status/{}", label.id),
            _ => format!("https://www.youtube.com/watch?v={}", label.id),
        };
        match ytdlp::dump(&url) {
            ytdlp::Read::Meta(meta) => found.push(("yt-dlp".to_string(), meta)),
            ytdlp::Read::Missing(message) => {
                let _ = writeln!(out, "    {message}");
            }
            ytdlp::Read::Failed(message) => {
                let _ = writeln!(out, "    warning: {message}");
            }
        }
    }
    if found.is_empty() {
        let mut why = Vec::new();
        if label.site == "youtube" && cfg.providers.youtube.api_key.is_empty() {
            why.push("no YouTube API key".to_string());
        }
        if label.site == "x" {
            why.push(
                "X has no free API for reading a post; yt-dlp or exiftool can supply it when installed"
                    .to_string(),
            );
        }
        if !crate::tools::installed("yt-dlp") {
            why.push(crate::tools::missing("yt-dlp"));
        }
        if !crate::tools::installed("exiftool") {
            why.push(crate::tools::missing("exiftool"));
        }
        let why = if why.is_empty() {
            "nothing came back".to_string()
        } else {
            why.join("; ")
        };
        let mut meta = Meta::new();
        meta.insert("resolution".into(), "unresolved".into());
        meta.insert("unresolved_reason".into(), "source_error".into());
        meta.insert("unresolved_detail".into(), why.clone());
        meta.insert("site".into(), label.site.clone());
        meta.insert("title".into(), label.id.clone());
        let _ = writeln!(out, "    {} {}: {why}", label.site, label.id);
        return Ok(Outcome {
            meta,
            found,
            unresolved: true,
        });
    }
    let mut meta = crate::sorter::merge(&found, &cfg.merge);
    meta.insert("kind".into(), KIND.into());
    meta.insert("site".into(), label.site.clone());
    let mut unresolved = false;
    if !forced.is_empty() {
        // ids from flags (--tmdb/--thetvdb/--imdb) name the show: no guessing
        let _ = writeln!(out, "    crossover: from flags");
        meta.extend(forced.clone());
        found.push(("flags".to_string(), forced.clone()));
    } else if let Some(name) = meta.get("channel").cloned() {
        let cands = crossover::candidates(&name, cfg, http, out);
        match res.resolve(&name, None, &cands) {
            Choice::Pick(i) => {
                let (src, m) = &cands[i];
                let _ = writeln!(
                    out,
                    "    crossover: \"{name}\" = {} ({src}, exact)",
                    m.get("show").map_or("?", |s| s)
                );
                let full = crossover::details(m, cfg, http, out).unwrap_or_else(|| m.clone());
                for k in &cfg.online.crossover_keys {
                    if let Some(v) = full.get(k) {
                        meta.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                found.push((src.clone(), full));
                // the show came from the channel mapping: evidence it is the same entity
                meta.insert("evidence".into(), "channel".into());
                for (src, m) in &cands {
                    found.push((crate::candidates::source(src), m.clone()));
                }
            }
            Choice::None => {
                let _ = writeln!(
                    out,
                    "    crossover: \"{name}\" is no known show; using {} data alone",
                    label.site
                );
            }
            Choice::Unresolved => {
                unresolved = true;
                let list: Vec<_> = cands
                    .iter()
                    .map(|(_, m)| {
                        format!(
                            "{} ({})",
                            m.get("show").map_or("?", |x| x),
                            m.get("year").map_or("?", |x| x)
                        )
                    })
                    .collect();
                let _ = writeln!(
                    out,
                    "    not possible with the available information: \"{name}\" could be {}",
                    list.join(" / ")
                );
                for (src, m) in &cands {
                    found.push((crate::candidates::source(src), m.clone()));
                }
                meta.insert("resolution".into(), "unresolved".into());
                meta.insert("unresolved_reason".into(), "ambiguous".into());
                meta.insert(
                    "unresolved_detail".into(),
                    format!("\"{name}\" could be {}", list.join(" / ")),
                );
            }
        }
    }
    Ok(Outcome {
        meta,
        found,
        unresolved,
    })
}

/// "20240131" -> "2024-01-31"
pub fn iso_date(d: &str) -> Option<String> {
    let d = d.get(..10).unwrap_or(d);
    if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) {
        return Some(format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]));
    }
    (d.len() == 10).then(|| d.to_string())
}

/// Common fields both the info.json and the API give.
#[allow(clippy::too_many_arguments)]
pub fn put_common(
    m: &mut Meta,
    title: &str,
    channel: &str,
    channel_id: &str,
    desc: &str,
    date: &str,
    tags: &[String],
    thumb: &str,
) {
    let mut put = |k: &str, v: &str| {
        if !v.is_empty() {
            m.insert(k.into(), v.into());
        }
    };
    put("title", title);
    put("channel", channel);
    put("channel_id", channel_id);
    put("description", desc);
    put("poster_url", thumb);
    put("keywords", &tags.join(","));
    if let Some(d) = iso_date(date) {
        put("year", &d[..4]);
        put("date", &d);
    }
    m.insert("kind".into(), KIND.into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fake::FakeHttp;
    use crate::resolver;
    use serde_json::json;

    fn cfg() -> Config {
        let mut c = crate::config::load(Some(Path::new("config/jack.toml"))).unwrap();
        c.providers.youtube.api_key = "k".into();
        c.providers.tmdb.api_key = "t".into();
        c.providers.tmdb.read_token = String::new();
        c
    }
    fn yt(channel: &str) -> serde_json::Value {
        json!({"items":[{"snippet":{"title":"Ep 1","channelTitle":channel,"channelId":"UC1","publishedAt":"2024-01-31T00:00:00Z","description":"d","tags":["a","b"],"thumbnails":{"high":{"url":"http://i/h.jpg"}}}}]})
    }
    fn lbl() -> Label {
        Label {
            site: "youtube".into(),
            id: "abcdefghijk".into(),
            reader: "flag".into(),
            ..Default::default()
        }
    }

    #[test]
    fn confident_crossover_merges_show_ids() {
        let h = FakeHttp::new(vec![
            ("/videos?", yt("The Why Files")),
            (
                "/search/tv",
                json!({"results":[{"id":7,"name":"The Why Files","first_air_date":"2021-01-01"},{"id":8,"name":"Why Women Kill"}]}),
            ),
            ("/tv/7/external_ids", json!({"imdb_id":"tt1","tvdb_id":99})),
        ]);
        let mut out = String::new();
        let o = fetch(
            &lbl(),
            &Meta::new(),
            &cfg(),
            &h,
            &resolver::Strict::default(),
            &mut out,
        )
        .unwrap();
        assert!(!o.unresolved, "{out}");
        assert_eq!(o.meta["kind"], KIND);
        assert_eq!(
            (
                o.meta["show"].as_str(),
                o.meta["tmdb_series_id"].as_str(),
                o.meta["imdb_id"].as_str(),
                o.meta["thetvdb_series_id"].as_str()
            ),
            ("The Why Files", "7", "tt1", "99")
        );
        assert_eq!(o.meta["date"], "2024-01-31");
        // one videos.list, no YouTube search
        assert!(!h.calls.borrow().iter().any(|c| c.contains("/search?")));
    }

    #[test]
    fn no_match_uses_youtube_alone_and_ambiguous_is_unresolved() {
        let h = FakeHttp::new(vec![
            ("/videos?", yt("Dr. Shawn Baker")),
            (
                "/search/tv",
                json!({"results":[{"id":1,"name":"Baker's Dozen"}]}),
            ),
        ]);
        let mut out = String::new();
        let o = fetch(
            &lbl(),
            &Meta::new(),
            &cfg(),
            &h,
            &resolver::Strict::default(),
            &mut out,
        )
        .unwrap();
        assert!(!o.unresolved && !o.meta.contains_key("show"), "{out}");
        assert_eq!(o.meta["channel"], "Dr. Shawn Baker");

        let h = FakeHttp::new(vec![
            ("/videos?", yt("The Office")),
            (
                "/search/tv",
                json!({"results":[{"id":2316,"name":"The Office","first_air_date":"2005"},{"id":2996,"name":"The Office","first_air_date":"2001"}]}),
            ),
        ]);
        let mut out = String::new();
        let o = fetch(
            &lbl(),
            &Meta::new(),
            &cfg(),
            &h,
            &resolver::Strict::default(),
            &mut out,
        )
        .unwrap();
        assert!(
            o.unresolved && out.contains("not possible with the available information"),
            "{out}"
        );
        assert!(!o.meta.contains_key("show"));
        assert_eq!(
            o.found.iter().filter(|(s, _)| s == "tmdb:search").count(),
            2
        );
    }

    #[test]
    fn custom_resolver_hook_decides() {
        struct Sammy;
        impl resolver::Resolver for Sammy {
            fn resolve(&self, _: &str, _: Option<&str>, c: &[resolver::Candidate]) -> Choice {
                if c.is_empty() {
                    Choice::None
                } else {
                    Choice::Pick(c.len() - 1)
                }
            }
        }
        let h = FakeHttp::new(vec![
            ("/videos?", yt("The Office")),
            (
                "/search/tv",
                json!({"results":[{"id":2316,"name":"The Office"},{"id":2996,"name":"The Office"}]}),
            ),
            ("/tv/2996/external_ids", json!({})),
        ]);
        let mut out = String::new();
        let o = fetch(&lbl(), &Meta::new(), &cfg(), &h, &Sammy, &mut out).unwrap();
        assert_eq!(o.meta["tmdb_series_id"], "2996");
    }

    #[test]
    fn show_flags_skip_crossover_search() {
        let h = FakeHttp::new(vec![("/videos?", yt("The Office"))]);
        let f: Meta = [("thetvdb_series_id".to_string(), "73244".to_string())].into();
        let o = fetch(
            &lbl(),
            &f,
            &cfg(),
            &h,
            &resolver::Strict::default(),
            &mut String::new(),
        )
        .unwrap();
        assert_eq!(
            (o.meta["thetvdb_series_id"].as_str(), o.unresolved),
            ("73244", false)
        );
        assert_eq!(
            h.calls
                .borrow()
                .iter()
                .filter(|c| !c.starts_with("hdr"))
                .count(),
            1
        );
    }

    #[test]
    fn flags_take_id_or_url() {
        assert_eq!(
            from_flags(Some("https://youtu.be/abcdefghijk"), None)
                .unwrap()
                .id,
            "abcdefghijk"
        );
        let x = from_flags(None, Some("https://x.com/u/status/123")).unwrap();
        assert_eq!((x.site.as_str(), x.id.as_str()), ("x", "123"));
    }
}
