//! Handling one media file: read it, look it up, and say what it is.
use crate::config::{Config, Settings};
use crate::meta::Meta;
use crate::providers::{self, Http};
use crate::{online, tools};
use crate::{print::*, scan::*};
use std::fmt::Write as _;
use std::path::Path;

pub(crate) fn process(
    path: &Path,
    s: &Settings,
    cfg: &Config,
    http: &dyn Http,
    out: &mut String,
    pr: &mut Printed,
    hk: &mut crate::settle::Hooks,
) -> Result<(), String> {
    use crate::escalate::Step;
    let file = path.to_string_lossy().into_owned();
    let dry = Settings::flag(s.dry_run);
    let print = Settings::flag(s.print);
    // the label first: the file's own metadata, kept as a source of its own
    let own = if cfg.embedded.enabled {
        crate::filemeta::read(path, &cfg.online.ffprobe, &cfg.embedded.tags)
    } else {
        Meta::new()
    };
    if !own.is_empty() {
        let _ = writeln!(
            out,
            "    embedded: {}",
            own.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    // look in jack.db before searching
    let guessed = query(path, s, cfg).ok();
    // a user's id outranks the file's tags: they don't pick the jack.db record either
    let no_tags = Meta::new();
    let ids = crate::reuse::known_ids(
        guessed
            .as_ref()
            .unwrap_or(&crate::providers::Query::default()),
        if s.has_id() { &no_tags } else { &own },
    );
    let dbp = std::path::Path::new(&cfg.archive.database);
    let opened = if cfg.archive.database == ":memory:" || dbp.exists() {
        Some(crate::archive::open(&cfg.archive))
    } else {
        None
    };
    if let Some(Err(e)) = &opened {
        let _ = writeln!(out, "    warning: archive: {e}");
    }
    let plan = match &opened {
        Some(Ok(a)) => crate::reuse::plan(a, path, &ids, &cfg.providers.order, &cfg.completeness)?,
        _ => crate::reuse::Plan::Fresh,
    };
    let note = crate::reuse::report(&plan);
    if !note.is_empty() {
        let _ = writeln!(out, "    {note}");
    }
    // an online video is fetched by its own site id: that id identifies the video
    let mut label = online_label(path, s, cfg);
    if let Some(item) = label.as_mut()
        && item.info.is_empty()
        && tools::installed("exiftool")
    {
        match online::exif::read(path) {
            online::exif::Read::Meta(read) if !read.is_empty() => {
                let _ = writeln!(out, "    exiftool: read metadata from the file");
                item.info.extend(read);
            }
            online::exif::Read::Failed(message) => {
                let _ = writeln!(out, "    warning: {message}");
            }
            online::exif::Read::Missing(message) => {
                let _ = writeln!(out, "    {message}");
            }
            online::exif::Read::Meta(_) => {}
        }
    }
    let label = label;
    let by_site_id = label.is_some();
    let (mut meta, mut found, mut unresolved) = if let crate::reuse::Plan::Skip {
        handoff, ..
    } = &plan
    {
        let u = handoff
            .entry
            .get("resolution")
            .is_some_and(|r| r == "unresolved");
        (handoff.entry.clone(), handoff.candidates.clone(), u)
    } else {
        match label {
            Some(l) => {
                let o = match online::fetch(
                    &l,
                    &show_flags(s),
                    cfg,
                    http,
                    &crate::resolver::Strict {
                        threshold: cfg.matching.fuzzy_threshold,
                    },
                    out,
                ) {
                    Ok(o) => o,
                    Err(e) => {
                        return incomplete(
                            path,
                            Step::Lookup,
                            "source_error",
                            e,
                            &own,
                            s,
                            cfg,
                            hk,
                            out,
                        );
                    }
                };
                (o.meta, o.found, o.unresolved)
            }
            None => {
                let q = match guessed
                    .as_ref()
                    .cloned()
                    .map(Ok)
                    .unwrap_or_else(|| query(path, s, cfg))
                {
                    Ok(q) => q,
                    Err(e) => {
                        return incomplete(
                            path,
                            Step::Parse,
                            "needs_llm_windup",
                            e,
                            &own,
                            s,
                            cfg,
                            hk,
                            out,
                        );
                    }
                };
                let (seed, only): (&[_], Option<&[String]>) = match &plan {
                    crate::reuse::Plan::Fill {
                        handoff, sources, ..
                    } => (handoff.candidates.as_slice(), Some(sources.as_slice())),
                    _ => (&[], None),
                };
                let (m, mut f) = match crate::fetch::fetch_from(&q, cfg, http, out, seed, only) {
                    Ok(x) => x,
                    Err(e) => {
                        let r = if e.starts_with(crate::providers::ID_NOT_FOUND) {
                            "id_not_found"
                        } else if e.contains("no match") || e.starts_with("skipped") {
                            "no_match"
                        } else {
                            "source_error"
                        };
                        return incomplete(path, Step::Lookup, r, e, &own, s, cfg, hk, out);
                    }
                };
                let mut m = m;
                for (_, candidate) in f.iter_mut() {
                    let title = candidate
                        .get("show")
                        .or(candidate.get("title"))
                        .map_or("", |value| value.as_str());
                    let score = crate::similarity::score(&q.title, title);
                    candidate.insert("match_score".into(), format!("{score:.4}"));
                }
                let decision =
                    crate::decide::decide_by(&q, s, &f, &crate::decide::Rules::from(&cfg.matching));
                let score_text = format!("{:.4}", decision.score());
                m.insert("match_score".into(), score_text.clone());
                let recorded: Meta = [("match_score".into(), score_text.clone())].into();
                crate::escalate::apply(
                    &mut m,
                    Step::Match,
                    "jack",
                    &recorded,
                    &format!("similarity {score_text}"),
                );
                if let crate::decide::Decision::Unresolved { reason, detail, .. } = decision {
                    m.insert("resolution".into(), "unresolved".into());
                    m.insert("unresolved_reason".into(), reason.to_string());
                    m.insert("unresolved_detail".into(), detail);
                }
                let u = m.contains_key("resolution");
                (m, f, u)
            }
        }
    };
    if unresolved {
        // deterministic rules could not decide: escalate with everything known
        let (r, d) = (
            meta.get("unresolved_reason").cloned().unwrap_or_default(),
            meta.get("unresolved_detail").cloned().unwrap_or_default(),
        );
        let mut all = found.clone();
        if !own.is_empty() {
            all.push(("embedded".into(), own.clone()));
        }
        if hk.settle(Step::Match, &file, &r, &d, &mut meta, &all, false, out) {
            for k in ["resolution", "unresolved_reason", "unresolved_detail"] {
                meta.remove(k);
            }
            unresolved = false;
        }
    }
    if unresolved && !crate::escalate::sammy_available() && !cfg.windup {
        unresolved = !pick(cfg, &mut meta, &found, out);
    }
    // the user's id outranks the file's tags: tags that disagree with it look wrong
    let user_anchor = s.has_id();
    if user_anchor && !own.is_empty() {
        let bad = crate::facts::suspect(&own, &found);
        if !bad.is_empty() {
            let _ = writeln!(
                out,
                "    file tags look wrong: {}; not used and not counted as a source",
                bad.join(", ")
            );
            meta.insert("suspect_file_tags".into(), bad.join("; "));
        }
    }
    // only an id is proof; the user's --name/--year or channel mapping settles names, not identity
    let id_flag = [
        &s.imdb,
        &s.tmdb,
        &s.thetvdb,
        &s.wikidata,
        &s.wikipedia,
        &s.youtube,
        &s.x,
    ]
    .iter()
    .any(|v| v.as_ref().is_some_and(|v| !v.is_empty()))
        || guessed.as_ref().is_some_and(|q| {
            [&q.imdb, &q.tmdb, &q.thetvdb, &q.wikidata, &q.wikipedia]
                .iter()
                .any(|v| v.as_ref().is_some_and(|v| !v.is_empty()))
        });
    let user_named = s.name.is_some() && s.year.is_some();
    let ev = crate::facts::evidence(&meta, &own, id_flag || by_site_id, user_named);
    if !own.is_empty() {
        if !unresolved && !user_anchor {
            // the file's facts stand unless independent evidence confirms the source means the same thing
            if let Some(e) = ev {
                meta.insert("evidence".into(), e.into());
            }
            let kept = crate::facts::apply(&mut meta, &own, ev);
            for (f, o, v) in &kept {
                let _ = writeln!(
                    out,
                    "    conflict: {f}: file says \"{o}\", source says \"{v}\"; keeping the file's value; recorded"
                );
            }
            if !kept.is_empty() {
                // tagged with the file's values; still open for research unless escalation decides
                let d: Vec<String> = kept
                    .iter()
                    .map(|(f, o, v)| format!("{f}: file \"{o}\" vs source \"{v}\""))
                    .collect();
                let exact = kept
                    .iter()
                    .all(|(f, _, _)| crate::facts::EXACT.contains(&f.as_str()));
                let (step, reason) = if exact {
                    (Step::Conflict, "needs_llm_windup")
                } else {
                    (Step::Refine, "needs_flags")
                };
                let mut all = found.clone();
                all.push(("embedded".into(), own.clone()));
                if !hk.settle(
                    step,
                    &file,
                    reason,
                    &d.join("; "),
                    &mut meta,
                    &all,
                    true,
                    out,
                ) {
                    meta.insert("research_reason".into(), reason.into());
                    meta.insert("research_detail".into(), d.join("; "));
                }
            }
        }
        found.push(("embedded".into(), own));
    }
    // the user's own word is used, but recorded as theirs (Sammy can review it later)
    if matches!(ev, Some("user") | Some("channel")) && !unresolved {
        let asserted: Meta = ["show", "title", "year"]
            .iter()
            .filter_map(|k| meta.get(*k).map(|v| (k.to_string(), v.clone())))
            .collect();
        let how = if ev == Some("user") {
            "--name/--year"
        } else {
            "channel mapping"
        };
        crate::escalate::apply(&mut meta, Step::Match, "user_asserted", &asserted, how);
    }
    let missing = crate::completeness::missing(&meta, &cfg.completeness);
    if !missing.is_empty() && !unresolved {
        meta.insert("missing_fields".into(), missing.join(","));
    }
    let stage = crate::stage::of(
        unresolved,
        crate::facts::verified(ev),
        meta.contains_key("research_reason") || !missing.is_empty(),
    );
    if stage == "incomplete" && !missing.is_empty() {
        let _ = writeln!(out, "    incomplete: missing {}", missing.join(", "));
    }
    meta.insert("lifecycle".into(), stage.into());
    if let Some(i) = s.imdb.as_deref().and_then(providers::imdb_id) {
        meta.entry("imdb_id".into()).or_insert(i);
    }
    let _ = writeln!(out, "    ids: {}", found_ids(&meta, s));
    if !dry && !print {
        match crate::handoff::hand_over(path, &meta, &found, &cfg.archive) {
            Ok(w) => {
                let _ = writeln!(out, "    {w}");
            }
            Err(e) => {
                let _ = writeln!(out, "    warning: store: {e}");
            }
        }
    }
    hk.done.push((file.clone(), meta.clone()));
    if unresolved {
        if crate::escalate::sammy_available() {
            // Sammy rows of the can't-decide table. The live client is a later card.
            let _ = writeln!(
                out,
                "    sammy: I would talk this through, but Sammy is not connected yet"
            );
        }
        let _ = writeln!(
            out,
            "    unresolved: {}",
            meta.get("unresolved_detail")
                .map_or("I couldn't decide", |d| d.as_str())
        );
        if print {
            print_meta(path, s, cfg, &meta, pr);
        }
        return Ok(());
    }
    let title = meta
        .get("show")
        .or(meta.get("title"))
        .map_or("?", |value| value.as_str());
    let year = meta.get("year").map_or("?", |value| value.as_str());
    let score = meta.get("match_score").map_or("?", |value| value.as_str());
    let _ = writeln!(out, "    identified: {title} ({year}), score {score}");
    if print {
        print_meta(path, s, cfg, &meta, pr);
    }
    Ok(())
}

/// Ids in the metadata (else the requested ones).
pub(crate) fn found_ids(m: &Meta, s: &Settings) -> String {
    let v: Vec<String> = [
        "imdb_id",
        "tmdb_id",
        "tmdb_episode_id",
        "thetvdb_id",
        "thetvdb_episode_id",
        "wikidata_id",
    ]
    .iter()
    .filter_map(|k| {
        m.get(*k)
            .map(|v| format!("{}={v}", k.trim_end_matches("_id")))
    })
    .collect();
    if v.is_empty() { ids(s) } else { v.join(" ") }
}

pub(crate) fn ids(s: &Settings) -> String {
    let v: Vec<String> = [
        ("imdb", &s.imdb),
        ("tmdb", &s.tmdb),
        ("thetvdb", &s.thetvdb),
        ("youtube", &s.youtube),
        ("wikidata", &s.wikidata),
        ("wikipedia", &s.wikipedia),
        ("x", &s.x),
        ("name", &s.name),
        ("year", &s.year),
    ]
    .iter()
    .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={v}")))
    .collect();
    if v.is_empty() {
        "-".into()
    } else {
        v.join(" ")
    }
}

/// Online label: flags first, then what the file itself says.
fn online_label(path: &Path, s: &Settings, cfg: &Config) -> Option<online::Label> {
    online::from_flags(s.youtube.as_deref(), s.x.as_deref()).or_else(|| {
        cfg.online
            .enabled
            .then(|| online::detect(path, &cfg.online))
            .flatten()
    })
}

/// Show ids given as flags alongside an online video.
fn show_flags(s: &Settings) -> Meta {
    let mut m = Meta::new();
    for (k, v) in [
        ("tmdb_series_id", &s.tmdb),
        ("thetvdb_series_id", &s.thetvdb),
        ("imdb_id", &s.imdb),
    ] {
        if let Some(v) = v.as_ref().filter(|v| !v.is_empty()) {
            m.insert(k.into(), v.trim_start_matches("/tv/").into());
        }
    }
    m
}

/// Nothing usable came back, or the file could not be read: still kept (marked incomplete),
/// escalated, explained. Never silently dropped. Returns the original error.
#[allow(clippy::too_many_arguments)]
fn incomplete(
    path: &Path,
    step: crate::escalate::Step,
    reason: &str,
    e: String,
    own: &Meta,
    s: &Settings,
    cfg: &Config,
    hk: &mut crate::settle::Hooks,
    out: &mut String,
) -> Result<(), String> {
    let mut m = Meta::new();
    m.insert("kind".into(), "unknown".into());
    let stem = path
        .file_stem()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_default();
    m.insert("title".into(), own.get("title").cloned().unwrap_or(stem));
    m.insert("resolution".into(), "unresolved".into());
    m.insert("unresolved_reason".into(), reason.into());
    m.insert("unresolved_detail".into(), e.clone());
    let found: Vec<(String, Meta)> = if own.is_empty() {
        vec![]
    } else {
        vec![("embedded".into(), own.clone())]
    };
    // a decision here only adds facts; the entry stays incomplete until a later run confirms it
    hk.settle(
        step,
        &path.to_string_lossy(),
        reason,
        &e,
        &mut m,
        &found,
        false,
        out,
    );
    m.insert("completeness".into(), "incomplete".into());
    // file purgatory: only the file's own values, kept under a stable id until identified or merged
    m.insert("lifecycle".into(), crate::stage::UNDETERMINED.into());
    if !Settings::flag(s.dry_run) {
        match crate::handoff::hand_over(path, &m, &found, &cfg.archive) {
            Ok(w) => {
                let _ = writeln!(out, "    kept as incomplete: {w}");
            }
            Err(x) => {
                let _ = writeln!(out, "    warning: store: {x}");
            }
        }
    }
    hk.done.push((path.to_string_lossy().into_owned(), m));
    Ok(())
}

/// Can't decide, no Sammy: list the candidates by source and take the user's picks (typed on a
/// terminal, or `--pick`). Picked candidates are confirming sources; the rest are ignored.
/// True when the picks settle the match.
fn pick(cfg: &Config, meta: &mut Meta, found: &[(String, Meta)], out: &mut String) -> bool {
    let title = meta
        .get("show")
        .or(meta.get("title"))
        .cloned()
        .unwrap_or_default();
    let list = crate::pick::list(&title, found);
    let typed = match &cfg.picks {
        Some(p) => {
            for line in list.lines() {
                let _ = writeln!(out, "    {line}");
            }
            Some(p.clone())
        }
        None if cfg.candidates.ask && crate::offer::person_is_here() => crate::offer::ask(
            &list,
            true,
            &mut || {
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                line
            },
            out,
        ),
        None => {
            // not on a terminal: show the list once so `--pick` can name the numbers
            if !list.is_empty() {
                let _ = writeln!(
                    out,
                    "    candidates (pick with --pick \"N ...\", at most one per source):"
                );
                for line in list.lines() {
                    let _ = writeln!(out, "      {line}");
                }
            }
            None
        }
    };
    let Some(typed) = typed else { return false };
    let picks = match list.parse(&typed) {
        Ok(p) if p.is_empty() => return false,
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(out, "    pick: {e}");
            return false;
        }
    };
    let rows: Vec<(String, Meta)> = picks
        .iter()
        .map(|&i| {
            (
                crate::pick::base(&found[i].0).to_string(),
                found[i].1.clone(),
            )
        })
        .collect();
    match crate::decide::picked(&rows, &crate::decide::Rules::from(&cfg.matching)) {
        crate::decide::Decision::Sure { .. } => {
            let mut m = crate::sorter::merge(&rows, &cfg.merge);
            if let Some(sc) = meta.get("match_score") {
                m.insert("match_score".into(), sc.clone());
            }
            m.insert("evidence".into(), "user".into());
            let names: Vec<&str> = rows.iter().map(|(s, _)| s.as_str()).collect();
            let asserted: Meta = ["show", "title", "year"]
                .iter()
                .filter_map(|k| m.get(*k).map(|v| (k.to_string(), v.clone())))
                .collect();
            crate::escalate::apply(
                &mut m,
                crate::escalate::Step::Match,
                "user_picked",
                &asserted,
                &names.join(", "),
            );
            let _ = writeln!(
                out,
                "    picked: {} (confirmed by your pick)",
                names.join(", ")
            );
            *meta = m;
            true
        }
        crate::decide::Decision::Unresolved { reason, detail, .. } => {
            let _ = writeln!(out, "    pick: {detail}");
            meta.insert("unresolved_reason".into(), reason.into());
            meta.insert("unresolved_detail".into(), detail);
            false
        }
    }
}
