//! Candidate picks when Jack can't decide and Sammy isn't there. The list is grouped by source
//! and numbered across the whole list. The user picks at most one per source: picked candidates
//! count as confirming sources, unpicked ones are ignored.
use crate::meta::Meta;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct CandidatesCfg {
    /// Ask on a terminal (never in a pipe or for another windup).
    pub ask: bool,
    /// Hits kept from each source's search list (`--limit N` for one run).
    pub per_source: usize,
    /// How many more per source each "more" at the prompt asks for.
    pub more: usize,
    /// Sources left out of the list: Jack has no id lookup for them yet, so a pick can't be
    /// fetched in full.
    pub unpickable: Vec<String>,
}

impl Default for CandidatesCfg {
    fn default() -> Self {
        CandidatesCfg {
            ask: true,
            per_source: 5,
            more: 5,
            unpickable: vec!["tvmaze".into(), "itunes".into()],
        }
    }
}

/// "tmdb:search" and "tmdb" are one source.
pub fn base(source: &str) -> &str {
    source.split(':').next().unwrap_or(source)
}

/// One numbered row: `index` points into the found list.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub n: usize,
    pub index: usize,
    pub text: String,
}

/// Candidates grouped by source (first-seen order), and the sources that found nothing usable.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct List {
    pub groups: Vec<(String, Vec<Row>)>,
    pub not_found: Vec<String>,
}

