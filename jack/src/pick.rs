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
    /// Hits kept from each source's search list.
    pub per_source: usize,
}

impl Default for CandidatesCfg {
    fn default() -> Self {
        CandidatesCfg {
            ask: true,
            per_source: 5,
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

pub fn list(query_title: &str, found: &[(String, Meta)]) -> List {
    let mut l = List::default();
    let mut n = 0;
    for (index, (source, m)) in found.iter().enumerate() {
        if source == "embedded" {
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
        n += 1;
        l.groups[pos].1.push(Row {
            n,
            index,
            text: format!("{n}. {t} ({year}){id}, score {score}"),
        });
    }
    l
}

impl List {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
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
                "not found: wikidata, itunes",
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
}
