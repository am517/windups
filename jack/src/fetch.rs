//! Querying providers and merging their answers.
use crate::config::Config;
use crate::meta::Meta;
use crate::providers::{self, Http, Omdb, Provider, Query, Tmdb, Tvdb};
use crate::sorter;
use crate::sources::itunes::Itunes;
use crate::sources::tvmaze::Tvmaze;
use crate::sources::wikidata::Wikidata;
use crate::sources::youtube::Youtube;
use crate::sources::{enrich, wikipedia};
use std::fmt::Write as _;

/// Composite metadata from all providers in [providers] order, merged per [merge].
pub(crate) fn provider_cfgs(cfg: &Config) -> (providers::TmdbCfg, providers::TvdbCfg) {
    let pc = &cfg.providers;
    let mut tc = pc.tmdb.clone();
    tc.keep_original = pc.keep_original;
    if tc.language.is_empty() {
        tc.language = pc.language.clone();
    }
    let mut vc = pc.thetvdb.clone();
    vc.keep_original = pc.keep_original;
    if vc.language.is_empty() {
        vc.language = pc
            .language_codes
            .get(&pc.language)
            .cloned()
            .unwrap_or_else(|| pc.language.clone());
    }
    (tc, vc)
}

/// Fetch from the providers, optionally starting from `seed` (prior jack.db candidates)
/// and, when `only` is set, querying just those providers.
pub(crate) fn fetch_from(
    q: &Query,
    cfg: &Config,
    http: &dyn Http,
    out: &mut String,
    seed: &[(String, Meta)],
    only: Option<&[String]>,
) -> Result<(Meta, Vec<(String, Meta)>), String> {
    let mut q = q.clone();
    let pc = &cfg.providers;
    let (tc, vc) = provider_cfgs(cfg);
    let tmdb = Tmdb { cfg: tc, http };
    let tvdb = Tvdb::new(vc, http);
    let omdb = Omdb {
        cfg: cfg.providers.omdb.clone(),
        http,
    };
    let wikidata = Wikidata {
        cfg: pc.wikidata.clone(),
        http,
    };
    let tvmaze = Tvmaze {
        cfg: pc.tvmaze.clone(),
        http,
    };
    let itunes = Itunes {
        cfg: pc.itunes.clone(),
        http,
    };
    let youtube = Youtube {
        cfg: pc.youtube.clone(),
        http,
    };
    let refresh: Vec<String> = only.map(|s| s.to_vec()).unwrap_or_default();
    let mut found: Vec<(String, Meta)> = seed
        .iter()
        .filter(|(n, _)| {
            only.is_none()
                || !refresh
                    .iter()
                    .any(|s| n == s || n.starts_with(&format!("{s}:")))
        })
        .cloned()
        .collect();
    // A wikidata or wikipedia id is trusted on its own. Resolve it first so the other
    // sources can be asked about the same work.
    if q.wikidata.is_some() || q.wikipedia.is_some() {
        match wikidata.lookup(&q) {
            Ok(Some(meta)) => {
                let _ = writeln!(
                    out,
                    "    wikidata: {} (cross-checking the other sources)",
                    meta.get("wikidata_id").map_or("?", |id| id.as_str())
                );
                if q.imdb.is_none() {
                    q.imdb = meta.get("imdb_id").cloned();
                }
                if q.tmdb.is_none() {
                    q.tmdb = meta
                        .get("tmdb_series_id")
                        .map(|id| format!("/tv/{id}"))
                        .or_else(|| meta.get("tmdb_id").cloned());
                }
                if q.thetvdb.is_none() {
                    q.thetvdb = meta
                        .get("thetvdb_series_id")
                        .cloned()
                        .or_else(|| meta.get("thetvdb_id").cloned());
                }
                if q.title.is_empty()
                    && let Some(title) = meta.get("title").or_else(|| meta.get("wikipedia_title"))
                {
                    q.title = title.clone();
                }
                if q.year.is_none() {
                    q.year = meta.get("year").cloned();
                }
                found.push(("wikidata".into(), meta));
            }
            Ok(None) => {
                let _ = writeln!(
                    out,
                    "    warning: wikidata: no item for the id; still checking the other sources"
                );
            }
            Err(error) => {
                let _ = writeln!(out, "    warning: wikidata: {error}");
            }
        }
    }
    let (mut errs, mut skipped, mut cands) = (vec![], vec![], vec![]);
    for name in &cfg.providers.order {
        if let Some(only) = only
            && !only.iter().any(|s| s == name)
        {
            continue;
        }
        if name == "wikidata" && found.iter().any(|(found_name, _)| found_name == "wikidata") {
            continue;
        }
        let p: &dyn Provider = match name.as_str() {
            "tmdb" => &tmdb,
            "thetvdb" => &tvdb,
            "omdb" => &omdb,
            "wikidata" => &wikidata,
            "tvmaze" => &tvmaze,
            "itunes" => &itunes,
            "youtube" => &youtube,
            x => return Err(format!("[providers] order: unknown provider '{x}'")),
        };
        // extra sources reuse the ids the main providers already found
        let late = matches!(
            name.as_str(),
            "omdb" | "wikidata" | "tvmaze" | "itunes" | "youtube"
        );
        let eq = if late {
            enrich::with_found_ids(&q, &found)
        } else {
            q.clone()
        };
        if !p.handles(&eq) {
            skipped.push(p.name());
            continue;
        }
        match p.lookup_all(&eq) {
            Ok((m, c)) => {
                if let Some(mut m) = m {
                    if let Some(w) = m.remove(providers::WARNING) {
                        errs.push(format!("{}: {w}", p.name()));
                    }
                    found.push((name.clone(), m))
                }
                // the search list from the same call, capped per source ([candidates] per_source)
                cands.extend(
                    c.into_iter()
                        .take(cfg.candidates.per_source)
                        .map(|m| (crate::candidates::source(name), m)),
                );
            }
            // a user-supplied id that no kind of this source has: stop here, never search by title
            Err(e) if e.starts_with(providers::ID_NOT_FOUND) => return Err(e),
            Err(e) => errs.push(format!("{}: {e}", p.name())),
        }
    }
    if !skipped.is_empty() && (!found.is_empty() || !cands.is_empty()) {
        let _ = writeln!(
            out,
            "    skipped: {} (not used for this kind of title)",
            skipped.join(", ")
        );
    }
    if found.is_empty() && cands.is_empty() {
        return Err(
            if errs.is_empty() && skipped.len() == cfg.providers.order.len() {
                format!(
                    "skipped: {} not used for this kind of title",
                    skipped.join(", ")
                )
            } else if errs.is_empty() {
                format!("no match for '{}'", q.title)
            } else {
                errs.join("; ")
            },
        );
    }
    for e in errs {
        let _ = writeln!(out, "    warning: {e}");
    }
    let mut m = sorter::merge(&found, &cfg.merge);
    match providers::merge_genres(
        found
            .iter()
            .filter_map(|(_, x)| x.get("genre").map(String::as_str)),
        &pc.genre_case,
        &pc.genre_separator,
        &pc.genre_aliases,
    ) {
        Some(g) => m.insert("genre".into(), g),
        None => m.remove("genre"),
    };
    if !m.contains_key("description")
        && let Some(t) = m.get("wikipedia_title").cloned()
    {
        match wikipedia::description(&pc.wikipedia, http, &t) {
            Ok(Some(d)) => {
                m.insert("description".into(), d);
            }
            Ok(None) => {}
            Err(e) => {
                let _ = writeln!(out, "    warning: wikipedia: {e}");
            }
        }
    }
    let ep = m.get("kind").is_some_and(|k| k == "episode");
    if ep
        && !m.contains_key("episode_title")
        && cfg.matching.episode_title_from_filename
        && let Some(t) = &q.episode_title
    {
        let _ = writeln!(
            out,
            "    warning: no episode title from providers; using the filename's ({t})"
        );
        m.insert("episode_title".into(), t.clone());
    }
    if ep
        && pc.tv_show_year
        && let (Some(s), Some(y)) = (m.get("show").cloned(), m.get("year").cloned())
    {
        m.insert("show".into(), format!("{s} ({y})"));
    }
    if pc.original_in_description
        && let (Some(o), Some(d)) = (
            m.get("original_title").cloned(),
            m.get("description").cloned(),
        )
    {
        m.insert("description".into(), format!("{d}\n\nOriginal title: {o}"));
    }
    found.extend(cands);
    Ok((m, found))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fake::FakeHttp;
    use serde_json::json;

    fn cfg(order: &[&str]) -> Config {
        let mut c = Config::default();
        c.providers.order = order.iter().map(|x| x.to_string()).collect();
        c.providers.tmdb.read_token = "T".into();
        c.providers.tmdb.base_url = "http://tmdb".into();
        c.providers.omdb.api_key = "K".into();
        c.providers.omdb.base_url = "http://omdb/".into();
        c.providers.tvmaze.base_url = "http://tm".into();
        c
    }

    #[test]
    fn tv_only_provider_skips_movie_without_no_match() {
        let h = FakeHttp::new(vec![]);
        let q = Query {
            title: "The Matrix".into(),
            kind: Some("movie".into()),
            ..Default::default()
        };
        let e = fetch_from(&q, &cfg(&["tvmaze"]), &h, &mut String::new(), &[], None).unwrap_err();
        assert!(e.starts_with("skipped") && !e.contains("no match"), "{e}");
        assert!(h.calls.borrow().is_empty());
    }

    #[test]
    fn omdb_uses_imdb_id_found_by_earlier_provider() {
        let h = FakeHttp::new(vec![
            (
                "/search/movie",
                json!({"results":[{"id":603,"title":"The Matrix","release_date":"1999-03-30"}]}),
            ),
            (
                "/movie/603",
                json!({"id":603,"title":"The Matrix","release_date":"1999-03-30","imdb_id":"tt0133093"}),
            ),
            (
                "i=tt0133093",
                json!({"Title":"The Matrix","Year":"1999","imdbID":"tt0133093","imdbRating":"8.7"}),
            ),
        ]);
        let q = Query {
            title: "The Matrix".into(),
            year: Some("1999".into()),
            kind: Some("movie".into()),
            ..Default::default()
        };
        let (m, _) = fetch_from(
            &q,
            &cfg(&["tmdb", "omdb"]),
            &h,
            &mut String::new(),
            &[],
            None,
        )
        .unwrap();
        assert_eq!(m.get("rating_imdb").map(String::as_str), Some("8.7"));
        assert!(h.calls.borrow().iter().any(|u| u.contains("&i=tt0133093")));
    }
}
