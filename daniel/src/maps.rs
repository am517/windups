//! Writing the maps catalog: layers, and features with WGS84 points and/or GeoJSON geometry.
//! Importing the same source feature again (same `uid`) updates it rather than duplicating it.
use rusqlite::{Connection, OptionalExtension, params};

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

/// A layer by name: its id, created if new.
pub fn layer(
    c: &Connection,
    name: &str,
    kind: Option<&str>,
    source: Option<&str>,
) -> Result<i64, String> {
    if name.trim().is_empty() {
        return Err("name_empty: a layer needs a name".into());
    }
    c.execute(
        "INSERT OR IGNORE INTO maps_layers(name, kind, source) VALUES (?1, ?2, ?3)",
        params![name.trim(), kind, source],
    )
    .map_err(err)?;
    c.query_row(
        "SELECT id FROM maps_layers WHERE name = ?1",
        [name.trim()],
        |r| r.get(0),
    )
    .map_err(err)
}

#[derive(Debug, Default, Clone)]
pub struct Feature<'a> {
    pub layer_id: i64,
    pub parent_id: Option<i64>,
    pub kind: Option<&'a str>,
    /// (latitude, longitude), WGS84.
    pub point: Option<(f64, f64)>,
    /// A GeoJSON geometry object, when more than a point.
    pub geometry: Option<&'a str>,
    pub label: &'a str,
    pub label_alt: Option<&'a str>,
    pub label_alt_language: Option<&'a str>,
    pub source: Option<&'a str>,
    pub uid: Option<&'a str>,
    pub verified: bool,
    pub place_entity_id: Option<i64>,
}

/// Why a feature can't be stored, if it can't.
pub fn invalid(f: &Feature) -> Option<String> {
    if f.label.trim().is_empty() {
        return Some("label_empty: a feature needs a label".into());
    }
    if let Some((lat, lon)) = f.point
        && !((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon))
    {
        return Some(format!("off_the_globe: {lat},{lon} is not a WGS84 point"));
    }
    if f.point.is_none() && f.geometry.is_none() {
        return Some("no_location: a feature needs a point or a geometry".into());
    }
    if let Some(g) = f.geometry {
        let ok = serde_json::from_str::<serde_json::Value>(g)
            .ok()
            .is_some_and(|v| v.get("type").and_then(|t| t.as_str()).is_some());
        if !ok {
            return Some(
                "geometry_not_geojson: geometry must be a GeoJSON object with a type".into(),
            );
        }
    }
    None
}

/// Stores a feature and returns its id. With a `uid` already present, that row is updated.
pub fn feature(c: &Connection, f: &Feature) -> Result<i64, String> {
    if let Some(why) = invalid(f) {
        return Err(why);
    }
    let (lat, lon) = f.point.unzip();
    let existing: Option<i64> = match f.uid {
        Some(u) => c
            .query_row("SELECT id FROM maps_features WHERE uid = ?1", [u], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?,
        None => None,
    };
    let values = params![
        f.layer_id,
        f.parent_id,
        f.kind,
        lat,
        lon,
        f.geometry,
        f.label.trim(),
        f.label_alt,
        f.label_alt_language,
        f.source,
        f.uid,
        f.verified,
        f.place_entity_id
    ];
    match existing {
        Some(id) => {
            c.execute(
                "UPDATE maps_features SET layer_id = ?1, parent_id = ?2, kind = ?3, lat = ?4, lon = ?5, geometry = ?6, label = ?7,
                 label_alt = ?8, label_alt_language = ?9, source = ?10, uid = ?11, verified = ?12, place_entity_id = ?13,
                 updated_at = CURRENT_TIMESTAMP WHERE id = ?14",
                rusqlite::params_from_iter(values.iter().copied().chain([&id as &dyn rusqlite::ToSql])),
            )
            .map_err(err)?;
            Ok(id)
        }
        None => {
            c.execute(
                "INSERT INTO maps_features(layer_id, parent_id, kind, lat, lon, geometry, label, label_alt, label_alt_language, source, uid, verified, place_entity_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                values,
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

    fn maps(d: &std::path::Path) -> Connection {
        catalog::open(Catalog::Maps, &d.join("maps.db")).unwrap()
    }

    #[test]
    fn a_layer_is_made_once_and_a_reimported_feature_is_updated_not_duplicated() {
        let d = tempfile::tempdir().unwrap();
        let c = maps(d.path());
        let l = layer(&c, "places", Some("places"), Some("notes")).unwrap();
        assert_eq!(layer(&c, "places", None, None).unwrap(), l);
        let park = Feature {
            layer_id: l,
            point: Some((35.0, 135.7)),
            label: "Park",
            uid: Some("p-1"),
            ..Feature::default()
        };
        let id = feature(&c, &park).unwrap();
        let moved = Feature {
            point: Some((35.1, 135.8)),
            label_alt: Some("公園"),
            label_alt_language: Some("ja"),
            verified: true,
            ..park.clone()
        };
        assert_eq!(feature(&c, &moved).unwrap(), id);
        let (lat, alt, verified): (f64, String, bool) = c
            .query_row(
                "SELECT lat, label_alt, verified FROM maps_features",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((lat, alt.as_str(), verified), (35.1, "公園", true));
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM maps_labels_fts WHERE maps_labels_fts MATCH '公園'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "the word index followed the update");
        let playground = Feature {
            layer_id: l,
            parent_id: Some(id),
            point: Some((35.1, 135.8)),
            label: "Playground",
            ..Feature::default()
        };
        assert!(feature(&c, &playground).unwrap() > id);
    }

    #[test]
    fn a_feature_needs_a_label_and_a_real_location() {
        let base = Feature {
            layer_id: 1,
            label: "X",
            point: Some((0.0, 0.0)),
            ..Feature::default()
        };
        assert!(invalid(&base).is_none());
        let off = Feature {
            point: Some((91.0, 0.0)),
            ..base.clone()
        };
        assert!(invalid(&off).unwrap().starts_with("off_the_globe"));
        let nowhere = Feature {
            point: None,
            ..base.clone()
        };
        assert!(invalid(&nowhere).unwrap().starts_with("no_location"));
        let shape = Feature {
            point: None,
            geometry: Some(r#"{"type":"LineString","coordinates":[[0,0],[1,1]]}"#),
            ..base.clone()
        };
        assert!(invalid(&shape).is_none());
        let junk = Feature {
            geometry: Some("[1,2]"),
            ..base.clone()
        };
        assert!(invalid(&junk).unwrap().starts_with("geometry_not_geojson"));
        let blank = Feature { label: " ", ..base };
        assert!(invalid(&blank).unwrap().starts_with("label_empty"));
    }
}
