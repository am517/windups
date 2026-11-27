//! "Everything connected to X": resolve a name to entities (never guessing between several), walk
//! relationships a few hops with a recursive CTE, and gather mentions in the allowed catalogs.
//! Reading only.
use crate::catalog::Catalog;
use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Entity {
    pub id: i64,
    pub kind: String,
    pub name: String,
}

/// An entity reached from the start: how many hops away and by which relationship.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Linked {
    pub entity: Entity,
    pub hops: i64,
    pub rel: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Mention {
    pub catalog: String,
    pub record_id: i64,
    pub chunk_id: Option<i64>,
    pub locator: Option<String>,
    pub surface: String,
    pub entity_id: i64,
    pub needs_review: bool,
}

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

/// Chosen entities whose canonical name or an alias is `name` (any case). Several = the caller
/// shows them and lets the person pick.
pub fn resolve(people: &Connection, name: &str) -> Result<Vec<Entity>, String> {
    let mut st = people
        .prepare(
            "SELECT DISTINCT e.id, e.kind, e.name FROM people_entities e
             LEFT JOIN people_aliases a ON a.entity_id = e.id
             WHERE e.status = 'chosen' AND (e.name = ?1 COLLATE NOCASE OR a.name = ?1 COLLATE NOCASE)
             ORDER BY e.id",
        )
        .map_err(err)?;
    st.query_map([name], |r| {
        Ok(Entity {
            id: r.get(0)?,
            kind: r.get(1)?,
            name: r.get(2)?,
        })
    })
    .map_err(err)?
    .collect::<Result<_, _>>()
    .map_err(err)
}

/// One entity by id, whatever its status (a person asked for it by number).
pub fn entity(people: &Connection, id: i64) -> Result<Option<Entity>, String> {
    use rusqlite::OptionalExtension;
    people
        .query_row(
            "SELECT id, kind, name FROM people_entities WHERE id = ?1",
            [id],
            |r| {
                Ok(Entity {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    name: r.get(2)?,
                })
            },
        )
        .optional()
        .map_err(err)
}

/// Entities within `depth` hops of `start` (relationships either way), nearest first, at most
/// `limit`. The start itself is not included.
pub fn neighbors(
    people: &Connection,
    start: i64,
    depth: u32,
    limit: usize,
) -> Result<Vec<Linked>, String> {
    let mut st = people
        .prepare(
            "WITH RECURSIVE walk(id, hops, rel) AS (
               SELECT ?1, 0, ''
               UNION
               SELECT CASE WHEN r.from_id = w.id THEN r.to_id ELSE r.from_id END, w.hops + 1, r.rel
               FROM walk w JOIN people_relationships r ON r.from_id = w.id OR r.to_id = w.id
               WHERE w.hops < ?2
             )
             SELECT e.id, e.kind, e.name, min(w.hops) AS hops, w.rel
             FROM walk w JOIN people_entities e ON e.id = w.id
             WHERE w.id <> ?1 AND e.status = 'chosen'
             GROUP BY e.id ORDER BY hops, e.name LIMIT ?3",
        )
        .map_err(err)?;
    st.query_map(params![start, depth, limit as i64], |r| {
        Ok(Linked {
            entity: Entity {
                id: r.get(0)?,
                kind: r.get(1)?,
                name: r.get(2)?,
            },
            hops: r.get(3)?,
            rel: r.get(4)?,
        })
    })
    .map_err(err)?
    .collect::<Result<_, _>>()
    .map_err(err)
}

