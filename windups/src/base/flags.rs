//! Universal CLI flags every windup honors, so a caller can talk to any of them the same way.
//! These are flags, not commands: they modify how a run speaks, and they never prompt.
//!
//! - `--json` / `-j`: the result as JSON on stdout, reasons in their own fields; no prompts or
//!   commentary (windups_base_spec §1). Replaces `--emit-json`, `--print-format json` and sidecars.
//! - `--windup` / `-w`: same output as `-j`; kept only until windups talk over Noise (card #7).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flags {
    /// Called by another windup: same as `json` (until Noise).
    pub windup: bool,
    /// The result as JSON on stdout; no prompts.
    pub json: bool,
}

impl Flags {
    /// Pulls `-w/--windup` and `-j/--json` out of `args`. Other tokens are returned as `rest`.
    /// Lone `-w`/`-j` and the long forms are recognized; unknown tokens are kept in order.
    pub fn take(args: &[String]) -> (Self, Vec<String>) {
        let mut f = Self::default();
        let mut rest = Vec::with_capacity(args.len());
        for a in args {
            match a.as_str() {
                "--windup" | "-w" => f.windup = true,
                "--json" | "-j" => f.json = true,
                _ => rest.push(a.clone()),
            }
        }
        (f, rest)
    }
}

/// A one-line machine-readable result or failure (`--windup` stdout).
pub fn result_json(ok: bool, items: &[serde_json::Value]) -> String {
    serde_json::json!({
        "v": 1,
        "ok": ok,
        "items": items,
    })
    .to_string()
}

/// A whole-run failure when no items were processed (usage or config).
pub fn error_json(error: &str) -> String {
    serde_json::json!({
        "v": 1,
        "ok": false,
        "error": error,
        "items": [],
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn take_pulls_windup_and_json_leaving_the_rest() {
        let (f, rest) = Flags::take(&args(&["movie.mkv", "-w", "--year", "1999", "-j"]));
        assert!(f.windup && f.json);
        assert_eq!(rest, args(&["movie.mkv", "--year", "1999"]));
        let (f, rest) = Flags::take(&args(&["--windup", "--json", "ask", "summaries"]));
        assert!(f.windup && f.json);
        assert_eq!(rest, args(&["ask", "summaries"]));
        let (f, rest) = Flags::take(&args(&["status"]));
        assert!(!f.windup && !f.json && rest == args(&["status"]));
    }

    #[test]
    fn result_and_error_json_are_v1() {
        let j: serde_json::Value = serde_json::from_str(&error_json("no files given")).unwrap();
        assert_eq!(j["v"], 1);
        assert_eq!(j["ok"], false);
        assert_eq!(j["error"], "no files given");
        let item = serde_json::json!({"file": "a.mkv", "ok": true});
        let j: serde_json::Value = serde_json::from_str(&result_json(true, &[item])).unwrap();
        assert!(j["ok"].as_bool().unwrap() && j["items"].as_array().unwrap().len() == 1);
    }
}
