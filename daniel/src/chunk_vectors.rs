//! Chunk vectors in a catalog's text layer: what Tom still has to embed, storing his vectors (with
//! model id and version, only for the chunk text he was given), and searching one model at a time.
use crate::catalog::Catalog;
use rusqlite::{Connection, OptionalExtension, params};
use windups::vectors::{Model, cosine, from_blob, to_blob};

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

fn text_layer(catalog: Catalog) -> Result<&'static str, String> {
    if catalog.has_text_layer() {
        Ok(catalog.name())
    } else {
        Err(format!(
            "no_text_layer: the {} catalog holds no text",
            catalog.name()
        ))
    }
}

/// Chunks with no vector from this model and version, or whose text changed since: (id, text,
/// content hash), oldest first, at most `limit`.
pub fn not_embedded(
    c: &Connection,
    catalog: Catalog,
    model: Model,
    limit: usize,
) -> Result<Vec<(i64, String, String)>, String> {
    let n = text_layer(catalog)?;
    let mut st = c
        .prepare(&format!(
            "SELECT ch.id, ch.text, ch.content_hash FROM {n}_chunks ch
             LEFT JOIN {n}_vectors v ON v.chunk_id = ch.id AND v.model = ?1 AND v.model_version = ?2
             WHERE v.id IS NULL OR v.content_hash <> ch.content_hash
             ORDER BY ch.id LIMIT ?3"
        ))
        .map_err(err)?;
    st.query_map(params![model.id, model.version, limit as i64], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })
    .map_err(err)?
    .collect::<Result<_, _>>()
    .map_err(err)
}

