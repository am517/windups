//! `daniel find [--catalog NAME]... [--limit N] [--db PATH] [-j] WORDS...`: parse, search, print.
use crate::catalog::{self, Catalog};
use crate::config::{Config, FindCfg};
use crate::find::{self, Hit, Skipped};
use std::fmt::Write;
use std::path::PathBuf;

pub fn usage() -> &'static str {
    "usage: daniel find [--catalog NAME]... [--limit N] [--db PATH] [-j] WORDS..."
}

#[derive(Debug, PartialEq)]
pub struct Request {
    pub catalogs: Vec<Catalog>,
    pub limit: usize,
    pub media_db: String,
    pub json: bool,
    pub words: String,
}

/// Catalogs default to `[find] catalogs`; personal records only when named with `--catalog`.
pub fn parse(args: &[String], cfg: &FindCfg) -> Result<Request, String> {
    let mut r = Request {
        catalogs: Vec::new(),
        limit: cfg.limit,
        media_db: String::new(),
        json: false,
        words: String::new(),
    };
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or(format!("{a} needs a value\n{}", usage()))
        };
        match a.as_str() {
            "--catalog" => {
                let v = val()?;
                let c = Catalog::parse(&v).ok_or(format!(
                    "unknown catalog {v}: use media, people, records, maps or photos"
                ))?;
                if !r.catalogs.contains(&c) {
                    r.catalogs.push(c);
                }
            }
            "--limit" => {
                let v = val()?;
                r.limit = v
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or(format!("--limit needs a number above 0, not {v}"))?;
            }
            "--db" => r.media_db = val()?,
            "-j" | "--json" => r.json = true,
            s if s.starts_with('-') => return Err(format!("unknown argument: {s}\n{}", usage())),
            s => words.push(s.to_string()),
        }
    }
    if r.catalogs.is_empty() {
        r.catalogs = cfg
            .catalogs
            .iter()
            .filter_map(|n| Catalog::parse(n))
            .filter(|c| *c != Catalog::Records)
            .collect();
    }
    r.words = words.join(" ");
    if find::fts_query(&r.words).is_none() {
        return Err(format!("nothing to find\n{}", usage()));
    }
    Ok(r)
}

/// Runs the search; returns the text to print (human or JSON).
pub fn run(args: &[String]) -> Result<String, String> {
    let cfg = Config::load(Some(&windups::paths::windup_config("daniel")))?;
    let r = parse(args, &cfg.find)?;
    let media = if r.media_db.is_empty() {
        windups::Settings::load()?.database_path()
    } else {
        PathBuf::from(&r.media_db)
    };
    let dir = catalog::daniel_dir();
    let q = find::fts_query(&r.words).ok_or_else(|| usage().to_string())?;
    let (mut lists, mut skipped) = (Vec::new(), Vec::new());
    for c in &r.catalogs {
        let path = catalog::locate(*c, &cfg.catalogs, &dir, &media);
        let found = find::open_read_only(&path).and_then(|conn| match conn {
            None => Err("no catalog yet".to_string()),
            Some(conn) => find::in_catalog(&conn, *c, &q, r.limit),
        });
        match found {
            Ok(h) => lists.push(h),
            Err(reason) => skipped.push(Skipped {
                catalog: c.name(),
                reason,
            }),
        }
    }
    let hits = find::merge(lists, r.limit);
    Ok(if r.json {
        json(&r, &hits, &skipped)
    } else {
        human(&r, &hits, &skipped)
    })
}

pub fn human(r: &Request, hits: &[Hit], skipped: &[Skipped]) -> String {
    let names: Vec<&str> = r.catalogs.iter().map(|c| c.name()).collect();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} for \"{}\" in {} (at most {}):",
        match hits.len() {
            0 => "Nothing found".to_string(),
            1 => "1 candidate".to_string(),
            n => format!("{n} candidates"),
        },
        r.words,
        names.join(", "),
        r.limit
    );
    for (i, h) in hits.iter().enumerate() {
        let at = h
            .locator
            .as_deref()
            .map_or(String::new(), |l| format!(" [{l}]"));
        let _ = writeln!(
            out,
            "  {}. {:<7} {} #{}{at}: {}",
            i + 1,
            h.catalog,
            h.kind,
            h.id,
            h.text
        );
    }
    for s in skipped {
        let _ = writeln!(out, "  skipped {}: {}", s.catalog, s.reason);
    }
    out
}

pub fn json(r: &Request, hits: &[Hit], skipped: &[Skipped]) -> String {
    let candidates: Vec<serde_json::Value> = hits
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let mut v = serde_json::to_value(h).unwrap_or_default();
            v["n"] = (i + 1).into();
            v
        })
        .collect();
    let names: Vec<&str> = r.catalogs.iter().map(|c| c.name()).collect();
    serde_json::json!({
        "v": 1,
        "ok": true,
        "query": r.words,
        "catalogs": names,
        "limit": r.limit,
        "candidates": candidates,
        "skipped": skipped,
    })
    .to_string()
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> FindCfg {
        FindCfg {
            limit: 5,
            catalogs: ["media", "people", "records", "maps", "photos"]
                .map(String::from)
                .to_vec(),
        }
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn personal_records_are_searched_only_when_named() {
        let r = parse(&args(&["kyoto"]), &cfg()).unwrap();
        assert!(
            !r.catalogs.contains(&Catalog::Records),
            "even if config lists it"
        );
        let r = parse(&args(&["--catalog", "records", "tax"]), &cfg()).unwrap();
        assert_eq!(r.catalogs, [Catalog::Records]);
    }

    #[test]
    fn limit_and_json_are_parsed_and_bad_input_is_explained() {
        let r = parse(&args(&["--limit", "2", "-j", "a", "b"]), &cfg()).unwrap();
        assert_eq!((r.limit, r.json, r.words.as_str()), (2, true, "a b"));
        assert!(
            parse(&args(&["--limit", "0", "a"]), &cfg())
                .unwrap_err()
                .contains("above 0")
        );
        assert!(
            parse(&args(&["--catalog", "films", "a"]), &cfg())
                .unwrap_err()
                .contains("unknown catalog")
        );
        assert!(
            parse(&args(&[]), &cfg())
                .unwrap_err()
                .starts_with("nothing to find")
        );
    }

    #[test]
    fn output_says_what_was_found_and_what_was_skipped() {
        let r = parse(&args(&["kyoto"]), &cfg()).unwrap();
        let hits = [Hit {
            catalog: "maps",
            kind: "place",
            id: 1,
            record_id: Some(1),
            locator: Some("35.01160,135.76810".into()),
            text: "Kyoto Station".into(),
        }];
        let skipped = [Skipped {
            catalog: "photos",
            reason: "no catalog yet".into(),
        }];
        let h = human(&r, &hits, &skipped);
        assert!(
            h.starts_with("1 candidate for \"kyoto\" in media, people, maps, photos (at most 5):"),
            "{h}"
        );
        assert!(
            h.contains("  1. maps    place #1 [35.01160,135.76810]: Kyoto Station"),
            "{h}"
        );
        assert!(h.contains("skipped photos: no catalog yet"), "{h}");
        let j: serde_json::Value = serde_json::from_str(&json(&r, &hits, &skipped)).unwrap();
        assert_eq!(j["candidates"][0]["n"], 1);
        assert_eq!(j["candidates"][0]["catalog"], "maps");
        assert_eq!(j["skipped"][0]["reason"], "no catalog yet");
        assert!(human(&r, &[], &[]).starts_with("Nothing found for"));
    }
}
