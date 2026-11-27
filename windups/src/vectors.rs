//! Embeddings for meaning search. The model is swappable (anything implementing `Embedder`);
//! vectors are stored as f32 BLOBs with their model id and version, never mixed in a search, and
//! searched by brute-force cosine for now
//! (sqlite-vec can index the same table later without a schema change).
use rusqlite::{Connection, params};

/// A small local model (e.g. fastembed-rs, later). Not bundled: no heavy deps in the shared crate.
pub trait Embedder {
    fn model(&self) -> Model<'_>;
    fn embed(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// Which model made a vector: its id and its version. Both are stored with every vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model<'a> {
    pub id: &'a str,
    pub version: &'a str,
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

// chunks_exact, not as_chunks (Rust 1.88+), so older distro toolchains still build
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
fn from_blob(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn store(
    c: &Connection,
    owner: &str,
    owner_id: &str,
    model: Model,
    v: &[f32],
    content_hash: Option<&str>,
) -> rusqlite::Result<()> {
    // one vector per owner and model id: a new version replaces the old one
    c.execute(
        "INSERT INTO embeddings(owner, owner_id, model, model_version, dim, vector, content_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(owner, owner_id, model) DO UPDATE SET model_version = excluded.model_version, dim = excluded.dim, vector = excluded.vector, content_hash = excluded.content_hash, created_at = CURRENT_TIMESTAMP",
        params![owner, owner_id, model.id, model.version, v.len() as i64, to_blob(v), content_hash],
    )?;
    Ok(())
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut d, mut na, mut nb) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(b) {
        d += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        d / (na.sqrt() * nb.sqrt())
    }
}

/// Top `k` (owner, owner_id, score) for `query`, among vectors of the same model, version and size.
pub fn search(
    c: &Connection,
    model: Model,
    query: &[f32],
    k: usize,
) -> rusqlite::Result<Vec<(String, String, f32)>> {
    let mut st = c.prepare(
        "SELECT owner, owner_id, vector FROM embeddings WHERE model = ?1 AND model_version = ?2 AND dim = ?3",
    )?;
    let mut hits: Vec<(String, String, f32)> = st
        .query_map(params![model.id, model.version, query.len() as i64], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                cosine(query, &from_blob(&r.get::<_, Vec<u8>>(2)?)),
            ))
        })?
        .collect::<Result<_, _>>()?;
    hits.sort_by(|a, b| b.2.total_cmp(&a.2));
    hits.truncate(k);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn store_and_search_by_model() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        let m1 = Model {
            id: "m1",
            version: "1",
        };
        let m2 = Model {
            id: "m2",
            version: "1",
        };
        store(&c, "text", "1", m1, &[1.0, 0.0], None).unwrap();
        store(&c, "text", "2", m1, &[0.0, 1.0], None).unwrap();
        store(&c, "text", "2", m1, &[0.6, 0.8], Some("h")).unwrap();
        store(&c, "text", "3", m2, &[1.0, 0.0, 0.0], None).unwrap();
        let r = search(&c, m1, &[1.0, 0.1], 5).unwrap();
        assert_eq!(
            r.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(),
            ["1", "2"]
        );
        let stored: String = c
            .query_row(
                "SELECT model_version FROM embeddings WHERE owner_id = '1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, "1", "the version is stored with the vector");
        assert_eq!(from_blob(&to_blob(&[1.5, -2.0])), [1.5, -2.0]);
    }

    #[test]
    fn a_search_never_mixes_model_versions() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        let v1 = Model {
            id: "m",
            version: "1",
        };
        let v2 = Model {
            id: "m",
            version: "2",
        };
        store(&c, "text", "1", v1, &[1.0, 0.0], None).unwrap();
        assert_eq!(search(&c, v2, &[1.0, 0.0], 5).unwrap().len(), 0);
        store(&c, "text", "1", v2, &[1.0, 0.0], None).unwrap();
        assert_eq!(search(&c, v1, &[1.0, 0.0], 5).unwrap().len(), 0, "replaced");
        assert_eq!(search(&c, v2, &[1.0, 0.0], 5).unwrap().len(), 1);
    }
}
