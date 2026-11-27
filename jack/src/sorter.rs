//! Merge provider answers into one set of facts. Jack does not sort or rename files.
use crate::meta::Meta;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct MergeCfg {
    pub default: Vec<String>,
    pub fields: BTreeMap<String, Vec<String>>,
    /// Fields where the longest value wins (Jack's fallback until Sammy picks).
    pub longest: Vec<String>,
}

/// For each field, the first provider (per its priority list, then any) with a value wins;
/// `longest` fields take the longest value (ties: priority order).
pub fn merge(results: &[(String, Meta)], cfg: &MergeCfg) -> Meta {
    let mut out = Meta::new();
    let keys: std::collections::BTreeSet<&String> =
        results.iter().flat_map(|(_, m)| m.keys()).collect();
    for k in keys {
        let order = cfg.fields.get(k).unwrap_or(&cfg.default);
        if cfg.longest.contains(k) {
            let ranked = order
                .iter()
                .filter_map(|p| results.iter().find(|(n, _)| n == p))
                .chain(results.iter().filter(|(n, _)| !order.contains(n)));
            if let Some(v) = ranked
                .filter_map(|(_, m)| m.get(k))
                .rev()
                .max_by_key(|v| v.chars().count())
            {
                out.insert(k.clone(), v.clone());
            }
            continue;
        }
        let by_prio = order.iter().find_map(|p| {
            results
                .iter()
                .find(|(n, _)| n == p)
                .and_then(|(_, m)| m.get(k))
        });
        if let Some(v) = by_prio.or_else(|| results.iter().find_map(|(_, m)| m.get(k))) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}
