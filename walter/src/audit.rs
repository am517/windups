//! `walter audit`: Walter the auditor reads every windup's database (read-only, through
//! `windups::base::audit`), reports what he finds and the command that fixes it. He never writes
//! to another windup; he only flags.
use std::fmt::Write as _;
use std::path::PathBuf;
use windups::base::audit::{Check, Finding, Standard, audit};

/// (windup name, its database) for every `<name>/<name>.db` under the roots, each once.
pub fn discover(roots: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for d in dirs {
            let Some(name) = d.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
                continue;
            };
            let db = d.join(format!("{name}.db"));
            let key = db.canonicalize().unwrap_or_else(|_| db.clone());
            if db.is_file() && seen.insert(key) {
                out.push((name, db));
            }
        }
    }
    out
}

/// Audits each database; one finding per problem, including a database Walter can't read.
pub fn run(dbs: &[(String, PathBuf)], checks: &[&dyn Check]) -> (Vec<Finding>, Vec<String>) {
    let mut found = Vec::new();
    let mut clean = Vec::new();
    for (name, db) in dbs {
        match audit(name, db, checks) {
            Ok(f) if f.is_empty() => clean.push(name.clone()),
            Ok(f) => found.extend(f),
            Err(e) => found.push(Finding {
                windup: name.clone(),
                what: format!("can't read its database ({e})"),
                fix: format!("{name} health"),
            }),
        }
    }
    (found, clean)
}

pub fn report(roots: &[PathBuf], stale_after_days: u64) -> String {
    let dbs = discover(roots);
    let std = Standard {
        stale_after_days,
        now: windups::base::events::now(),
    };
    let (found, clean) = run(&dbs, &[&std]);
    let mut o = format!(
        "audited {} windup(s): {} finding(s)\n",
        dbs.len(),
        found.len()
    );
    for f in &found {
        let _ = writeln!(o, "  {}: {} -> run: {}", f.windup, f.what, f.fix);
    }
    if !clean.is_empty() {
        let _ = writeln!(o, "  all good: {}", clean.join(", "));
    }
    o
}

pub fn roots(cfg: &crate::config::Config) -> Vec<PathBuf> {
    cfg.audit
        .roots
        .iter()
        .map(|r| windups::paths::expand(r))
        .collect()
}
