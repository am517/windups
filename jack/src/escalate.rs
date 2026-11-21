//! Per-step escalation. At every pipeline step Jack first tries a deterministic, unambiguous
//! answer; if it is still ambiguous it hands everything it has to an `Escalation` (Sammy, later).
//! The answer is a decision or a deferral. Decisions are recorded with provenance (source, step,
//! time) in the decisions table; a deferral means: log the item as incomplete and explain it.
use crate::meta::Meta;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    /// Filename / embedded tags -> query.
    Parse,
    /// A source failed or returned nothing.
    Lookup,
    /// Several (or no) exact candidates.
    Match,
    /// The file's name and a source's name differ without independent evidence.
    Refine,
    /// Sources disagree about a fact.
    Conflict,
}

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::Parse => "parse",
            Step::Lookup => "lookup",
            Step::Match => "match",
            Step::Refine => "refine",
            Step::Conflict => "conflict",
        }
    }
}

/// Everything known at the step.
/// Read by a real Sammy; the no-op stub ignores it.
#[allow(dead_code)]
pub struct Context<'a> {
    pub step: Step,
    pub file: &'a str,
    /// unresolved reason (no_match, ambiguous, needs_flags, ...)
    pub reason: &'a str,
    pub detail: &'a str,
    /// What Jack has so far (merged).
    pub meta: &'a Meta,
    /// Every source answer and search hit, the file's own tags included.
    pub found: &'a [(String, Meta)],
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Decide comes from a real Sammy
pub enum Verdict {
    /// Values to apply, plus a note saying why.
    Decide(Meta, String),
    Defer,
}

pub trait Escalation {
    /// Who decides ("sammy"); recorded as the decision's source.
    fn source(&self) -> &'static str;
    fn decide(&self, ctx: &Context) -> Verdict;
}

/// Sammy is not connected. The can't-decide rows that talk to her stay here until she is.
pub fn sammy_available() -> bool {
    false
}

/// No Sammy yet: always defers (item logged incomplete + static explanation).
pub struct NoSammy;

impl Escalation for NoSammy {
    fn source(&self) -> &'static str {
        "sammy"
    }
    fn decide(&self, _: &Context) -> Verdict {
        Verdict::Defer
    }
}

/// Applies a decision: the values go into meta, and a provenance entry is queued for the store
/// (meta key "decisions": a JSON list of {source, step, decision, note, at}).
pub fn apply(meta: &mut Meta, step: Step, source: &str, values: &Meta, note: &str) {
    meta.extend(values.clone());
    let mut list: Vec<serde_json::Value> = meta
        .get("decisions")
        .and_then(|d| serde_json::from_str(d).ok())
        .unwrap_or_default();
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    list.push(serde_json::json!({"source": source, "step": step.name(), "decision": values, "note": note, "at": at}));
    meta.insert(
        "decisions".into(),
        serde_json::Value::from(list).to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stub_defers_and_decisions_carry_provenance() {
        let m = Meta::new();
        let c = Context {
            step: Step::Match,
            file: "a",
            reason: "ambiguous",
            detail: "",
            meta: &m,
            found: &[],
        };
        assert!(
            !sammy_available(),
            "Sammy is a later card; this path stays a stub"
        );
        assert_eq!(NoSammy.decide(&c), Verdict::Defer);
        let mut meta = Meta::new();
        let v: Meta = [("year".to_string(), "2005".to_string())].into();
        apply(&mut meta, Step::Match, "sammy", &v, "US version");
        apply(&mut meta, Step::Refine, "sammy", &Meta::new(), "same show");
        let d: Vec<serde_json::Value> = serde_json::from_str(&meta["decisions"]).unwrap();
        assert_eq!(
            (
                meta["year"].as_str(),
                d.len(),
                d[0]["step"].as_str(),
                d[1]["source"].as_str()
            ),
            ("2005", 2, Some("match"), Some("sammy"))
        );
    }
}
