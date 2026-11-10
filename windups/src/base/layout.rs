//! Where a windup lives. A windup folder holds three things: the binary, one database
//! (`<name>.db`) and a TOML config (`config.toml`). Paths resolve in this order:
//! 1. an explicit `--config PATH` (the folder is that file's folder);
//! 2. the `<NAME>_HOME` environment variable (e.g. `JACK_HOME`);
//! 3. the binary's own folder, when it already holds `config.toml` or `<name>.db`;
//! 4. the legacy split install (`~/.config/windups/<name>/`), so today's .deb keeps working.
//!
//! Snapshots go outside the folder (default `~/.local/share/windups/backups/<name>/`) so the
//! folder stays three things.
use std::path::{Path, PathBuf};

pub const CONFIG_NAME: &str = "config.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Explicit,
    Env,
    BinaryFolder,
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub name: String,
    pub dir: PathBuf,
    pub config: PathBuf,
    pub db: PathBuf,
    pub backups: PathBuf,
    pub source: Source,
}

/// Inputs to resolution, so it can be tested without touching the real environment.
#[derive(Default)]
pub struct Probe {
    pub explicit: Option<PathBuf>,
    pub env_home: Option<PathBuf>,
    pub exe_dir: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

impl Probe {
    /// The real environment for windup `name`.
    pub fn real(name: &str, explicit: Option<&Path>) -> Self {
        let env = format!("{}_HOME", name.to_ascii_uppercase());
        Self {
            explicit: explicit.map(|p| crate::paths::expand(&p.to_string_lossy())),
            env_home: std::env::var_os(env)
                .filter(|v| !v.is_empty())
                .map(|v| crate::paths::expand(&v.to_string_lossy())),
            // current_exe follows symlinks on Linux, so a /usr/bin link finds the real folder
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf)),
            home: std::env::var_os("HOME").map(PathBuf::from),
        }
    }
}

pub fn resolve(name: &str, p: &Probe) -> Layout {
    let db_name = format!("{name}.db");
    let home = p.home.clone().unwrap_or_default();
    let backups = home.join(".local/share/windups/backups").join(name);
    let at = |dir: PathBuf, config: PathBuf, source| Layout {
        name: name.into(),
        db: dir.join(&db_name),
        dir,
        config,
        backups: backups.clone(),
        source,
    };
    if let Some(c) = &p.explicit {
        let dir = c
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        return at(dir, c.clone(), Source::Explicit);
    }
    if let Some(d) = &p.env_home {
        return at(d.clone(), d.join(CONFIG_NAME), Source::Env);
    }
    if let Some(d) = &p.exe_dir {
        if d.join(CONFIG_NAME).is_file() || d.join(&db_name).is_file() {
            return at(d.clone(), d.join(CONFIG_NAME), Source::BinaryFolder);
        }
    }
    let dir = home.join(".config/windups").join(name);
    at(dir.clone(), dir.join(CONFIG_NAME), Source::Legacy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_explicit_env_binary_folder_legacy() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("opt/jack");
        std::fs::create_dir_all(&bin).unwrap();
        let mut p = Probe {
            exe_dir: Some(bin.clone()),
            home: Some(d.path().join("home")),
            ..Default::default()
        };
        let l = resolve("jack", &p);
        assert_eq!(
            l.source,
            Source::Legacy,
            "an empty binary folder is not a windup folder"
        );
        assert_eq!(
            l.config,
            d.path().join("home/.config/windups/jack/config.toml")
        );
        assert_eq!(l.db, d.path().join("home/.config/windups/jack/jack.db"));
        assert_eq!(
            l.backups,
            d.path().join("home/.local/share/windups/backups/jack")
        );

        std::fs::write(bin.join("jack.db"), b"").unwrap();
        let l = resolve("jack", &p);
        assert_eq!(
            (l.source, l.db),
            (Source::BinaryFolder, bin.join("jack.db"))
        );
        assert_eq!(l.config, bin.join("config.toml"));

        p.env_home = Some(d.path().join("elsewhere"));
        let l = resolve("jack", &p);
        assert_eq!((l.source, l.dir), (Source::Env, d.path().join("elsewhere")));

        p.explicit = Some(d.path().join("x/my.toml"));
        let l = resolve("jack", &p);
        assert_eq!(l.source, Source::Explicit);
        assert_eq!(
            (l.config, l.db),
            (d.path().join("x/my.toml"), d.path().join("x/jack.db"))
        );
    }
}
