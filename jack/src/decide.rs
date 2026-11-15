//! Is the information enough to act on? A single match is exactly one identity that
//! agrees across more than one source. Name or date variations on that same identity go
//! to the LLM windup (`needs_judgment`); different works, or a hit from only one source,
//! stay unresolved so the user can give an id, or --year to narrow several works with the same name.
//! `--name` and `--year` only filter the search; they are not a trusted match.
use crate::candidates::is_search;
use crate::config::Settings;
use crate::meta::Meta;
use crate::providers::Query;
use crate::resolver::{exact, norm};

#[derive(Debug, PartialEq)]
pub enum Decision {
    Sure,
    /// (reason from unresolved_reasons, detail)
    Unresolved(&'static str, String),
}

fn year(m: &Meta) -> Option<String> {
    m.get("series_year")
        .or(m.get("year"))
        .map(|y| y.chars().take(4).collect())
}

fn label(m: &Meta) -> String {
    let t = m.get("show").or(m.get("title")).map_or("?", |s| s);
    format!("{t} ({})", year(m).unwrap_or_else(|| "?".into()))
}

fn source_base(s: &str) -> &str {
    s.split(':').next().unwrap_or(s)
}

/// True when at least two hits share the same external id (same work, cross-checked).
fn shared_id(hits: &[&(String, Meta)]) -> bool {
    const KEYS: &[&str] = &[
        "imdb_id",
        "tmdb_id",
        "tmdb_series_id",
        "thetvdb_id",
        "thetvdb_series_id",
    ];
    for k in KEYS {
        let vals: Vec<&str> = hits
            .iter()
            .filter_map(|(_, m)| m.get(*k).filter(|v| !v.is_empty()).map(|s| s.as_str()))
            .collect();
        if vals.len() >= 2 && vals.iter().all(|v| *v == vals[0]) {
            return true;
        }
    }
    false
}

fn id_tags(m: &Meta) -> Vec<String> {
    [
        "imdb_id",
        "tmdb_id",
        "tmdb_series_id",
        "thetvdb_id",
        "thetvdb_series_id",
    ]
    .iter()
    .filter_map(|k| {
        m.get(*k)
            .filter(|v| !v.is_empty())
            .map(|v| format!("{k}:{v}"))
    })
    .collect()
}

fn share_id(a: &Meta, b: &Meta) -> bool {
    let a = id_tags(a);
    a.iter().any(|t| id_tags(b).iter().any(|u| u == t))
}

fn title_of(m: &Meta) -> String {
    norm(m.get("show").or(m.get("title")).map_or("", |s| s))
}

fn years_compatible(a: &Meta, b: &Meta, year_tolerance: u32) -> bool {
    match (year(a), year(b)) {
        (Some(x), Some(y)) => year_spread(&[x, y]) <= year_tolerance as i64,
        _ => true,
    }
}

fn same_work(a: &Meta, b: &Meta, year_tolerance: u32) -> bool {
    if share_id(a, b) {
        return true;
    }
    let t = title_of(a);
    !t.is_empty() && t == title_of(b) && years_compatible(a, b, year_tolerance)
}

/// Records that are the same work as a hit, even when the title is worded differently.
fn companions<'a>(
    members: &[&'a (String, Meta)],
    found: &[&'a (String, Meta)],
) -> Vec<&'a (String, Meta)> {
    let known: std::collections::BTreeSet<String> =
        members.iter().flat_map(|(_, m)| id_tags(m)).collect();
    if known.is_empty() {
        return Vec::new();
    }
    found
        .iter()
        .copied()
        .filter(|row| {
            !members.iter().any(|m| std::ptr::eq(*m, *row))
                && id_tags(&row.1).iter().any(|id| known.contains(id))
        })
        .collect()
}

fn cluster<'a>(
    members: &[&'a (String, Meta)],
    year_tolerance: u32,
) -> Vec<Vec<&'a (String, Meta)>> {
    let mut clusters: Vec<Vec<&(String, Meta)>> =
        members.iter().copied().map(|r| vec![r]).collect();
    let mut changed = true;
    while changed {
        changed = false;
        let n = clusters.len();
        'outer: for i in 0..n {
            for j in i + 1..n {
                let merge = clusters[i].iter().any(|(_, a)| {
                    clusters[j]
                        .iter()
                        .any(|(_, b)| same_work(a, b, year_tolerance))
                });
                if merge {
                    let other = clusters.remove(j);
                    clusters[i].extend(other);
                    changed = true;
                    break 'outer;
                }
            }
        }
    }
    clusters
}

