//! The file's own metadata is the truth for facts, not for wording. A source's value may replace
//! it only when it says the same thing (same words, other spacing/case/separators), or when
//! evidence confirms the same entity: an id (shared with the file, or given by the user), or the
//! user's own word (--name/--year, channel mapping). A matching year is a hint, never proof. Without evidence, the file's value stays and the difference is
//! recorded (conflict, plus an entry for research). Similar wording is never proof ("Joe Rogan"
//! vs "The Joe Rogan Experience" could be different things). Originals are always stored.
use crate::meta::Meta;
use crate::resolver::norm;

/// Fields where the file's value is a fact.
pub const FACTS: [&str; 7] = [
    "title",
    "show",
    "year",
    "date",
    "season",
    "episode",
    "episode_title",
];
/// Exact facts: any difference is a contradiction, evidence or not.
pub const EXACT: [&str; 4] = ["year", "date", "season", "episode"];

fn y4(v: &str) -> String {
    v.chars().take(4).collect()
}

/// The file's own tags that disagree with every found source holding the same field, used when
/// the user gave an id: that id outranks the file, so these look like wrong file data (e.g. an
/// earlier bad write). Ids must match exactly; facts as in [`same`]. Search lists don't count.
pub fn suspect(own: &Meta, found: &[(String, Meta)]) -> Vec<String> {
    const IDS: [&str; 5] = [
        "imdb_id",
        "tmdb_id",
        "thetvdb_id",
        "tmdb_series_id",
        "thetvdb_series_id",
    ];
    let rows: Vec<&Meta> = found
        .iter()
        .filter(|(s, _)| s != "embedded" && !crate::candidates::is_search(s))
        .map(|(_, m)| m)
        .collect();
    let mut bad = vec![];
    for k in IDS.iter().chain(["title", "show", "year"].iter()) {
        let Some(v) = own.get(*k).filter(|v| !v.is_empty()) else {
            continue;
        };
        let mut vals: Vec<&str> = rows
            .iter()
            .filter_map(|m| m.get(*k))
            .map(|x| x.as_str())
            .filter(|x| !x.is_empty())
            .collect();
        vals.sort();
        vals.dedup();
        if vals.is_empty() {
            continue;
        }
        let agrees = vals.iter().any(|x| {
            if IDS.contains(k) {
                *x == v.as_str()
            } else {
                same(k, v, x)
            }
        });
        if !agrees {
            bad.push(format!("{k} {v} (sources say {})", vals.join(" / ")));
        }
    }
    bad
}

/// Same value, only written differently.
pub fn same(field: &str, a: &str, b: &str) -> bool {
    if field == "year" {
        y4(a) == y4(b)
    } else {
        norm(a) == norm(b)
    }
}

/// Why the source's answer is the same entity as the file, if anything independent says so.
/// `id_flag`: the user gave an id (--imdb/--tmdb/--thetvdb); `user_named`: the user gave --name/--year.
pub fn evidence(meta: &Meta, own: &Meta, id_flag: bool, user_named: bool) -> Option<&'static str> {
    // only an id proves identity: one the user gave (--imdb/--tmdb/--thetvdb) or one shared with the file
    if id_flag
        || own
            .iter()
            .any(|(k, v)| k.ends_with("_id") && meta.get(k) == Some(v))
    {
        return Some("id");
    }
    // the user's own word: --name/--year, or their channel mapping; settles names, not identity
    if user_named
        || meta
            .get("evidence")
            .is_some_and(|e| e == "flag" || e == "user")
    {
        return Some("user");
    }
    if meta.get("evidence").is_some_and(|e| e == "channel") {
        return Some("channel");
    }
    // a matching name or year is a strong hint, never proof
    None
}

/// Identified by an id (the only absolute evidence)?
pub fn verified(evidence: Option<&str>) -> bool {
    evidence == Some("id")
}

/// Puts the file's facts back wherever they are not confirmed. Returns (field, file value, source value) kept from the file.
pub fn apply(meta: &mut Meta, own: &Meta, evidence: Option<&str>) -> Vec<(String, String, String)> {
    let mut kept = vec![];
    for f in FACTS {
        let Some(o) = own.get(f).filter(|v| !v.trim().is_empty()) else {
            continue;
        };
        let o = if f == "year" { y4(o) } else { o.clone() };
        match meta.get(f).cloned() {
            Some(v) if same(f, &o, &v) => {}
            Some(_) if evidence.is_some() && !EXACT.contains(&f) => {} // confirmed same entity: the fuller name stands
            Some(v) => {
                meta.insert(f.into(), o.clone());
                kept.push((f.to_string(), o, v));
            }
            None => {
                meta.insert(f.into(), o);
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(kv: &[(&str, &str)]) -> Meta {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
    #[test]
    fn wording_is_not_proof_evidence_is() {
        let own = m(&[("show", "Joe Rogan")]);
        // no evidence: the file's name stays, even for a similar-looking fuller name
        let mut meta = m(&[("show", "The Joe Rogan Experience")]);
        assert_eq!(evidence(&meta, &own, false, false), None);
        assert_eq!(apply(&mut meta, &own, None)[0].0, "show");
        assert_eq!(meta["show"], "Joe Rogan");
        // a shared id confirms it: the fuller name stands
        let own = m(&[("show", "Joe Rogan"), ("imdb_id", "tt1")]);
        let mut meta = m(&[("show", "The Joe Rogan Experience"), ("imdb_id", "tt1")]);
        let e = evidence(&meta, &own, false, false);
        assert_eq!(e, Some("id"));
        assert!(apply(&mut meta, &own, e).is_empty());
        assert_eq!(meta["show"], "The Joe Rogan Experience");
        // formatting only is always fine
        let mut meta = m(&[("title", "Breaking Bad")]);
        assert!(apply(&mut meta, &m(&[("title", "breaking.bad")]), None).is_empty());
        // a matching year is a hint, never proof; the user's word settles names; only an id verifies
        assert_eq!(
            evidence(
                &m(&[("year", "2010")]),
                &m(&[("year", "2010-05-01")]),
                false,
                false
            ),
            None
        );
        assert_eq!(
            evidence(&m(&[("evidence", "channel")]), &Meta::new(), false, false),
            Some("channel")
        );
        assert_eq!(
            evidence(&Meta::new(), &Meta::new(), false, true),
            Some("user")
        );
        assert_eq!(
            evidence(&Meta::new(), &Meta::new(), true, false),
            Some("id")
        );
        assert!(
            verified(Some("id"))
                && !verified(Some("user"))
                && !verified(Some("channel"))
                && !verified(None)
        );
        // exact facts are never replaced, evidence or not; gaps are filled from the file
        let mut meta = m(&[("year", "2011")]);
        let k = apply(
            &mut meta,
            &m(&[("year", "2010-05-01"), ("season", "2")]),
            Some("flag"),
        );
        assert_eq!(
            (
                k[0].0.as_str(),
                meta["year"].as_str(),
                meta["season"].as_str()
            ),
            ("year", "2010", "2")
        );
    }
}
