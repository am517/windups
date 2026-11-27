//! Writing the personal photos and videos catalog: an item, and the file(s) it is stored in.
//! A file without Larry's hash yet is stored as hash pending (NULL), never with a made-up one.
use rusqlite::{Connection, OptionalExtension, params};

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

#[derive(Debug, Default, Clone)]
pub struct Item<'a> {
    /// photo | video
    pub kind: &'a str,
    pub taken_at: Option<&'a str>,
    /// (latitude, longitude), WGS84.
    pub point: Option<(f64, f64)>,
    /// maps_features.id of its place (another catalog).
    pub place_feature_id: Option<i64>,
    pub title: Option<&'a str>,
}

pub fn add_item(c: &Connection, it: &Item) -> Result<i64, String> {
    if !matches!(it.kind, "photo" | "video") {
        return Err(format!("unknown_kind: {} (use photo or video)", it.kind));
    }
    let (lat, lon) = it.point.unzip();
    c.execute(
        "INSERT INTO photos_items(kind, taken_at, lat, lon, place_feature_id, title) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![it.kind, it.taken_at, lat, lon, it.place_feature_id, it.title],
    )
    .map_err(|e| format!("item_not_stored: {e}"))?;
    Ok(c.last_insert_rowid())
}

/// Stores (or updates, by path) a file of an item. `hash` None = hash pending. When a new path has
/// a hash already known under another path that no longer exists, that row moves to the new path
/// (the file was moved); if the old path is still there, it is a copy and gets its own row.
pub fn add_file(
    c: &Connection,
    item_id: i64,
    path: &str,
    size: Option<i64>,
    mtime: Option<&str>,
    hash: Option<&str>,
) -> Result<i64, String> {
    let by_path: Option<i64> = c
        .query_row("SELECT id FROM photos_files WHERE path = ?1", [path], |r| {
            r.get(0)
        })
        .optional()
        .map_err(err)?;
    let moved: Option<i64> = match (by_path, hash) {
        (None, Some(h)) => c
            .query_row(
                "SELECT id, path FROM photos_files WHERE hash = ?1 AND item_id = ?2 ORDER BY id LIMIT 1",
                params![h, item_id],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(err)?
            .filter(|(_, old)| !std::path::Path::new(old).exists())
            .map(|(id, _)| id),
        _ => None,
    };
    match by_path.or(moved) {
        Some(id) => {
            c.execute(
                "UPDATE photos_files SET item_id = ?1, path = ?2, size = ?3, mtime = ?4, hash = COALESCE(?5, hash) WHERE id = ?6",
                params![item_id, path, size, mtime, hash, id],
            )
            .map_err(err)?;
            Ok(id)
        }
        None => {
            c.execute(
                "INSERT INTO photos_files(item_id, path, size, mtime, hash) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![item_id, path, size, mtime, hash],
            )
            .map_err(err)?;
            Ok(c.last_insert_rowid())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{self, Catalog};

    #[test]
    fn a_file_waits_for_its_hash_and_a_moved_file_keeps_its_row() {
        let d = tempfile::tempdir().unwrap();
        let c = catalog::open(Catalog::Photos, &d.path().join("photos.db")).unwrap();
        let item = add_item(
            &c,
            &Item {
                kind: "photo",
                point: Some((35.0, 135.7)),
                place_feature_id: Some(1),
                ..Item::default()
            },
        )
        .unwrap();
        let old = d.path().join("old.jpg").to_string_lossy().to_string();
        let new = d.path().join("new.jpg").to_string_lossy().to_string();
        let copy = d.path().join("copy.jpg").to_string_lossy().to_string();
        std::fs::write(&new, b"x").unwrap();
        std::fs::write(&copy, b"x").unwrap();
        let f = add_file(&c, item, &old, Some(10), None, None).unwrap();
        let pending: Option<String> = c
            .query_row("SELECT hash FROM photos_files WHERE id = ?1", [f], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(pending, None, "hash pending, not invented");
        assert_eq!(
            add_file(&c, item, &old, Some(10), None, Some("b3:aa")).unwrap(),
            f
        );
        let moved = add_file(&c, item, &new, Some(10), None, Some("b3:aa")).unwrap();
        assert_eq!(moved, f, "same bytes, old path gone: the same file, moved");
        let copied = add_file(&c, item, &copy, Some(10), None, Some("b3:aa")).unwrap();
        assert_ne!(
            copied, f,
            "the moved file is still there, so this is a copy"
        );
        let n: i64 = c
            .query_row("SELECT count(*) FROM photos_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        assert!(
            add_item(
                &c,
                &Item {
                    kind: "scan",
                    ..Item::default()
                }
            )
            .unwrap_err()
            .starts_with("unknown_kind")
        );
        assert!(
            add_item(
                &c,
                &Item {
                    kind: "photo",
                    point: Some((0.0, 200.0)),
                    ..Item::default()
                }
            )
            .is_err()
        );
    }
}
