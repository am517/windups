//! Where a windup lives. A windup folder holds its binary, one database (`<name>.db`), a TOML
//! config (`config.toml`) and its own `backups/`; a windup is responsible only for its own folder.
//! Paths resolve in this order:
//! 1. an explicit `--config PATH` (the folder is that file's folder);
//! 2. the `<NAME>_HOME` environment variable (e.g. `JACK_HOME`);
//! 3. the binary's own folder, when it already holds `config.toml` or `<name>.db`;
//! 4. `~/windups/<name>/` when it exists;
//! 5. the legacy split install (`~/.config/windups/<name>/`) when it exists and 4 doesn't;
//! 6. otherwise `~/windups/<name>/`, the default (first run creates it: see `install`).
use std::path::{Path, PathBuf};

pub const CONFIG_NAME: &str = "config.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Explicit,
    Env,
    BinaryFolder,
    /// `~/windups/<name>/` (existing, or the default for a new windup).
    Home,
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
    let at = |dir: PathBuf, config: PathBuf, source| Layout {
        name: name.into(),
        db: dir.join(&db_name),
        backups: dir.join("backups"),
        dir,
        config,
        source,
    };
    let holds = |d: &Path| d.join(CONFIG_NAME).is_file() || d.join(&db_name).is_file();
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
        if holds(d) {
            return at(d.clone(), d.join(CONFIG_NAME), Source::BinaryFolder);
        }
    }
    let own = home.join("windups").join(name);
    let legacy = home.join(".config/windups").join(name);
    if !own.is_dir() && holds(&legacy) {
        return at(legacy.clone(), legacy.join(CONFIG_NAME), Source::Legacy);
    }
    at(own.clone(), own.join(CONFIG_NAME), Source::Home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_explicit_env_binary_folder_home_legacy() {
        let d = tempfile::tempdir().unwrap();
        let h = d.path().join("home");
        let bin = d.path().join("usr/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let mut p = Probe {
            exe_dir: Some(bin.clone()),
            home: Some(h.clone()),
            ..Default::default()
        };
        let l = resolve("jack", &p);
        assert_eq!(
            l.source,
            Source::Home,
            "a new windup defaults to ~/windups/<name>/"
        );
        assert_eq!(l.config, h.join("windups/jack/config.toml"));
        assert_eq!(l.db, h.join("windups/jack/jack.db"));
        assert_eq!(
            l.backups,
            h.join("windups/jack/backups"),
            "backups live in the folder"
        );

        let legacy = h.join(".config/windups/jack");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("config.toml"), b"").unwrap();
        let l = resolve("jack", &p);
        assert_eq!((l.source, l.db), (Source::Legacy, legacy.join("jack.db")));
        assert_eq!(l.backups, legacy.join("backups"));

        std::fs::create_dir_all(h.join("windups/jack")).unwrap();
        assert_eq!(
            resolve("jack", &p).source,
            Source::Home,
            "~/windups wins over ~/.config"
        );

        std::fs::write(bin.join("jack.db"), b"").unwrap();
        let l = resolve("jack", &p);
        assert_eq!(
            (l.source, l.db),
            (Source::BinaryFolder, bin.join("jack.db"))
        );

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
