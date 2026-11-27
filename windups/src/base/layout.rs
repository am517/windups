//! Where a windup lives. A windup folder holds its binary, one database (`<name>.db`), a TOML
//! config (`config.toml`) and its own `backups/`. A windup is responsible only for its own folder.
//! Paths resolve in this order:
//! 1. an explicit `--config PATH` (the folder is that file's folder);
//! 2. the `<NAME>_HOME` environment variable (e.g. `JACK_HOME`);
//! 3. the binary's own folder, when it already holds `config.toml` or `<name>.db`;
//! 4. the per-user data folder when it already exists, or the legacy split install
//!    (`~/.config/windups/<name>/`) when that holds a config or database and the data folder does not;
//! 5. otherwise the data folder, which the first run creates (see `install`).
//!
//! The data folder is `~/Library/Application Support/Windups/<name>/` on macOS and
//! `$XDG_DATA_HOME/windups/<name>/` (default `~/.local/share/windups/<name>/`) on Linux.
//! `~/.local/bin/<name>` stays a symlink to the binary inside that folder. Paths that contain a
//! space (Application Support) are built with `Path`, never spliced into an unquoted shell string.
use std::path::{Path, PathBuf};

pub const CONFIG_NAME: &str = "config.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Explicit,
    Env,
    BinaryFolder,
    /// The platform data folder (existing, or the default for a new windup).
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
    /// Home directory used when `source` is [`Source::Home`] or [`Source::Legacy`].
    pub home: Option<PathBuf>,
    /// `"macos"`, `"linux"`, or another `std::env::consts::OS` value.
    pub os: String,
    /// `XDG_DATA_HOME` when set (Linux). Ignored on macOS.
    pub xdg_data_home: Option<PathBuf>,
}

/// Inputs to resolution, so it can be tested without touching the real environment.
pub struct Probe {
    pub explicit: Option<PathBuf>,
    pub env_home: Option<PathBuf>,
    pub exe_dir: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub os: String,
    pub xdg_data_home: Option<PathBuf>,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            explicit: None,
            env_home: None,
            exe_dir: None,
            home: None,
            os: std::env::consts::OS.to_string(),
            xdg_data_home: None,
        }
    }
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
            os: std::env::consts::OS.to_string(),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        }
    }
}

/// Root that holds one folder per windup.
///
/// macOS: `~/Library/Application Support/Windups` (the space is part of the directory name).
/// Other systems: `$XDG_DATA_HOME/windups`, or `~/.local/share/windups` when that variable is unset.
pub fn data_root(home: &Path, os: &str, xdg_data_home: Option<&Path>) -> PathBuf {
    if os == "macos" {
        home.join("Library")
            .join("Application Support")
            .join("Windups")
    } else if let Some(xdg) = xdg_data_home.filter(|p| !p.as_os_str().is_empty()) {
        xdg.join("windups")
    } else {
        home.join(".local").join("share").join("windups")
    }
}

/// `<data root>/<name>/`.
pub fn windup_dir(home: &Path, name: &str, os: &str, xdg_data_home: Option<&Path>) -> PathBuf {
    data_root(home, os, xdg_data_home).join(name)
}

pub fn resolve(name: &str, p: &Probe) -> Layout {
    let db_name = format!("{name}.db");
    let home = p.home.clone().unwrap_or_default();
    let os = if p.os.is_empty() {
        std::env::consts::OS.to_string()
    } else {
        p.os.clone()
    };
    let at = |dir: PathBuf, config: PathBuf, source| Layout {
        name: name.into(),
        db: dir.join(&db_name),
        backups: dir.join("backups"),
        dir,
        config,
        source,
        home: p.home.clone(),
        os: os.clone(),
        xdg_data_home: p.xdg_data_home.clone(),
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
    if let Some(d) = &p.exe_dir
        && holds(d)
    {
        return at(d.clone(), d.join(CONFIG_NAME), Source::BinaryFolder);
    }
    let own = windup_dir(&home, name, &os, p.xdg_data_home.as_deref());
    let legacy = home.join(".config/windups").join(name);
    if !own.is_dir() && holds(&legacy) {
        return at(legacy.clone(), legacy.join(CONFIG_NAME), Source::Legacy);
    }
    at(own.clone(), own.join(CONFIG_NAME), Source::Home)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(home: &Path, os: &str) -> Probe {
        Probe {
            home: Some(home.to_path_buf()),
            os: os.into(),
            ..Default::default()
        }
    }

    #[test]
    fn macos_and_linux_data_dirs() {
        let h = Path::new("/home/ada");
        let mac = resolve("jack", &probe(h, "macos"));
        assert_eq!(mac.source, Source::Home);
        assert_eq!(
            mac.dir,
            h.join("Library/Application Support/Windups/jack"),
            "macOS keeps the space in Application Support"
        );
        assert_eq!(mac.config, mac.dir.join("config.toml"));
        assert_eq!(mac.db, mac.dir.join("jack.db"));
        assert_eq!(mac.backups, mac.dir.join("backups"));
        assert!(
            mac.dir
                .display()
                .to_string()
                .contains("Application Support"),
            "{}",
            mac.dir.display()
        );

        let lin = resolve("jack", &probe(h, "linux"));
        assert_eq!(lin.dir, h.join(".local/share/windups/jack"));
        assert_eq!(lin.db, lin.dir.join("jack.db"));

        let mut xdg = probe(h, "linux");
        xdg.xdg_data_home = Some(PathBuf::from("/xdg"));
        let x = resolve("jack", &xdg);
        assert_eq!(x.dir, PathBuf::from("/xdg/windups/jack"));

        // XDG is a Linux convention; macOS stays in Application Support.
        let mut mac_xdg = probe(h, "macos");
        mac_xdg.xdg_data_home = Some(PathBuf::from("/xdg"));
        assert_eq!(
            resolve("jack", &mac_xdg).dir,
            h.join("Library/Application Support/Windups/jack")
        );
    }

    #[test]
    fn order_explicit_env_binary_folder_home_legacy() {
        let d = tempfile::tempdir().unwrap();
        let h = d.path().join("home");
        let bin = d.path().join("usr/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let mut p = Probe {
            exe_dir: Some(bin.clone()),
            home: Some(h.clone()),
            os: "linux".into(),
            ..Default::default()
        };
        let l = resolve("jack", &p);
        assert_eq!(
            l.source,
            Source::Home,
            "a new windup defaults to the platform data folder"
        );
        assert_eq!(l.config, h.join(".local/share/windups/jack/config.toml"));
        assert_eq!(l.db, h.join(".local/share/windups/jack/jack.db"));
        assert_eq!(
            l.backups,
            h.join(".local/share/windups/jack/backups"),
            "backups live in the folder"
        );

        let legacy = h.join(".config/windups/jack");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("config.toml"), b"").unwrap();
        let l = resolve("jack", &p);
        assert_eq!((l.source, l.db), (Source::Legacy, legacy.join("jack.db")));
        assert_eq!(l.backups, legacy.join("backups"));

        std::fs::create_dir_all(h.join(".local/share/windups/jack")).unwrap();
        assert_eq!(
            resolve("jack", &p).source,
            Source::Home,
            "the data folder wins over ~/.config"
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
