//! Metadata providers (TMDB, TheTVDB) behind a common trait; HTTP behind a trait for testing.
use crate::meta::Meta;
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;

pub trait Http {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Value, String>;
    fn post(&self, url: &str, body: &Value) -> Result<Value, String>;
    fn bytes(&self, url: &str) -> Result<Vec<u8>, String>;
}

pub struct Ureq;

fn uerr(url: &str, e: ureq::Error) -> String {
    let u = url.split('?').next().unwrap_or(url);
    match e {
        ureq::Error::Status(c, r) => format!(
            "{u}: HTTP {c} {}",
            r.into_string()
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
        ),
        e => format!("{u}: {e}"),
    }
}

impl Http for Ureq {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Value, String> {
        let mut r = ureq::get(url);
        for (k, v) in headers {
            r = r.set(k, v);
        }
        r.call()
            .map_err(|e| uerr(url, e))?
            .into_json()
            .map_err(|e| e.to_string())
    }
    fn post(&self, url: &str, body: &Value) -> Result<Value, String> {
        ureq::post(url)
            .send_json(body)
            .map_err(|e| uerr(url, e))?
            .into_json()
            .map_err(|e| e.to_string())
    }
    fn bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let mut v = Vec::new();
        std::io::Read::read_to_end(
            &mut ureq::get(url)
                .call()
                .map_err(|e| uerr(url, e))?
                .into_reader(),
            &mut v,
        )
        .map_err(|e| e.to_string())?;
        Ok(v)
    }
}

/// What we know about a file before lookup.
#[derive(Debug, Default, Clone)]
pub struct Query {
    pub title: String,
    pub year: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub imdb: Option<String>,
    pub tmdb: Option<String>,
    pub thetvdb: Option<String>,
    /// Episode title guessed from the filename (text after SxxExx), a last-resort fallback.
    pub episode_title: Option<String>,
    /// Forced type: Some("tv") / Some("movie"); None = guess.
    pub kind: Option<String>,
}

impl Query {
    pub fn is_tv(&self) -> bool {
        if let Some(k) = &self.kind {
            return k == "tv";
        }
        self.season.is_some()
            || self.thetvdb.is_some()
            || self.tmdb.as_deref().is_some_and(|t| t.contains("/tv/"))
    }
}

