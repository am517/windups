//! First run: makes the per-user windup folder `~/windups/<name>/` and the `~/.local/bin/<name>`
//! link, so a system package (which may not write into anyone's home) only has to ship the
//! binary. Idempotent; never replaces anything it didn't make.
use super::layout::{Layout, Source};
use std::path::Path;

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
fn link(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("symlinks need a unix system"))
}

/// Sets up `l` (only for the default `~/windups/<name>/` folder). `exe` is the running binary,
/// `bin_dir` usually `~/.local/bin`, `config` the starter config written if there is none.
/// Returns one line per thing it made or left alone.
pub fn first_run(
    l: &Layout,
    exe: &Path,
    bin_dir: &Path,
    config: &str,
) -> Result<Vec<String>, String> {
    if l.source != Source::Home {
        return Ok(vec![]);
    }
    let e = |p: &Path, x: std::io::Error| format!("{}: {x}", p.display());
    let mut notes = Vec::new();
    if !l.dir.is_dir() {
        std::fs::create_dir_all(&l.dir).map_err(|x| e(&l.dir, x))?;
        notes.push(format!("made {}", l.dir.display()));
    }
    let own = l.dir.join(&l.name);
    let exe = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    if own.symlink_metadata().is_err() && exe != own {
        link(&exe, &own).map_err(|x| e(&own, x))?;
        notes.push(format!("linked {} -> {}", own.display(), exe.display()));
    }
    if !l.config.exists() {
        std::fs::write(&l.config, config).map_err(|x| e(&l.config, x))?;
        notes.push(format!("wrote {}", l.config.display()));
    }
    let on_path = bin_dir.join(&l.name);
    match on_path.symlink_metadata() {
        Err(_) => {
            std::fs::create_dir_all(bin_dir).map_err(|x| e(bin_dir, x))?;
            link(&own, &on_path).map_err(|x| e(&on_path, x))?;
            notes.push(format!("linked {} -> {}", on_path.display(), own.display()));
        }
        Ok(_) if std::fs::read_link(&on_path).is_ok_and(|t| t == own) => {}
        Ok(_) => notes.push(format!("left {} alone (not ours)", on_path.display())),
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::layout::{Probe, resolve};

    #[test]
    fn makes_the_folder_and_links_once_and_never_clobbers() {
        let d = tempfile::tempdir().unwrap();
        let exe = d.path().join("usr/bin/jack");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"#!").unwrap();
        let home = d.path().join("home");
        let l = resolve(
            "jack",
            &Probe {
                home: Some(home.clone()),
                ..Default::default()
            },
        );
        let bin = home.join(".local/bin");
        let n = first_run(&l, &exe, &bin, "# jack\n").unwrap();
        assert_eq!(n.len(), 4, "{n:?}");
        let own = home.join("windups/jack/jack");
        assert_eq!(
            std::fs::read_link(&own).unwrap(),
            exe.canonicalize().unwrap()
        );
        assert_eq!(std::fs::read_link(bin.join("jack")).unwrap(), own);
        assert_eq!(std::fs::read_to_string(&l.config).unwrap(), "# jack\n");
        assert!(
            first_run(&l, &exe, &bin, "x").unwrap().is_empty(),
            "idempotent"
        );
        std::fs::remove_file(bin.join("jack")).unwrap();
        std::fs::write(bin.join("jack"), b"someone else's").unwrap();
        let n = first_run(&l, &exe, &bin, "x").unwrap();
        assert!(n[0].contains("not ours"), "{n:?}");
        assert_eq!(std::fs::read(bin.join("jack")).unwrap(), b"someone else's");
        assert_eq!(std::fs::read_to_string(&l.config).unwrap(), "# jack\n");
    }

    #[test]
    fn only_the_default_folder_is_set_up() {
        let d = tempfile::tempdir().unwrap();
        let l = resolve(
            "jack",
            &Probe {
                env_home: Some(d.path().join("j")),
                home: Some(d.path().into()),
                ..Default::default()
            },
        );
        assert!(
            first_run(&l, Path::new("/x"), d.path(), "")
                .unwrap()
                .is_empty()
        );
        assert!(!d.path().join("j").exists());
    }
}
