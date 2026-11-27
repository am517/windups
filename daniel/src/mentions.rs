//! Storing mentions in the people catalog, only from an extractor whose quality is final, and
//! finding the known entities an extractor should be told about.
use crate::catalog::Catalog;
use crate::config::MentionsCfg;
use rusqlite::{Connection, params};
use std::collections::BTreeSet;
use windups::mentions::{ChunkInput, ExtractorInfo, FoundMention, KnownEntity, Quality, Target};

#[derive(Debug, PartialEq, Eq)]
pub struct Stored {
    pub written: usize,
    pub already: usize,
}

/// Distinct lowercase words of `text`, at most `max`.
fn words(text: &str, max: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .filter(|w| seen.insert(w.clone()))
        .take(max)
        .collect()
}

/// Entities whose canonical name or an alias appears in `text` (word index first, then a whole-name
/// check), for the extractor's `ChunkInput::known`. Reading only: nothing is written.
pub fn known_entities(
    people: &Connection,
    text: &str,
    cfg: &MentionsCfg,
) -> Result<Vec<KnownEntity>, String> {
    let ws = words(text, cfg.max_query_words);
    if ws.is_empty() {
        return Ok(Vec::new());
    }
    let query = ws
        .iter()
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join(" OR ");
    let err = |e: rusqlite::Error| e.to_string();
    let mut ids = people
        .prepare("SELECT DISTINCT entity_id FROM people_names_fts WHERE people_names_fts MATCH ?1")
        .map_err(err)?;
    let ids: Vec<i64> = ids
        .query_map([query], |r| r.get(0))
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    let lower = text.to_lowercase();
    let mut known = Vec::new();
    for id in ids {
        let (kind, name): (String, String) = people
            .query_row(
                "SELECT kind, name FROM people_entities WHERE id = ?1 AND status = 'chosen'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(err)?;
        let mut st = people
            .prepare("SELECT name FROM people_aliases WHERE entity_id = ?1 ORDER BY id")
            .map_err(err)?;
        let aliases: Vec<String> = st
            .query_map([id], |r| r.get(0))
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)?;
        let appears = std::iter::once(&name)
            .chain(&aliases)
            .any(|n| lower.contains(&n.to_lowercase()));
        if appears {
            known.push(KnownEntity {
                id,
                kind,
                name,
                aliases,
            });
        }
        if known.len() >= cfg.known_limit {
            break;
        }
    }
    Ok(known)
}