/// Mentions of any of `ids` in the allowed catalogs, sure ones first, at most `limit`.
pub fn mentions_of(
    people: &Connection,
    ids: &[i64],
    catalogs: &[Catalog],
    limit: usize,
) -> Result<Vec<Mention>, String> {
    if ids.is_empty() || catalogs.is_empty() {
        return Ok(Vec::new());
    }
    let list = |n: usize, from: usize| {
        (from..from + n)
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let sql = format!(
        "SELECT catalog, record_id, chunk_id, locator, surface, entity_id, needs_review FROM people_mentions
         WHERE entity_id IN ({}) AND catalog IN ({}) ORDER BY needs_review, confidence DESC, id LIMIT {}",
        list(ids.len(), 1),
        list(catalogs.len(), ids.len() + 1),
        limit
    );
    let mut values: Vec<rusqlite::types::Value> = ids.iter().map(|i| (*i).into()).collect();
    values.extend(catalogs.iter().map(|c| c.name().to_string().into()));
    let mut st = people.prepare(&sql).map_err(err)?;
    st.query_map(rusqlite::params_from_iter(values), |r| {
        Ok(Mention {
            catalog: r.get(0)?,
            record_id: r.get(1)?,
            chunk_id: r.get(2)?,
            locator: r.get(3)?,
            surface: r.get(4)?,
            entity_id: r.get(5)?,
            needs_review: r.get(6)?,
        })
    })
    .map_err(err)?
    .collect::<Result<_, _>>()
    .map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn people(d: &std::path::Path) -> Connection {
        let c = catalog::open(Catalog::People, &d.join("people.db")).unwrap();
        c.execute_batch(
            "INSERT INTO people_entities(kind, name) VALUES
               ('person', 'Ada'), ('place', 'London'), ('place', 'England'), ('person', 'Charles'), ('person', 'Ada');
             INSERT INTO people_aliases(entity_id, name) VALUES (1, 'Countess');
             INSERT INTO people_relationships(from_id, to_id, rel, source) VALUES
               (1, 2, 'lived_in', 'test'), (2, 3, 'part_of', 'test'), (4, 1, 'worked_with', 'test');
             INSERT INTO people_mentions(catalog, record_id, chunk_id, locator, surface, entity_id, confidence, needs_review, extractor, extractor_version) VALUES
               ('media', 10, 1, 'p. 4', 'Ada', 1, 0.95, 0, 'x', '1'),
               ('photos', 3, NULL, 'left face', 'Ada', 1, 0.6, 1, 'x', '1'),
               ('records', 8, 2, NULL, 'Ada', 1, 0.99, 0, 'x', '1');",
        )
        .unwrap();
        c
    }

    #[test]
    fn a_name_shared_by_two_entities_returns_both_and_an_alias_resolves() {
        let d = tempfile::tempdir().unwrap();
        let c = people(d.path());
        assert_eq!(
            resolve(&c, "ada").unwrap().len(),
            2,
            "never a guess between them"
        );
        assert_eq!(resolve(&c, "countess").unwrap()[0].id, 1);
        assert!(resolve(&c, "nobody").unwrap().is_empty());
    }

    #[test]
    fn neighbors_are_walked_both_ways_and_stop_at_the_depth() {
        let d = tempfile::tempdir().unwrap();
        let c = people(d.path());
        let one: Vec<String> = neighbors(&c, 1, 1, 10)
            .unwrap()
            .into_iter()
            .map(|l| l.entity.name)
            .collect();
        assert_eq!(one, ["Charles", "London"]);
        let two = neighbors(&c, 1, 2, 10).unwrap();
        let england = two.iter().find(|l| l.entity.name == "England").unwrap();
        assert_eq!((england.hops, england.rel.as_str()), (2, "part_of"));
        assert_eq!(neighbors(&c, 1, 2, 1).unwrap().len(), 1, "the limit holds");
    }

    #[test]
    fn mentions_come_only_from_allowed_catalogs_sure_ones_first() {
        let d = tempfile::tempdir().unwrap();
        let c = people(d.path());
        let m = mentions_of(&c, &[1], &[Catalog::Media, Catalog::Photos], 10).unwrap();
        let got: Vec<(&str, bool)> = m
            .iter()
            .map(|x| (x.catalog.as_str(), x.needs_review))
            .collect();
        assert_eq!(
            got,
            [("media", false), ("photos", true)],
            "records not asked for"
        );
        assert!(
            mentions_of(&c, &[], &[Catalog::Media], 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(entity(&c, 4).unwrap().unwrap().name, "Charles");
    }
}
