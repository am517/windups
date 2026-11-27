//! `daniel connected [--id N] [--depth N] [--catalog NAME]... [--limit N] [-j] NAME`: parse, look up, print.
use crate::catalog::{self, Catalog};
use crate::config::{Config, ConnectedCfg};
use crate::connected::{self, Entity, Linked, Mention};
use std::fmt::Write;

pub fn usage() -> &'static str {
    "usage: daniel connected [--id N] [--depth N] [--catalog NAME]... [--limit N] [-j] NAME"
}

#[derive(Debug, PartialEq)]
pub struct Request {
    pub id: Option<i64>,
    pub name: String,
    pub depth: u32,
    pub catalogs: Vec<Catalog>,
    pub limit: usize,
    pub json: bool,
}

/// Mentions come from `[connected] catalogs`; personal records only when named with `--catalog`.
pub fn parse(args: &[String], cfg: &ConnectedCfg) -> Result<Request, String> {
    let mut r = Request {
        id: None,
        name: String::new(),
        depth: cfg.depth,
        catalogs: Vec::new(),
        limit: cfg.limit,
        json: false,
    };
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or(format!("{a} needs a value\n{}", usage()))
        };
        let number = |v: String| -> Result<u64, String> {
            v.parse()
                .map_err(|_| format!("{a} needs a number, not {v}"))
        };
        match a.as_str() {
            "--id" => r.id = Some(number(val()?)? as i64),
            "--depth" => r.depth = number(val()?)? as u32,
            "--limit" => {
                r.limit = number(val()?)? as usize;
                if r.limit == 0 {
                    return Err("--limit needs a number above 0".into());
                }
            }
            "--catalog" => {
                let v = val()?;
                let c = Catalog::parse(&v).ok_or(format!(
                    "unknown catalog {v}: use media, people, records, maps or photos"
                ))?;
                if !r.catalogs.contains(&c) {
                    r.catalogs.push(c);
                }
            }
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
    r.name = words.join(" ");
    if r.id.is_none() && r.name.trim().is_empty() {
        return Err(format!("say who or what\n{}", usage()));
    }
    Ok(r)
}

/// What the lookup came to.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    NotFound,
    /// Several entities have that name: the person picks one with --id.
    Several(Vec<Entity>),
    Found {
        entity: Entity,
        linked: Vec<Linked>,
        mentions: Vec<Mention>,
    },
}

pub fn look_up(people: &rusqlite::Connection, r: &Request) -> Result<Outcome, String> {
    let start = match r.id {
        Some(id) => match connected::entity(people, id)? {
            Some(e) => e,
            None => return Ok(Outcome::NotFound),
        },
        None => {
            let mut found = connected::resolve(people, &r.name)?;
            match found.len() {
                0 => return Ok(Outcome::NotFound),
                1 => found.remove(0),
                _ => return Ok(Outcome::Several(found)),
            }
        }
    };
    let linked = connected::neighbors(people, start.id, r.depth, r.limit)?;
    let mentions = connected::mentions_of(people, &[start.id], &r.catalogs, r.limit)?;
    Ok(Outcome::Found {
        entity: start,
        linked,
        mentions,
    })
}

pub fn run(args: &[String]) -> Result<String, String> {
    let cfg = Config::load(Some(&windups::paths::windup_config("daniel")))?;
    let r = parse(args, &cfg.connected)?;
    let dir = catalog::daniel_dir();
    let path = catalog::locate(Catalog::People, &cfg.catalogs, &dir, &dir.join("media.db"));
    let Some(people) = crate::find::open_read_only(&path)? else {
        return Ok(if r.json {
            serde_json::json!({"v": 1, "ok": false, "reason": "no_people_catalog"}).to_string()
                + "\n"
        } else {
            "There is no people catalog yet, so nothing is connected to anything.\n".into()
        });
    };
    let o = look_up(&people, &r)?;
    Ok(if r.json { json(&o) } else { human(&r, &o) })
}

