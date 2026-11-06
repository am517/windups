//! Saving what Jack fetched: into media.db, or (no database) a JSON file beside the media.
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use windups::handoff::Fields as Meta;

/// When Jack writes a versioned handoff JSON beside the media (`<file><fallback_suffix>`).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum HandoffJson {
    #[default]
    Auto,
    Always,
    Off,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct StoreCfg {
    pub enabled: bool,
    pub database: String,
    pub fallback_suffix: String,
    pub busy_timeout_secs: u64,
    /// Jack only: `auto` writes `<file>.jack.json` when the DB is disabled or unavailable;
    /// `always` also writes it when handing over in-process; `off` never writes it.
    pub handoff_json: HandoffJson,
}

/// rating_<source> meta keys stored in ratings.
const RATINGS: [&str; 5] = ["imdb", "rotten_tomatoes", "metacritic", "tmdb", "tvmaze"];

/// "6.3/10" -> (6.3, 10), "36%" -> (36, 100), "8.4" -> (8.4, 10).
fn rating(v: &str) -> Option<(f64, f64)> {
    let v = v.trim();
    let (val, scale) = match (v.strip_suffix('%'), v.split_once('/')) {
        (Some(p), _) => (p, "100"),
        (_, Some((a, b))) => (a, b),
        _ => (v, "10"),
    };
    Some((val.trim().parse().ok()?, scale.trim().parse().ok()?))
}

/// Meta key -> groups.kind.
const GROUPS: [(&str, &str); 2] = [("franchise", "franchise"), ("series_group", "collection")];
/// Meta key -> art kind.
const ART: [(&str, &str); 4] = [
    ("poster_url", "poster"),
    ("backdrop_url", "backdrop"),
    ("still_url", "still"),
    ("season_poster_url", "season"),
];
/// Meta key -> (source, scope) in external_ids.
const IDS: [(&str, &str, &str); 14] = [
    ("youtube_video_id", "youtube", "video"),
    ("x_video_id", "x", "post"),
    ("wikidata_id", "wikidata", ""),
    ("tvmaze_id", "tvmaze", "show"),
    ("tvmaze_episode_id", "tvmaze", "episode"),
    ("itunes_id", "itunes", ""),
    ("youtube_trailer_id", "youtube", "trailer"),
    ("imdb_id", "imdb", ""),
    ("tmdb_id", "tmdb", ""),
    ("tmdb_series_id", "tmdb", "tv"),
    ("tmdb_episode_id", "tmdb", "episode"),
    ("thetvdb_id", "thetvdb", ""),
    ("thetvdb_series_id", "thetvdb", "series"),
    ("thetvdb_episode_id", "thetvdb", "episode"),
];

const SHOW_IDS: [&str; 4] = ["imdb_id", "tmdb_series_id", "thetvdb_series_id", "tmdb_id"];

/// Channel as an entity (its name kept as a source name), matched show as its own parent media row.
fn online_extras(tx: &Transaction, id: i64, meta: &Meta) -> rusqlite::Result<()> {
    if let Some(name) = meta.get("channel") {
        let site = meta.get("site").map_or("youtube", |s| s.as_str());
        let by_id = |cid: &String| {
            tx.query_row("SELECT entity_id FROM external_ids WHERE source = ?1 AND scope = 'channel' AND value = ?2 AND entity_id IS NOT NULL", params![site, cid], |r| r.get::<_, i64>(0)).optional()
        };
        let found = match meta.get("channel_id") {
            Some(c) => by_id(c)?,
            None => tx.query_row("SELECT id FROM entities WHERE kind = 'organization' AND name = ?1 AND source = ?2", params![name, site], |r| r.get(0)).optional()?,
        };
        let eid = match found {
            Some(e) => e,
            None => {
                tx.execute("INSERT INTO entities(kind, name, source, fetched_at) VALUES ('organization', ?1, ?2, CURRENT_TIMESTAMP)", params![name, site])?;
                let e = tx.last_insert_rowid();
                if let Some(c) = meta.get("channel_id") {
                    tx.execute("INSERT OR IGNORE INTO external_ids(entity_id, source, scope, value) VALUES (?1, ?2, 'channel', ?3)", params![e, site, c])?;
                }
                e
            }
        };
        tx.execute("INSERT OR IGNORE INTO media_entities(media_id, entity_id, role, source) VALUES (?1, ?2, 'channel', ?3)", params![id, eid, site])?;
        windups::names::add_source_name(tx, "entities", eid, site, name)?;
    }
    if let Some(show) = meta.get("show") {
        let sid = show_row(tx, meta, show)?;
        tx.execute(
            "UPDATE media SET parent_id = ?2 WHERE id = ?1",
            params![id, sid],
        )?;
        let src = meta.get("show_source").map_or("crossover", |s| s.as_str());
        windups::names::add_source_name(tx, "media", sid, src, show)?;
        if let Some(ch) = meta.get("channel") {
            // the channel's name for this show, untouched (display name stays the user's choice)
            windups::names::add_source_name(
                tx,
                "media",
                sid,
                meta.get("site").map_or("youtube", |s| s.as_str()),
                ch,
            )?;
        }
    }
    Ok(())
}

