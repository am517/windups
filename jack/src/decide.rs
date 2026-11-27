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

#[derive(Debug)]
pub enum Decision {
    Sure {
        score: f64,
    },
    /// `reason` is a stable code (`no_match`, `ambiguous`, `needs_flags`, `needs_judgment`).
    /// `score` is the best normalized Levenshtein score Jack saw, or 1.0 for a trusted id.
    Unresolved {
        reason: &'static str,
        detail: String,
        score: f64,
    },
}

impl Decision {
    #[cfg(test)]
    pub fn is_sure(&self) -> bool {
        matches!(self, Decision::Sure { .. })
    }
    pub fn score(&self) -> f64 {
        match self {
            Decision::Sure { score } | Decision::Unresolved { score, .. } => *score,
        }
    }
    #[cfg(test)]
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Decision::Unresolved { reason, .. } => Some(reason),
            Decision::Sure { .. } => None,
        }
    }
    fn sure(score: f64) -> Self {
        Decision::Sure { score }
    }
    fn unresolved(reason: &'static str, detail: impl Into<String>, score: f64) -> Self {
        Decision::Unresolved {
            reason,
            detail: detail.into(),
            score,
        }
    }
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

/// Distinct sources among `rows` ("tmdb" and "tmdb:search" are one source).
fn confirming(rows: &[&(String, Meta)]) -> usize {
    rows.iter()
        .map(|(src, _)| src.split(':').next().unwrap_or(src))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

/// The user's picks: sure when they name one work and enough sources were picked.
pub fn picked(rows: &[(String, Meta)], r: &Rules) -> Decision {
    let rows: Vec<&(String, Meta)> = rows.iter().collect();
    if rows.is_empty() {
        return Decision::unresolved("no_match", "nothing was picked", 0.0);
    }
    match anchored(&rows, r) {
        Decision::Unresolved { reason, detail, .. } => Decision::unresolved(
            reason,
            detail.replace("the given id is one work, but found sources", "the picks"),
            1.0,
        ),
        sure => sure,
    }
}

/// A user's id: sure when the found sources agree on one work and enough of them found it.
fn anchored(fetched: &[&(String, Meta)], r: &Rules) -> Decision {
    let split = dissent(fetched, r.year_tolerance);
    if !split.is_empty() {
        return Decision::unresolved(
            "sources_disagree",
            format!(
                "the given id is one work, but found sources name different works: {}",
                split.join(" | ")
            ),
            1.0,
        );
    }
    if !fetched.is_empty() && confirming(fetched) < r.min_confirming {
        return Decision::unresolved(
            "too_few_sources",
            format!(
                "{} found source(s) confirm the given id; [matching] min_confirming is {}",
                confirming(fetched),
                r.min_confirming
            ),
            1.0,
        );
    }
    Decision::sure(1.0)
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

/// The [matching] settings the decision uses.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub year_tolerance: u32,
    pub threshold: f64,
    /// A record more than this many years from the user's `--year` is a different work.
    pub user_year_tolerance: u32,
    /// Found sources that must confirm the work (sources that found nothing don't count).
    pub min_confirming: usize,
}

impl From<&crate::matcher::MatchCfg> for Rules {
    fn from(c: &crate::matcher::MatchCfg) -> Self {
        Rules {
            year_tolerance: c.year_tolerance,
            threshold: c.fuzzy_threshold,
            user_year_tolerance: c.user_year_tolerance,
            min_confirming: c.min_confirming.max(1),
        }
    }
}

#[cfg(test)]
pub fn decide(
    q: &Query,
    s: &Settings,
    found: &[(String, Meta)],
    year_tolerance: u32,
    threshold: f64,
) -> Decision {
    decide_by(
        q,
        s,
        found,
        &Rules {
            year_tolerance,
            threshold,
            user_year_tolerance: 1,
            min_confirming: 1,
        },
    )
}

/// Records from different works among `rows` (fetched details, not search lists), as
/// "source: Title (year)" groups; empty when they all agree.
fn dissent(rows: &[&(String, Meta)], year_tolerance: u32) -> Vec<String> {
    let groups = cluster(rows, year_tolerance);
    if groups.len() < 2 {
        return vec![];
    }
    groups
        .iter()
        .map(|g| {
            g.iter()
                .map(|(src, m)| format!("{src}: {}", label(m)))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect()
}

/// Consensus: a match is accepted when at least `min_confirming` found sources confirm it
/// and no found source dissents. Sources that found nothing count neither way. A user's id
/// anchors the work; a user's `--year` rules out records from another year.
pub fn decide_by(q: &Query, s: &Settings, found: &[(String, Meta)], r: &Rules) -> Decision {
    let year_tolerance = r.year_tolerance;
    let threshold = crate::similarity::threshold(r.threshold);
    let set = |o: &Option<String>| o.as_ref().is_some_and(|v| !v.is_empty());
    // only an id (a flag, or the same kind of id read from the file) is trusted alone
    let user_id = set(&q.imdb)
        || set(&q.tmdb)
        || set(&q.thetvdb)
        || set(&q.wikidata)
        || set(&q.wikipedia)
        || set(&s.imdb)
        || set(&s.tmdb)
        || set(&s.thetvdb)
        || set(&s.wikidata)
        || set(&s.wikipedia)
        || set(&s.youtube)
        || set(&s.x);
    let scored = |m: &Meta| {
        let title = m.get("show").or(m.get("title")).map_or("", |s| s.as_str());
        crate::similarity::score(&q.title, title)
    };
    let is_hit = |m: &Meta| {
        let mut m = m.clone();
        if let Some(y) = year(&m) {
            m.insert("year".into(), y);
        }
        exact(&q.title, q.year.as_deref(), &m)
    };
    // the file's own tags are not a source; a row with no title and no id found nothing
    let found: Vec<&(String, Meta)> = found
        .iter()
        .filter(|(src, m)| src != "embedded" && !crate::pick::is_empty_row(m))
        .collect();
    let best = found
        .iter()
        .map(|(_, meta)| scored(meta))
        .fold(0.0_f64, f64::max);
    let hits: Vec<&(String, Meta)> = found.iter().copied().filter(|(_, m)| is_hit(m)).collect();
    let fetched: Vec<&(String, Meta)> = found
        .iter()
        .copied()
        .filter(|(src, _)| !is_search(src))
        .collect();

    // the user's --year rules out a fetched record from another year
    if let Some(want) = s.year.as_deref().filter(|y| !y.is_empty()) {
        let want: String = want.chars().take(4).collect();
        let off: Vec<String> = fetched
            .iter()
            .filter(|(_, m)| {
                year(m)
                    .is_some_and(|y| year_spread(&[y, want.clone()]) > r.user_year_tolerance as i64)
            })
            .map(|(src, m)| format!("{src}: {}", label(m)))
            .collect();
        if !off.is_empty() {
            return Decision::unresolved(
                "year_mismatch",
                format!(
                    "you gave --year {want} (tolerance {}); found {}",
                    r.user_year_tolerance,
                    off.join(", ")
                ),
                best,
            );
        }
    }

    // one source listing several exact matches: they are different works
    let mut srcs: Vec<&str> = hits
        .iter()
        .filter(|(src, _)| is_search(src))
        .map(|(src, _)| src.as_str())
        .collect();
    srcs.sort();
    if srcs.windows(2).any(|w| w[0] == w[1]) {
        return Decision::unresolved(
            "ambiguous",
            format!(
                "more than one work matches \"{}\" at or above {threshold:.2} (best {best:.2})",
                q.title
            ),
            best,
        );
    }

    // fetched details must be about the exact match, not some other top result
    let chosen: Vec<&(String, Meta)> = found
        .iter()
        .copied()
        .filter(|(src, _)| !is_search(src))
        .collect();
    if !user_id
        && !hits.is_empty()
        && let Some((_src, m)) = chosen.iter().find(|(_, m)| {
            m.get("show")
                .or(m.get("title"))
                .is_some_and(|t| norm(t) != norm(&q.title))
                && !is_hit(m)
                && !hits.iter().any(|(_, h)| share_id(h, m))
        })
    {
        return Decision::unresolved(
            "needs_flags",
            format!(
                "a source's details are for {}, not \"{}\"",
                label(m),
                q.title
            ),
            best,
        );
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
            return anchored(&fetched, r);
        }
        return Decision::unresolved(
            "no_match",
            format!(
                "no result for \"{}\" reached {threshold:.2} (best {best:.2})",
                q.title
            ),
            best,
        );
    }

    let clusters = cluster(&members, year_tolerance);

    // an id flag anchors the work; every found source must agree with it
    if user_id {
        return anchored(&fetched, r);
    }

    if clusters.len() > 1 {
        // different identities: not a single match
        return Decision::unresolved(
            "needs_flags",
            format!("sources do not agree on a single work for \"{}\"", q.title),
            best,
        );
    }
    let hits = &clusters[0];
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
        return Decision::unresolved(
            "needs_flags",
            format!("sources do not agree on a single work for \"{}\"", q.title),
            best,
        );
    }

    let accepted = hits
        .iter()
        .map(|(_, meta)| scored(meta))
        .fold(0.0_f64, f64::max);

    if let Some(d) = variations(hits, year_tolerance) {
        return Decision::unresolved("needs_judgment", d, accepted);
    }

    // a found source whose details are some other work dissents
    let mut rows: Vec<&(String, Meta)> = hits.clone();
    rows.extend(fetched.iter().copied().filter(|row| {
        !hits.iter().any(|h| std::ptr::eq(*h, *row))
            && !hits
                .iter()
                .any(|(_, h)| same_work(h, &row.1, year_tolerance))
    }));
    let split = dissent(&rows, year_tolerance);
    if !split.is_empty() {
        return Decision::unresolved(
            "sources_disagree",
            format!("found sources name different works: {}", split.join(" | ")),
            best,
        );
    }
    if confirming(hits) < r.min_confirming {
        return Decision::unresolved(
            "too_few_sources",
            format!(
                "{} found source(s) confirm it; [matching] min_confirming is {}",
                confirming(hits),
                r.min_confirming
            ),
            accepted,
        );
    }

    // one work at or above the threshold is enough, from one source or several
    Decision::sure(if accepted > 0.0 { accepted } else { best })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d0(q: &Query, s: &Settings, f: &[(String, Meta)]) -> Decision {
        decide(q, s, f, 0, 0.90)
    }
    fn sure(d: &Decision) -> bool {
        d.is_sure()
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
        let one = d0(
            &q("The Matrix", Some("1999")),
            &none,
            &[m("tmdb", "The Matrix", "1999")],
        );
        assert!(sure(&one), "one result at 1.0 is enough: {one:?}");
        assert!((one.score() - 1.0).abs() < 1e-9);
        assert!(sure(&d0(
            &q("The Matrix", Some("1999")),
            &none,
            &[
                m("tmdb", "The Matrix", "1999"),
                m("thetvdb", "The Matrix", "1999"),
            ]
        )));
        assert_eq!(
            d0(
                &q("The Matrix", Some("1999")),
                &none,
                &[m("tmdb", "The Matrix Reloaded", "2003")]
            )
            .reason(),
            Some("no_match")
        );
        let normalized = d0(
            &q("breaking.bad", None),
            &none,
            &[
                m("tmdb", "Breaking Bad", "2008"),
                m("thetvdb", "Breaking Bad", "2008"),
            ],
        );
        assert!(sure(&normalized));
        assert!((normalized.score() - 1.0).abs() < 1e-9);
        assert_eq!(
            d0(
                &q("Breaking Bad", None),
                &none,
                &[m("tmdb", "Breaking Bad: Original Minisodes", "2009")]
            )
            .reason(),
            Some("no_match")
        );
        let named = Settings {
            name: Some("Breaking Bad".into()),
            year: Some("2008".into()),
            ..Default::default()
        };
        // --name/--year only filter the search; they are not a trusted match
        assert_eq!(
            d0(&q("Breaking Bad", Some("2008")), &named, &[]).reason(),
            Some("no_match"),
            "--name/--year do not invent a match"
        );
        let named99 = Settings {
            name: Some("The Matrix".into()),
            year: Some("1999".into()),
            ..Default::default()
        };
        assert!(
            sure(&d0(
                &q("The Matrix", Some("1999")),
                &named99,
                &[m("tmdb", "The Matrix", "1999")]
            )),
            "one result above the threshold is enough; --name/--year only filtered"
        );
        assert!(sure(&d0(
            &q("The Matrix", Some("1999")),
            &named99,
            &[
                m("tmdb", "The Matrix", "1999"),
                m("thetvdb", "The Matrix", "1999"),
            ]
        )));
        // an id flag is enough even with no (or one) source; other sources are still queried
        let trusted = d0(
            &Query {
                imdb: Some("tt1".into()),
                ..q("x", None)
            },
            &none,
            &[],
        );
        assert!(trusted.is_sure() && (trusted.score() - 1.0).abs() < 1e-9);
        assert!(
            d0(
                &Query {
                    tmdb: Some("603".into()),
                    ..q("The Matrix", Some("1999"))
                },
                &none,
                &[m("tmdb", "The Matrix", "1999")]
            )
            .is_sure()
        );
        assert!(
            d0(
                &Query {
                    wikidata: Some("Q83495".into()),
                    ..q("The Matrix", Some("1999"))
                },
                &none,
                &[]
            )
            .is_sure(),
            "a wikidata id is trusted alone"
        );
        assert!(
            d0(
                &Query {
                    wikipedia: Some("The Matrix".into()),
                    ..q("", None)
                },
                &none,
                &[]
            )
            .is_sure(),
            "a wikipedia title is trusted alone"
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
        assert_eq!(d.reason(), Some("ambiguous"), "{d:?}");
        // the year leaves a single result, so that result is accepted
        assert!(
            d0(&q("The Office", Some("2005")), &none, &f).is_sure(),
            "one result at or above the threshold"
        );
        let f2 = [
            m("tmdb", "The Office", "2005"),
            m("tmdb:search", "The Office", "2005"),
            m("thetvdb:search", "The Office", "2005"),
        ];
        let agreed = d0(&q("The Office", Some("2005")), &none, &f2);
        assert!(agreed.is_sure(), "{agreed:?}");
        assert!((agreed.score() - 1.0).abs() < 1e-9);
        // each list has one, but the sources disagree
        let f = [
            m("tmdb:search", "Dune", "2021"),
            m("thetvdb:search", "Dune", "1984"),
        ];
        assert_eq!(
            d0(&q("Dune", None), &none, &f).reason(),
            Some("needs_flags")
        );
        // exactly one match across all lists
        let f = [
            m("tmdb", "The Why Files", "2021"),
            m("tmdb:search", "The Why Files", "2021"),
            m("tmdb:search", "Why Women Kill", "2019"),
            m("thetvdb:search", "The Why Files", "2021"),
        ];
        assert!(d0(&q("The Why Files", None), &none, &f).is_sure());
        // the exact match was listed but the details fetched are another work's
        let f = [
            m("tmdb", "Why Women Kill", "2019"),
            m("tmdb:search", "Why Women Kill", "2019"),
            m("tmdb:search", "The Why Files", "2021"),
        ];
        assert_eq!(
            d0(&q("The Why Files", None), &none, &f).reason(),
            Some("needs_flags")
        );
    }
    #[test]
    fn year_tolerance_is_a_hint_for_agreement() {
        let none = Settings::default();
        let f = [
            m("tmdb:search", "Amelie", "2001"),
            m("tvmaze:search", "Amelie", "2002"),
        ];
        assert!(decide(&q("Amelie", None), &none, &f, 1, 0.90).is_sure());
        assert_eq!(
            decide(&q("Amelie", None), &none, &f, 0, 0.90).reason(),
            Some("needs_flags"),
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
        assert_eq!(
            d0(&q("The Matrix", Some("1999")), &none, &f).reason(),
            Some("needs_judgment")
        );
    }

    fn ids(src: &str, t: &str, y: &str, pairs: &[(&str, &str)]) -> (String, Meta) {
        let (s, mut meta) = m(src, t, y);
        for (k, v) in pairs {
            meta.insert(k.to_string(), v.to_string());
        }
        (s, meta)
    }

    fn mountain_flags() -> Settings {
        Settings {
            thetvdb: Some("27498".into()),
            year: Some("2017".into()),
            ..Default::default()
        }
    }

    fn mountain_q() -> Query {
        Query {
            title: "Mountain".into(),
            year: Some("2017".into()),
            thetvdb: Some("27498".into()),
            ..Default::default()
        }
    }

    /// Ground truth: Mountain (2017 documentary), TMDB 462145, IMDb tt6203570, TheTVDB
    /// movies id 27498. The file's own year (2018) is not a source.
    fn mountain_found() -> Vec<(String, Meta)> {
        vec![
            ids(
                "thetvdb",
                "Mountain",
                "2017",
                &[
                    ("thetvdb_id", "27498"),
                    ("imdb_id", "tt6203570"),
                    ("tmdb_id", "462145"),
                ],
            ),
            ids(
                "tmdb",
                "Mountain",
                "2017",
                &[("tmdb_id", "462145"), ("imdb_id", "tt6203570")],
            ),
            ids("embedded", "Mountain", "2018", &[]),
        ]
    }

    #[test]
    fn consensus_accepts_confirmed_work_and_ignores_sources_that_found_nothing() {
        // TVmaze found nothing, so it is not in the list and counts neither way
        let d = d0(&mountain_q(), &mountain_flags(), &mountain_found());
        assert!(sure(&d), "{d:?}");
    }

    #[test]
    fn a_user_year_blocks_a_record_from_another_year() {
        let mut f = mountain_found();
        f.push(ids(
            "omdb",
            "Brokeback Mountain",
            "2005",
            &[("imdb_id", "tt0388795"), ("tmdb_id", "142")],
        ));
        let d = d0(&mountain_q(), &mountain_flags(), &f);
        assert_eq!(d.reason(), Some("year_mismatch"), "{d:?}");
        // a search list entry from another year is only a candidate, not a dissent
        let mut f = mountain_found();
        f.push(m("tmdb:search", "Brokeback Mountain", "2005"));
        assert!(sure(&d0(&mountain_q(), &mountain_flags(), &f)));
        // the tolerance comes from [matching] user_year_tolerance
        let mut f = mountain_found();
        f[1].1.insert("year".into(), "2018".into());
        let strict = Rules {
            year_tolerance: 1,
            threshold: 0.90,
            user_year_tolerance: 0,
            min_confirming: 1,
        };
        let s = mountain_flags();
        assert_eq!(
            decide_by(&mountain_q(), &s, &f, &strict).reason(),
            Some("year_mismatch")
        );
        assert!(
            decide_by(
                &mountain_q(),
                &s,
                &f,
                &Rules {
                    user_year_tolerance: 1,
                    ..strict
                }
            )
            .is_sure()
        );
    }

    #[test]
    fn a_found_source_naming_another_work_dissents_even_with_a_user_id() {
        let mut f = mountain_found();
        f.push(ids(
            "omdb",
            "Brokeback Mountain",
            "2017",
            &[("imdb_id", "tt0388795")],
        ));
        let no_year = Settings {
            thetvdb: Some("27498".into()),
            ..Default::default()
        };
        let d = d0(&mountain_q(), &no_year, &f);
        assert_eq!(d.reason(), Some("sources_disagree"), "{d:?}");
        // without an id: one confirming source and a dissenting one is not a match
        let q = q("Mountain", Some("2017"));
        let f = vec![
            m("tmdb", "Mountain", "2017"),
            ids("thetvdb", "Mountain", "2005", &[("thetvdb_id", "9")]),
        ];
        assert!(!sure(&d0(&q, &Settings::default(), &f)));
    }

    #[test]
    fn min_confirming_is_read_from_config() {
        let r = Rules {
            year_tolerance: 1,
            threshold: 0.90,
            user_year_tolerance: 1,
            min_confirming: 2,
        };
        let one = vec![mountain_found()[0].clone()];
        assert_eq!(
            decide_by(&mountain_q(), &mountain_flags(), &one, &r).reason(),
            Some("too_few_sources")
        );
        assert!(decide_by(&mountain_q(), &mountain_flags(), &mountain_found(), &r).is_sure());
        let c = crate::matcher::MatchCfg {
            min_confirming: 0,
            ..Default::default()
        };
        assert_eq!(
            Rules::from(&c).min_confirming,
            1,
            "at least one source confirms"
        );
    }

    #[test]
    fn picks_confirm_one_work_and_picks_of_different_works_settle_nothing() {
        let r = Rules {
            year_tolerance: 1,
            threshold: 0.90,
            user_year_tolerance: 1,
            min_confirming: 1,
        };
        let one = vec![ids("tmdb", "Mountain", "2017", &[("tmdb_id", "462145")])];
        assert!(picked(&one, &r).is_sure());
        let two = vec![
            one[0].clone(),
            ids("thetvdb", "Mountain", "2005", &[("thetvdb_id", "555")]),
        ];
        assert_eq!(picked(&two, &r).reason(), Some("sources_disagree"));
        assert_eq!(picked(&[], &r).reason(), Some("no_match"));
    }
}
