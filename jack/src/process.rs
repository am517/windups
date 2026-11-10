//! Handling one media file: fetch, art, tag, sort.
use crate::config::{Config, Settings, suffixed};
use crate::meta::{self, Meta};
use crate::providers::{self, Http};
use crate::{art::*, fetch::*, print::*, scan::*};
use crate::{askart, online, sorter, tagger};
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
    let (print, dl) = (Settings::flag(s.print), Settings::flag(s.download_only));
    let msuf = s.meta_suffix.as_deref().unwrap_or(".meta.json");
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
    // an online video is fetched by its own site id: that id identifies the video
    let label = online_label(path, s, cfg);
    let by_site_id = label.is_some();
    let (mut meta, mut found, mut unresolved) = match label {
        Some(l) => {
            let o =
                match online::fetch(&l, &show_flags(s), cfg, http, &crate::resolver::Strict, out) {
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
            let q = match query(path, s, cfg) {
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
            let (m, f) = match fetch(&q, cfg, http, out) {
                Ok(x) => x,
                Err(e) => {
                    let r = if e.contains("no match") || e.starts_with("skipped") {
                        "no_match"
                    } else {
                        "source_error"
                    };
                    return incomplete(path, Step::Lookup, r, e, &own, s, cfg, hk, out);
                }
            };
            let mut m = m;
            if let crate::decide::Decision::Unresolved(reason, detail) =
                crate::decide::decide(&q, s, &f, cfg.matching.year_tolerance)
            {
                m.insert("resolution".into(), "unresolved".into());
                m.insert("unresolved_reason".into(), reason.to_string());
                m.insert("unresolved_detail".into(), detail);
            }
            let u = m.contains_key("resolution");
            (m, f, u)
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
    // only an id is proof; the user's --name/--year or channel mapping settles names, not identity
    let id_flag = [&s.imdb, &s.tmdb, &s.thetvdb]
        .iter()
        .any(|v| v.as_ref().is_some_and(|v| !v.is_empty()));
    let user_named = s.name.is_some() && s.year.is_some();
    let ev = crate::facts::evidence(&meta, &own, id_flag || by_site_id, user_named);
    if !own.is_empty() {
        if !unresolved {
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
    let t = &cfg.tagger;
    // art is kept beside the file only with -d; otherwise it's a temp file removed after tagging
    let pp = art_path(path, s, dl);
    if !dry && t.ask_art && (dl || (t.embed_artwork && !print)) {
        let prefs = |k: &str| {
            cfg.merge
                .fields
                .get(k)
                .unwrap_or(&cfg.merge.default)
                .clone()
        };
        let c = askart::candidates(
            &found,
            all_images(&found, cfg, http),
            &art_keys(&meta, s, cfg),
            prefs,
            t.ask_art_max,
        );
        let open = (t.ask_art_open_folder.unwrap_or(cfg!(target_os = "macos"))
            && !t.ask_art_open_command.is_empty())
        .then_some(t.ask_art_open_command.as_str());
        let o = askart::Opts {
            open_cmd: open,
            keep: dl || t.ask_art_keep,
        };
        match askart::ask(path, &pp, &c, http, &o, out) {
            Ok(Some(p)) => {
                meta.insert("poster_file".into(), p.to_string_lossy().into_owned());
            }
            Ok(None) => {}
            Err(e) => {
                let _ = writeln!(out, "    warning: artwork: {e}");
            }
        }
    } else if !dry && (dl || (t.embed_artwork && !print)) {
        // art is optional: never lose the metadata over it
        if let Err(e) = save_poster(&pp, s, cfg, &mut meta, http) {
            let _ = writeln!(out, "    warning: artwork: {e}");
        }
    }
    if let Some(g) = &s.grouping {
        meta.insert("grouping".into(), g.clone());
    }
    let _ = writeln!(out, "    ids: {}", found_ids(&meta, s));
    if !dry && (dl || !print) {
        match crate::handoff::hand_over(path, &meta, &found, &cfg.archive, cfg.emit_json.as_deref())
        {
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
        // stored with every candidate above; a wrong tag is worse than none
        let _ = writeln!(
            out,
            "    unresolved: stored with all candidates; not tagged or sorted"
        );
        if print {
            print_meta(path, s, cfg, &meta, pr);
        }
        return Ok(());
    }
    if print {
        print_meta(path, s, cfg, &meta, pr);
        if !dl {
            return Ok(());
        }
    }
    if dl {
        let mp = suffixed(path, msuf);
        if !dry {
            meta::save(&mp, &meta)?;
        }
        let _ = writeln!(out, "    save metadata: {}", mp.display());
        return Ok(());
    }
    if let Some(i) = s.imdb.as_deref().and_then(providers::imdb_id) {
        meta.entry("imdb_id".into()).or_insert(i);
    }
    let tagged = tag(path, &meta, cfg, dry, out);
    if let Some(p) = meta.get("poster_file") {
        let _ = std::fs::remove_file(p);
    }
    tagged?;
    if s.output.is_some() || cfg.sort.enabled {
        let to = sorter::destination(s.output.as_deref(), path, &meta, &cfg.sort, &cfg.naming);
        let _ = writeln!(out, "    {}: {}", cfg.sort.mode, to.display());
        if !dry {
            sorter::apply(path, &to, &cfg.sort)?;
        }
    }
    Ok(())
}

pub(crate) fn tag(
    path: &Path,
    meta: &Meta,
    cfg: &Config,
    dry: bool,
    out: &mut String,
) -> Result<(), String> {
    let cmd = tagger::command(path, meta, &cfg.tagger)?;
    for c in std::iter::once(cmd).chain(tagger::extra_commands(path, meta, &cfg.tagger)) {
        let _ = writeln!(out, "    tag: {}", tagger::show(&c));
        if dry {
            tagger::discard(&c);
        } else {
            tagger::run(&c)?;
        }
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
        match crate::handoff::hand_over(path, &m, &found, &cfg.archive, cfg.emit_json.as_deref()) {
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