/// The show as its own media row, found again by any of its ids; the ids point at it.
fn show_row(tx: &Transaction, meta: &Meta, show: &str) -> rusqlite::Result<i64> {
    let ids: Vec<_> = IDS
        .iter()
        .filter(|(k, _, _)| SHOW_IDS.contains(k))
        .filter_map(|(k, src, scope)| meta.get(*k).map(|v| (*src, *scope, v)))
        .collect();
    let mut found = None;
    for (src, scope, v) in &ids {
        found = tx.query_row("SELECT media_id FROM external_ids WHERE source = ?1 AND scope = ?2 AND value = ?3 AND media_id IS NOT NULL", params![src, scope, v], |r| r.get::<_, i64>(0)).optional()?;
        if found.is_some() {
            break;
        }
    }
    let sid = match found {
        Some(s) => s,
        None => {
            tx.execute("INSERT INTO media(kind, title, status, source, fetched_at) VALUES ('show', ?1, 'chosen', 'crossover', CURRENT_TIMESTAMP)", [show])?;
            tx.last_insert_rowid()
        }
    };
    for (src, scope, v) in ids {
        tx.execute("INSERT INTO external_ids(media_id, source, scope, value) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(source, scope, value) DO UPDATE SET media_id = excluded.media_id", params![sid, src, scope, v])?;
    }
    Ok(sid)
}

pub fn db_path(cfg: &StoreCfg) -> Result<PathBuf, String> {
    if cfg.database.is_empty() {
        Ok(windups::Settings::load()?.database_path())
    } else {
        Ok(PathBuf::from(&cfg.database))
    }
}

/// Writes the entry into media.db. If the database cannot be opened, returns an error;
/// Jack then writes the versioned handoff JSON beside the media when configured to.
pub fn save(
    path: &Path,
    meta: &Meta,
    found: &[(String, Meta)],
    conflicts: &[windups::handoff::Conflict],
    cfg: &StoreCfg,
    fper: &dyn larry::fingerprint::Fingerprinter,
    fcfg: &larry::fingerprint::FingerprintCfg,
) -> Result<String, String> {
    let db = db_path(cfg).and_then(|p| windups::db::open(&p).map(|c| (p, c)));
    match db {
        Ok((p, mut c)) => {
            c.busy_timeout(std::time::Duration::from_secs(cfg.busy_timeout_secs))
                .map_err(|e| e.to_string())?;
            let id =
                record_with(&mut c, path, meta, found, conflicts).map_err(|e| e.to_string())?;
            if let Some(fp) = meta.get("file_fingerprint") {
                // hints only: open proposals, never merge
                larry::fingerprint::propose_similar(&c, id, fp, fper, fcfg)
                    .map_err(|e| e.to_string())?;
            }
            Ok(format!("database: media #{id} in {}", p.display()))
        }
        Err(e) => Err(format!("no database ({e})")),
    }
}

/// Writes the chosen (merged) row plus every provider answer as a candidate row. Returns the chosen id.
pub fn record(
    c: &mut Connection,
    path: &Path,
    meta: &Meta,
    found: &[(String, Meta)],
) -> rusqlite::Result<i64> {
    record_with(c, path, meta, found, &[])
}

