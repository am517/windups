//! Telling the user about each item Jack could not settle: why, and what would help.
//! He does not offer candidate lists; choosing is his job.
//! Without an LLM: static templates from [explain] in jack.toml. Sammy can implement
//! `Explainer` later for thoughtful explanations.
use serde::Deserialize;
use std::collections::BTreeMap;

/// The line Jack exits with when he could not settle on a single match.
pub const MORE_CONTEXT: &str = "I couldn't figure it out. Please give more context: an id (--imdb, --tmdb, or --thetvdb) lets me trust a single result; --year can narrow several works with the same name, but is not enough on its own.";

/// One unsettled item.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub file: String,
    pub reason: String,
    pub detail: String,
    /// Tagged anyway (with the file's own values) but still open; false = not tagged.
    pub tagged: bool,
}

pub trait Explainer {
    fn explain(&self, item: &Item) -> String;
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct ExplainCfg {
    /// reason -> template; placeholders {file}, {reason}, {detail}.
    pub templates: BTreeMap<String, String>,
    /// For reasons without a template.
    pub fallback: String,
}

/// Static templates.
pub struct Templates<'a>(pub &'a ExplainCfg);

impl Explainer for Templates<'_> {
    fn explain(&self, i: &Item) -> String {
        let t = self
            .0
            .templates
            .get(&i.reason)
            .filter(|t| !t.is_empty())
            .unwrap_or(&self.0.fallback);
        let t = if t.is_empty() {
            "{reason}: {detail}"
        } else {
            t.as_str()
        };
        t.replace("{file}", &i.file)
            .replace("{reason}", &i.reason)
            .replace("{detail}", &i.detail)
    }
}

/// End-of-run report: one line per item.
pub fn report(items: &[Item], ex: &dyn Explainer) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut s = format!(
        "\n{} item(s) need attention (all kept in the database, marked incomplete):\n",
        items.len()
    );
    for i in items {
        s += &format!(
            "  {}{}\n    {}\n",
            i.file,
            if i.tagged {
                " (tagged with the file's own values)"
            } else {
                " (not tagged)"
            },
            ex.explain(i)
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_and_report() {
        let cfg = crate::config::load(Some(std::path::Path::new("config/jack.toml")))
            .unwrap()
            .explain;
        let i = Item {
            file: "a.mkv".into(),
            reason: "ambiguous".into(),
            detail: "\"The Office\" could be The Office (2005) / The Office (2001)".into(),
            tagged: false,
        };
        let e = Templates(&cfg).explain(&i);
        assert!(
            e.contains("couldn't figure it out") && e.contains("--year"),
            "{e}"
        );
        let odd = Item {
            reason: "made_up".into(),
            detail: "x".into(),
            ..i.clone()
        };
        assert!(Templates(&cfg).explain(&odd).contains("made_up"));
        assert!(Templates(&ExplainCfg::default()).explain(&odd) == "made_up: x");
        let r = report(&[i], &Templates(&cfg));
        assert!(
            r.contains("1 item(s)") && r.contains("a.mkv (not tagged)"),
            "{r}"
        );
        assert!(report(&[], &Templates(&cfg)).is_empty());
    }
}
