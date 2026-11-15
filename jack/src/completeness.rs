//! Which fields an entry must have before it can be complete, per media kind (from [completeness]).
use crate::meta::Meta;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct CompletenessCfg {
    /// kind -> fields that must be filled.
    pub required: BTreeMap<String, Vec<String>>,
    /// kind -> fields nice to have (listed, never block complete).
    pub optional: BTreeMap<String, Vec<String>>,
    /// For kinds without a list.
    pub default_required: Vec<String>,
}

/// Required fields that are missing or empty, in config order.
pub fn missing(meta: &Meta, cfg: &CompletenessCfg) -> Vec<String> {
    let kind = meta.get("kind").map_or("movie", |k| k.as_str());
    cfg.required
        .get(kind)
        .unwrap_or(&cfg.default_required)
        .iter()
        .filter(|f| meta.get(*f).is_none_or(|v| v.trim().is_empty()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn required_fields_per_kind() {
        let t: toml::Value = toml::from_str(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/config/jack.toml"))
                .unwrap(),
        )
        .unwrap();
        let cfg: CompletenessCfg = t["completeness"].clone().try_into().unwrap();
        let m = |kv: &[(&str, &str)]| -> Meta {
            kv.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        let movie = m(&[
            ("kind", "movie"),
            ("title", "T"),
            ("year", "1999"),
            ("genre", "g"),
            ("poster_url", "u"),
            ("description", " "),
        ]);
        assert!(
            missing(&movie, &cfg).is_empty(),
            "poster and description are not required for submission"
        );
        assert!(
            cfg.optional["movie"].iter().any(|f| f == "cast"),
            "cast is optional"
        );
        assert_eq!(missing(&m(&[("kind", "brand_new_kind")]), &cfg), ["title"]);
    }
}