/// Stores one vector. `content_hash` is the hash of the text that was embedded; if the chunk has
/// changed since, the vector is refused (`chunk_changed`) rather than stored against new text.
pub fn store(
    c: &Connection,
    catalog: Catalog,
    chunk_id: i64,
    model: Model,
    vector: &[f32],
    content_hash: &str,
) -> Result<(), String> {
    let n = text_layer(catalog)?;
    if model.id.trim().is_empty() || model.version.trim().is_empty() {
        return Err("model_unnamed: a vector needs its model id and version".into());
    }
    let current: Option<String> = c
        .query_row(
            &format!("SELECT content_hash FROM {n}_chunks WHERE id = ?1"),
            [chunk_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    match current {
        None => return Err(format!("chunk_not_found: chunk {chunk_id}")),
        Some(h) if h != content_hash => {
            return Err(format!(
                "chunk_changed: chunk {chunk_id} has new text; embed it again"
            ));
        }
        Some(_) => {}
    }
    c.execute(
        &format!(
            "INSERT INTO {n}_vectors(chunk_id, model, model_version, dim, vector, content_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(chunk_id, model, model_version) DO UPDATE SET dim = excluded.dim, vector = excluded.vector, content_hash = excluded.content_hash, created_at = CURRENT_TIMESTAMP"
        ),
        params![chunk_id, model.id, model.version, vector.len() as i64, to_blob(vector), content_hash],
    )
    .map(|_| ())
    .map_err(err)
}

/// Top `k` (chunk id, score) among this model's and version's vectors of the same size.
pub fn search(
    c: &Connection,
    catalog: Catalog,
    model: Model,
    query: &[f32],
    k: usize,
) -> Result<Vec<(i64, f32)>, String> {
    let n = text_layer(catalog)?;
    let mut st = c
        .prepare(&format!(
            "SELECT chunk_id, vector FROM {n}_vectors WHERE model = ?1 AND model_version = ?2 AND dim = ?3"
        ))
        .map_err(err)?;
    let mut hits: Vec<(i64, f32)> = st
        .query_map(params![model.id, model.version, query.len() as i64], |r| {
            Ok((
                r.get(0)?,
                cosine(query, &from_blob(&r.get::<_, Vec<u8>>(1)?)),
            ))
        })
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    hits.sort_by(|a, b| b.1.total_cmp(&a.1));
    hits.truncate(k);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use crate::config::RecordsCfg;
    use crate::text::{Chunk, Extract, add_chunk, add_extract};

    const BGE: Model = Model {
        id: "BAAI/bge-m3",
        version: "test-1",
    };

    fn media_with_chunks(d: &std::path::Path, texts: &[&str]) -> Connection {
        let c = catalog::open(Catalog::Media, &d.join("media.db")).unwrap();
        let off = RecordsCfg::default();
        let e = add_extract(
            &c,
            Catalog::Media,
            &off,
            &Extract {
                record_id: 1,
                file_hash: None,
                method: "subtitle",
                language: None,
                quality: "ok",
                made_by: "test 1",
            },
        )
        .unwrap();
        for t in texts {
            add_chunk(
                &c,
                Catalog::Media,
                &off,
                &Chunk {
                    extract_id: e,
                    parent_id: None,
                    level: 3,
                    locator: None,
                    language: None,
                    text: t,
                },
            )
            .unwrap();
        }
        c
    }

    #[test]
    fn tom_gets_what_is_not_embedded_for_his_model_and_version() {
        let d = tempfile::tempdir().unwrap();
        let c = media_with_chunks(d.path(), &["one", "two"]);
        let todo = not_embedded(&c, Catalog::Media, BGE, 10).unwrap();
        assert_eq!(todo.len(), 2);
        store(&c, Catalog::Media, todo[0].0, BGE, &[1.0, 0.0], &todo[0].2).unwrap();
        assert_eq!(not_embedded(&c, Catalog::Media, BGE, 10).unwrap().len(), 1);
        let newer = Model {
            version: "test-2",
            ..BGE
        };
        assert_eq!(
            not_embedded(&c, Catalog::Media, newer, 10).unwrap().len(),
            2,
            "a new version re-embeds"
        );
        c.execute(
            "UPDATE media_chunks SET text = 'one, edited', content_hash = 'b3:new' WHERE id = ?1",
            [todo[0].0],
        )
        .unwrap();
        assert_eq!(
            not_embedded(&c, Catalog::Media, BGE, 10).unwrap().len(),
            2,
            "changed text re-embeds"
        );
    }

    #[test]
    fn a_vector_for_old_text_or_without_a_model_version_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let c = media_with_chunks(d.path(), &["one"]);
        let e = store(&c, Catalog::Media, 1, BGE, &[1.0], "b3:stale").unwrap_err();
        assert!(e.starts_with("chunk_changed"), "{e}");
        let hash = larry::hash::text_hash("one");
        let unnamed = Model {
            id: "m",
            version: "",
        };
        assert!(
            store(&c, Catalog::Media, 1, unnamed, &[1.0], &hash)
                .unwrap_err()
                .starts_with("model_unnamed")
        );
        assert!(
            store(&c, Catalog::Media, 9, BGE, &[1.0], &hash)
                .unwrap_err()
                .starts_with("chunk_not_found")
        );
        assert!(
            store(&c, Catalog::Maps, 1, BGE, &[1.0], &hash)
                .unwrap_err()
                .starts_with("no_text_layer")
        );
    }

    #[test]
    fn search_ranks_by_meaning_within_one_model_version() {
        let d = tempfile::tempdir().unwrap();
        let c = media_with_chunks(d.path(), &["one", "two"]);
        let h = |t: &str| larry::hash::text_hash(t);
        store(&c, Catalog::Media, 1, BGE, &[1.0, 0.0], &h("one")).unwrap();
        store(&c, Catalog::Media, 2, BGE, &[0.0, 1.0], &h("two")).unwrap();
        let other = Model {
            id: "other",
            version: "1",
        };
        store(&c, Catalog::Media, 2, other, &[1.0, 0.0], &h("two")).unwrap();
        let hits = search(&c, Catalog::Media, BGE, &[0.9, 0.1], 5).unwrap();
        assert_eq!(hits.iter().map(|x| x.0).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(
            search(&c, Catalog::Media, other, &[1.0, 0.0], 5)
                .unwrap()
                .len(),
            1
        );
    }
}
