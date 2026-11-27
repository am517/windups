//! Writing the people catalog: entities, their other names, ids and relationships. Never a silent
//! merge: an id or name that points at a second entity opens a proposal instead.
use rusqlite::{Connection, OptionalExtension, params};

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

const KINDS: [&str; 7] = [
    "person",
    "place",
    "thing",
    "idea",
    "topic",
    "organization",
    "work",
];

/// Adds a chosen entity and returns its id. The same name twice makes two entities on purpose
/// (two people can share a name); `possible_same` proposals are how they may later be joined.
pub fn add_entity(
    c: &Connection,
    kind: &str,
    name: &str,
    description: Option<&str>,
    source: &str,
) -> Result<i64, String> {
    if !KINDS.contains(&kind) {
        return Err(format!("unknown_kind: {kind} (use {})", KINDS.join(", ")));
    }
    if name.trim().is_empty() {
        return Err("name_empty: an entity needs a name".into());
    }
    c.execute(
        "INSERT INTO people_entities(kind, name, description, source) VALUES (?1, ?2, ?3, ?4)",
        params![kind, name.trim(), description, source],
    )
    .map_err(err)?;
    Ok(c.last_insert_rowid())
}

/// Another name for an entity. Adding it again is a no-op.
pub fn add_alias(
    c: &Connection,
    entity_id: i64,
    name: &str,
    language: &str,
    source: &str,
) -> Result<(), String> {
    c.execute(
        "INSERT OR IGNORE INTO people_aliases(entity_id, name, language, source) VALUES (?1, ?2, ?3, ?4)",
        params![entity_id, name.trim(), language, source],
    )
    .map(|_| ())
    .map_err(err)
}

/// Opens (or keeps) a `possible_same` proposal between two entities, lower id first.
pub fn propose_same(
    c: &Connection,
    a: i64,
    b: i64,
    score: Option<f64>,
    source: &str,
) -> Result<(), String> {
    let (lo, hi) = (a.min(b), a.max(b));
    c.execute(
        "INSERT OR IGNORE INTO people_proposals(kind, entity_id, other_id, score, source) VALUES ('possible_same', ?1, ?2, ?3, ?4)",
        params![lo, hi, score, source],
    )
    .map(|_| ())
    .map_err(err)
}

#[derive(Debug, PartialEq, Eq)]
pub enum IdOutcome {
    Added,
    AlreadyThere,
    /// The id already belongs to another entity: a proposal was opened, nothing merged.
    Proposed {
        other: i64,
    },
}

/// A known id in another system (never a guessed one). Strong evidence, never a merge.
pub fn add_external_id(
    c: &Connection,
    entity_id: i64,
    source: &str,
    scope: &str,
    value: &str,
) -> Result<IdOutcome, String> {
    let owner: Option<i64> = c
        .query_row(
            "SELECT entity_id FROM people_external_ids WHERE source = ?1 AND scope = ?2 AND value = ?3",
            params![source, scope, value],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    match owner {
        Some(o) if o == entity_id => Ok(IdOutcome::AlreadyThere),
        Some(o) => {
            propose_same(c, entity_id, o, Some(1.0), &format!("{source} id"))?;
            Ok(IdOutcome::Proposed { other: o })
        }
        None => {
            c.execute(
                "INSERT INTO people_external_ids(entity_id, source, scope, value) VALUES (?1, ?2, ?3, ?4)",
                params![entity_id, source, scope, value],
            )
            .map_err(err)?;
            Ok(IdOutcome::Added)
        }
    }
}

/// entity -[rel]-> entity, from a named source, with how sure that source is (0..=1, if known).
pub fn relate(
    c: &Connection,
    from: i64,
    to: i64,
    rel: &str,
    source: &str,
    confidence: Option<f64>,
) -> Result<(), String> {
    if from == to {
        return Err("self_relationship: an entity can't relate to itself".into());
    }
    if confidence.is_some_and(|x| !(0.0..=1.0).contains(&x)) {
        return Err("confidence_out_of_range".into());
    }
    c.execute(
        "INSERT INTO people_relationships(from_id, to_id, rel, source, confidence) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(from_id, to_id, rel, source) DO UPDATE SET confidence = excluded.confidence",
        params![from, to, rel, source, confidence],
    )
    .map(|_| ())
    .map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{self, Catalog};

    fn people(d: &std::path::Path) -> Connection {
        catalog::open(Catalog::People, &d.join("people.db")).unwrap()
    }

    fn count(c: &Connection, sql: &str) -> i64 {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn a_shared_name_makes_two_entities_and_bad_input_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let c = people(d.path());
        let a = add_entity(&c, "person", "John Smith", None, "test").unwrap();
        let b = add_entity(&c, "person", "John Smith", None, "test").unwrap();
        assert_ne!(a, b);
        assert!(
            add_entity(&c, "ghost", "x", None, "t")
                .unwrap_err()
                .starts_with("unknown_kind")
        );
        assert!(
            add_entity(&c, "person", " ", None, "t")
                .unwrap_err()
                .starts_with("name_empty")
        );
        add_alias(&c, a, "J. Smith", "", "test").unwrap();
        add_alias(&c, a, "J. Smith", "", "test").unwrap();
        assert_eq!(count(&c, "SELECT count(*) FROM people_aliases"), 1);
    }

    #[test]
    fn an_id_already_on_another_entity_opens_a_proposal_and_merges_nothing() {
        let d = tempfile::tempdir().unwrap();
        let c = people(d.path());
        let a = add_entity(&c, "person", "Ada Lovelace", None, "test").unwrap();
        let b = add_entity(&c, "person", "Augusta Ada King", None, "test").unwrap();
        assert_eq!(
            add_external_id(&c, a, "wikidata", "", "Q7259").unwrap(),
            IdOutcome::Added
        );
        assert_eq!(
            add_external_id(&c, a, "wikidata", "", "Q7259").unwrap(),
            IdOutcome::AlreadyThere
        );
        assert_eq!(
            add_external_id(&c, b, "wikidata", "", "Q7259").unwrap(),
            IdOutcome::Proposed { other: a }
        );
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM people_entities WHERE status = 'chosen'"
            ),
            2
        );
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM people_proposals WHERE status = 'open' AND entity_id = 1 AND other_id = 2"
            ),
            1
        );
    }

    #[test]
    fn relationships_are_kept_per_source_and_checked() {
        let d = tempfile::tempdir().unwrap();
        let c = people(d.path());
        let a = add_entity(&c, "person", "Ada", None, "t").unwrap();
        let l = add_entity(&c, "place", "London", None, "t").unwrap();
        relate(&c, a, l, "lived_in", "book", Some(0.9)).unwrap();
        relate(&c, a, l, "lived_in", "book", Some(1.0)).unwrap();
        relate(&c, a, l, "lived_in", "letter", None).unwrap();
        assert_eq!(count(&c, "SELECT count(*) FROM people_relationships"), 2);
        assert!(
            relate(&c, a, a, "is", "t", None)
                .unwrap_err()
                .starts_with("self_relationship")
        );
        assert!(relate(&c, a, l, "x", "t", Some(2.0)).is_err());
    }
}
