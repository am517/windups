//! Daniel's purpose catalogs: which there are, where each lives, and bringing its schema up to date.
use crate::config::CatalogsCfg;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Catalog {
    Media,
    People,
    Records,
    Maps,
    Photos,
}

const PEOPLE_1: &str = include_str!("../schema/people_001.sql");
const PEOPLE_2: &str = include_str!("../schema/people_002.sql");
const RECORDS_1: &str = include_str!("../schema/records_001.sql");
const MAPS_1: &str = include_str!("../schema/maps_001.sql");
const PHOTOS_1: &str = include_str!("../schema/photos_001.sql");

impl Catalog {
    pub const ALL: [Catalog; 5] = [
        Catalog::Media,
        Catalog::People,
        Catalog::Records,
        Catalog::Maps,
        Catalog::Photos,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Catalog::Media => "media",
            Catalog::People => "people",
            Catalog::Records => "records",
            Catalog::Maps => "maps",
            Catalog::Photos => "photos",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    /// Whether it has the shared text layer (extracts, chunks, word index, vectors).
    pub fn has_text_layer(self) -> bool {
        matches!(self, Catalog::Media | Catalog::Records | Catalog::Photos)
    }

    /// Ordered migrations of a catalog Daniel keeps himself (media.db's live in the shared crate).
    fn migrations(self) -> Vec<String> {
        let with_text = |sql: &str| format!("{sql}\n{}", windups::db::text_layer(self.name()));
        match self {
            Catalog::Media => Vec::new(),
            Catalog::People => vec![PEOPLE_1.to_string(), PEOPLE_2.to_string()],
            Catalog::Records => vec![with_text(RECORDS_1)],
            Catalog::Maps => vec![MAPS_1.to_string()],
            Catalog::Photos => vec![with_text(PHOTOS_1)],
        }
    }

    pub fn latest(self) -> i64 {
        match self {
            Catalog::Media => windups::db::latest_version(),
            _ => self.migrations().len() as i64,
        }
    }
}

/// Where a catalog lives: its configured path, or `<name>.db` in Daniel's folder. Media is the
/// shared media.db, given by the caller.
pub fn locate(catalog: Catalog, cfg: &CatalogsCfg, dir: &Path, media: &Path) -> PathBuf {
    let set = match catalog {
        Catalog::Media => return media.to_path_buf(),
        Catalog::People => &cfg.people,
        Catalog::Records => &cfg.records,
        Catalog::Maps => &cfg.maps,
        Catalog::Photos => &cfg.photos,
    };
    if set.trim().is_empty() {
        dir.join(format!("{}.db", catalog.name()))
    } else {
        windups::paths::expand(set)
    }
}

/// Daniel's own folder (the standard windup layout).
pub fn daniel_dir() -> PathBuf {
    windups::base::layout::resolve(
        "daniel",
        &windups::base::layout::Probe::real("daniel", None),
    )
    .dir
}

/// Opens a catalog, creating its file and folders if needed, and migrates it.
pub fn open(catalog: Catalog, path: &Path) -> Result<Connection, String> {
    if catalog == Catalog::Media {
        return windups::db::open(path);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut c = Connection::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    c.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
        .map_err(|e| e.to_string())?;
    migrate(&mut c, catalog)?;
    Ok(c)
}

/// A catalog's schema version. Daniel's own catalogs keep it in `catalog_versions`, so several
/// may share one file; media.db uses `PRAGMA user_version`.
pub fn version(c: &Connection, catalog: Catalog) -> Result<i64, String> {
    if catalog == Catalog::Media {
        return windups::db::version(c);
    }
    let table: bool = c
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'catalog_versions'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        > 0;
    if !table {
        return Ok(0);
    }
    c.query_row(
        "SELECT version FROM catalog_versions WHERE catalog = ?1",
        [catalog.name()],
        |r| r.get(0),
    )
    .optional()
    .map(|v| v.unwrap_or(0))
    .map_err(|e| e.to_string())
}

/// Forward-only migrations, one transaction each, so a failed one leaves the file as it was.
fn migrate(c: &mut Connection, catalog: Catalog) -> Result<Vec<i64>, String> {
    let migrations = catalog.migrations();
    let current = version(c, catalog)?;
    let latest = migrations.len() as i64;
    if current > latest {
        return Err(format!(
            "the {} catalog is version {current}, newer than this Daniel knows ({latest}); please update",
            catalog.name()
        ));
    }
    let mut applied = Vec::new();
    for (i, sql) in migrations.iter().enumerate().skip(current as usize) {
        let v = (i + 1) as i64;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS catalog_versions (catalog TEXT PRIMARY KEY, version INTEGER NOT NULL);",
        )
        .map_err(|e| e.to_string())?;
        tx.execute_batch(sql)
            .map_err(|e| format!("{} catalog migration {v}: {e}", catalog.name()))?;
        tx.execute(
            "INSERT INTO catalog_versions(catalog, version) VALUES (?1, ?2)
             ON CONFLICT(catalog) DO UPDATE SET version = excluded.version",
            params![catalog.name(), v],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        applied.push(v);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(c: &Connection, sql: &str) -> i64 {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn every_catalog_opens_in_its_own_file_at_the_latest_version() {
        let d = tempfile::tempdir().unwrap();
        let cfg = CatalogsCfg::default();
        for cat in Catalog::ALL {
            let p = locate(cat, &cfg, d.path(), &d.path().join("media.db"));
            assert_eq!(p, d.path().join(format!("{}.db", cat.name())));
            let c = open(cat, &p).unwrap();
            assert_eq!(version(&c, cat).unwrap(), cat.latest(), "{}", cat.name());
        }
    }

    #[test]
    fn catalogs_can_share_one_file_by_config() {
        let d = tempfile::tempdir().unwrap();
        let one = d.path().join("all.db").to_string_lossy().to_string();
        let cfg = CatalogsCfg {
            people: one.clone(),
            records: one.clone(),
            maps: one.clone(),
            photos: one.clone(),
        };
        let media = d.path().join("all.db");
        for cat in Catalog::ALL {
            let c = open(cat, &locate(cat, &cfg, d.path(), &media)).unwrap();
            assert_eq!(version(&c, cat).unwrap(), cat.latest());
        }
        // reopening is a no-op, and each catalog kept its own version
        let c = open(Catalog::Photos, &media).unwrap();
        assert_eq!(count(&c, "SELECT count(*) FROM catalog_versions"), 4);
        assert_eq!(Catalog::People.latest(), 2);
        for cat in Catalog::ALL {
            assert_eq!(version(&c, cat).unwrap(), cat.latest());
        }
    }

    #[test]
    fn a_newer_catalog_than_this_daniel_knows_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("maps.db");
        drop(open(Catalog::Maps, &p).unwrap());
        let c = Connection::open(&p).unwrap();
        c.execute("UPDATE catalog_versions SET version = 99", [])
            .unwrap();
        let e = open(Catalog::Maps, &p).unwrap_err();
        assert!(e.contains("newer than this Daniel knows"), "{e}");
    }

    #[test]
    fn names_and_aliases_are_found_by_word_and_a_photo_links_people_and_places() {
        let d = tempfile::tempdir().unwrap();
        let people = open(Catalog::People, &d.path().join("people.db")).unwrap();
        people
            .execute(
                "INSERT INTO people_entities(kind, name) VALUES ('place', 'Kyoto')",
                [],
            )
            .unwrap();
        people
            .execute(
                "INSERT INTO people_aliases(entity_id, name, language) VALUES (1, '京都', 'ja')",
                [],
            )
            .unwrap();
        let hit = |q: &str| {
            count(
                &people,
                &format!(
                    "SELECT count(*) FROM people_names_fts WHERE people_names_fts MATCH '{q}'"
                ),
            )
        };
        assert_eq!(hit("kyoto"), 1);
        assert_eq!(hit("京都"), 1);
        people.execute("DELETE FROM people_aliases", []).unwrap();
        assert_eq!(hit("京都"), 0, "the alias left the index with its row");

        let maps = open(Catalog::Maps, &d.path().join("maps.db")).unwrap();
        maps.execute("INSERT INTO maps_layers(name) VALUES ('places')", [])
            .unwrap();
        maps.execute(
            "INSERT INTO maps_features(layer_id, lat, lon, label, label_alt, label_alt_language, place_entity_id) VALUES (1, 35.01, 135.77, 'Kyoto', '京都', 'ja', 1)",
            [],
        )
        .unwrap();
        assert!(
            maps.execute(
                "INSERT INTO maps_features(layer_id, lat, lon, label) VALUES (1, 95, 0, 'off the globe')",
                [],
            )
            .is_err()
        );
        let photos = open(Catalog::Photos, &d.path().join("photos.db")).unwrap();
        photos
            .execute(
                "INSERT INTO photos_items(kind, lat, lon, place_feature_id) VALUES ('photo', 35.01, 135.77, 1)",
                [],
            )
            .unwrap();
        people
            .execute(
                "INSERT INTO people_mentions(catalog, record_id, locator, surface, entity_id, confidence, extractor, extractor_version) VALUES ('photos', 1, 'whole image', 'Kyoto', 1, 1.0, 'test', '1')",
                [],
            )
            .unwrap();
        assert_eq!(
            count(
                &people,
                "SELECT count(*) FROM people_mentions WHERE entity_id = 1 AND catalog = 'photos'"
            ),
            1
        );
    }

    #[test]
    fn a_mention_is_either_a_link_or_a_proposal_never_both() {
        let d = tempfile::tempdir().unwrap();
        let c = open(Catalog::People, &d.path().join("people.db")).unwrap();
        c.execute(
            "INSERT INTO people_entities(kind, name) VALUES ('person', 'Ada')",
            [],
        )
        .unwrap();
        let add = |entity: &str, proposed: &str| {
            c.execute(
                &format!("INSERT INTO people_mentions(catalog, record_id, surface, entity_id, proposed_name, confidence, extractor, extractor_version) VALUES ('media', 1, 'Ada', {entity}, {proposed}, 0.9, 'x', '1')"),
                [],
            )
        };
        assert!(add("1", "'Ada'").is_err());
        assert!(add("NULL", "NULL").is_err());
        assert!(add("1", "NULL").is_ok());
        assert!(add("1", "NULL").is_err(), "the same mention is kept once");
    }
}