/// Stores what a final extractor found, all or nothing. A placeholder is refused (it may only
/// preview). Low confidence and proposed entities are stored with `needs_review`.
pub fn store(
    people: &mut Connection,
    extractor: &ExtractorInfo,
    input: &ChunkInput,
    found: &[FoundMention],
    cfg: &MentionsCfg,
) -> Result<Stored, String> {
    if extractor.quality != Quality::Final {
        return Err(format!(
            "mentions_placeholder: {} {} is a placeholder; it may preview, but only a final extractor writes mentions",
            extractor.id, extractor.version
        ));
    }
    if Catalog::parse(&input.catalog).is_none() {
        return Err(format!("unknown_catalog: {}", input.catalog));
    }
    if let Some((m, why)) = found
        .iter()
        .find_map(|m| windups::mentions::invalid(input, m).map(|w| (m, w)))
    {
        return Err(format!("mention_invalid: {why} at {}..{}", m.start, m.end));
    }
    let tx = people.transaction().map_err(|e| e.to_string())?;
    let mut stored = Stored {
        written: 0,
        already: 0,
    };
    for m in found {
        let (entity, proposed, kind) = match &m.target {
            Target::Entity(id) => (Some(*id), None, None),
            Target::Proposed { name, kind } => (None, Some(name.as_str()), Some(kind.as_str())),
        };
        let review = m.needs_review || proposed.is_some() || m.confidence < cfg.review_below;
        let n = tx
            .execute(
                "INSERT OR IGNORE INTO people_mentions(catalog, record_id, chunk_id, locator, surface, start_offset, end_offset, entity_id, proposed_name, proposed_kind, confidence, needs_review, extractor, extractor_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    input.catalog,
                    input.record_id,
                    input.chunk_id,
                    input.locator,
                    m.surface,
                    m.start as i64,
                    m.end as i64,
                    entity,
                    proposed,
                    kind,
                    m.confidence,
                    review,
                    extractor.id,
                    extractor.version
                ],
            )
            .map_err(|e| format!("mention_not_stored: {e}"))?;
        if n == 1 {
            stored.written += 1;
        } else {
            stored.already += 1;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn cfg() -> MentionsCfg {
        MentionsCfg {
            review_below: 0.8,
            known_limit: 50,
            max_query_words: 200,
        }
    }

    fn people(d: &std::path::Path) -> Connection {
        let c = catalog::open(Catalog::People, &d.join("people.db")).unwrap();
        c.execute_batch(
            "INSERT INTO people_entities(kind, name) VALUES ('person', 'Ada Lovelace'), ('place', 'Kyoto'), ('person', 'Ada Other');
             INSERT INTO people_aliases(entity_id, name) VALUES (1, 'Countess of Lovelace');",
        )
        .unwrap();
        c
    }

    fn input(text: &str) -> ChunkInput {
        ChunkInput {
            catalog: "media".into(),
            record_id: 7,
            chunk_id: Some(3),
            locator: Some("p. 12".into()),
            language: Some("en".into()),
            text: text.into(),
            known: vec![],
        }
    }

    fn info(quality: Quality) -> ExtractorInfo {
        ExtractorInfo {
            id: "fake".into(),
            version: "1".into(),
            quality,
        }
    }

    fn at(text: &str, surface: &str, target: Target, confidence: f64) -> FoundMention {
        let start = text.find(surface).unwrap();
        FoundMention {
            start,
            end: start + surface.len(),
            surface: surface.into(),
            target,
            confidence,
            needs_review: false,
        }
    }

    #[test]
    fn known_entities_are_those_whose_whole_name_or_alias_appears() {
        let d = tempfile::tempdir().unwrap();
        let c = people(d.path());
        let k = known_entities(&c, "The Countess of Lovelace wrote from Kyoto.", &cfg()).unwrap();
        let names: Vec<&str> = k.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            ["Ada Lovelace", "Kyoto"],
            "Ada Other shares no whole name"
        );
        assert!(known_entities(&c, "", &cfg()).unwrap().is_empty());
    }

    #[test]
    fn a_placeholder_extractor_never_writes_mentions() {
        let d = tempfile::tempdir().unwrap();
        let mut c = people(d.path());
        let text = "Kyoto in spring";
        let found = [at(text, "Kyoto", Target::Entity(2), 0.99)];
        let e = store(
            &mut c,
            &info(Quality::Placeholder),
            &input(text),
            &found,
            &cfg(),
        )
        .unwrap_err();
        assert!(e.starts_with("mentions_placeholder"), "{e}");
        let n: i64 = c
            .query_row("SELECT count(*) FROM people_mentions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn a_final_extractor_stores_once_and_unsure_or_new_entities_need_review() {
        let d = tempfile::tempdir().unwrap();
        let mut c = people(d.path());
        let text = "Ada Lovelace met Babbage in Kyoto";
        let found = [
            at(text, "Ada Lovelace", Target::Entity(1), 0.95),
            at(text, "Kyoto", Target::Entity(2), 0.5),
            at(
                text,
                "Babbage",
                Target::Proposed {
                    name: "Charles Babbage".into(),
                    kind: "person".into(),
                },
                0.9,
            ),
        ];
        let s = store(&mut c, &info(Quality::Final), &input(text), &found, &cfg()).unwrap();
        assert_eq!(
            s,
            Stored {
                written: 3,
                already: 0
            }
        );
        let again = store(&mut c, &info(Quality::Final), &input(text), &found, &cfg()).unwrap();
        assert_eq!(
            again,
            Stored {
                written: 0,
                already: 3
            }
        );
        let review: Vec<(String, bool)> = c
            .prepare("SELECT surface, needs_review FROM people_mentions ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            review,
            [
                ("Ada Lovelace".to_string(), false),
                ("Kyoto".to_string(), true),
                ("Babbage".to_string(), true)
            ]
        );
        let kind: String = c
            .query_row(
                "SELECT proposed_kind FROM people_mentions WHERE proposed_name IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kind, "person");
        let count: i64 = c
            .query_row("SELECT count(*) FROM people_entities", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3, "a proposal creates no entity");
    }

    #[test]
    fn one_bad_mention_stores_nothing() {
        let d = tempfile::tempdir().unwrap();
        let mut c = people(d.path());
        let text = "Kyoto";
        let mut bad = at(text, "Kyoto", Target::Entity(2), 0.9);
        bad.surface = "Tokyo".into();
        let found = [at(text, "Kyoto", Target::Entity(2), 0.9), bad];
        let e = store(&mut c, &info(Quality::Final), &input(text), &found, &cfg()).unwrap_err();
        assert!(
            e.starts_with("mention_invalid: surface_differs_from_text"),
            "{e}"
        );
        let n: i64 = c
            .query_row("SELECT count(*) FROM people_mentions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