fn year_spread(years: &[String]) -> i64 {
    let nums: Vec<i64> = years.iter().filter_map(|y| y.parse().ok()).collect();
    nums.iter()
        .max()
        .zip(nums.iter().min())
        .map_or(0, |(a, b)| a - b)
}

/// Same work, but sources do not agree on the name or the date.
fn variations(hits: &[&(String, Meta)], year_tolerance: u32) -> Option<String> {
    let mut years: Vec<String> = hits.iter().filter_map(|(_, m)| year(m)).collect();
    years.sort();
    years.dedup();
    let mut names: Vec<String> = hits
        .iter()
        .filter_map(|(_, m)| m.get("show").or(m.get("title")).map(|s| norm(s)))
        .collect();
    names.sort();
    names.dedup();
    let year_off = years.len() > 1 && year_spread(&years) > year_tolerance as i64;
    if !year_off && names.len() <= 1 {
        return None;
    }
    let list: Vec<String> = hits
        .iter()
        .map(|(src, m)| format!("{src}: {}", label(m)))
        .collect();
    Some(list.join(", "))
}

pub fn decide(q: &Query, s: &Settings, found: &[(String, Meta)], year_tolerance: u32) -> Decision {
    let set = |o: &Option<String>| o.as_ref().is_some_and(|v| !v.is_empty());
    // only an id flag is a trusted match; --name/--year only filter the search
    let user_id = set(&q.imdb)
        || set(&q.tmdb)
        || set(&q.thetvdb)
        || set(&s.imdb)
        || set(&s.tmdb)
        || set(&s.thetvdb)
        || set(&s.youtube)
        || set(&s.x);
    let is_hit = |m: &Meta| {
        let mut m = m.clone();
        if let Some(y) = year(&m) {
            m.insert("year".into(), y);
        }
        exact(&q.title, q.year.as_deref(), &m)
    };
    let found: Vec<&(String, Meta)> = found.iter().filter(|(src, _)| src != "embedded").collect();
    let hits: Vec<&(String, Meta)> = found.iter().copied().filter(|(_, m)| is_hit(m)).collect();

    // one source listing several exact matches: they are different works
    let mut srcs: Vec<&str> = hits
        .iter()
        .filter(|(src, _)| is_search(src))
        .map(|(src, _)| src.as_str())
        .collect();
    srcs.sort();
    if srcs.windows(2).any(|w| w[0] == w[1]) {
        return Decision::Unresolved(
            "ambiguous",
            format!("more than one work matches \"{}\"", q.title),
        );
    }

    // fetched details must be about the exact match, not some other top result
    let chosen: Vec<&(String, Meta)> = found
        .iter()
        .copied()
        .filter(|(src, _)| !is_search(src))
        .collect();
    if !user_id && !hits.is_empty() {
        if let Some((_src, m)) = chosen.iter().find(|(_, m)| {
            m.get("show")
                .or(m.get("title"))
                .is_some_and(|t| norm(t) != norm(&q.title))
                && !is_hit(m)
                && !hits.iter().any(|(_, h)| share_id(h, m))
        }) {
            return Decision::Unresolved(
                "needs_flags",
                format!(
                    "a source's details are for {}, not \"{}\"",
                    label(m),
                    q.title
                ),
            );
        }
    }

    // cluster exact hits (or, when the user gave an id, the fetched details)
    let mut members: Vec<&(String, Meta)> = if hits.is_empty() && user_id {
        chosen
    } else {
        hits.clone()
    };
    // a source may list the same work under a different title; keep it if the ids agree
    members.extend(companions(&members, &found));
    if members.is_empty() {
        if user_id {
            // the id identifies the work; other sources may still have been queried and found nothing
            return Decision::Sure;
        }
        return Decision::Unresolved("no_match", format!("no source has exactly \"{}\"", q.title));
    }

    let clusters = cluster(&members, year_tolerance);

    // an id flag identifies the work; other sources were still queried to fill ids/facts
    if user_id {
        return Decision::Sure;
    }

    if clusters.len() > 1 {
        // different identities: not a single match
        return Decision::Unresolved(
            "needs_flags",
            format!("sources do not agree on a single work for \"{}\"", q.title),
        );
    }
    let hits = &clusters[0];
    let mut bases: Vec<&str> = hits.iter().map(|(s, _)| source_base(s)).collect();
    bases.sort();
    bases.dedup();

    let years: Vec<String> = hits.iter().filter_map(|(_, m)| year(m)).collect();
    // same title, years farther than tolerance, no shared id: different works
    if years
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        > 1
        && year_spread(&years) > year_tolerance as i64
        && !shared_id(hits)
    {
        return Decision::Unresolved(
            "needs_flags",
            format!("sources do not agree on a single work for \"{}\"", q.title),
        );
    }

    if let Some(d) = variations(hits, year_tolerance) {
        return Decision::Unresolved("needs_judgment", d);
    }

    if bases.len() < 2 {
        return Decision::Unresolved(
            "needs_flags",
            format!(
                "only one source found \"{}\"; another source must agree, or give an id (--imdb, --tmdb, or --thetvdb)",
                q.title
            ),
        );
    }
    Decision::Sure
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d0(q: &Query, s: &Settings, f: &[(String, Meta)]) -> Decision {
        decide(q, s, f, 0)
    }
    fn m(src: &str, t: &str, y: &str) -> (String, Meta) {
        (
            src.into(),
            [
                ("title".to_string(), t.to_string()),
                ("year".to_string(), y.to_string()),
            ]
            .into(),
        )
    }
    fn q(t: &str, y: Option<&str>) -> Query {
        Query {
            title: t.into(),
            year: y.map(String::from),
            ..Default::default()
        }
    }
    #[test]
    fn acts_only_on_sure_information() {
        let none = Settings::default();
        assert!(
            matches!(
                d0(
                    &q("The Matrix", Some("1999")),
                    &none,
                    &[m("tmdb", "The Matrix", "1999")]
                ),
                Decision::Unresolved("needs_flags", _)
            ),
            "one source is not a single match"
        );
        assert_eq!(
            d0(
                &q("The Matrix", Some("1999")),
                &none,
                &[
                    m("tmdb", "The Matrix", "1999"),
                    m("thetvdb", "The Matrix", "1999"),
                ]
            ),
            Decision::Sure
        );
        assert!(matches!(
            d0(
                &q("The Matrix", Some("1999")),
                &none,
                &[m("tmdb", "The Matrix Reloaded", "2003")]
            ),
            Decision::Unresolved("no_match", _)
        ));
        assert_eq!(
            d0(
                &q("breaking.bad", None),
                &none,
                &[
                    m("tmdb", "Breaking Bad", "2008"),
                    m("thetvdb", "Breaking Bad", "2008"),
                ]
            ),
            Decision::Sure
        );
        assert!(matches!(
            d0(
                &q("Breaking Bad", None),
                &none,
                &[m("tmdb", "Breaking Bad: Original Minisodes", "2009")]
            ),
            Decision::Unresolved("no_match", _)
        ));
        let named = Settings {
            name: Some("Breaking Bad".into()),
            year: Some("2008".into()),
            ..Default::default()
        };
        // --name/--year only filter the search; they are not a trusted match
        assert!(matches!(
            d0(&q("Breaking Bad", Some("2008")), &named, &[]),
            Decision::Unresolved("no_match", _)
        ));
        assert!(
            matches!(
                d0(
                    &q("The Matrix", Some("1999")),
                    &named,
                    &[m("tmdb", "The Matrix", "1999")]
                ),
                Decision::Unresolved("needs_flags", _)
            ),
            "one source is still not enough with --name/--year"
        );
        assert_eq!(
            d0(
                &q("The Matrix", Some("1999")),
                &named,
                &[
                    m("tmdb", "The Matrix", "1999"),
                    m("thetvdb", "The Matrix", "1999"),
                ]
            ),
            Decision::Sure
        );
        // an id flag is enough even with no (or one) source; other sources are still queried
        assert_eq!(
            d0(
                &Query {
                    imdb: Some("tt1".into()),
                    ..q("x", None)
                },
                &none,
                &[]
            ),
            Decision::Sure
        );
        assert_eq!(
            d0(
                &Query {
                    tmdb: Some("603".into()),
                    ..q("The Matrix", Some("1999"))
                },
                &none,
                &[m("tmdb", "The Matrix", "1999")]
            ),
            Decision::Sure
        );
    }
    #[test]
    fn whole_lists_decide_uniqueness() {
        let none = Settings::default();
        // one search list with two exact names: ambiguous, even if the top one was fetched
        let f = [
            m("tmdb", "The Office", "2005"),
            m("tmdb:search", "The Office", "2005"),
            m("tmdb:search", "The Office", "2001"),
            m("tmdb:search", "The Office Mixup", "2020"),
        ];
        let d = d0(&q("The Office", None), &none, &f);
        assert!(matches!(&d, Decision::Unresolved("ambiguous", _)), "{d:?}");
        // with the year it is unique on that list, but still only one source
        assert!(matches!(
            d0(&q("The Office", Some("2005")), &none, &f),
            Decision::Unresolved("needs_flags", _)
        ));
        let f2 = [
            m("tmdb", "The Office", "2005"),
            m("tmdb:search", "The Office", "2005"),
            m("thetvdb:search", "The Office", "2005"),
        ];
        assert_eq!(
            d0(&q("The Office", Some("2005")), &none, &f2),
            Decision::Sure
        );
        // each list has one, but the sources disagree
        let f = [
            m("tmdb:search", "Dune", "2021"),
            m("thetvdb:search", "Dune", "1984"),
        ];
        assert!(matches!(
            d0(&q("Dune", None), &none, &f),
            Decision::Unresolved("needs_flags", _)
        ));
        // exactly one exact match across all lists, sources agree
        let f = [
            m("tmdb", "The Why Files", "2021"),
            m("tmdb:search", "The Why Files", "2021"),
            m("tmdb:search", "Why Women Kill", "2019"),
            m("thetvdb:search", "The Why Files", "2021"),
        ];
        assert_eq!(d0(&q("The Why Files", None), &none, &f), Decision::Sure);
        // the exact match was listed but the details fetched are another work's
        let f = [
            m("tmdb", "Why Women Kill", "2019"),
            m("tmdb:search", "Why Women Kill", "2019"),
            m("tmdb:search", "The Why Files", "2021"),
        ];
        assert!(matches!(
            d0(&q("The Why Files", None), &none, &f),
            Decision::Unresolved("needs_flags", _)
        ));
    }
    #[test]
    fn year_tolerance_is_a_hint_for_agreement() {
        let none = Settings::default();
        let f = [
            m("tmdb:search", "Amelie", "2001"),
            m("tvmaze:search", "Amelie", "2002"),
        ];
        assert_eq!(decide(&q("Amelie", None), &none, &f, 1), Decision::Sure);
        assert!(
            matches!(
                decide(&q("Amelie", None), &none, &f, 0),
                Decision::Unresolved("needs_flags", _)
            ),
            "years beyond tolerance with no shared id are different works"
        );
    }
    #[test]
    fn same_identity_name_variations_need_judgment() {
        let none = Settings::default();
        let f = [
            (
                "tmdb".into(),
                [
                    ("title".to_string(), "The Matrix".into()),
                    ("year".to_string(), "1999".into()),
                    ("imdb_id".to_string(), "tt0133093".into()),
                ]
                .into(),
            ),
            (
                "omdb".into(),
                [
                    ("title".to_string(), "Matrix, The".into()),
                    ("year".to_string(), "1999".into()),
                    ("imdb_id".to_string(), "tt0133093".into()),
                ]
                .into(),
            ),
        ];
        assert!(matches!(
            d0(&q("The Matrix", Some("1999")), &none, &f),
            Decision::Unresolved("needs_judgment", _)
        ));
    }
}
