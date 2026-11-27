//! `--print` output.
use crate::config::{Config, Settings};
use crate::meta::Meta;
use std::fmt::Write as _;
use std::path::Path;

/// `--print` output: (text, json entries).
#[derive(Default)]
pub(crate) struct Printed {
    pub(crate) text: String,
    pub(crate) json: Vec<serde_json::Value>,
}

/// With `-j`, the facts go into the JSON result (one entry per file); otherwise as text.
pub(crate) fn print_meta(path: &Path, _s: &Settings, cfg: &Config, meta: &Meta, p: &mut Printed) {
    let m = meta.clone();
    if cfg.windup {
        p.json
            .push(serde_json::json!({"file": path.to_string_lossy(), "metadata": m}));
    } else {
        let _ = writeln!(p.text, "{}", path.display());
        for (k, v) in &m {
            let _ = writeln!(p.text, "  {k}: {}", v.replace('\n', " "));
        }
        p.text.push('\n');
    }
}
