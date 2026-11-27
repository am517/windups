//! Open media.db and bring its schema up to date.
use rusqlite::Connection;
use std::path::Path;

/// Ordered migrations; index + 1 is the schema version (PRAGMA user_version).
const MIGRATIONS: &[&str] = &[
    include_str!("../schema/media.sql"),
    include_str!("../schema/002_unresolved_names_conflicts_vectors.sql"),
    include_str!("../schema/003_needs_flags_decisions.sql"),
    include_str!("../schema/004_lifecycle_merges.sql"),
    include_str!("../schema/005_works.sql"),
    include_str!("../schema/006_returns.sql"),
    include_str!("../schema/007_work_ids_walter.sql"),
    include_str!("../schema/008_windup_state.sql"),
    include_str!("../schema/009_text_layer_model_version.sql"),
];

/// The text layer every text-holding catalog has (extracts, chunks, word index, vectors), with
/// `@c@` standing for the catalog name. media.db has it as migration 9.
pub const TEXT_LAYER: &str = include_str!("../schema/text_layer.sql");

/// The text layer for one catalog.
pub fn text_layer(catalog: &str) -> String {
    TEXT_LAYER.replace("@c@", catalog)
}

pub fn latest_version() -> i64 {
    MIGRATIONS.len() as i64
}

/// Open (creating folders and file if needed) and migrate.
pub fn open(path: &Path) -> Result<Connection, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
        .map_err(|e| e.to_string())?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn version(conn: &Connection) -> Result<i64, String> {
    crate::base::schema::version(conn)
}

pub fn migrate(conn: &mut Connection) -> Result<(), String> {
    crate::base::schema::migrate(conn, "media.db", MIGRATIONS).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creates_and_migrates() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/media.db");
        let c = open(&p).unwrap();
        assert_eq!(version(&c).unwrap(), latest_version());
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='media'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        let n: i64 = c
            .query_row("SELECT count(*) FROM unresolved_reasons", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 6);
    }
    #[test]
    fn v1_database_upgrades() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("media.db");
        {
            let c = rusqlite::Connection::open(&p).unwrap();
            c.execute_batch(MIGRATIONS[0]).unwrap();
            c.execute("INSERT INTO media(kind, title) VALUES ('movie', 'Old')", [])
                .unwrap();
        }
        let c = open(&p).unwrap();
        assert_eq!(version(&c).unwrap(), latest_version());
        let t: String = c
            .query_row("SELECT title FROM media", [], |r| r.get(0))
            .unwrap();
        assert_eq!(t, "Old");
    }
    #[test]
    fn media_text_layer_is_the_shared_template() {
        let body: String = TEXT_LAYER.lines().skip(1).collect::<Vec<_>>().join("\n");
        assert!(MIGRATIONS[8].contains(&body.replace("@c@", "media")));
        assert!(!text_layer("records").contains("@c@"));
    }
    #[test]
    fn reopen_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("media.db");
        drop(open(&p).unwrap());
        assert_eq!(version(&open(&p).unwrap()).unwrap(), latest_version());
    }
}
