//! Deciding among candidates. Jack accepts only one result at or above the similarity
//! threshold. Sammy (a future LLM windup) can implement `Resolver` and be passed in.
use crate::meta::Meta;
use crate::similarity;

pub use crate::similarity::norm;

/// (source, metadata)
pub type Candidate = (String, Meta);

#[derive(Debug, PartialEq)]
pub enum Choice {
    Pick(usize),
    /// No candidate matches exactly.
    None,
    /// More than one matches exactly: not possible with the available information.
    Unresolved,
}

pub trait Resolver {
    /// `title` and `year` (if known) are what the label says; candidates carry "title"/"show" and "year".
    fn resolve(&self, title: &str, year: Option<&str>, cands: &[Candidate]) -> Choice;
}

/// One candidate at or above the threshold (and the same year, when a year was given).
#[derive(Clone, Copy)]
pub struct Strict {
    pub threshold: f64,
}

impl Default for Strict {
    fn default() -> Self {
        Self { threshold: 0.90 }
    }
}

impl Resolver for Strict {
    fn resolve(&self, title: &str, year: Option<&str>, c: &[Candidate]) -> Choice {
        let threshold = similarity::threshold(self.threshold);
        let hits: Vec<usize> = (0..c.len())
            .filter(|&i| above(title, year, &c[i].1, threshold))
            .collect();
        match hits.as_slice() {
            [] => Choice::None,
            [i] => Choice::Pick(*i),
            _ => Choice::Unresolved,
        }
    }
}

fn above(title: &str, year: Option<&str>, meta: &Meta, threshold: f64) -> bool {
    let candidate = meta.get("show").or(meta.get("title")).map_or("", |s| s);
    similarity::score(title, candidate) >= threshold
        && year.is_none_or(|y| meta.get("year").is_some_and(|my| my == y))
}

/// Same normalized title, or a single score at or above 0.90, and the same year when one was given.
pub fn exact(title: &str, year: Option<&str>, m: &Meta) -> bool {
    above(title, year, m, 0.90)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(t: &str, y: &str) -> Candidate {
        (
            "tmdb".into(),
            [
                ("title".to_string(), t.to_string()),
                ("year".to_string(), y.to_string()),
            ]
            .into(),
        )
    }
    #[test]
    fn strict_only_exact_unique() {
        let s = Strict::default();
        assert_eq!(
            s.resolve(
                "The Why Files",
                None,
                &[c("Why Women Kill", "2019"), c("The Why Files", "2021")]
            ),
            Choice::Pick(1)
        );
        assert_eq!(
            s.resolve(
                "The Office",
                None,
                &[c("The Office", "2005"), c("The Office", "2001")]
            ),
            Choice::Unresolved
        );
        assert_eq!(
            s.resolve(
                "The Office",
                Some("2005"),
                &[c("The Office", "2005"), c("The Office", "2001")]
            ),
            Choice::Pick(0)
        );
        // one mark away is still the only result at or above 0.90
        assert_eq!(
            s.resolve("The Why Files", None, &[c("The Why Files?", "2005")]),
            Choice::Pick(0)
        );
        assert_eq!(
            s.resolve("the  why files", None, &[c("The Why Files", "2021")]),
            Choice::Pick(0)
        );
        assert_eq!(
            s.resolve(
                "Joe Rogan Experience",
                None,
                &[c("The Joe Rogan Experience", "2009")]
            ),
            Choice::None
        );
        assert_eq!(s.resolve("Dr. Shawn Baker", None, &[]), Choice::None);
        assert_eq!(
            s.resolve("breaking.bad", None, &[c("Breaking Bad", "2008")]),
            Choice::Pick(0)
        );
        assert_eq!(
            s.resolve("Breaking_Bad", None, &[c("Breaking-Bad", "2008")]),
            Choice::Pick(0)
        );
    }
}
