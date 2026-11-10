//! `migrate`: move a windup from the legacy split folder (`~/.config/windups/<name>/`) into its own
//! folder (`~/windups/<name>/`). Only when asked: old folders are never moved automatically.
//!
//! Copy into a staging folder beside the target, verify (SQLite integrity check plus per-table row
//! counts on every database, byte comparison plus hash for every other file), and only then rename
//! the staging folder into place, so the lookup prefers the new folder from then on. The old folder
//! stays, with a `MIGRATED.txt` saying where the data went, when, and the check results. A failed
//! check removes the staging copy and switches nothing.
use rusqlite::{Connection, OpenFlags};
use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const NOTE: &str = "MIGRATED.txt";

pub struct Outcome {
    pub text: String,
    /// The new folder, once switched over (None for a dry run or nothing to do).
    pub switched_to: Option<PathBuf>,
    /// Config paths rewritten in the new copy, and warnings about paths left alone.
    pub notes: Vec<String>,
}

/// (legacy folder, target folder) for windup `name` under `home`.
pub fn folders(name: &str, home: &Path) -> (PathBuf, PathBuf) {
    (
        home.join(".config/windups").join(name),
        home.join("windups").join(name),
    )
}

pub fn run(name: &str, home: &Path, dry_run: bool) -> Result<Outcome, String> {
    run_with(name, home, dry_run, &|_| {})
}

/// `tamper` sees the staging copy before verification (tests use it to make a check fail).
fn run_with(
    name: &str,
    home: &Path,
    dry_run: bool,
    tamper: &dyn Fn(&Path),
) -> Result<Outcome, String> {
    let (old, new) = folders(name, home);
    let nothing = |text: String| {
        Ok(Outcome {
            text,
            switched_to: None,
            notes: vec![],
        })
    };
    if old.join(NOTE).is_file() && new.is_dir() {
        return nothing(format!(
            "already migrated to {} (see {})",
            new.display(),
            old.join(NOTE).display()
        ));
    }
    let entries = list(&old)?;
    if entries.is_empty() {
        return nothing(format!(
            "nothing to migrate: {} is empty or missing",
            old.display()
        ));
    }
    if new.symlink_metadata().is_ok() && !is_empty_dir(&new) {
        return Err(format!(
            "refused: {} already holds data; nothing was copied (merge by hand, or move it aside)",
            new.display()
        ));
    }
    let paths = config_plan(name, home, &old, &new);
    let notes: Vec<String> = paths
        .rewrites
        .iter()
        .map(|r| format!("config.toml: rewrote {r}"))
        .chain(
            paths
                .warnings
                .iter()
                .map(|w| format!("config.toml: warning: {w}")),
        )
        .collect();
    if dry_run {
        let mut t = format!(
            "dry run: would copy {} item(s) from {} to {}, verify, then switch over:\n",
            entries.len(),
            old.display(),
            new.display()
        );
        for e in &entries {
            let _ = writeln!(t, "  {}", e.display());
        }
        for n in &notes {
            let _ = writeln!(t, "  would: {n}");
        }
        return nothing(t);
    }
    let parent = new.parent().ok_or("no parent folder")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let stage = parent.join(format!(".{name}.migrating"));
    if stage.symlink_metadata().is_ok() {
        return Err(format!(
            "refused: {} exists (an interrupted migration?); remove it and try again",
            stage.display()
        ));
    }
    let expected = paths.text.as_deref();
    let result = copy_all(&old, &stage, &entries)
        .and_then(|()| match expected {
            Some(t) => {
                std::fs::write(stage.join(CONFIG), t).map_err(|e| format!("config.toml: {e}"))
            }
            None => Ok(()),
        })
        .and_then(|()| {
            tamper(&stage);
            verify(&old, &stage, &entries, expected)
        });
    let checks = match result {
        Ok(c) => c,
        Err(e) => {
            let cleaned = match std::fs::remove_dir_all(&stage) {
                Ok(()) => "the partial copy was removed".to_string(),
                Err(r) => format!(
                    "could not remove the partial copy {} ({r})",
                    stage.display()
                ),
            };
            return Err(format!(
                "migration failed, nothing switched ({cleaned}; {} untouched): {e}",
                old.display()
            ));
        }
    };
    if new.is_dir() {
        std::fs::remove_dir(&new).map_err(|e| format!("{}: {e}", new.display()))?;
    }
    std::fs::rename(&stage, &new).map_err(|e| format!("switch to {}: {e}", new.display()))?;
    let at = super::events::fmt_utc(super::events::now());
    let mut note = format!(
        "This folder was migrated.\nData went to: {}\nWhen: {at}\nThe windup now uses the new folder; \
         this one is kept as it was. Delete it once you're happy.\n\nChecks (all passed):\n",
        new.display()
    );
    for c in &checks {
        let _ = writeln!(note, "  {c}");
    }
    if !notes.is_empty() {
        note += "\nConfig paths (only the new copy's config changed):\n";
        for n in &notes {
            let _ = writeln!(note, "  {n}");
        }
    }
    let mut text = format!(
        "migrated {} item(s) from {} to {}; all checks passed:\n",
        entries.len(),
        old.display(),
        new.display()
    );
    for c in checks.iter().chain(&notes) {
        let _ = writeln!(text, "  {c}");
    }
    if let Err(e) = std::fs::write(old.join(NOTE), note) {
        let _ = writeln!(
            text,
            "warning: couldn't write {}: {e}",
            old.join(NOTE).display()
        );
    }
    Ok(Outcome {
        text,
        switched_to: Some(new),
        notes,
    })
}