/// `record`, plus the disagreements the handing windup found between sources about this work.
pub fn record_with(
    c: &mut Connection,
    path: &Path,
    meta: &Meta,
    found: &[(String, Meta)],
    conflicts: &[windups::handoff::Conflict],
) -> rusqlite::Result<i64> {
    let tx = c.transaction()?;
    let online = meta.get("kind").is_some_and(|k| k == "online_video");
    let existing = match find_existing(&tx, meta, online)? {
        Some(i) => Some(i),
        // the same file again: its entry (even an undetermined one) is identified in place, same id
        None => tx
            .query_row(
                "SELECT media_id FROM files WHERE path = ?1 AND media_id IS NOT NULL",
                [path.to_string_lossy()],
                |r| r.get(0),
            )
            .optional()?
            .or(moved(&tx, path, meta)?),
    };
    // a merged entry points at the one that stands now
    let existing = existing
        .map(|i| windups::lifecycle::resolve_id(&tx, i))
        .transpose()?;
    let id = upsert_media(&tx, existing, meta, "merged", "chosen", None)?;
    add_details(&tx, id, meta, "merged", true)?;
    for g in meta
        .get("genre")
        .into_iter()
        .flat_map(|g| g.split(','))
        .map(str::trim)
        .filter(|g| !g.is_empty())
    {
        tx.execute("INSERT OR IGNORE INTO genres(name) VALUES (?1)", [g])?;
        tx.execute("INSERT OR IGNORE INTO media_genres(media_id, genre_id, source) SELECT ?1, id, 'merged' FROM genres WHERE name = ?2", params![id, g])?;
    }
    for src in RATINGS {
        if let Some((value, scale)) = meta.get(&format!("rating_{src}")).and_then(|v| rating(v)) {
            let votes = meta
                .get(&format!("rating_{src}_votes"))
                .and_then(|v| v.parse::<i64>().ok());
            tx.execute(
                "INSERT OR REPLACE INTO ratings(media_id, source, value, scale, votes, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                params![id, src, value, scale, votes],
            )?;
        }
    }
    for (k, kind) in GROUPS {
        if let Some(n) = meta.get(k) {
            tx.execute(
                "INSERT OR IGNORE INTO groups(name, kind, source) VALUES (?1, ?2, 'wikidata')",
                params![n, kind],
            )?;
            tx.execute("INSERT OR IGNORE INTO media_groups(media_id, group_id, source) SELECT ?1, id, 'wikidata' FROM groups WHERE name = ?2 AND kind = ?3", params![id, n, kind])?;
        }
    }
    if online {
        online_extras(&tx, id, meta)?;
    }
    // unresolved: kept with all candidates and a reason, so a later run (or Sammy) can decide
    let unresolved = meta.get("resolution").is_some_and(|r| r == "unresolved");
    if unresolved {
        let reason = meta
            .get("unresolved_reason")
            .map_or("needs_llm_windup", |s| s.as_str());
        windups::unresolved::mark(
            &tx,
            id,
            "jack",
            reason,
            meta.get("unresolved_detail").map_or("", |s| s.as_str()),
        )?;
    } else {
        windups::unresolved::resolve(&tx, id, "jack")?;
        // tagged, but something is left to look into (e.g. the file and a source disagree)
        if let Some(r) = meta.get("research_reason") {
            windups::unresolved::mark(
                &tx,
                id,
                "jack",
                r,
                meta.get("research_detail").map_or("", |s| s.as_str()),
            )?;
        }
        // disagreements between sources about this same work: recorded, not settled
        for (field, vals) in conflicts {
            windups::conflicts::record(&tx, id, field, vals)?;
        }
    }
    for (k, src, scope) in IDS {
        // an online video's show ids belong to the show, not to the video
        if online && SHOW_IDS.contains(&k) {
            continue;
        }
        if let Some(v) = meta.get(k) {
            tx.execute("INSERT INTO external_ids(media_id, source, scope, value) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(source, scope, value) DO UPDATE SET media_id = excluded.media_id", params![id, src, scope, v])?;
        }
    }
    for (src, m) in found {
        let cid = upsert_media(&tx, None, m, src, "candidate", Some(id))?;
        add_details(&tx, cid, m, src, false)?;
    }
    // where the entry stands now; every move is kept in status_history
    if let Some(stage) = meta.get("lifecycle") {
        let why = meta
            .get("unresolved_reason")
            .or(meta.get("research_reason"))
            .or(meta.get("evidence"))
            .map_or("", |s| s.as_str());
        windups::lifecycle::set(&tx, id, stage, "jack", why)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        // when Jack last looked; Walter decides from this, the stage and the reason when to call again
        tx.execute(
            "UPDATE media SET last_checked = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = ?1",
            [id],
        )?;
    }
    // escalation decisions, with provenance (who, which step, when)
    if let Some(list) = meta
        .get("decisions")
        .and_then(|d| serde_json::from_str::<Vec<serde_json::Value>>(d).ok())
    {
        for d in list {
            let g = |k: &str| d[k].as_str().unwrap_or("").to_string();
            windups::decisions::record(
                &tx,
                id,
                "jack",
                &g("step"),
                &g("source"),
                &d["decision"].to_string(),
                &g("note"),
            )?;
        }
    }
    let size = std::fs::metadata(path).ok().map(|m| m.len() as i64);
    tx.execute("INSERT INTO files(path, media_id, hash, size, fingerprint) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(path) DO UPDATE SET media_id = excluded.media_id, hash = coalesce(excluded.hash, hash), size = coalesce(excluded.size, size), fingerprint = coalesce(excluded.fingerprint, fingerprint)", params![path.to_string_lossy(), id, meta.get("file_hash"), size, meta.get("file_fingerprint")])?;
    // the work this is a version of (identified entries only)
    if !matches!(
        meta.get("lifecycle").map(String::as_str),
        Some("undetermined") | Some("not_identified")
    ) {
        if let Some(r) = ruth::reference(meta) {
            let ids = ruth::ids(meta);
            windups::works::link(
                &tx,
                id,
                &windups::works::Work {
                    key: &ruth::key(&r.text),
                    reference: &r.text,
                    provisional: r.provisional,
                    ids: &ids,
                },
            )?;
        }
    }
    tx.commit()?;
    Ok(id)
}

