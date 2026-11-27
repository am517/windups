//! Running one step's escalation: Sammy decides (recorded with provenance) or defers (the item is
//! kept, marked incomplete, and explained to the user now and at the end of the run).
use crate::escalate::{self, Context, Escalation, Step, Verdict};
use crate::explain::{Explainer, Item};
use crate::meta::Meta;
use std::fmt::Write as _;

pub struct Hooks<'a> {
    pub esc: &'a dyn Escalation,
    pub ex: &'a dyn Explainer,
    /// Everything left open this run, for the end-of-run report.
    pub open: Vec<Item>,
    /// Every item finished this run, for the optional review pass.
    pub done: Vec<(String, crate::meta::Meta)>,
    /// Each item's candidate list, for `-j` (file, list JSON).
    pub lists: Vec<(String, serde_json::Value)>,
}

impl Hooks<'_> {
    /// True when the escalation decided (values applied to `meta`).
    #[allow(clippy::too_many_arguments)]
    pub fn settle(
        &mut self,
        step: Step,
        file: &str,
        reason: &str,
        detail: &str,
        meta: &mut Meta,
        found: &[(String, Meta)],
        tagged: bool,
        out: &mut String,
    ) -> bool {
        let ctx = Context {
            step,
            file,
            reason,
            detail,
            meta,
            found,
        };
        match self.esc.decide(&ctx) {
            Verdict::Decide(v, note) => {
                let src = self.esc.source();
                escalate::apply(meta, step, src, &v, &note);
                let _ = writeln!(out, "    {}: {src} decided ({note}); recorded", step.name());
                true
            }
            Verdict::Defer => {
                meta.insert("completeness".into(), "incomplete".into());
                let i = Item {
                    file: file.into(),
                    reason: reason.into(),
                    detail: detail.into(),
                    tagged,
                };
                let _ = writeln!(out, "    needs attention: {}", self.ex.explain(&i));
                self.open.push(i);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Sure;
    impl Escalation for Sure {
        fn source(&self) -> &'static str {
            "sammy"
        }
        fn decide(&self, c: &Context) -> Verdict {
            assert_eq!(
                (c.step, c.found.len()),
                (Step::Match, 1),
                "gets all the data"
            );
            Verdict::Decide(
                [("year".to_string(), "2005".to_string())].into(),
                "US version".into(),
            )
        }
    }
    #[test]
    fn defer_keeps_and_explains_decide_records() {
        let cfg = crate::explain::ExplainCfg::default();
        let ex = crate::explain::Templates(&cfg);
        let found = vec![("tmdb:search".to_string(), Meta::new())];
        let (mut m, mut out) = (Meta::new(), String::new());
        let mut h = Hooks {
            esc: &escalate::NoSammy,
            ex: &ex,
            open: vec![],
            done: vec![],
            lists: vec![],
        };
        assert!(!h.settle(
            Step::Match,
            "a.mkv",
            "ambiguous",
            "x or y",
            &mut m,
            &found,
            false,
            &mut out
        ));
        assert_eq!(
            (h.open.len(), m["completeness"].as_str()),
            (1, "incomplete")
        );
        assert!(out.contains("needs attention: ambiguous: x or y"), "{out}");
        let mut h = Hooks {
            esc: &Sure,
            ex: &ex,
            open: vec![],
            done: vec![],
            lists: vec![],
        };
        let mut m = Meta::new();
        assert!(h.settle(
            Step::Match,
            "a.mkv",
            "ambiguous",
            "x or y",
            &mut m,
            &found,
            false,
            &mut out
        ));
        assert!(
            h.open.is_empty()
                && m["year"] == "2005"
                && m["decisions"].contains("\"step\":\"match\"")
        );
    }
}
