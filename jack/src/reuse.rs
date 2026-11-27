//! Look in jack.db before searching the network. If the file or its identity is already
//! known, only search to fill missing objective data or missing sources.
use crate::archive::Archive;
use crate::completeness::{self, CompletenessCfg};
use crate::meta::Meta;
use crate::objective::is_objective;
use crate::providers::Query;
use std::path::Path;
use windups::handoff::Handoff;

#[derive(Debug)]
pub enum Plan {
    /// Objective data from every configured source is already here; do not search.
    Skip { id: String, handoff: Handoff },
    /// Known, but some sources or objective fields are missing; search only those sources.
    Fill {
        id: String,
        handoff: Handoff,
        sources: Vec<String>,
    },
    /// Not in jack.db; a full search.
    Fresh,
}

fn keys(path: &Path) -> Vec<String> {
    let mut v = vec![path.to_string_lossy().into_owned()];
    if let Ok(c) = path.canonicalize() {
        let s = c.to_string_lossy().into_owned();
        if !v.contains(&s) {
            v.push(s);
        }
    }
    v
}

/// Ids already known from flags or the file's own label, used to find a prior record.
pub fn known_ids(q: &Query, embedded: &Meta) -> Vec<(String, String)> {
    let mut v = Vec::new();
    let mut add = |k: &str, val: Option<&String>| {
        if let Some(s) = val.filter(|s| !s.is_empty()) {
            v.push((k.to_string(), s.clone()));
        }
    };
    add("imdb_id", q.imdb.as_ref().or(embedded.get("imdb_id")));
    add(
        "tmdb_id",
        q.tmdb
            .as_ref()
            .or(embedded.get("tmdb_id"))
            .or(embedded.get("tmdb_series_id")),
    );
    add(
        "thetvdb_id",
        q.thetvdb
            .as_ref()
            .or(embedded.get("thetvdb_id"))
            .or(embedded.get("thetvdb_series_id")),
    );
    v
}

fn source_base(s: &str) -> &str {
    s.split(':').next().unwrap_or(s)
}

fn present_sources(h: &Handoff) -> Vec<String> {
    let mut v: Vec<String> = h
        .candidates
        .iter()
        .map(|(s, _)| source_base(s).to_string())
        .collect();
    v.sort();
    v.dedup();
    v
}

fn missing_sources(h: &Handoff, order: &[String]) -> Vec<String> {
    let have = present_sources(h);
    order
        .iter()
        .filter(|s| !have.iter().any(|h| h == *s))
        .cloned()
        .collect()
}

fn missing_objective(h: &Handoff, cfg: &CompletenessCfg) -> Vec<String> {
    completeness::missing(&h.entry, cfg)
        .into_iter()
        .filter(|f| is_objective(f))
        .collect()
}

/// What to search for this file, given jack.db and the configured source list.
pub fn plan(
    archive: &Archive,
    path: &Path,
    ids: &[(String, String)],
    order: &[String],
    complete: &CompletenessCfg,
) -> Result<Plan, String> {
    let found = archive.by_file(&keys(path))?.or(archive.by_ids(ids)?);
    let Some((id, h)) = found else {
        return Ok(Plan::Fresh);
    };
    let sources = missing_sources(&h, order);
    let fields = missing_objective(&h, complete);
    if sources.is_empty() && fields.is_empty() {
        return Ok(Plan::Skip { id, handoff: h });
    }
    // if only objective fields are missing, still re-ask the sources we already have
    let sources = if sources.is_empty() {
        present_sources(&h)
    } else {
        sources
    };
    Ok(Plan::Fill {
        id,
        handoff: h,
        sources,
    })
}

pub fn report(plan: &Plan) -> String {
    match plan {
        Plan::Skip { id, .. } => format!("already in jack.db ({id}); search skipped"),
        Plan::Fill { id, sources, .. } => {
            format!(
                "already in jack.db ({id}); fetching missing sources: {}",
                sources.join(", ")
            )
        }
        Plan::Fresh => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Archive, ArchiveCfg};
    use crate::completeness::CompletenessCfg;
    use std::collections::BTreeMap;

    fn open(d: &tempfile::TempDir) -> Archive {
        crate::archive::open(&ArchiveCfg {
            database: d.path().join("jack.db").to_string_lossy().into_owned(),
            ..Default::default()
        })
        .unwrap()
    }

    fn cfg() -> CompletenessCfg {
        CompletenessCfg {
            default_required: vec!["title".into(), "year".into()],
            required: BTreeMap::from([("movie".into(), vec!["title".into(), "year".into()])]),
            ..Default::default()
        }
    }

    fn stored(title: &str, year: &str, sources: &[&str]) -> Handoff {
        let mut h = crate::archive::tests::handoff(title);
        h.entry.insert("kind".into(), "movie".into());
        h.entry.insert("year".into(), year.into());
        h.entry.insert("title".into(), title.into());
        h.candidates = sources
            .iter()
            .map(|s| ((*s).into(), crate::archive::tests::handoff(title).entry))
            .collect();
        h
    }

    #[test]
    fn skips_when_known_and_complete_fills_missing_sources() {
        let d = tempfile::tempdir().unwrap();
        let media = d.path().join("The.Matrix.1999.mkv");
        std::fs::write(&media, b"x").unwrap();
        let a = open(&d);
        let mut h = stored("The Matrix", "1999", &["tmdb", "thetvdb"]);
        h.file = media.clone();
        h.entry.insert("imdb_id".into(), "tt0133093".into());
        a.write(&h).unwrap();
        let p = plan(&a, &media, &[], &["tmdb".into(), "thetvdb".into()], &cfg()).unwrap();
        assert!(matches!(p, Plan::Skip { .. }), "complete: {p:?}");
        assert!(report(&p).contains("search skipped"));

        let p = plan(
            &a,
            &media,
            &[],
            &["tmdb".into(), "thetvdb".into(), "omdb".into()],
            &cfg(),
        )
        .unwrap();
        match p {
            Plan::Fill { sources, .. } => assert_eq!(sources, ["omdb"]),
            other => panic!("{other:?}"),
        }

        let other = d.path().join("other.mkv");
        std::fs::write(&other, b"x").unwrap();
        let p = plan(&a, &other, &[], &["tmdb".into()], &cfg()).unwrap();
        assert!(matches!(p, Plan::Fresh));

        // identity (id) finds the same record under a new path
        let p = plan(
            &a,
            &other,
            &[("imdb_id".into(), "tt0133093".into())],
            &["tmdb".into(), "thetvdb".into()],
            &cfg(),
        )
        .unwrap();
        assert!(matches!(p, Plan::Skip { .. }), "by id: {p:?}");
    }
}
