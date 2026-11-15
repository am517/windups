//! Objective vs subjective fields. Jack keeps everything in jack.db. The record he passes
//! to the database windup carries only objective facts (name, date, type, ids, ...).
//! Subjective data (summaries, images) stays here; other windups ask Jack for it (`jack ask`).
use crate::meta::Meta;
use windups::handoff::Handoff;

/// Fields Jack will not pass to the database windup.
pub fn is_subjective(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.contains("description")
        || k.contains("summary")
        || k.contains("overview")
        || k.contains("plot")
        || k.contains("poster")
        || k.contains("artwork")
        || k.contains("thumbnail")
        || k.contains("backdrop")
        || k.contains("still")
        || k == "image"
        || k.ends_with("_image")
        || k.starts_with("image_")
        || k.contains("poster_file")
        || k.contains("art_url")
}

pub fn is_objective(key: &str) -> bool {
    !is_subjective(key)
}

pub fn filter_meta(m: &Meta) -> Meta {
    m.iter()
        .filter(|(k, _)| is_objective(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// A copy of the handoff with subjective fields stripped from the entry, candidates, and
/// conflicts. The original in jack.db is left whole.
pub fn for_database(h: &Handoff) -> Handoff {
    Handoff {
        file: h.file.clone(),
        entry: filter_meta(&h.entry),
        candidates: h
            .candidates
            .iter()
            .map(|(src, m)| (src.clone(), filter_meta(m)))
            .collect(),
        conflicts: h
            .conflicts
            .iter()
            .filter(|(field, _)| is_objective(field))
            .cloned()
            .collect(),
        from: h.from.clone(),
        caller: h.caller.clone(),
    }
}

/// Rewrite a stored handoff JSON v1 into the objective-only payload (or return the original
/// text if it is not valid v1, so a bad row is not silently dropped).
pub fn json_for_database(handoff_json: &str) -> Result<String, String> {
    let h = Handoff::read_json(handoff_json.as_bytes())?;
    let o = for_database(&h);
    let mut buf = Vec::with_capacity(handoff_json.len());
    o.write_json(&mut buf)?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn meta(kv: &[(&str, &str)]) -> Meta {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn database_payload_drops_summaries_and_images_keeps_ids() {
        let h = Handoff {
            file: PathBuf::from("/media/The.Matrix.1999.mkv"),
            entry: meta(&[
                ("kind", "movie"),
                ("title", "The Matrix"),
                ("year", "1999"),
                ("imdb_id", "tt0133093"),
                ("description", "A long plot."),
                ("poster_url", "http://img/p.jpg"),
                ("lifecycle", "unverified"),
            ]),
            candidates: vec![(
                "tmdb".into(),
                meta(&[
                    ("title", "The Matrix"),
                    ("description", "TMDB plot"),
                    ("poster_url", "http://t/p.jpg"),
                    ("tmdb_id", "603"),
                ]),
            )],
            conflicts: vec![
                (
                    "year".into(),
                    vec![
                        ("tmdb".into(), "1999".into()),
                        ("embedded".into(), "1998".into()),
                    ],
                ),
                (
                    "description".into(),
                    vec![("tmdb".into(), "A".into()), ("omdb".into(), "B".into())],
                ),
            ],
            from: "jack".into(),
            caller: None,
        };
        let o = for_database(&h);
        assert_eq!(o.entry.get("title").map(String::as_str), Some("The Matrix"));
        assert_eq!(
            o.entry.get("imdb_id").map(String::as_str),
            Some("tt0133093")
        );
        assert!(!o.entry.contains_key("description"));
        assert!(!o.entry.contains_key("poster_url"));
        assert_eq!(
            o.candidates[0].1.get("tmdb_id").map(String::as_str),
            Some("603")
        );
        assert!(!o.candidates[0].1.contains_key("description"));
        assert_eq!(o.conflicts.len(), 1);
        assert_eq!(o.conflicts[0].0, "year");
        // the original is untouched
        assert_eq!(
            h.entry.get("description").map(String::as_str),
            Some("A long plot.")
        );
        let json = json_for_database(&{
            let mut b = Vec::new();
            h.write_json(&mut b).unwrap();
            String::from_utf8(b).unwrap()
        })
        .unwrap();
        assert!(
            !json.contains("A long plot.") && json.contains("tt0133093"),
            "{json}"
        );
    }
}