fn ids(m: &Meta) -> Vec<String> {
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

fn title(m: &Meta) -> Option<&str> {
    m.get("show")
        .or(m.get("title"))
        .map(|t| t.as_str())
        .filter(|t| !t.is_empty())
}

/// No title and no id: nothing to show or count.
pub fn is_empty_row(m: &Meta) -> bool {
    title(m).is_none() && ids(m).is_empty()
}

#[cfg(test)]
pub fn list(query_title: &str, found: &[(String, Meta)]) -> List {
    list_without(query_title, found, &CandidatesCfg::default().unpickable)
}

/// The list, leaving out the sources in `skip` (quietly: they are not even "not found").
pub fn list_without(query_title: &str, found: &[(String, Meta)], skip: &[String]) -> List {
    let mut l = List::default();
    let mut n = 0;
    for (index, (source, m)) in found.iter().enumerate() {
        if source == "embedded" || skip.iter().any(|s| s == base(source)) {
            continue;
        }
        let src = base(source).to_string();
        if is_empty_row(m) {
            if !l.not_found.contains(&src) && !l.groups.iter().any(|(g, _)| *g == src) {
                l.not_found.push(src);
            }
            continue;
        }
        l.not_found.retain(|s| *s != src);
        let pos = match l.groups.iter().position(|(g, _)| *g == src) {
            Some(p) => p,
            None => {
                l.groups.push((src, vec![]));
                l.groups.len() - 1
            }
        };
        // the fetched record and its own search hit are one candidate
        let dup = l.groups[pos].1.iter().any(|r| {
            let o = &found[r.index].1;
            let (a, b) = (ids(o), ids(m));
            (!a.is_empty() && a.iter().any(|x| b.contains(x)))
                || (title(o) == title(m) && o.get("year") == m.get("year"))
        });
        if dup {
            continue;
        }
        let t = title(m).unwrap_or("?");
        let year = m.get("year").map_or("?", |y| y.as_str());
        let score = m
            .get("match_score")
            .map(|s| s.chars().take(4).collect::<String>())
            .unwrap_or_else(|| format!("{:.2}", crate::similarity::score(query_title, t)));
        let id = ids(m)
            .into_iter()
            .next()
            .map(|i| format!(", {i}"))
            .unwrap_or_default();
        l.groups[pos].1.push(Row {
            n: 0,
            index,
            text: format!("{t} ({year}){id}, score {score}"),
        });
    }
    // numbered in the order shown
    for (_, rows) in l.groups.iter_mut() {
        for r in rows.iter_mut() {
            n += 1;
            r.n = n;
            r.text = format!("{n}. {}", r.text);
        }
    }
    l
}

/// The full record of a picked search hit, fetched from its own source by the hit's id.
/// An error when the source has no id lookup, the hit has no id, or the fetch fails.
pub fn details(
    source: &str,
    hit: &Meta,
    cfg: &crate::config::Config,
    http: &dyn crate::providers::Http,
) -> Result<Meta, String> {
    let tv = hit.get("kind").is_some_and(|k| k != "movie");
    let mut q = crate::providers::Query {
        title: title(hit).unwrap_or("").to_string(),
        year: hit.get("year").cloned(),
        kind: Some(if tv { "tv" } else { "movie" }.into()),
        ..Default::default()
    };
    let id = match source {
        "tmdb" => {
            q.tmdb = hit.get("tmdb_series_id").or(hit.get("tmdb_id")).cloned();
            q.tmdb.clone()
        }
        "thetvdb" => {
            q.thetvdb = hit.get("thetvdb_id").cloned();
            q.thetvdb.clone()
        }
        _ => return Err(format!("{source} has no lookup by id in Jack yet")),
    };
    if id.is_none_or(|i| i.is_empty()) {
        return Err("the hit has no id to look up".into());
    }
    let only = [source.to_string()];
    let (_, f) = crate::fetch::fetch_from(&q, cfg, http, &mut String::new(), &[], Some(&only))?;
    f.into_iter()
        .find(|(s, _)| s == source)
        .map(|(_, m)| m)
        .ok_or(format!("{source} returned no details"))
}

impl List {
    /// The list for `-j`: every candidate by source, with its number, facts, ids and score.
    pub fn json(&self, found: &[(String, Meta)]) -> serde_json::Value {
        let groups: Vec<serde_json::Value> = self
            .groups
            .iter()
            .map(|(src, rows)| {
                let rows: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|r| {
                        let (s, m) = &found[r.index];
                        let ids: serde_json::Map<String, serde_json::Value> = m
                            .iter()
                            .filter(|(k, v)| k.ends_with("_id") && !v.is_empty())
                            .map(|(k, v)| (k.clone(), v.clone().into()))
                            .collect();
                        serde_json::json!({
                            "n": r.n,
                            "from": if crate::candidates::is_search(s) { "search" } else { "lookup" },
                            "kind": m.get("kind"),
                            "title": title(m),
                            "year": m.get("year"),
                            "ids": ids,
                            "score": m.get("match_score").and_then(|v| v.parse::<f64>().ok()),
                        })
                    })
                    .collect();
                serde_json::json!({ "source": src, "rows": rows })
            })
            .collect();
        serde_json::json!({ "groups": groups, "not_found": self.not_found })
    }
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
    /// The lines with the rows that are the chosen work marked `*` (shared id, or same title
    /// and year as `chosen`).
    pub fn marked(&self, found: &[(String, Meta)], chosen: &Meta) -> Vec<String> {
        let mine = ids(chosen);
        let mut v = vec![];
        for (src, rows) in &self.groups {
            v.push(format!("{src}:"));
            for r in rows {
                let m = &found[r.index].1;
                let same = ids(m).iter().any(|i| mine.contains(i))
                    || (title(m).is_some()
                        && title(m) == title(chosen)
                        && m.get("year") == chosen.get("year"));
                v.push(format!("  {}{}", r.text, if same { "  *" } else { "" }));
            }
        }
        if !self.not_found.is_empty() {
            v.push(format!("not found: {}", self.not_found.join(", ")));
        }
        v
    }
    /// The lines to show, once.
    pub fn lines(&self) -> Vec<String> {
        let mut v = vec![];
        for (src, rows) in &self.groups {
            v.push(format!("{src}:"));
            v.extend(rows.iter().map(|r| format!("  {}", r.text)));
        }
        if !self.not_found.is_empty() {
            v.push(format!("not found: {}", self.not_found.join(", ")));
        }
        v
    }
    /// Picks typed as numbers separated by spaces (or commas). Empty or `0`: none.
    /// At most one per source. Returns indexes into the found list.
    pub fn parse(&self, text: &str) -> Result<Vec<usize>, String> {
        let mut picked: Vec<(String, usize, usize)> = vec![];
        for tok in text.split([' ', ',', '\t', '\n']).filter(|t| !t.is_empty()) {
            let n: usize = tok
                .parse()
                .map_err(|_| format!("\"{tok}\" is not a number from the list"))?;
            if n == 0 {
                continue;
            }
            let (src, row) = self
                .groups
                .iter()
                .find_map(|(s, rows)| rows.iter().find(|r| r.n == n).map(|r| (s, r)))
                .ok_or(format!("{n} is not in the list"))?;
            if let Some((_, m, _)) = picked.iter().find(|(s, _, _)| s == src) {
                return Err(format!(
                    "{m} and {n} are both from {src}; pick at most one per source"
                ));
            }
            picked.push((src.clone(), n, row.index));
        }
        Ok(picked.into_iter().map(|(_, _, i)| i).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(pairs: &[(&str, &str)]) -> Meta {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn found() -> Vec<(String, Meta)> {
        vec![
            (
                "thetvdb".into(),
                m(&[
                    ("title", "Mountain"),
                    ("year", "2017"),
                    ("thetvdb_id", "27498"),
                ]),
            ),
            (
                "tmdb".into(),
                m(&[
                    ("title", "Brokeback Mountain"),
                    ("year", "2005"),
                    ("tmdb_id", "142"),
                ]),
            ),
            (
                "tmdb:search".into(),
                m(&[
                    ("title", "Mountain"),
                    ("year", "2017"),
                    ("tmdb_id", "462145"),
                ]),
            ),
            (
                "tmdb:search".into(),
                m(&[
                    ("title", "Brokeback Mountain"),
                    ("year", "2005"),
                    ("tmdb_id", "142"),
                ]),
            ),
            ("wikidata".into(), m(&[])),
            ("itunes".into(), m(&[("title", "")])),
            ("embedded".into(), m(&[("title", "Brokeback Mountain")])),
        ]
    }

    #[test]
    fn candidates_are_grouped_by_source_once_and_empty_rows_are_not_found() {
        let l = list("Mountain", &found());
        assert_eq!(
            l.lines(),
            [
                "thetvdb:",
                "  1. Mountain (2017), thetvdb_id:27498, score 1.00",
                "tmdb:",
                "  2. Brokeback Mountain (2005), tmdb_id:142, score 0.44",
                "  3. Mountain (2017), tmdb_id:462145, score 1.00",
                "not found: wikidata",
            ]
        );
    }

    #[test]
    fn picks_are_at_most_one_per_source() {
        let l = list("Mountain", &found());
        assert_eq!(l.parse("1 3"), Ok(vec![0, 2]));
        assert_eq!(l.parse(""), Ok(vec![]));
        assert_eq!(l.parse("0"), Ok(vec![]));
        assert_eq!(l.parse("3,1"), Ok(vec![2, 0]));
        assert!(
            l.parse("2 3")
                .unwrap_err()
                .contains("at most one per source")
        );
        assert!(l.parse("9").unwrap_err().contains("not in the list"));
        assert!(l.parse("x").unwrap_err().contains("not a number"));
    }

    #[test]
    fn unpickable_sources_are_left_out_and_chosen_rows_are_marked() {
        let mut f = found();
        f.push((
            "tvmaze:search".into(),
            m(&[("title", "Mountain"), ("year", "2017"), ("tvmaze_id", "3")]),
        ));
        f.push((
            "itunes".into(),
            m(&[("title", "Mountain"), ("year", "2017")]),
        ));
        let l = list_without("Mountain", &f, &CandidatesCfg::default().unpickable);
        let text = l.lines().join("\n");
        assert!(
            !text.contains("tvmaze") && !text.contains("itunes"),
            "{text}"
        );
        let chosen = m(&[
            ("title", "Mountain"),
            ("year", "2017"),
            ("tmdb_id", "462145"),
        ]);
        let marked = l.marked(&f, &chosen);
        assert!(
            marked
                .iter()
                .any(|x| x.contains("Mountain (2017), thetvdb_id:27498") && x.ends_with("*"))
        );
        assert!(
            marked
                .iter()
                .any(|x| x.contains("tmdb_id:462145") && x.ends_with("*"))
        );
        assert!(
            marked
                .iter()
                .any(|x| x.contains("Brokeback") && !x.ends_with("*"))
        );
    }
}