pub trait Provider {
    fn name(&self) -> &'static str;
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String> {
        self.lookup_all(q).map(|(m, _)| m)
    }
    /// The answer plus every hit its search returned (same single call; empty without a search).
    fn lookup_all(&self, q: &Query) -> Result<(Option<Meta>, Vec<Meta>), String> {
        Ok((self.lookup(q)?, vec![]))
    }
    /// False: this provider does not cover this kind of query (e.g. TV-only for a movie); skipped silently.
    fn handles(&self, _q: &Query) -> bool {
        true
    }
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct TmdbCfg {
    pub read_token: String,
    pub api_key: String,
    /// Env vars read when read_token / api_key are both empty.
    pub read_token_env: String,
    pub api_key_env: String,
    pub base_url: String,
    pub image_base: String,
    /// TMDB image size segment, e.g. "w780" or "original" (empty: none).
    pub poster_size: String,
    pub backdrop_size: String,
    pub still_size: String,
    /// Preferred language for landscape art (ISO 639-1, e.g. "en"); null-language art is the fallback.
    pub image_language: String,
    pub language: String,
    /// Filled from [providers] keep_original.
    #[serde(skip)]
    pub keep_original: bool,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct TvdbCfg {
    /// Preferred language for background art (ISO 639-2, e.g. "eng"); untagged is the fallback.
    pub image_language: String,
    /// "full" or "thumbnail" (TheTVDB's "_t" variant).
    pub poster_size: String,
    pub api_key: String,
    /// Env var read when api_key is empty (e.g. THETVDB_API_KEY).
    pub api_key_env: String,
    pub pin: String,
    pub base_url: String,
    /// Translation language (ISO 639-2, e.g. "eng"); empty: from [providers] language via language_codes.
    pub language: String,
    #[serde(skip)]
    pub keep_original: bool,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct OmdbCfg {
    pub api_key: String,
    /// Env var read when api_key is empty.
    pub api_key_env: String,
    pub base_url: String,
    /// "short" or "full".
    pub plot: String,
}

/// No Debug on the provider configs: they hold keys. This one prints only the order.
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct ProvidersCfg {
    pub order: Vec<String>,
    /// Primary display language (ISO 639-1, e.g. "en").
    pub language: String,
    /// Also keep original-language names (original_title / original_show / original_episode_title).
    pub keep_original: bool,
    /// ISO 639-1 -> ISO 639-2 (TheTVDB translations).
    pub language_codes: std::collections::BTreeMap<String, String>,
    /// Append " (year)" to TV show names.
    pub tv_show_year: bool,
    /// Append "Original title: ..." to the description.
    pub original_in_description: bool,
    /// Genres from all providers: "lower", "title" or "asis"; joined with genre_separator.
    pub genre_case: String,
    pub genre_separator: String,
    /// Normalized genre (lowercase, '-'/'_' as spaces) -> the name to use, e.g. "sci fi" = "science fiction".
    pub genre_aliases: std::collections::BTreeMap<String, String>,
    pub tmdb: TmdbCfg,
    pub thetvdb: TvdbCfg,
    pub omdb: OmdbCfg,
    pub wikidata: crate::sources::wikidata::WikidataCfg,
    pub wikipedia: crate::sources::wikipedia::WikipediaCfg,
    pub tvmaze: crate::sources::tvmaze::TvmazeCfg,
    pub itunes: crate::sources::itunes::ItunesCfg,
    pub youtube: crate::sources::youtube::YoutubeCfg,
}

impl std::fmt::Debug for ProvidersCfg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvidersCfg")
            .field("order", &self.order)
            .finish_non_exhaustive()
    }
}

/// "a, b" from `[{"name": "a"}, {"name": "b"}]`.
fn join_names(v: &Value) -> Option<String> {
    let n: Vec<String> = v.as_array()?.iter().filter_map(|g| s(&g["name"])).collect();
    (!n.is_empty()).then(|| n.join(", "))
}

/// "Science-Fiction" -> "science fiction": the form genres are compared in.
pub fn genre_key(g: &str) -> String {
    g.to_lowercase()
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// All genres (comma lists) merged: deduped case-insensitively, cased, sorted, joined.
pub fn merge_genres<'a>(
    lists: impl Iterator<Item = &'a str>,
    case: &str,
    sep: &str,
    aliases: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    let mut out: Vec<String> = vec![];
    let mut seen: Vec<String> = vec![];
    // an alias may name several genres: "sci fi & fantasy" = "science fiction, fantasy"
    let names: Vec<String> = lists
        .flat_map(|l| l.split(','))
        .map(str::trim)
        .filter(|g| !g.is_empty() && *g != "N/A")
        .flat_map(|g| match aliases.get(&genre_key(g)) {
            Some(a) => a.split(',').map(|x| x.trim().to_string()).collect(),
            None => vec![g.to_string()],
        })
        .collect();
    for g in names.iter().map(String::as_str) {
        let key = genre_key(g);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let g = match case {
            "lower" => g.to_lowercase(),
            "title" => g
                .split(' ')
                .map(|w| {
                    let mut c = w.chars();
                    c.next().map_or(String::new(), |f| {
                        f.to_uppercase()
                            .chain(c.flat_map(char::to_lowercase))
                            .collect()
                    })
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => g.to_string(),
        };
        if !out.iter().any(|o| o.to_lowercase() == g.to_lowercase()) {
            out.push(g);
        }
    }
    out.sort_by_key(|g| g.to_lowercase());
    (!out.is_empty()).then(|| out.join(sep))
}

pub fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub(crate) fn s(v: &Value) -> Option<String> {
    match v {
        Value::String(x) if !x.is_empty() => Some(x.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// rating_<src> (0-10) and rating_<src>_votes; 0 votes means unrated.
pub(crate) fn put_rating(m: &mut Meta, src: &str, avg: &Value, votes: &Value) {
    if avg.as_f64().is_some_and(|a| a > 0.0) && votes.as_u64() != Some(0) {
        put(m, &format!("rating_{src}"), s(avg));
        put(m, &format!("rating_{src}_votes"), s(votes));
    }
}

pub(crate) fn put(m: &mut Meta, k: &str, v: Option<String>) {
    if let Some(v) = v {
        m.insert(k.into(), v);
    }
}

/// IMDb / TMDB ids carried on a TheTVDB record, so the other sources can be asked about the same work.
fn remote_ids(m: &mut Meta, v: &Value) {
    let Some(ids) = v.get("remoteIds").and_then(|x| x.as_array()) else {
        return;
    };
    for r in ids {
        let src = s(&r["sourceName"]).unwrap_or_default().to_ascii_lowercase();
        let id = s(&r["id"]);
        if src.contains("imdb") {
            put(m, "imdb_id", id);
        } else if src.contains("tmdb") || src.contains("themoviedb") {
            let series = id.clone();
            put(m, "tmdb_id", id);
            put(m, "tmdb_series_id", series);
        }
    }
}

/// First run of digits after `marker` (or anywhere if marker is empty).
fn digits_after(x: &str, marker: &str) -> Option<String> {
    let i = if marker.is_empty() {
        0
    } else {
        x.find(marker)? + marker.len()
    };
    let d: String = x[i..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    (!d.is_empty()).then_some(d)
}

/// "The Office (US)" -> "The Office": a 2-3 letter uppercase country tag in parentheses.
pub fn strip_country_suffix(n: &str) -> String {
    let t = n.trim_end();
    if let Some(i) = t.rfind(" (") {
        let tag = &t[i + 2..t.len().saturating_sub(1)];
        if t.ends_with(')')
            && (2..=3).contains(&tag.len())
            && tag.chars().all(|c| c.is_ascii_uppercase())
        {
            return t[..i].to_string();
        }
    }
    t.to_string()
}

/// TheTVDB disambiguates duplicate names with a trailing " (YYYY)"; strip it.
pub fn strip_year_suffix(n: &str) -> String {
    let t = n.trim_end();
    match t.strip_suffix(')').and_then(|x| x.rsplit_once(" (")) {
        Some((head, y)) if y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()) => {
            head.trim_end().to_string()
        }
        _ => t.to_string(),
    }
}

fn norm(x: &str) -> String {
    strip_year_suffix(x)
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Strips a size segment ("/original", "/w780", "/h632") and trailing slashes; true if changed.
pub fn normalize_image_base(b: &str) -> (String, bool) {
    let t = b.trim_end_matches('/');
    let t = match t.rsplit_once('/') {
        Some((head, seg))
            if seg == "original"
                || (seg.len() > 1
                    && seg[1..].chars().all(|c| c.is_ascii_digit())
                    && matches!(&seg[..1], "w" | "h")) =>
        {
            head
        }
        _ => t,
    };
    (t.to_string(), t != b)
}

/// Original-language name, when kept and different from the display name.
fn put_original(m: &mut Meta, keep: bool, key: &str, orig: Option<String>, shown: Option<&String>) {
    if keep {
        put(m, key, orig.filter(|o| Some(o) != shown));
    }
}

pub fn imdb_id(x: &str) -> Option<String> {
    digits_after(x, "tt").map(|d| format!("tt{d}"))
}

/// An --ask-art candidate image with API metadata (w/h 0 = unknown; lrank: 0 primary, 1 original,
/// 2 untagged/unknown, 3 other language).
#[derive(Debug, Clone)]
pub struct Img {
    pub provider: String,
    pub kind: &'static str,
    pub url: String,
    pub lang: String,
    pub lrank: u8,
    pub w: u64,
    pub h: u64,
}

fn lrank(l: &str, primary: &str, orig: &str) -> u8 {
    match l {
        "" => 2,
        l if l == primary => 0,
        l if l == orig => 1,
        _ => 3,
    }
}

pub struct Tmdb<'a> {
    pub cfg: TmdbCfg,
    pub http: &'a dyn Http,
}

impl Tmdb<'_> {
    fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value, String> {
        let mut url = format!(
            "{}{path}?language={}",
            self.cfg.base_url,
            enc(&self.cfg.language)
        );
        let mut h = vec![];
        if !self.cfg.read_token.is_empty() {
            h.push(("Authorization", format!("Bearer {}", self.cfg.read_token)));
        } else if !self.cfg.api_key.is_empty() {
            url += &format!("&api_key={}", enc(&self.cfg.api_key));
        } else {
            return Err("no API key; set [providers.tmdb] read_token with --tmdb-token TOKEN --set (or build with JACK_TMDB_TOKEN)".into());
        }
        for (k, v) in params {
            url += &format!("&{k}={}", enc(v));
        }
        self.http.get(&url, &h)
    }
    /// All images (--ask-art only): /images for the movie or series, season and episode, in the
    /// primary + original language and untagged. Failures are skipped.
    pub fn images(&self, m: &Meta) -> Vec<Img> {
        let pl = self
            .cfg
            .language
            .split('-')
            .next()
            .unwrap_or("")
            .to_string();
        let ol = m.get("original_language").cloned().unwrap_or_default();
        let incl = [(
            "include_image_language",
            format!("{pl},{ol},null").replace(",,", ","),
        )];
        let tv = m.get("kind").is_some_and(|k| k == "episode") || m.contains_key("tmdb_series_id");
        let mut paths: Vec<(String, &[(&str, &'static str)])> = vec![];
        if tv {
            let Some(id) = m.get("tmdb_series_id") else {
                return vec![];
            };
            paths.push((
                format!("/tv/{id}/images"),
                &[("posters", "poster"), ("backdrops", "backdrop")],
            ));
            if let Some(se) = m.get("season") {
                paths.push((
                    format!("/tv/{id}/season/{se}/images"),
                    &[("posters", "season")],
                ));
                if let Some(ep) = m.get("episode") {
                    paths.push((
                        format!("/tv/{id}/season/{se}/episode/{ep}/images"),
                        &[("stills", "still")],
                    ));
                }
            }
        } else if let Some(id) = m.get("tmdb_id") {
            paths.push((
                format!("/movie/{id}/images"),
                &[("posters", "poster"), ("backdrops", "backdrop")],
            ));
        }
        let mut v = vec![];
        for (path, lists) in paths {
            let Ok(r) = self.get(&path, &incl) else {
                continue;
            };
            for (field, kind) in lists {
                for a in r[*field].as_array().into_iter().flatten() {
                    let Some(url) = self.img_sz(&a["file_path"], "original") else {
                        continue;
                    };
                    let lang = s(&a["iso_639_1"]).unwrap_or_default();
                    let (w, h) = (
                        a["width"].as_u64().unwrap_or(0),
                        a["height"].as_u64().unwrap_or(0),
                    );
                    v.push(Img {
                        provider: "tmdb".into(),
                        kind,
                        url,
                        lrank: lrank(&lang, &pl, &ol),
                        lang,
                        w,
                        h,
                    });
                }
            }
        }
        v
    }
    fn img(&self, v: &Value) -> Option<String> {
        self.img_sz(v, &self.cfg.poster_size)
    }
    fn img_sz(&self, v: &Value, size: &str) -> Option<String> {
        let sz = if size.is_empty() {
            String::new()
        } else {
            format!("/{size}")
        };
        s(v).map(|p| format!("{}{sz}{p}", self.cfg.image_base.trim_end_matches('/')))
    }
}

/// Meta key a provider uses for a non-fatal warning (removed before merging).
pub const WARNING: &str = "_warning";

impl Tmdb<'_> {
    /// S/E as numbered, else via episode groups (season = group order), else absolute numbering in season 1.
    fn episode(&self, id: &str, d: &Value, se: u32, ep: u32) -> Option<Value> {
        if let Ok(e) = self.get(&format!("/tv/{id}/season/{se}/episode/{ep}"), &[]) {
            return Some(e);
        }
        if let Ok(g) = self.get(&format!("/tv/{id}/episode_groups"), &[]) {
            for gid in g["results"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| s(&r["id"]))
            {
                let Ok(x) = self.get(&format!("/tv/episode_group/{gid}"), &[]) else {
                    continue;
                };
                let grp = x["groups"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|v| v["order"].as_u64() == Some(se as u64));
                if let Some(e) = grp.and_then(|v| {
                    v["episodes"]
                        .as_array()?
                        .iter()
                        .find(|e| e["order"].as_u64() == Some(ep as u64 - 1))
                        .cloned()
                }) {
                    return Some(e);
                }
            }
        }
        let before: u64 = d["seasons"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| {
                v["season_number"]
                    .as_u64()
                    .is_some_and(|n| n >= 1 && n < se as u64)
            })
            .filter_map(|v| v["episode_count"].as_u64())
            .sum();
        if se > 1 && before > 0 {
            return self
                .get(
                    &format!("/tv/{id}/season/1/episode/{}", before + ep as u64),
                    &[],
                )
                .ok();
        }
        None
    }
}

impl Provider for Tmdb<'_> {
    fn name(&self) -> &'static str {
        "tmdb"
    }
    fn lookup_all(&self, q: &Query) -> Result<(Option<Meta>, Vec<Meta>), String> {
        let mut c = vec![];
        Ok((self.find(q, &mut c)?, c))
    }
}

impl Tmdb<'_> {
    fn find(&self, q: &Query, cands: &mut Vec<Meta>) -> Result<Option<Meta>, String> {
        let mut tv = q.is_tv();
        let mut id = q.tmdb.as_deref().and_then(|t| {
            digits_after(t, if t.contains('/') { "/movie/" } else { "" })
                .or_else(|| digits_after(t, "/tv/"))
        });
        if id.is_none() {
            if let Some(tt) = q.imdb.as_deref().and_then(imdb_id) {
                let r = self.get(
                    &format!("/find/{tt}"),
                    &[("external_source", "imdb_id".into())],
                )?;
                if let Some(x) = r["tv_results"][0]["id"]
                    .as_u64()
                    .filter(|_| q.kind.as_deref() != Some("movie"))
                {
                    (tv, id) = (true, Some(x.to_string()));
                } else {
                    id = r["movie_results"][0]["id"].as_u64().map(|x| x.to_string());
                }
            }
        }
        // A trusted id from another source: look this work up, don't take a loose title hit.
        if id.is_none() {
            if let Some(tvdb) = q
                .thetvdb
                .as_deref()
                .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            {
                let r = self.get(
                    &format!("/find/{tvdb}"),
                    &[("external_source", "tvdb_id".into())],
                )?;
                if let Some(x) = r["tv_results"][0]["id"]
                    .as_u64()
                    .filter(|_| q.kind.as_deref() != Some("movie"))
                {
                    (tv, id) = (true, Some(x.to_string()));
                } else if q.kind.as_deref() != Some("tv") {
                    if let Some(x) = r["movie_results"][0]["id"].as_u64() {
                        (tv, id) = (false, Some(x.to_string()));
                    }
                }
            }
        }
        let trusted = [&q.imdb, &q.tmdb, &q.thetvdb]
            .iter()
            .any(|v| v.as_ref().is_some_and(|s| !s.is_empty()));
        if id.is_none() && !q.title.is_empty() && !trusted {
            let mut p = vec![("query", q.title.clone())];
            if let Some(y) = &q.year {
                p.push((if tv { "first_air_date_year" } else { "year" }, y.clone()));
            }
            let r = self.get(if tv { "/search/tv" } else { "/search/movie" }, &p)?;
            id = r["results"][0]["id"].as_u64().map(|x| x.to_string());
            cands.extend(crate::candidates::tmdb(&r, tv));
        }
        let Some(id) = id else { return Ok(None) };
        let d = self.get(
            &format!("/{}/{id}", if tv { "tv" } else { "movie" }),
            &[
                ("append_to_response", "external_ids,images".into()),
                (
                    "include_image_language",
                    format!("{},null", self.cfg.image_language),
                ),
            ],
        )?;
        let mut m = Meta::new();
        let name = s(&d[if tv { "name" } else { "title" }]);
        put_original(
            &mut m,
            self.cfg.keep_original,
            "original_title",
            s(&d[if tv {
                "original_name"
            } else {
                "original_title"
            }]),
            name.as_ref(),
        );
        let date = s(&d[if tv { "first_air_date" } else { "release_date" }]);
        put(
            &mut m,
            "year",
            date.as_ref().map(|x| x.chars().take(4).collect()),
        );
        put(&mut m, "description", s(&d["overview"]));
        put_rating(&mut m, "tmdb", &d["vote_average"], &d["vote_count"]);
        put(&mut m, "genre", join_names(&d["genres"]));
        put(&mut m, "poster_url", self.img(&d["poster_path"]));
        put(&mut m, "tmdb_id", Some(id.clone()));
        put(&mut m, "original_language", s(&d["original_language"]));
        let bd = d["images"]["backdrops"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let pick = bd
            .iter()
            .find(|b| b["iso_639_1"].as_str() == Some(self.cfg.image_language.as_str()))
            .or_else(|| bd.iter().find(|b| b["iso_639_1"].is_null()))
            .map(|b| b["file_path"].clone())
            .unwrap_or_else(|| d["backdrop_path"].clone());
        put(
            &mut m,
            "backdrop_url",
            self.img_sz(&pick, &self.cfg.backdrop_size),
        );
        put(
            &mut m,
            "imdb_id",
            s(&d["imdb_id"]).or_else(|| s(&d["external_ids"]["imdb_id"])),
        );
        if tv {
            put(&mut m, "tmdb_series_id", Some(id.clone()));
            m.insert("kind".into(), "episode".into());
            put(&mut m, "show", name.clone());
            put(&mut m, "title", name);
            if let Some(o) = m.get("original_title").cloned() {
                m.insert("original_show".into(), o);
            }
            if let Some(se) = q.season {
                // season poster is optional; ignore failures
                if let Ok(sd) = self.get(&format!("/tv/{id}/season/{se}"), &[]) {
                    put(&mut m, "season_poster_url", self.img(&sd["poster_path"]));
                }
            }
            if let (Some(se), Some(ep)) = (q.season, q.episode) {
                m.insert("season".into(), se.to_string());
                m.insert("episode".into(), ep.to_string());
                // a missing episode (other numbering) keeps the series data; other providers fill in
                let Some(e) = self.episode(&id, &d, se, ep) else {
                    m.insert(WARNING.into(), format!("no S{se:02}E{ep:02} in TMDB (tried episode groups and absolute numbering); series data only"));
                    return Ok(Some(m));
                };
                let name = s(&e["name"]);
                // original-language episode name: same episode in the show's original language
                let ol = s(&d["original_language"]).filter(|l| {
                    self.cfg.keep_original && !self.cfg.language.starts_with(l.as_str())
                });
                if let (Some(ol), Some(sn), Some(en)) =
                    (ol, s(&e["season_number"]), s(&e["episode_number"]))
                {
                    let t = Tmdb {
                        cfg: TmdbCfg {
                            language: ol,
                            ..self.cfg.clone()
                        },
                        http: self.http,
                    };
                    let o = t
                        .get(&format!("/tv/{id}/season/{sn}/episode/{en}"), &[])
                        .ok()
                        .and_then(|o| s(&o["name"]));
                    put_original(&mut m, true, "original_episode_title", o, name.as_ref());
                }
                put(&mut m, "episode_title", name);
                put(&mut m, "description", s(&e["overview"]));
                put_rating(&mut m, "tmdb", &e["vote_average"], &e["vote_count"]);
                put(&mut m, "date", s(&e["air_date"]));
                put(&mut m, "tmdb_episode_id", s(&e["id"]));
                put(
                    &mut m,
                    "still_url",
                    self.img_sz(&e["still_path"], &self.cfg.still_size),
                );
            }
        } else {
            m.insert("kind".into(), "movie".into());
            put(&mut m, "title", name);
            put(&mut m, "date", date);
        }
        Ok(Some(m))
    }
}

pub struct Tvdb<'a> {
    pub cfg: TvdbCfg,
    pub http: &'a dyn Http,
    token: RefCell<Option<String>>,
}

impl<'a> Tvdb<'a> {
    pub fn new(cfg: TvdbCfg, http: &'a dyn Http) -> Self {
        Tvdb {
            cfg,
            http,
            token: RefCell::new(None),
        }
    }
    fn get(&self, path: &str) -> Result<Value, String> {
        if self.token.borrow().is_none() {
            if self.cfg.api_key.is_empty() {
                return Err("no API key; set [providers.thetvdb] api_key with --tvdb-key KEY --set (and --tvdb-pin PIN if required), or build with JACK_TVDB_KEY".into());
            }
            let mut b = json!({ "apikey": self.cfg.api_key });
            if !self.cfg.pin.is_empty() {
                b["pin"] = json!(self.cfg.pin);
            }
            let r = self
                .http
                .post(&format!("{}/login", self.cfg.base_url), &b)?;
            let t = s(&r["data"]["token"]).ok_or("TheTVDB: login returned no token")?;
            *self.token.borrow_mut() = Some(t);
        }
        let t = self.token.borrow().clone().unwrap_or_default();
        Ok(self.http.get(
            &format!("{}{path}", self.cfg.base_url),
            &[("Authorization", format!("Bearer {t}"))],
        )?["data"]
            .clone())
    }
}

impl Tvdb<'_> {
    /// `/<kind>/<id>/translations/<language>` (Null if none).
    /// All artworks (--ask-art only): series/movie extended artworks (poster, background) and the
    /// season's (poster), with language and size. Failures are skipped.
    pub fn images(&self, m: &Meta) -> Vec<Img> {
        let tv =
            m.get("kind").is_some_and(|k| k == "episode") || m.contains_key("thetvdb_series_id");
        let id = if tv {
            m.get("thetvdb_series_id")
        } else {
            m.get("thetvdb_id")
        };
        let Some(id) = id else { return vec![] };
        let Ok(x) = self.get(&format!(
            "/{}/{id}/extended",
            if tv { "series" } else { "movies" }
        )) else {
            return vec![];
        };
        let (pl, ol) = (
            self.cfg.image_language.clone(),
            s(&x["originalLanguage"]).unwrap_or_default(),
        );
        let mut v = vec![];
        let add = |arts: &Value, v: &mut Vec<Img>| {
            for a in arts.as_array().into_iter().flatten() {
                let kind = match a["type"].as_u64() {
                    Some(2 | 14) => "poster",
                    Some(3 | 15) => "backdrop",
                    Some(7) => "season",
                    _ => continue,
                };
                let Some(url) = s(&a["image"]) else { continue };
                let lang = s(&a["language"]).unwrap_or_default();
                let (w, h) = (
                    a["width"].as_u64().unwrap_or(0),
                    a["height"].as_u64().unwrap_or(0),
                );
                v.push(Img {
                    provider: "thetvdb".into(),
                    kind,
                    url,
                    lrank: lrank(&lang, &pl, &ol),
                    lang,
                    w,
                    h,
                });
            }
        };
        add(&x["artworks"], &mut v);
        if let Some(se) = m.get("season").and_then(|x| x.parse::<u64>().ok()) {
            let ss = x["seasons"].as_array().cloned().unwrap_or_default();
            let hit = ss.iter().find(|v| {
                v["number"].as_u64() == Some(se)
                    && v["type"]["type"].as_str().is_none_or(|t| t == "official")
            });
            if let Some(sid) = hit.and_then(|h| s(&h["id"])) {
                if let Ok(sx) = self.get(&format!("/seasons/{sid}/extended")) {
                    add(&sx["artwork"], &mut v);
                }
            }
        }
        v
    }
    fn translation(&self, kind: &str, id: &str) -> Value {
        self.translation_in(kind, id, &self.cfg.language)
    }
    fn translation_in(&self, kind: &str, id: &str, lang: &str) -> Value {
        if lang.is_empty() {
            return Value::Null;
        }
        self.get(&format!("/{kind}/{id}/translations/{lang}"))
            .unwrap_or(Value::Null)
    }
    /// Best search hit: exact title (any name/translation/alias), the query year, not a future entry, then relevance.
    fn best(&self, r: &Value, q: &Query) -> Option<String> {
        let now: u64 = 1970
            + std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() / 31_556_952);
        let want = norm(&q.title);
        let hits = r.as_array()?;
        let key = |h: &Value| {
            let mut names: Vec<String> = [&h["name"], &h["name_translated"]]
                .iter()
                .filter_map(|v| s(v))
                .collect();
            for v in [&h["translations"], &h["aliases"]] {
                match v {
                    Value::Object(o) => names.extend(o.values().filter_map(s)),
                    Value::Array(a) => names.extend(a.iter().filter_map(s)),
                    _ => {}
                }
            }
            let year = s(&h["year"]).and_then(|y| y.parse::<u64>().ok());
            let exact = names.iter().any(|n| norm(n) == want);
            let year_ok = q
                .year
                .as_ref()
                .is_none_or(|y| year.map(|x| x.to_string()) == Some(y.clone()));
            (!exact, !year_ok, year.is_some_and(|y| y > now))
        };
        hits.iter()
            .enumerate()
            .min_by_key(|(i, h)| (key(h), *i))
            .and_then(|(_, h)| s(&h["tvdb_id"]))
    }
    /// Movie search (only when the type is forced to movie).
    fn movie(&self, q: &Query, cands: &mut Vec<Meta>) -> Result<Option<Meta>, String> {
        let id = match q
            .thetvdb
            .as_deref()
            .filter(|x| x.chars().all(|c| c.is_ascii_digit()))
        {
            Some(x) => x.to_string(),
            None if !q.title.is_empty() => {
                let yr = q
                    .year
                    .as_ref()
                    .map(|y| format!("&year={}", enc(y)))
                    .unwrap_or_default();
                let r = self.get(&format!("/search?type=movie&query={}{yr}", enc(&q.title)))?;
                cands.extend(crate::candidates::tvdb(&r, false));
                let Some(id) = self.best(&r, q) else {
                    return Ok(None);
                };
                id
            }
            None => return Ok(None),
        };
        let d = self.get(&format!("/movies/{id}"))?;
        let mut m = Meta::new();
        m.insert("kind".into(), "movie".into());
        let orig = s(&d["name"]).map(|n| strip_year_suffix(&n));
        let tr = self.translation("movies", &id);
        let name = s(&tr["name"])
            .map(|n| strip_year_suffix(&n))
            .or(orig.clone());
        put_original(
            &mut m,
            self.cfg.keep_original,
            "original_title",
            orig,
            name.as_ref(),
        );
        put(&mut m, "title", name);
        put(
            &mut m,
            "description",
            s(&tr["overview"]).or(s(&d["overview"])),
        );
        put(&mut m, "year", s(&d["year"]));
        put(&mut m, "poster_url", s(&d["image"]));
        put(&mut m, "thetvdb_id", Some(id));
        Ok(Some(m))
    }
}

impl Provider for Tvdb<'_> {
    fn name(&self) -> &'static str {
        "thetvdb"
    }
    fn lookup_all(&self, q: &Query) -> Result<(Option<Meta>, Vec<Meta>), String> {
        let mut c = vec![];
        Ok((self.find(q, &mut c)?, c))
    }
}