const CONFIG: &str = super::layout::CONFIG_NAME;

/// Rewrites for the old folder's config.toml: its absolute and `~/` spellings move to the new ones.
fn config_plan(name: &str, home: &Path, old: &Path, new: &Path) -> super::relocate::Plan {
    let Ok(text) = std::fs::read_to_string(old.join(CONFIG)) else {
        return super::relocate::Plan::default();
    };
    let tilde = |p: &Path| {
        p.strip_prefix(home)
            .map(|r| format!("~/{}", r.display()))
            .unwrap_or_else(|_| format!("~/.config/windups/{name}"))
    };
    let moves = [
        (old.display().to_string(), new.display().to_string()),
        (tilde(old), tilde(new)),
    ];
    super::relocate::plan(&text, &moves)
}

fn is_empty_dir(p: &Path) -> bool {
    std::fs::read_dir(p).is_ok_and(|mut d| d.next().is_none())
}

/// Every file, folder and link under `root`, relative, parents first. SQLite journals and sockets
/// are left out (a journal is part of its database; a socket is not data).
fn list(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut todo = vec![PathBuf::new()];
    while let Some(rel) = todo.pop() {
        let Ok(rd) = std::fs::read_dir(root.join(&rel)) else {
            continue;
        };
        let mut items: Vec<_> = rd.flatten().collect();
        items.sort_by_key(|e| e.file_name());
        for e in items {
            let r = rel.join(e.file_name());
            let n = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            if n.ends_with("-journal") || n.ends_with("-wal") || n.ends_with("-shm") {
                continue;
            }
            if ft.is_dir() {
                out.push(r.clone());
                todo.push(r);
            } else if ft.is_file() || ft.is_symlink() {
                out.push(r);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn is_sqlite(p: &Path) -> bool {
    let mut head = [0u8; 16];
    std::fs::File::open(p)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok()
        && &head == b"SQLite format 3\0"
}

fn ro(p: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(p, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("{}: {e}", p.display()))
}

fn copy_all(from: &Path, to: &Path, entries: &[PathBuf]) -> Result<(), String> {
    std::fs::create_dir(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for rel in entries {
        let (s, d) = (from.join(rel), to.join(rel));
        let err = |e: &dyn std::fmt::Display| format!("copy {}: {e}", s.display());
        let meta = s.symlink_metadata().map_err(|e| err(&e))?;
        if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&s).map_err(|e| err(&e))?;
            symlink(&target, &d).map_err(|e| err(&e))?;
        } else if meta.is_dir() {
            std::fs::create_dir_all(&d).map_err(|e| err(&e))?;
        } else if is_sqlite(&s) {
            // the online backup API gives a consistent copy even if the windup has it open
            ro(&s)?
                .backup(rusqlite::DatabaseName::Main, &d, None)
                .map_err(|e| err(&e))?;
        } else {
            std::fs::copy(&s, &d).map_err(|e| err(&e))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn symlink(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
fn symlink(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("links aren't supported here"))
}

/// One line per checked item, or the first failure.
/// `config` is the expected rewritten config.toml, when paths in it were rewritten.
fn verify(
    from: &Path,
    to: &Path,
    entries: &[PathBuf],
    config: Option<&str>,
) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for rel in entries {
        let (s, d) = (from.join(rel), to.join(rel));
        let r = rel.display();
        let meta = s.symlink_metadata().map_err(|e| format!("{r}: {e}"))?;
        if meta.file_type().is_symlink() {
            let (a, b) = (std::fs::read_link(&s), std::fs::read_link(&d));
            match (a, b) {
                (Ok(a), Ok(b)) if a == b => lines.push(format!("{r}: link -> {}", a.display())),
                _ => return Err(format!("{r}: the link differs")),
            }
        } else if meta.is_dir() {
            if !d.is_dir() {
                return Err(format!("{r}: folder missing in the copy"));
            }
        } else if is_sqlite(&s) {
            let ok: String = ro(&d)?
                .query_row("PRAGMA integrity_check", [], |x| x.get(0))
                .map_err(|e| format!("{r}: integrity check: {e}"))?;
            if ok != "ok" {
                return Err(format!("{r}: integrity check says {ok}"));
            }
            let (a, b) = (counts(&ro(&s)?)?, counts(&ro(&d)?)?);
            if a != b {
                return Err(format!("{r}: row counts differ (old {a:?}, copy {b:?})"));
            }
            let rows: Vec<String> = a.iter().map(|(t, n)| format!("{t}={n}")).collect();
            lines.push(format!("{r}: integrity ok; rows {}", rows.join(" ")));
        } else if let (Some(want), true) = (config, rel.as_os_str() == CONFIG) {
            let (a, b) = (fnv(want.as_bytes()), hash(&d)?);
            if a != b {
                return Err(format!(
                    "{r}: doesn't match the expected rewrite (hash {a:032x} vs {b:032x})"
                ));
            }
            lines.push(format!(
                "{r}: {} bytes, fnv1a-128 {a:032x} (matches the expected rewrite)",
                want.len()
            ));
        } else {
            let (a, b) = (hash(&s)?, hash(&d)?);
            if a != b {
                return Err(format!("{r}: contents differ (hash {a:032x} vs {b:032x})"));
            }
            lines.push(format!("{r}: {} bytes, fnv1a-128 {a:032x}", meta.len()));
        }
    }
    Ok(lines)
}

fn counts(c: &Connection) -> Result<Vec<(String, i64)>, String> {
    let mut st = c
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .map_err(|e| e.to_string())?;
    let names: Vec<String> = st
        .query_map([], |r| r.get(0))
        .and_then(|m| m.collect())
        .map_err(|e| e.to_string())?;
    names
        .into_iter()
        .map(|t| {
            let q = format!("SELECT count(*) FROM \"{}\"", t.replace('"', "\"\""));
            let n = c
                .query_row(&q, [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            Ok((t, n))
        })
        .collect()
}

const FNV_PRIME: u128 = 0x0000000001000000000000000000013B;
const FNV_START: u128 = 0x6c62272e07bb014262b821756295c58d;

fn fnv_more(mut h: u128, bytes: &[u8]) -> u128 {
    for &b in bytes {
        h = (h ^ u128::from(b)).wrapping_mul(FNV_PRIME);
    }
    h
}

fn fnv(bytes: &[u8]) -> u128 {
    fnv_more(FNV_START, bytes)
}

/// FNV-1a 128, streamed in 64 KiB pieces (low memory; for equality, not security).
fn hash(p: &Path) -> Result<u128, String> {
    let mut h = FNV_START;
    let mut f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut buf = vec![0u8; 64 << 10];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", p.display()))?;
        if n == 0 {
            return Ok(h);
        }
        h = fnv_more(h, &buf[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy(home: &Path) -> PathBuf {
        let (old, _) = folders("toy", home);
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("config.toml"), "[core]\n# keep me\n").unwrap();
        let c = Connection::open(old.join("toy.db")).unwrap();
        c.execute_batch(
            "CREATE TABLE records(id TEXT); INSERT INTO records VALUES ('a'),('b');
             CREATE TABLE events(seq INTEGER PRIMARY KEY, at INTEGER, kind TEXT, detail TEXT);",
        )
        .unwrap();
        old
    }

    fn rows(p: &Path) -> i64 {
        Connection::open(p)
            .unwrap()
            .query_row("SELECT count(*) FROM records", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn copies_verifies_switches_and_leaves_a_note() {
        let d = tempfile::tempdir().unwrap();
        let old = legacy(d.path());
        let dry = run("toy", d.path(), true).unwrap();
        assert!(
            dry.text.starts_with("dry run: would copy 2 item(s)"),
            "{}",
            dry.text
        );
        assert!(
            !d.path().join("windups").exists(),
            "a dry run writes nothing"
        );
        let o = run("toy", d.path(), false).unwrap();
        let new = d.path().join("windups/toy");
        assert_eq!(o.switched_to.as_deref(), Some(new.as_path()));
        assert!(
            o.text
                .contains("toy.db: integrity ok; rows events=0 records=2"),
            "{}",
            o.text
        );
        assert!(
            o.text.contains("config.toml: 17 bytes, fnv1a-128 "),
            "{}",
            o.text
        );
        assert_eq!(rows(&new.join("toy.db")), 2);
        assert_eq!(rows(&old.join("toy.db")), 2, "the old folder stays");
        let note = std::fs::read_to_string(old.join(NOTE)).unwrap();
        assert!(
            note.contains(&format!("Data went to: {}", new.display())),
            "{note}"
        );
        assert!(
            note.contains("When: ") && note.contains("Checks (all passed):"),
            "{note}"
        );
        assert!(!d.path().join("windups/.toy.migrating").exists());
        let l = super::super::layout::resolve(
            "toy",
            &super::super::layout::Probe {
                home: Some(d.path().into()),
                ..Default::default()
            },
        );
        assert_eq!(l.dir, new, "the lookup prefers the new folder now");
        // running it again is harmless
        let again = run("toy", d.path(), false).unwrap();
        assert!(again.text.starts_with("already migrated"), "{}", again.text);
        assert!(again.switched_to.is_none());
    }

    #[test]
    fn a_failed_check_switches_nothing_and_cleans_up() {
        let d = tempfile::tempdir().unwrap();
        let old = legacy(d.path());
        let e = run_with("toy", d.path(), false, &|s: &Path| {
            std::fs::write(s.join("config.toml"), "changed").unwrap();
        })
        .err()
        .unwrap();
        assert!(e.contains("config.toml: contents differ"), "{e}");
        assert!(e.contains("the partial copy was removed"), "{e}");
        assert!(!d.path().join("windups/toy").exists(), "no switch");
        assert!(
            !d.path().join("windups/.toy.migrating").exists(),
            "cleaned up"
        );
        assert!(!old.join(NOTE).exists());
        let e = run_with("toy", d.path(), false, &|s: &Path| {
            let c = Connection::open(s.join("toy.db")).unwrap();
            c.execute("DELETE FROM records WHERE id = 'b'", []).unwrap();
        })
        .err()
        .unwrap();
        assert!(e.contains("toy.db: row counts differ"), "{e}");
        assert!(!d.path().join("windups/toy").exists());
        // and a clean run still works afterwards
        assert!(run("toy", d.path(), false).unwrap().switched_to.is_some());
    }

    #[test]
    fn refuses_when_the_target_already_holds_data() {
        let d = tempfile::tempdir().unwrap();
        let old = legacy(d.path());
        let new = d.path().join("windups/toy");
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("toy.db"), "mine").unwrap();
        for dry in [true, false] {
            let e = run("toy", d.path(), dry).err().unwrap();
            assert!(
                e.starts_with("refused: ") && e.contains("already holds data"),
                "{e}"
            );
        }
        assert_eq!(std::fs::read(new.join("toy.db")).unwrap(), b"mine");
        assert!(!old.join(NOTE).exists());
        // an empty target folder is fine
        std::fs::remove_file(new.join("toy.db")).unwrap();
        assert!(run("toy", d.path(), false).unwrap().switched_to.is_some());
    }

    #[test]
    fn config_paths_into_the_old_folder_are_rewritten_in_the_copy_only() {
        let d = tempfile::tempdir().unwrap();
        let old = legacy(d.path());
        let new = d.path().join("windups/toy");
        let orig = format!(
            "[archive]\ndatabase = \"{}/toy.db\"\nmedia = \"/srv/media\"\n",
            old.display()
        );
        std::fs::write(old.join("config.toml"), &orig).unwrap();
        let dry = run("toy", d.path(), true).unwrap();
        assert!(
            dry.text
                .contains("would: config.toml: rewrote archive.database"),
            "{}",
            dry.text
        );
        let o = run("toy", d.path(), false).unwrap();
        let rw = format!(
            "config.toml: rewrote archive.database: {0}/toy.db -> {1}/toy.db",
            old.display(),
            new.display()
        );
        let warn = "config.toml: warning: archive.media = /srv/media is outside the old folder; left as is";
        assert_eq!(o.notes, [rw.clone(), warn.to_string()]);
        assert!(
            o.text.contains("(matches the expected rewrite)"),
            "{}",
            o.text
        );
        assert_eq!(
            std::fs::read_to_string(old.join("config.toml")).unwrap(),
            orig,
            "original untouched"
        );
        let copy = std::fs::read_to_string(new.join("config.toml")).unwrap();
        assert_eq!(
            copy,
            orig.replace(&old.display().to_string(), &new.display().to_string())
        );
        let note = std::fs::read_to_string(old.join(NOTE)).unwrap();
        assert!(note.contains(&rw) && note.contains(warn), "{note}");
    }

    #[test]
    fn a_config_that_doesnt_match_the_expected_rewrite_fails() {
        let d = tempfile::tempdir().unwrap();
        let old = legacy(d.path());
        std::fs::write(
            old.join("config.toml"),
            "dir = \"~/.config/windups/toy/b\"\n",
        )
        .unwrap();
        let e = run_with("toy", d.path(), false, &|s: &Path| {
            std::fs::write(s.join("config.toml"), "dir = \"~/.config/windups/toy/b\"\n").unwrap();
        })
        .err()
        .unwrap();
        assert!(
            e.contains("config.toml: doesn't match the expected rewrite"),
            "{e}"
        );
        assert!(!d.path().join("windups/toy").exists());
    }

    #[test]
    fn nothing_to_migrate_without_an_old_folder() {
        let d = tempfile::tempdir().unwrap();
        let o = run("toy", d.path(), false).unwrap();
        assert!(o.text.starts_with("nothing to migrate"), "{}", o.text);
    }
}