pub fn human(r: &Request, o: &Outcome) -> String {
    let who = r.id.map_or(format!("\"{}\"", r.name), |i| format!("#{i}"));
    let mut out = String::new();
    match o {
        Outcome::NotFound => {
            let _ = writeln!(out, "No entity is named {who}.");
        }
        Outcome::Several(list) => {
            let _ = writeln!(
                out,
                "{} entities are named {who}; I won't guess. Pick one with --id:",
                list.len()
            );
            for e in list {
                let _ = writeln!(out, "  --id {}  {} ({})", e.id, e.name, e.kind);
            }
        }
        Outcome::Found {
            entity,
            linked,
            mentions,
        } => {
            let _ = writeln!(
                out,
                "{} ({}, #{}), within {} hops, {} at most:",
                entity.name, entity.kind, entity.id, r.depth, r.limit
            );
            if linked.is_empty() {
                let _ = writeln!(out, "  no related entities");
            }
            for l in linked {
                let _ = writeln!(
                    out,
                    "  {} hop(s): {} ({}, #{}) via {}",
                    l.hops, l.entity.name, l.entity.kind, l.entity.id, l.rel
                );
            }
            let names: Vec<&str> = r.catalogs.iter().map(|c| c.name()).collect();
            if mentions.is_empty() {
                let _ = writeln!(out, "  no mentions in {}", names.join(", "));
            }
            for m in mentions {
                let at = m
                    .locator
                    .as_deref()
                    .map_or(String::new(), |l| format!(" [{l}]"));
                let review = if m.needs_review {
                    " (needs review)"
                } else {
                    ""
                };
                let _ = writeln!(
                    out,
                    "  mentioned in {} #{}{at} as \"{}\"{review}",
                    m.catalog, m.record_id, m.surface
                );
            }
        }
    }
    out
}

pub fn json(o: &Outcome) -> String {
    let v = match o {
        Outcome::NotFound => serde_json::json!({"v": 1, "ok": false, "reason": "not_found"}),
        Outcome::Several(list) => {
            serde_json::json!({"v": 1, "ok": false, "reason": "several_match", "candidates": list})
        }
        Outcome::Found {
            entity,
            linked,
            mentions,
        } => {
            serde_json::json!({"v": 1, "ok": true, "entity": entity, "linked": linked, "mentions": mentions})
        }
    };
    v.to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ConnectedCfg {
        ConnectedCfg {
            depth: 2,
            limit: 5,
            catalogs: ["media", "records", "photos"].map(String::from).to_vec(),
        }
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn records_are_left_out_unless_named_and_bad_input_is_explained() {
        let r = parse(&args(&["Ada", "Lovelace"]), &cfg()).unwrap();
        assert_eq!(r.name, "Ada Lovelace");
        assert_eq!(r.catalogs, [Catalog::Media, Catalog::Photos]);
        let r = parse(&args(&["--id", "3", "--catalog", "records"]), &cfg()).unwrap();
        assert_eq!(
            (r.id, r.catalogs.as_slice()),
            (Some(3), &[Catalog::Records][..])
        );
        assert!(
            parse(&args(&[]), &cfg())
                .unwrap_err()
                .starts_with("say who or what")
        );
        assert!(
            parse(&args(&["--depth", "x", "a"]), &cfg())
                .unwrap_err()
                .contains("needs a number")
        );
    }

    #[test]
    fn several_matches_are_listed_for_a_pick_never_guessed() {
        let d = tempfile::tempdir().unwrap();
        let c = catalog::open(Catalog::People, &d.path().join("people.db")).unwrap();
        c.execute_batch(
            "INSERT INTO people_entities(kind, name) VALUES ('person', 'Ada'), ('place', 'Ada');
             INSERT INTO people_relationships(from_id, to_id, rel, source) VALUES (1, 2, 'named_after', 'test');",
        )
        .unwrap();
        let r = parse(&args(&["ada"]), &cfg()).unwrap();
        let o = look_up(&c, &r).unwrap();
        let h = human(&r, &o);
        assert!(
            h.starts_with("2 entities are named \"ada\"; I won't guess."),
            "{h}"
        );
        assert!(h.contains("--id 2  Ada (place)"), "{h}");
        assert!(json(&o).contains("\"reason\":\"several_match\""));
        let r = parse(&args(&["--id", "1"]), &cfg()).unwrap();
        let h = human(&r, &look_up(&c, &r).unwrap());
        assert!(
            h.contains("1 hop(s): Ada (place, #2) via named_after"),
            "{h}"
        );
        assert!(h.contains("no mentions in media, photos"), "{h}");
        let r = parse(&args(&["nobody"]), &cfg()).unwrap();
        assert_eq!(look_up(&c, &r).unwrap(), Outcome::NotFound);
    }
}