impl Tvdb<'_> {
    fn find(&self, q: &Query, cands: &mut Vec<Meta>) -> Result<Option<Meta>, String> {
        if q.kind.as_deref() == Some("movie") {
            return self.movie(q, cands);
        }
        if !q.is_tv() {
            return Ok(None);
        }
        let series = match q.thetvdb.as_deref() {
            Some(x) if x.chars().all(|c| c.is_ascii_digit()) => {
                self.get(&format!("/series/{x}"))?
            }
            Some(x) if x.contains("/series/") => {
                let slug = x
                    .split("/series/")
                    .nth(1)
                    .unwrap_or("")
                    .split(['/', '?', '#'])
                    .next()
                    .unwrap_or("");
                self.get(&format!("/series/slug/{}", enc(slug)))?
            }
            _ if !q.title.is_empty() => {
                let yr = q
                    .year
                    .as_ref()
                    .map(|y| format!("&year={}", enc(y)))
                    .unwrap_or_default();
                let r = self.get(&format!("/search?type=series&query={}{yr}", enc(&q.title)))?;
                cands.extend(crate::candidates::tvdb(&r, true));
                let Some(id) = self.best(&r, q) else {
                    return Ok(None);
                };
                self.get(&format!("/series/{id}"))?
            }
            _ => return Ok(None),
        };
        let Some(id) = s(&series["id"]) else {
            return Ok(None);
        };
        let mut m = Meta::new();
        m.insert("kind".into(), "episode".into());
        // `name` is the original-language name (may carry a " (YYYY)" disambiguator); `year` can be off,
        // so the year comes from firstAired.
        let orig = s(&series["name"]).map(|n| strip_year_suffix(&n));
        let tr = self.translation("series", &id);
        let name = s(&tr["name"])
            .map(|n| strip_year_suffix(&n))
            .or(orig.clone());
        put_original(
            &mut m,
            self.cfg.keep_original,
            "original_show",
            orig.clone(),
            name.as_ref(),
        );
        put_original(
            &mut m,
            self.cfg.keep_original,
            "original_title",
            orig,
            name.as_ref(),
        );
        put(&mut m, "show", name.clone());
        put(&mut m, "title", name);
        put(
            &mut m,
            "year",
            s(&series["firstAired"])
                .map(|d| d.chars().take(4).collect())
                .or(s(&series["year"])),
        );
        put(
            &mut m,
            "description",
            s(&tr["overview"]).or(s(&series["overview"])),
        );
        let img = |u: Option<String>| match self.cfg.poster_size.as_str() {
            "thumbnail" => u.map(|u| match u.rfind('.') {
                Some(i) if !u[i..].contains('/') => format!("{}_t{}", &u[..i], &u[i..]),
                _ => u,
            }),
            _ => u,
        };
        put(&mut m, "poster_url", img(s(&series["image"])));
        // extended record: season posters + background (fanart) artwork; optional
        if let Ok(x) = self.get(&format!(
            "/series/{}/extended",
            s(&series["id"]).unwrap_or_default()
        )) {
            if let Some(se) = q.season {
                let ss = x["seasons"].as_array().cloned().unwrap_or_default();
                let hit = ss.iter().find(|v| {
                    v["number"].as_u64() == Some(se as u64)
                        && v["type"]["type"].as_str().is_none_or(|t| t == "official")
                });
                put(
                    &mut m,
                    "season_poster_url",
                    img(hit.and_then(|v| s(&v["image"]))),
                );
            }
            let art = x["artworks"].as_array().cloned().unwrap_or_default();
            let bg: Vec<_> = art
                .iter()
                .filter(|a| a["type"].as_u64() == Some(3))
                .collect();
            let pick = bg
                .iter()
                .find(|a| a["language"].as_str() == Some(self.cfg.image_language.as_str()))
                .or_else(|| bg.iter().find(|a| a["language"].is_null()))
                .or(bg.first());
            put(
                &mut m,
                "backdrop_url",
                img(pick.and_then(|a| s(&a["image"]))),
            );
            put(&mut m, "genre", join_names(&x["genres"]));
        }
        put(&mut m, "thetvdb_id", Some(id.clone()));
        put(&mut m, "thetvdb_series_id", Some(id.clone()));
        remote_ids(&mut m, &series);
        if let (Some(se), Some(ep)) = (q.season, q.episode) {
            m.insert("season".into(), se.to_string());
            m.insert("episode".into(), ep.to_string());
            let r = self.get(&format!(
                "/series/{id}/episodes/default?page=0&season={se}&episodeNumber={ep}"
            ));
            let e = r.map(|r| r["episodes"][0].clone()).unwrap_or(Value::Null);
            if e.is_null() {
                m.insert(
                    WARNING.into(),
                    format!("no S{se:02}E{ep:02} in TheTVDB (default order); series data only"),
                );
                return Ok(Some(m));
            }
            let tr = s(&e["id"]).map_or(Value::Null, |eid| self.translation("episodes", &eid));
            // the episode list may already be translated; ask for the original language explicitly
            let ol = s(&series["originalLanguage"])
                .filter(|l| self.cfg.keep_original && *l != self.cfg.language);
            let otr = match (ol, s(&e["id"])) {
                (Some(l), Some(eid)) => self.translation_in("episodes", &eid, &l),
                _ => Value::Null,
            };
            let orig = s(&otr["name"]).or(s(&e["name"]));
            let name = s(&tr["name"]).or(s(&e["name"]));
            put_original(
                &mut m,
                self.cfg.keep_original,
                "original_episode_title",
                orig,
                name.as_ref(),
            );
            put(&mut m, "episode_title", name);
            put(
                &mut m,
                "description",
                s(&tr["overview"]).or(s(&e["overview"])),
            );
            put(&mut m, "date", s(&e["aired"]));
            put(&mut m, "thetvdb_episode_id", s(&e["id"]));
            put(&mut m, "still_url", s(&e["image"]));
        }
        Ok(Some(m))
    }
}

