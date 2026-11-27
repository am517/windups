//! `daniel find`: a short candidate list of word matches across the purpose catalogs (FTS5).
//! Reading only: a catalog that doesn't exist yet is skipped, never created.
use crate::catalog::Catalog;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Hit {
    pub catalog: &'static str,
    /// chunk | text | name | place
    pub kind: &'static str,
    /// The row found: chunk, texts, entity or feature id.
    pub id: i64,
    /// The catalog record it belongs to (media.id, records_items.id, ...), when there is one.
    pub record_id: Option<i64>,
    pub locator: Option<String>,
    pub text: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Skipped {
    pub catalog: &'static str,
    pub reason: String,
}

/// Words of the query as an FTS5 query: every word must appear (quotes dropped, each word quoted).
pub fn fts_query(words: &str) -> Option<String> {
    let q: Vec<String> = words
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\""))
        .collect();
    (!q.is_empty()).then(|| q.join(" "))
}

/// Opens an existing catalog read-only; None when its file isn't there.
pub fn open_read_only(path: &Path) -> Result<Option<Connection>, String> {
    if !path.exists() {
        return Ok(None);
    }
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

type Row = (i64, Option<i64>, Option<String>, String);

fn rows(c: &Connection, sql: &str, q: &str, limit: usize) -> Result<Vec<Row>, String> {
    let mut st = c.prepare(sql).map_err(|e| e.to_string())?;
    st.query_map(rusqlite::params![q, limit as i64], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<_, _>>()
    .map_err(|e| e.to_string())
}

/// Best matches in one catalog, best first, at most `limit`.
pub fn in_catalog(
    c: &Connection,
    catalog: Catalog,
    q: &str,
    limit: usize,
) -> Result<Vec<Hit>, String> {
    let name = catalog.name();
    let chunks = format!(
        "SELECT ch.id, ex.record_id, ch.locator, snippet({name}_chunks_fts, 0, '[', ']', '…', 12)
         FROM {name}_chunks_fts JOIN {name}_chunks ch ON ch.id = {name}_chunks_fts.rowid
         JOIN {name}_extracts ex ON ex.id = ch.extract_id
         WHERE {name}_chunks_fts MATCH ?1 ORDER BY bm25({name}_chunks_fts) LIMIT ?2"
    );
    let hit = |kind: &'static str| {
        move |(id, record_id, locator, text): Row| Hit {
            catalog: name,
            kind,
            id,
            record_id,
            locator,
            text,
        }
    };
    let mut hits: Vec<Hit> = Vec::new();
    match catalog {
        Catalog::Media => {
            let titles = "SELECT t.id, t.media_id, t.field, snippet(texts_fts, 0, '[', ']', '…', 12)
                 FROM texts_fts JOIN texts t ON t.id = texts_fts.rowid
                 WHERE texts_fts MATCH ?1 AND t.media_id IS NOT NULL ORDER BY bm25(texts_fts) LIMIT ?2";
            hits.extend(rows(c, titles, q, limit)?.into_iter().map(hit("text")));
            hits.extend(rows(c, &chunks, q, limit)?.into_iter().map(hit("chunk")));
        }
        Catalog::Records | Catalog::Photos => {
            hits.extend(rows(c, &chunks, q, limit)?.into_iter().map(hit("chunk")));
        }
        Catalog::People => {
            let names = "SELECT e.id, NULL, e.kind, e.name || CASE WHEN f.name <> e.name THEN ' (as ' || f.name || ')' ELSE '' END
                 FROM people_names_fts f JOIN people_entities e ON e.id = f.entity_id
                 WHERE people_names_fts MATCH ?1 AND e.status = 'chosen' ORDER BY bm25(people_names_fts) LIMIT ?2";
            hits.extend(rows(c, names, q, limit)?.into_iter().map(hit("name")));
        }
        Catalog::Maps => {
            let places = "SELECT m.id, m.layer_id, CASE WHEN m.lat IS NULL THEN NULL ELSE printf('%.5f,%.5f', m.lat, m.lon) END,
                 m.label || COALESCE(' / ' || m.label_alt, '')
                 FROM maps_labels_fts JOIN maps_features m ON m.id = maps_labels_fts.rowid
                 WHERE maps_labels_fts MATCH ?1 ORDER BY bm25(maps_labels_fts) LIMIT ?2";
            hits.extend(rows(c, places, q, limit)?.into_iter().map(hit("place")));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    hits.retain(|h| seen.insert((h.kind, h.id)));
    hits.truncate(limit);
    Ok(hits)
}

/// Takes the best of each catalog in turn (ranks from different indexes aren't comparable), up to
/// `limit` in all.
pub fn merge(per_catalog: Vec<Vec<Hit>>, limit: usize) -> Vec<Hit> {
    let mut lists: Vec<std::vec::IntoIter<Hit>> =
        per_catalog.into_iter().map(Vec::into_iter).collect();
    let mut out = Vec::new();
    while out.len() < limit {
        let before = out.len();
        for l in lists.iter_mut() {
            if out.len() < limit
                && let Some(h) = l.next()
            {
                out.push(h);
            }
        }
        if out.len() == before {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use crate::config::RecordsCfg;
    use crate::text::{Chunk, Extract, add_chunk, add_extract};

    fn media(d: &Path) -> Connection {
        let c = catalog::open(Catalog::Media, &d.join("media.db")).unwrap();
        c.execute_batch(
            "INSERT INTO media(kind, title) VALUES ('book', 'A Walk in Kyoto');
             INSERT INTO texts(media_id, field, source, text, chosen) VALUES (1, 'title', 'test', 'A Walk in Kyoto', 1);",
        )
        .unwrap();
        let off = RecordsCfg::default();
        let e = add_extract(
            &c,
            Catalog::Media,
            &off,
            &Extract {
                record_id: 1,
                file_hash: None,
                method: "pdf_text",
                language: Some("en"),
                quality: "ok",
                made_by: "test 1",
            },
        )
        .unwrap();
        add_chunk(
            &c,
            Catalog::Media,
            &off,
            &Chunk {
                extract_id: e,
                parent_id: None,
                level: 3,
                locator: Some("p. 12"),
                language: Some("en"),
                text: "The temples of Kyoto in autumn are quiet.",
            },
        )
        .unwrap();
        c
    }

    #[test]
    fn query_words_must_all_appear_and_quotes_cannot_break_the_query() {
        assert_eq!(
            fts_query("kyoto  temples").unwrap(),
            "\"kyoto\" \"temples\""
        );
        assert_eq!(fts_query("a\"b OR").unwrap(), "\"ab\" \"OR\"");
        assert_eq!(fts_query("  "), None);
    }

    #[test]
    fn titles_and_passages_are_found_with_their_locators() {
        let d = tempfile::tempdir().unwrap();
        let c = media(d.path());
        let q = fts_query("kyoto").unwrap();
        let hits = in_catalog(&c, Catalog::Media, &q, 5).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            (hits[0].kind, hits[0].locator.as_deref()),
            ("text", Some("title"))
        );
        assert_eq!(
            (hits[1].kind, hits[1].locator.as_deref()),
            ("chunk", Some("p. 12"))
        );
        assert!(hits[1].text.contains("[Kyoto]"), "{}", hits[1].text);
        assert_eq!(hits[1].record_id, Some(1));
        let one = in_catalog(&c, Catalog::Media, &fts_query("temples autumn").unwrap(), 5).unwrap();
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn names_and_places_are_found_and_merged_in_turns_up_to_the_limit() {
        let d = tempfile::tempdir().unwrap();
        let people = catalog::open(Catalog::People, &d.path().join("people.db")).unwrap();
        people
            .execute_batch(
                "INSERT INTO people_entities(kind, name) VALUES ('place', 'Kyoto');
                 INSERT INTO people_aliases(entity_id, name, language) VALUES (1, 'Kyoto City', 'en');",
            )
            .unwrap();
        let maps = catalog::open(Catalog::Maps, &d.path().join("maps.db")).unwrap();
        maps.execute_batch(
            "INSERT INTO maps_layers(name) VALUES ('places');
             INSERT INTO maps_features(layer_id, lat, lon, label, label_alt) VALUES (1, 35.0116, 135.7681, 'Kyoto Station', '京都駅');",
        )
        .unwrap();
        let q = fts_query("kyoto").unwrap();
        let p = in_catalog(&people, Catalog::People, &q, 5).unwrap();
        assert_eq!(p.len(), 1, "one entity, though two of its names match");
        let m = in_catalog(&maps, Catalog::Maps, &q, 5).unwrap();
        assert_eq!(m[0].locator.as_deref(), Some("35.01160,135.76810"));
        assert_eq!(m[0].text, "Kyoto Station / 京都駅");
        let media_hits = in_catalog(&media(d.path()), Catalog::Media, &q, 5).unwrap();
        let merged = merge(vec![media_hits, p, m], 3);
        let order: Vec<&str> = merged.iter().map(|h| h.catalog).collect();
        assert_eq!(order, ["media", "people", "maps"]);
    }

    #[test]
    fn a_missing_catalog_is_not_created_by_a_search() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("photos.db");
        assert!(open_read_only(&p).unwrap().is_none());
        assert!(!p.exists());
    }
}
