//! `daniel ingest` (alias `accept`): load versioned handoff JSON and store it in media.db.
use crate::{Store, StoreCfg};
use std::fmt::Write;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use windups::handoff::{Daniel, Handoff};

pub fn usage() -> &'static str {
    "usage: daniel ingest [--db PATH] <file.json ... | ->\n       daniel accept  [--db PATH] <file.json ... | ->\n       daniel --capabilities"
}

/// Parse CLI args after `ingest`/`accept`. Usage errors are `Err`; per-file failures stay in the report.
pub fn run(args: &[String]) -> Result<(String, usize), String> {
    let (cfg, inputs) = parse(args)?;
    if inputs.is_empty() {
        return Err(usage().into());
    }
    let store = Store::new(&cfg);
    let mut out = String::new();
    let mut failed = 0;
    for input in &inputs {
        match ingest_one(&store, input) {
            Ok(lines) => {
                for line in lines {
                    let _ = writeln!(out, "{line}");
                }
            }
            Err(e) => {
                failed += 1;
                let _ = writeln!(out, "{e}");
            }
        }
    }
    Ok((out, failed))
}

fn parse(args: &[String]) -> Result<(StoreCfg, Vec<Input>), String> {
    let mut db = String::new();
    let mut inputs = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--db" => {
                db = it
                    .next()
                    .cloned()
                    .ok_or_else(|| "--db needs a path".to_string())?;
            }
            "-h" | "--help" => return Err(usage().into()),
            "-" => inputs.push(Input::Stdin),
            s if s.starts_with('-') => return Err(format!("unknown argument: {s}\n{}", usage())),
            s => inputs.push(Input::File(PathBuf::from(s))),
        }
    }
    Ok((
        StoreCfg {
            enabled: true,
            database: db,
            fallback_suffix: ".jack.json".into(),
            busy_timeout_secs: 5,
            ..Default::default()
        },
        inputs,
    ))
}

enum Input {
    File(PathBuf),
    Stdin,
}

fn ingest_one(store: &Store<'_>, input: &Input) -> Result<Vec<String>, String> {
    let (label, handoffs) = match input {
        Input::File(p) => (p.display().to_string(), load_path(p)?),
        Input::Stdin => ("-".into(), load_stdin()?),
    };
    let mut lines = Vec::with_capacity(handoffs.len());
    for h in &handoffs {
        match store.accept(h) {
            Ok(r) => lines.push(format!("{label}: {r}")),
            Err(e) => return Err(format!("{label}: {e}")),
        }
    }
    Ok(lines)
}

fn load_path(p: &Path) -> Result<Vec<Handoff>, String> {
    let f = File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    Handoff::read_json_all(BufReader::new(f)).map_err(|e| format!("{}: {e}", p.display()))
}

fn load_stdin() -> Result<Vec<Handoff>, String> {
    Handoff::read_json_all(BufReader::new(std::io::stdin())).map_err(|e| format!("-: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_db_and_files_and_stdin() {
        let (cfg, inputs) = parse(&[
            "--db".into(),
            "/tmp/m.db".into(),
            "a.json".into(),
            "-".into(),
        ])
        .unwrap();
        assert_eq!(cfg.database, "/tmp/m.db");
        assert_eq!(inputs.len(), 2);
        assert!(parse(&["--nope".into()]).is_err());
        assert!(parse(&["--db".into()]).is_err());
    }
}