/// OMDb (https://www.omdbapi.com): ratings, rated, awards, plot, poster.
pub struct Omdb<'a> {
    pub cfg: OmdbCfg,
    pub http: &'a dyn Http,
}

impl Provider for Omdb<'_> {
    fn name(&self) -> &'static str {
        "omdb"
    }
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String> {
        if self.cfg.api_key.is_empty() {
            return Err("no api key ([providers.omdb] api_key or --omdb-key); skipped".into());
        }
        let mut url = format!(
            "{}?apikey={}&plot={}",
            self.cfg.base_url,
            enc(&self.cfg.api_key),
            enc(&self.cfg.plot)
        );
        match q.imdb.as_deref().and_then(imdb_id) {
            Some(i) => url += &format!("&i={i}"),
            None => {
                url += &format!(
                    "&t={}&type={}",
                    enc(&strip_year_suffix(&strip_country_suffix(&q.title))),
                    if q.is_tv() { "series" } else { "movie" }
                );
                if let Some(y) = &q.year {
                    url += &format!("&y={}", enc(y));
                }
            }
        }
        // never let the key reach an error message
        let d = self
            .http
            .get(&url, &[])
            .map_err(|e| e.replace(&self.cfg.api_key, "****"))?;
        if d["Response"].as_str() == Some("False") {
            return Ok(None);
        }
        let g = |k: &str| s(&d[k]).filter(|v| v != "N/A");
        let mut m = Meta::new();
        put(&mut m, "title", g("Title"));
        put(
            &mut m,
            "year",
            g("Year").map(|y| y.chars().take(4).collect()),
        );
        put(&mut m, "imdb_id", g("imdbID"));
        put(&mut m, "rated", g("Rated"));
        put(&mut m, "genre", g("Genre"));
        put(&mut m, "awards", g("Awards"));
        put(&mut m, "description", g("Plot"));
        put(&mut m, "poster_url", g("Poster"));
        put(&mut m, "rating_imdb", g("imdbRating"));
        put(
            &mut m,
            "rating_imdb_votes",
            g("imdbVotes").map(|v| v.replace(',', "")),
        );
        for r in d["Ratings"].as_array().into_iter().flatten() {
            let k = match r["Source"].as_str() {
                Some("Internet Movie Database") => "rating_imdb",
                Some("Rotten Tomatoes") => "rating_rotten_tomatoes",
                Some("Metacritic") => "rating_metacritic",
                _ => continue,
            };
            put(&mut m, k, s(&r["Value"]).filter(|v| v != "N/A"));
        }
        Ok(Some(m))
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;
    /// Responds with the first route whose key is a substring of the URL; records calls.
    pub struct FakeHttp {
        pub routes: Vec<(&'static str, Value)>,
        pub calls: RefCell<Vec<String>>,
    }
    impl FakeHttp {
        pub fn new(routes: Vec<(&'static str, Value)>) -> Self {
            FakeHttp {
                routes,
                calls: RefCell::new(vec![]),
            }
        }
        fn hit(&self, url: &str) -> Result<Value, String> {
            self.calls.borrow_mut().push(url.into());
            self.routes
                .iter()
                .find(|(k, _)| url.contains(k))
                .map(|r| r.1.clone())
                .ok_or(format!("unrouted {url}"))
        }
    }
    impl Http for FakeHttp {
        fn get(&self, url: &str, h: &[(&str, String)]) -> Result<Value, String> {
            self.calls.borrow_mut().push(format!("hdr {h:?}"));
            self.hit(url)
        }
        fn post(&self, url: &str, b: &Value) -> Result<Value, String> {
            self.calls.borrow_mut().push(format!("body {b}"));
            self.hit(url)
        }
        fn bytes(&self, url: &str) -> Result<Vec<u8>, String> {
            // a route value "@<path>" serves that file's bytes
            self.hit(url)
                .map(|v| match v.as_str().and_then(|p| p.strip_prefix('@')) {
                    Some(p) => std::fs::read(p).unwrap(),
                    None => b"JPEG".to_vec(),
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeHttp;
    use super::*;

    fn tmdb(h: &FakeHttp) -> Tmdb<'_> {
        let cfg = TmdbCfg {
            read_token: "tok".into(),
            base_url: "http://t".into(),
            image_base: "http://img".into(),
            language: "en-US".into(),
            ..Default::default()
        };
        Tmdb { cfg, http: h }
    }

    #[test]
    fn tmdb_episode_group_and_absolute_fallbacks() {
        let q = Query {
            title: "Show".into(),
            season: Some(2),
            episode: Some(3),
            ..Default::default()
        };
        let det = json!({"name":"Show","seasons":[{"season_number":0,"episode_count":5},{"season_number":1,"episode_count":10},{"season_number":2,"episode_count":4}]});
        let h = FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":7}]})),
            ("/tv/7/episode_groups", json!({"results":[{"id":"g1"}]})),
            (
                "/tv/episode_group/g1",
                json!({"groups":[{"order":2,"episodes":[{"name":"x"},{"order":2,"name":"Grouped"}]}]}),
            ),
            ("/tv/7?", det.clone()),
        ]);
        assert_eq!(
            tmdb(&h).lookup(&q).unwrap().unwrap()["episode_title"],
            "Grouped"
        );
        let h = FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":7}]})),
            ("/tv/7/season/1/episode/13", json!({"name":"Absolute"})),
            ("/tv/7?", det),
        ]);
        let m = tmdb(&h).lookup(&q).unwrap().unwrap();
        assert_eq!(
            (m["episode_title"].as_str(), m.contains_key(WARNING)),
            ("Absolute", false)
        );
    }

    #[test]
    fn country_suffix_stripped() {
        assert_eq!(strip_country_suffix("The Office (US)"), "The Office");
        assert_eq!(
            strip_country_suffix("Doctor Who (2005)"),
            "Doctor Who (2005)"
        );
        assert_eq!(
            strip_year_suffix(&strip_country_suffix("Doctor Who (2005)")),
            "Doctor Who"
        );
        assert_eq!(strip_country_suffix("Friends"), "Friends");
    }

    #[test]
    fn genres_merged() {
        let l = [
            "Animation, Comedy",
            "comedy, Family",
            "N/A",
            "Sci-Fi & Fantasy",
        ];
        assert_eq!(
            merge_genres(l.iter().copied(), "lower", ",", &Default::default()).unwrap(),
            "animation,comedy,family,sci-fi & fantasy"
        );
        assert_eq!(
            merge_genres(
                ["action ADVENTURE"].into_iter(),
                "title",
                ", ",
                &Default::default()
            )
            .unwrap(),
            "Action Adventure"
        );
        let al = [
            ("sci fi", "science fiction"),
            ("sci fi & fantasy", "science fiction, fantasy"),
        ]
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .into();
        assert_eq!(
            merge_genres(
                [
                    "Science Fiction, drama",
                    "science-fiction,Sci-Fi",
                    "DRAMA",
                    "Sci-Fi & Fantasy"
                ]
                .into_iter(),
                "lower",
                ",",
                &al
            )
            .unwrap(),
            "drama,fantasy,science fiction"
        );
        assert!(merge_genres([""].into_iter(), "lower", ",", &Default::default()).is_none());
    }

    #[test]
    fn helpers() {
        assert_eq!(
            strip_year_suffix("薬屋のひとりごと (2027)"),
            "薬屋のひとりごと"
        );
        assert_eq!(
            normalize_image_base("https://i/t/p/w500/"),
            ("https://i/t/p".into(), true)
        );
        assert_eq!(
            normalize_image_base("https://i/t/p"),
            ("https://i/t/p".into(), false)
        );
    }

    #[test]
    fn tmdb_movie_via_imdb_find() {
        let h = FakeHttp::new(vec![
            (
                "/find/tt0133093",
                json!({"movie_results":[{"id":603}],"tv_results":[]}),
            ),
            (
                "/movie/603",
                json!({"title":"The Matrix","release_date":"1999-03-30","overview":"Neo","genres":[{"name":"Action"}],"poster_path":"/p.jpg"}),
            ),
        ]);
        let m = tmdb(&h)
            .lookup(&Query {
                imdb: Some("https://www.imdb.com/title/tt0133093/".into()),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            (m["kind"].as_str(), m["title"].as_str(), m["year"].as_str()),
            ("movie", "The Matrix", "1999")
        );
        assert_eq!(m["poster_url"], "http://img/p.jpg");
        assert!(h.calls.borrow().iter().any(|c| c.contains("Bearer tok")));
    }

    #[test]
    fn tmdb_tv_search_and_episode() {
        let h = FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":42}]})),
            ("/tv/42/season/1?", json!({"poster_path":"/s1.jpg"})),
            (
                "/tv/42/season/1/episode/2",
                json!({"id":4202,"name":"Ep Two","air_date":"2016-07-11","overview":"o"}),
            ),
            (
                "/tv/42",
                json!({"name":"Saiki K","first_air_date":"2016-07-04","external_ids":{"imdb_id":"tt5"}}),
            ),
        ]);
        let q = Query {
            title: "Saiki K".into(),
            season: Some(1),
            episode: Some(2),
            ..Default::default()
        };
        let m = tmdb(&h).lookup(&q).unwrap().unwrap();
        assert_eq!(
            (
                m["show"].as_str(),
                m["episode_title"].as_str(),
                m["season"].as_str()
            ),
            ("Saiki K", "Ep Two", "1")
        );
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("query=Saiki%20K"))
        );
        assert_eq!(m["season_poster_url"], "http://img/s1.jpg");
        let mut t = tmdb(&h);
        t.cfg.poster_size = "w780".into();
        assert_eq!(t.img(&json!("/x.jpg")).unwrap(), "http://img/w780/x.jpg");
        assert_eq!(
            (
                m["tmdb_series_id"].as_str(),
                m["tmdb_episode_id"].as_str(),
                m["imdb_id"].as_str()
            ),
            ("42", "4202", "tt5")
        );
    }

    #[test]
    fn omdb_lookup_and_missing_key() {
        let body = json!({"Response":"True","Title":"Dune","Year":"1984","imdbID":"tt0087182","Rated":"PG-13","Awards":"Nominated for 1 Oscar.","Plot":"A plot.","Poster":"N/A",
            "Ratings":[{"Source":"Internet Movie Database","Value":"6.3/10"},{"Source":"Rotten Tomatoes","Value":"36%"},{"Source":"Metacritic","Value":"41/100"}]});
        let h = FakeHttp::new(vec![("http://o/", body)]);
        let o = Omdb {
            cfg: OmdbCfg {
                api_key: "SECRETKEY".into(),
                base_url: "http://o/".into(),
                plot: "short".into(),
                ..Default::default()
            },
            http: &h,
        };
        let m = o
            .lookup(&Query {
                title: "Dune".into(),
                year: Some("1984".into()),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                m["rating_imdb"].as_str(),
                m["rating_rotten_tomatoes"].as_str(),
                m["rating_metacritic"].as_str()
            ),
            ("6.3/10", "36%", "41/100")
        );
        assert_eq!(
            (m["rated"].as_str(), m["imdb_id"].as_str()),
            ("PG-13", "tt0087182")
        );
        assert!(!m.contains_key("poster_url"), "N/A skipped");
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("&t=Dune&type=movie&y=1984"))
        );
        o.lookup(&Query {
            title: "x".into(),
            imdb: Some("tt0087182".into()),
            season: Some(1),
            ..Default::default()
        })
        .unwrap();
        assert!(h.calls.borrow().last().unwrap().ends_with("&i=tt0087182"));
        let h = FakeHttp::new(vec![(
            "http://o/",
            json!({"Response":"False","Error":"Movie not found!"}),
        )]);
        let o = Omdb {
            cfg: OmdbCfg {
                api_key: "k".into(),
                base_url: "http://o/".into(),
                ..Default::default()
            },
            http: &h,
        };
        assert!(
            o.lookup(&Query {
                title: "zz".into(),
                ..Default::default()
            })
            .unwrap()
            .is_none()
        );
        let o = Omdb {
            cfg: OmdbCfg::default(),
            http: &h,
        };
        assert!(o.lookup(&Query::default()).unwrap_err().contains("skipped"));
        // errors never leak the key
        let o = Omdb {
            cfg: OmdbCfg {
                api_key: "SECRETKEY".into(),
                base_url: "http://none/".into(),
                ..Default::default()
            },
            http: &h,
        };
        assert!(
            !o.lookup(&Query::default())
                .unwrap_err()
                .contains("SECRETKEY")
        );
    }

    #[test]
    fn landscape_art_urls() {
        let h = FakeHttp::new(vec![
            ("/search/movie", json!({"results":[{"id":1}]})),
            (
                "/movie/1",
                json!({"title":"Dune","backdrop_path":"/def.jpg","images":{"backdrops":[{"iso_639_1":null,"file_path":"/null.jpg"},{"iso_639_1":"fr","file_path":"/fr.jpg"},{"iso_639_1":"en","file_path":"/en.jpg"}]}}),
            ),
        ]);
        let mut t = tmdb(&h);
        (t.cfg.image_language, t.cfg.backdrop_size, t.cfg.still_size) =
            ("en".into(), "w1280".into(), "w300".into());
        let m = t
            .lookup(&Query {
                title: "Dune".into(),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(m["backdrop_url"], "http://img/w1280/en.jpg");
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("include_image_language=en%2Cnull")
                    || c.contains("include_image_language=en,null")),
            "{:?}",
            h.calls.borrow()
        );
        t.cfg.image_language = "de".into();
        assert_eq!(
            t.lookup(&Query {
                title: "Dune".into(),
                ..Default::default()
            })
            .unwrap()
            .unwrap()["backdrop_url"],
            "http://img/w1280/null.jpg"
        );
        let h = FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":2}]})),
            (
                "/tv/2/season/1/episode/1",
                json!({"still_path":"/still.jpg"}),
            ),
            ("/tv/2?", json!({"name":"S","backdrop_path":"/bd.jpg"})),
        ]);
        let mut t = tmdb(&h);
        t.cfg.still_size = "w300".into();
        let m = t
            .lookup(&Query {
                title: "S".into(),
                season: Some(1),
                episode: Some(1),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            (m["still_url"].as_str(), m["backdrop_url"].as_str()),
            ("http://img/w300/still.jpg", "http://img/bd.jpg")
        );
        let h = FakeHttp::new(vec![
            ("/login", json!({"data":{"token":"T"}})),
            ("/search", json!({"data":[{"tvdb_id":"9"}]})),
            (
                "/series/9/extended",
                json!({"data":{"artworks":[{"type":2,"image":"http://a/poster.jpg"},{"type":3,"language":null,"image":"http://a/bg0.jpg"},{"type":3,"language":"eng","image":"http://a/bg.jpg"}]}}),
            ),
            (
                "/series/9/episodes",
                json!({"data":{"episodes":[{"image":"http://a/ep.jpg"}]}}),
            ),
            ("/series/9", json!({"data":{"id":9,"name":"S"}})),
        ]);
        let tv = Tvdb::new(
            TvdbCfg {
                api_key: "k".into(),
                base_url: "http://v".into(),
                image_language: "eng".into(),
                ..Default::default()
            },
            &h,
        );
        let m = tv
            .lookup(&Query {
                title: "S".into(),
                season: Some(1),
                episode: Some(1),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            (m["backdrop_url"].as_str(), m["still_url"].as_str()),
            ("http://a/bg.jpg", "http://a/ep.jpg")
        );
    }

    #[test]
    fn forced_type_picks_endpoints() {
        // tv forced without SxxExx: TMDB /search/tv, OMDb type=series
        let h = FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":2}]})),
            ("/tv/2", json!({"name":"Cosmos"})),
        ]);
        let tvq = Query {
            title: "Cosmos".into(),
            kind: Some("tv".into()),
            ..Default::default()
        };
        tmdb(&h).lookup(&tvq).unwrap().unwrap();
        assert!(h.calls.borrow().iter().any(|c| c.contains("/search/tv?")));
        let o = Omdb {
            cfg: OmdbCfg {
                api_key: "k".into(),
                base_url: "http://o/".into(),
                ..Default::default()
            },
            http: &h,
        };
        let _ = o.lookup(&tvq);
        assert!(h.calls.borrow().iter().any(|c| c.contains("type=series")));
        // movie forced despite a thetvdb id: TMDB movie, find ignores tv_results, TheTVDB movie search
        let h = FakeHttp::new(vec![
            (
                "/find/tt1",
                json!({"movie_results":[{"id":7}],"tv_results":[{"id":8}]}),
            ),
            ("/movie/7", json!({"title":"M"})),
            ("/login", json!({"data":{"token":"T"}})),
            ("/search", json!({"data":[{"tvdb_id":"5"}]})),
            (
                "/movies/5",
                json!({"data":{"name":"M","year":"2001","image":"http://a/m.jpg"}}),
            ),
        ]);
        let mq = Query {
            title: "M".into(),
            imdb: Some("tt1".into()),
            kind: Some("movie".into()),
            ..Default::default()
        };
        assert_eq!(tmdb(&h).lookup(&mq).unwrap().unwrap()["kind"], "movie");
        let m = Tvdb::new(
            TvdbCfg {
                api_key: "k".into(),
                base_url: "http://v".into(),
                ..Default::default()
            },
            &h,
        )
        .lookup(&mq)
        .unwrap()
        .unwrap();
        assert_eq!(
            (
                m["kind"].as_str(),
                m["year"].as_str(),
                m["thetvdb_id"].as_str()
            ),
            ("movie", "2001", "5")
        );
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("/search?type=movie&query=M"))
        );
    }

    #[test]
    fn year_filters_in_search_queries() {
        let h = FakeHttp::new(vec![
            ("/search/movie", json!({"results":[{"id":1}]})),
            ("/movie/1", json!({"title":"Dune"})),
        ]);
        tmdb(&h)
            .lookup(&Query {
                title: "Dune".into(),
                year: Some("1984".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("/search/movie?") && c.contains("query=Dune&year=1984")),
            "{:?}",
            h.calls.borrow()
        );
        let h = FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":2}]})),
            ("/tv/2", json!({"name":"Cosmos"})),
        ]);
        let q = Query {
            title: "Cosmos".into(),
            year: Some("1980".into()),
            season: Some(1),
            ..Default::default()
        };
        tmdb(&h).lookup(&q).unwrap();
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("first_air_date_year=1980"))
        );
        let h = FakeHttp::new(vec![
            ("/login", json!({"data":{"token":"T"}})),
            ("/search", json!({"data":[{"tvdb_id":"9"}]})),
            ("/series/9", json!({"data":{"id":9,"name":"Cosmos"}})),
        ]);
        Tvdb::new(
            TvdbCfg {
                api_key: "k".into(),
                base_url: "http://v".into(),
                ..Default::default()
            },
            &h,
        )
        .lookup(&q)
        .unwrap();
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("type=series&query=Cosmos&year=1980")),
            "{:?}",
            h.calls.borrow()
        );
    }

    #[test]
    fn tvdb_login_slug_episode_and_missing_keys() {
        let h = FakeHttp::new(vec![
            ("/login", json!({"data":{"token":"T"}})),
            (
                "/series/slug/the-disastrous-life",
                json!({"data":{"id":7,"name":"Saiki K","year":"2016"}}),
            ),
            (
                "/series/7/episodes",
                json!({"data":{"episodes":[{"name":"E","aired":"2016-07-04"}]}}),
            ),
            (
                "/series/7/extended",
                json!({"data":{"seasons":[{"number":1,"type":{"type":"dvd"},"image":"http://a/dvd.jpg"},{"number":1,"type":{"type":"official"},"image":"http://a/s1.jpg"}]}}),
            ),
        ]);
        let t = Tvdb::new(
            TvdbCfg {
                api_key: "k".into(),
                pin: "1234".into(),
                base_url: "http://v".into(),
                poster_size: "thumbnail".into(),
                ..Default::default()
            },
            &h,
        );
        let q = Query {
            thetvdb: Some("https://www.thetvdb.com/series/the-disastrous-life".into()),
            season: Some(1),
            episode: Some(1),
            ..Default::default()
        };
        let m = t.lookup(&q).unwrap().unwrap();
        assert_eq!(
            (m["show"].as_str(), m["episode_title"].as_str()),
            ("Saiki K", "E")
        );
        assert_eq!(m["season_poster_url"], "http://a/s1_t.jpg");
        assert_eq!(
            h.calls
                .borrow()
                .iter()
                .filter(|c| c.contains("/login"))
                .count(),
            1
        );
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("\"pin\":\"1234\""))
        );
        // movies are skipped; missing keys give clear errors
        assert!(
            t.lookup(&Query {
                title: "X".into(),
                ..Default::default()
            })
            .unwrap()
            .is_none()
        );
        let e = Tvdb::new(TvdbCfg::default(), &h).lookup(&q).unwrap_err();
        assert!(e.contains("[providers.thetvdb] api_key"));
        let mut tm = tmdb(&h);
        tm.cfg.read_token.clear();
        assert!(
            tm.lookup(&Query {
                title: "X".into(),
                ..Default::default()
            })
            .unwrap_err()
            .contains("[providers.tmdb]")
        );
    }

    #[test]
    fn full_image_lists() {
        let m: Meta = [
            ("kind", "episode"),
            ("tmdb_series_id", "7"),
            ("thetvdb_series_id", "9"),
            ("season", "3"),
            ("episode", "1"),
            ("original_language", "ja"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let a = |p: &str, l: Value, w: u64| json!({"file_path": p, "iso_639_1": l, "width": w, "height": w});
        let h = FakeHttp::new(vec![
            (
                "/tv/7/season/3/episode/1/images",
                json!({"stills":[a("/s.jpg", Value::Null, 1920)]}),
            ),
            (
                "/tv/7/season/3/images",
                json!({"posters":[a("/sp.jpg", json!("ja"), 1000)]}),
            ),
            (
                "/tv/7/images",
                json!({"posters":[a("/en.jpg", json!("en"), 500), a("/fr.jpg", json!("fr"), 2000)],"backdrops":[a("/b.jpg", Value::Null, 3840)]}),
            ),
        ]);
        let v = tmdb(&h).images(&m);
        let got: Vec<_> = v
            .iter()
            .map(|i| (i.kind, i.url.as_str(), i.lang.as_str(), i.lrank, i.w))
            .collect();
        assert_eq!(
            got,
            vec![
                ("poster", "http://img/original/en.jpg", "en", 0, 500),
                ("poster", "http://img/original/fr.jpg", "fr", 3, 2000),
                ("backdrop", "http://img/original/b.jpg", "", 2, 3840),
                ("season", "http://img/original/sp.jpg", "ja", 1, 1000),
                ("still", "http://img/original/s.jpg", "", 2, 1920)
            ]
        );
        assert!(
            h.calls
                .borrow()
                .iter()
                .filter(|c| c.contains("/images"))
                .all(|c| c.contains("include_image_language=en%2Cja%2Cnull")),
            "{:?}",
            h.calls.borrow()
        );
        let h = FakeHttp::new(vec![
            ("/login", json!({"data":{"token":"x"}})),
            (
                "/series/9/extended",
                json!({"data":{"originalLanguage":"jpn","seasons":[{"id":55,"number":3,"type":{"type":"official"}}],"artworks":[{"type":2,"image":"http://a/p.jpg","language":"jpn","width":680,"height":1000},{"type":3,"image":"http://a/bg.jpg","language":null,"width":1920,"height":1080},{"type":1,"image":"http://a/banner.jpg"}]}}),
            ),
            (
                "/seasons/55/extended",
                json!({"data":{"artwork":[{"type":7,"image":"http://a/s3.jpg","language":"eng","width":680,"height":1000}]}}),
            ),
        ]);
        let v = Tvdb::new(
            TvdbCfg {
                api_key: "k".into(),
                base_url: "http://v".into(),
                image_language: "eng".into(),
                ..Default::default()
            },
            &h,
        )
        .images(&m);
        let got: Vec<_> = v
            .iter()
            .map(|i| (i.kind, i.url.as_str(), i.lrank, i.w, i.h))
            .collect();
        assert_eq!(
            got,
            vec![
                ("poster", "http://a/p.jpg", 1, 680, 1000),
                ("backdrop", "http://a/bg.jpg", 2, 1920, 1080),
                ("season", "http://a/s3.jpg", 0, 680, 1000)
            ]
        );
    }

    #[test]
    fn a_thetvdb_id_is_looked_up_on_tmdb_instead_of_a_title_guess() {
        let h = FakeHttp::new(vec![
            (
                "/find/27498",
                json!({"tv_results":[{"id":55,"name":"Mountain"}],"movie_results":[]}),
            ),
            (
                "/tv/55",
                json!({"name":"Mountain","first_air_date":"2019-01-01","external_ids":{"imdb_id":"tt9"}}),
            ),
        ]);
        let m = tmdb(&h)
            .lookup(&Query {
                title: "Mountain".into(),
                thetvdb: Some("27498".into()),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(m["tmdb_series_id"], "55");
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|c| c.contains("external_source=tvdb_id")),
            "{:?}",
            h.calls.borrow()
        );
        assert!(
            !h.calls.borrow().iter().any(|c| c.contains("/search/")),
            "a trusted id is not a title search: {:?}",
            h.calls.borrow()
        );
    }
}
