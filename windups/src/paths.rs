//! Path helpers: `~` expansion and standard locations.
use std::path::PathBuf;

/// Expand a leading `~` to the home directory.
pub fn expand(p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ if p == "~" => std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| p.into()),
        _ => PathBuf::from(p),
    }
}

/// Shared settings file: ~/.config/windups/windups.toml
pub fn shared_config() -> PathBuf {
    expand("~/.config/windups/windups.toml")
}

/// A windup's own config, resolved the same way as `base::layout` (no explicit `--config`).
pub fn windup_config(name: &str) -> PathBuf {
    crate::base::layout::resolve(name, &crate::base::layout::Probe::real(name, None)).config
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_path_unchanged() {
        assert_eq!(expand("/a/b"), PathBuf::from("/a/b"));
    }
    #[test]
    fn tilde_expands() {
        assert!(!expand("~/x").starts_with("~"));
    }
}