/// A re-run updates the same entry (found by any known id) instead of adding a duplicate.
/// The same content whose old path is gone (renamed or moved): its entry; the old file row is
/// dropped so the new path takes over. A copy whose original is still there is a separate file.
fn moved(tx: &Transaction, path: &Path, meta: &Meta) -> rusqlite::Result<Option<i64>> {
    let Some(h) = meta.get("file_hash") else {
        return Ok(None);
    };
    let olds: Vec<(String, i64)> = tx
        .prepare("SELECT path, media_id FROM files WHERE hash = ?1 AND media_id IS NOT NULL AND path <> ?2 ORDER BY id")?
        .query_map(params![h, path.to_string_lossy()], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (p, id) in olds {
        if !Path::new(&p).exists() {
            tx.execute("DELETE FROM files WHERE path = ?1", [&p])?;
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn find_existing(tx: &Transaction, meta: &Meta, online: bool) -> rusqlite::Result<Option<i64>> {
    // an online video's show ids find its show, not the video
    for (k, src, scope) in IDS
        .iter()
        .filter(|(k, _, _)| !(online && SHOW_IDS.contains(k)))
    {
        if let Some(v) = meta.get(*k) {
            let r = tx.query_row("SELECT media_id FROM external_ids WHERE source = ?1 AND scope = ?2 AND value = ?3 AND media_id IS NOT NULL", params![src, scope, v], |r| r.get(0)).optional()?;
            if r.is_some() {
                return Ok(r);
            }
        }
    }
    Ok(None)
}

fn upsert_media(
    tx: &Transaction,
    existing: Option<i64>,
    m: &Meta,
    source: &str,
    status: &str,
    canonical: Option<i64>,
) -> rusqlite::Result<i64> {
    let g = |k: &str| m.get(k).cloned();
    let n = |k: &str| {
        m.get(k)
            .and_then(|v| v.get(..4.min(v.len()))?.parse::<i64>().ok())
    };
    let title = g("episode_title").or(g("title")).or(g("show"));
    let raw = serde_json::to_string(m).unwrap_or_default();
    let vals = params![
        g("kind").unwrap_or_else(|| "movie".into()),
        title,
        g("original_title"),
        n("year"),
        m.get("season").and_then(|v| v.parse::<i64>().ok()),
        m.get("episode").and_then(|v| v.parse::<i64>().ok()),
        g("rated"),
        status,
        canonical,
        source,
        raw
    ];
    if let Some(id) = existing {
        tx.execute("DELETE FROM media WHERE canonical_id = ?1", [id])?;
        for t in [
            "texts",
            "art",
            "media_genres",
            "media_groups",
            "ratings",
            "media_entities",
        ] {
            tx.execute(&format!("DELETE FROM {t} WHERE media_id = ?1"), [id])?;
        }
        tx.execute("UPDATE media SET kind=?1, title=?2, original_title=?3, year=?4, season=?5, episode=?6, rated=?7, status=?8, canonical_id=?9, source=?10, raw_json=?11, fetched_at=CURRENT_TIMESTAMP, updated_at=CURRENT_TIMESTAMP WHERE id = ?12", rusqlite::params_from_iter(vals.iter().copied().chain([&id as &dyn rusqlite::ToSql])))?;
        return Ok(id);
    }
    tx.execute("INSERT INTO media(kind, title, original_title, year, season, episode, rated, status, canonical_id, source, raw_json, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, CURRENT_TIMESTAMP)", vals)?;
    Ok(tx.last_insert_rowid())
}

/// Texts and art for one row; on the chosen row they're marked chosen.
fn add_details(
    tx: &Transaction,
    id: i64,
    m: &Meta,
    source: &str,
    chosen: bool,
) -> rusqlite::Result<()> {
    for (field, key) in [
        ("title", "title"),
        ("title", "episode_title"),
        ("description", "description"),
        ("tagline", "tagline"),
    ] {
        if let Some(t) = m.get(key) {
            tx.execute("INSERT INTO texts(media_id, field, source, text, chosen, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)", params![id, field, source, t, chosen])?;
        }
    }
    for (key, kind) in ART {
        if let Some(u) = m.get(key) {
            tx.execute("INSERT INTO art(media_id, kind, source, url, chosen, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)", params![id, kind, source, u, chosen])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(kv: &[(&str, &str)]) -> Meta {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
    #[test]
    fn rating_formats() {
        assert_eq!(rating("6.3/10"), Some((6.3, 10.0)));
        assert_eq!(rating("36%"), Some((36.0, 100.0)));
        assert_eq!(rating("41/100"), Some((41.0, 100.0)));
        assert_eq!(rating("8.4"), Some((8.4, 10.0)));
        assert_eq!(rating("N/A"), None);
    }

    fn count(c: &Connection, sql: &str) -> i64 {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn online_video_channel_entity_show_parent_names_and_unresolved() {
        let d = tempfile::tempdir().unwrap();
        let mut c = windups::db::open(&d.path().join("m.db")).unwrap();
        let m: Meta = [
            ("kind", "online_video"),
            ("title", "Ep"),
            ("channel", "The Why Files"),
            ("channel_id", "UC1"),
            ("site", "youtube"),
            ("youtube_video_id", "abcdefghijk"),
            ("show", "The Why Files"),
            ("tmdb_series_id", "7"),
            ("resolution", "unresolved"),
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .into();
        let f = vec![("tmdb".to_string(), m.clone())];
        record(&mut c, &d.path().join("a.mp4"), &m, &f).unwrap();
        record(
            &mut c,
            &d.path().join("b.mp4"),
            &[("youtube_video_id", "zzzzzzzzzzz")]
                .iter()
                .fold(m.clone(), |mut a, (k, v)| {
                    a.insert(k.to_string(), v.to_string());
                    a
                }),
            &f,
        )
        .unwrap();
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM entities WHERE kind='organization'"
            ),
            1
        );
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM media_entities WHERE role='channel'"
            ),
            2
        );
        // the show is its own parent row, holding the show ids; both videos hang off it
        assert_eq!(count(&c, "SELECT count(*) FROM media WHERE kind='show'"), 1);
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM media WHERE parent_id = (SELECT id FROM media WHERE kind='show' AND status='chosen' AND source='crossover')"
            ),
            2
        );
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM external_ids WHERE source='tmdb' AND media_id = (SELECT id FROM media WHERE kind='show' AND source='crossover')"
            ),
            1
        );
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM unresolved WHERE reason='needs_llm_windup' AND status='open'"
            ),
            2
        );
        assert_eq!(
            count(&c, "SELECT count(*) FROM texts WHERE field='resolution'"),
            0
        );
        assert_eq!(
            count(&c, "SELECT count(*) FROM media WHERE status='candidate'"),
            2
        );
        // names: channel and show keep every source name; display names auto-filled, user-editable
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM source_names WHERE media_id IS NOT NULL"
            ),
            2
        );
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM entities WHERE display_name = 'The Why Files' AND display_name_set_by = 'auto:youtube'"
            ),
            1
        );
        let sid = count(
            &c,
            "SELECT id FROM media WHERE kind='show' AND source='crossover'",
        );
        windups::names::set_display_name(&c, "media", sid, "Why Files").unwrap();
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM source_names WHERE name = 'The Why Files'"
            ),
            3
        );
        let mut m2 = m.clone();
        m2.remove("resolution");
        let vid = record(&mut c, &d.path().join("a.mp4"), &m2, &f).unwrap();
        assert_eq!(
            count(
                &c,
                &format!(
                    "SELECT count(*) FROM unresolved WHERE media_id = {vid} AND status='resolved'"
                )
            ),
            1
        );
    }

    #[test]
    fn records_the_conflicts_handed_over() {
        let d = tempfile::tempdir().unwrap();
        let mut c = windups::db::open(&d.path().join("m.db")).unwrap();
        let merged = mk(&[
            ("kind", "movie"),
            ("title", "The Matrix"),
            ("year", "1999"),
            ("tmdb_id", "603"),
        ]);
        let found = vec![
            (
                "tmdb".to_string(),
                mk(&[("title", "The Matrix"), ("year", "1999")]),
            ),
            (
                "embedded".to_string(),
                mk(&[("title", "The.Matrix"), ("year", "1998")]),
            ),
        ];
        let year = vec![(
            "year".to_string(),
            vec![
                ("tmdb".into(), "1999".into()),
                ("embedded".into(), "1998".into()),
            ],
        )];
        record_with(&mut c, Path::new("/m/a.mp4"), &merged, &found, &year).unwrap();
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM conflicts WHERE field='year' AND status='open'"
            ),
            1
        );
        assert_eq!(count(&c, "SELECT count(*) FROM conflicts"), 1);
        assert_eq!(
            count(&c, "SELECT year FROM media WHERE status='chosen'"),
            1999
        );
        assert_eq!(
            count(&c, "SELECT count(*) FROM media WHERE source='embedded'"),
            1
        );
    }

    #[test]
    fn records_chosen_candidates_texts_art_genres_ids_and_rerun_updates() {
        let d = tempfile::tempdir().unwrap();
        let mut c = windups::db::open(&d.path().join("media.db")).unwrap();
        let merged = mk(&[
            ("kind", "movie"),
            ("title", "The Matrix"),
            ("year", "1999"),
            ("description", "Neo"),
            ("poster_url", "http://p"),
            ("genre", "action,sci-fi"),
            ("imdb_id", "tt0133093"),
            ("tmdb_id", "603"),
            ("wikidata_id", "Q83495"),
            ("franchise", "The Matrix"),
            ("rating_imdb", "8.7/10"),
            ("rating_imdb_votes", "2100000"),
            ("rating_rotten_tomatoes", "83%"),
            ("rating_tmdb", "8.2"),
        ]);
        let found = vec![
            (
                "tmdb".to_string(),
                mk(&[("title", "The Matrix"), ("poster_url", "http://p")]),
            ),
            (
                "omdb".to_string(),
                mk(&[("title", "Matrix"), ("year", "1999")]),
            ),
        ];
        let id = record(&mut c, Path::new("/m/a.mp4"), &merged, &found).unwrap();
        assert_eq!(
            count(&c, "SELECT count(*) FROM media WHERE status='candidate'"),
            2
        );
        assert_eq!(
            count(&c, "SELECT count(*) FROM media WHERE raw_json IS NULL"),
            0
        );
        assert_eq!(
            count(&c, "SELECT year FROM media WHERE status='chosen'"),
            1999
        );
        assert_eq!(count(&c, "SELECT count(*) FROM texts WHERE chosen=1"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM art"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM media_genres"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM external_ids"), 3);
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM media_groups JOIN groups ON groups.id = group_id WHERE kind = 'franchise'"
            ),
            1
        );
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM texts_fts WHERE texts_fts MATCH 'neo'"
            ),
            1
        );
        // re-run: same entry, no duplicates
        assert_eq!(
            record(&mut c, Path::new("/m/a.mp4"), &merged, &found).unwrap(),
            id
        );
        assert_eq!(count(&c, "SELECT count(*) FROM media"), 3);
        assert_eq!(count(&c, "SELECT count(*) FROM art"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM files"), 1);
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM ratings WHERE source = 'rotten_tomatoes' AND value = 83 AND scale = 100"
            ),
            1
        );
        assert_eq!(
            count(
                &c,
                "SELECT votes FROM ratings WHERE source = 'imdb' AND value = 8.7 AND scale = 10"
            ),
            2100000
        );
        assert_eq!(count(&c, "SELECT count(*) FROM ratings"), 3);
        assert_eq!(count(&c, "SELECT count(*) FROM media_groups"), 1);
    }

    #[test]
    fn save_errors_when_database_unavailable() {
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("a.mp4");
        let meta = mk(&[("title", "T")]);
        let found = vec![("tmdb".to_string(), meta.clone())];
        let cfg = StoreCfg {
            enabled: true,
            database: d.path().join("db/media.db").to_string_lossy().into(),
            fallback_suffix: ".jack.json".into(),
            busy_timeout_secs: 1,
            ..Default::default()
        };
        assert!(
            save(
                &m,
                &meta,
                &found,
                &[],
                &cfg,
                &larry::fingerprint::NoFingerprint,
                &Default::default()
            )
            .unwrap()
            .starts_with("database: media #1")
        );
        assert!(!d.path().join("a.mp4.jack.json").exists());
        // unopenable database (its folder is a file): Jack writes the sidecar, save itself errors
        std::fs::write(d.path().join("blocker"), "").unwrap();
        let bad = StoreCfg {
            database: d.path().join("blocker/media.db").to_string_lossy().into(),
            ..cfg
        };
        let e = save(
            &m,
            &meta,
            &found,
            &[],
            &bad,
            &larry::fingerprint::NoFingerprint,
            &Default::default(),
        )
        .unwrap_err();
        assert!(e.contains("no database"), "{e}");
        assert!(!d.path().join("a.mp4.jack.json").exists());
    }

    #[test]
    fn moved_file_is_refound_by_hash_and_versions_share_a_work() {
        let d = tempfile::tempdir().unwrap();
        let mut c = windups::db::open(&d.path().join("m.db")).unwrap();
        let movie = |extra: &[(&str, &str)]| {
            let mut m = mk(&[
                ("kind", "movie"),
                ("title", "Heat"),
                ("year", "1995"),
                ("lifecycle", "unverified"),
            ]);
            m.extend(mk(extra));
            m
        };
        let old = d.path().join("old.mkv");
        let a = record(&mut c, &old, &movie(&[("file_hash", "b3:aa")]), &[]).unwrap();
        // renamed: the old path is gone, same content -> same entry, path moves
        let new = d.path().join("new.mkv");
        std::fs::write(&new, b"x").unwrap();
        let b = record(&mut c, &new, &movie(&[("file_hash", "b3:aa")]), &[]).unwrap();
        assert_eq!(a, b);
        assert_eq!(count(&c, "SELECT count(*) FROM files"), 1);
        // a different encode of the same work: its own entry, same work key
        let other = record(
            &mut c,
            &d.path().join("heat-720p.mkv"),
            &movie(&[("file_hash", "b3:bb")]),
            &[],
        )
        .unwrap();
        assert_ne!(other, a);
        assert_eq!(
            count(
                &c,
                "SELECT count(DISTINCT work_id) FROM media WHERE work_id IS NOT NULL"
            ),
            1
        );
        assert_eq!(
            count(&c, "SELECT count(*) FROM media WHERE work_id IS NOT NULL"),
            2
        );
    }
}
