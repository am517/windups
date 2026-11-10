//! Jack on the shared core: the universal commands (`jack status`, `jack backup`, `jack talk ...`;
//! see windups/COMMANDS.md) plus his own action, `tag`.
use crate::archive::Archive;
use crate::config::{self, Config};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use windups::base::commands::{self, Action, Command, Hooks, Windup};
use windups::base::layout::Layout;

pub struct Jack {
    pub cfg: Config,
    pub layout: Layout,
    pub archive: Archive,
    config_arg: Option<PathBuf>,
}

/// `jack <universal command> ...`, unless a file or folder of that name is meant.
pub fn is_command(args: &[String]) -> bool {
    commands::parse(args).is_some() && !Path::new(&args[0]).exists()
}

fn split_config(args: &[String]) -> (Option<PathBuf>, Vec<String>) {
    let (mut cfg, mut rest) = (None, Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.strip_prefix("--config") {
            Some("") => cfg = it.next().map(PathBuf::from),
            Some(v) if v.starts_with('=') => cfg = Some(PathBuf::from(&v[1..])),
            _ => rest.push(a.clone()),
        }
    }
    (cfg, rest)
}

impl Jack {
    pub fn open(config_arg: Option<&Path>) -> Result<Self, String> {
        let cfg = config::load(config_arg)?;
        let archive = Archive::open(&cfg.archive)?;
        Ok(Jack {
            layout: config::layout(config_arg),
            cfg,
            archive,
            config_arg: config_arg.map(Path::to_path_buf),
        })
    }
}

/// Sync to the configured target, then keep the event log within its limits. One report line.
pub fn sync_with(a: &mut Archive, cfg: &Config) -> String {
    let line = match crate::sync::connect(&cfg.archive.sync_target, &cfg.store) {
        Ok(mut t) => crate::sync::report(a, t.as_deref_mut(), cfg.archive.sync_batch),
        Err(e) => format!("archive: sync skipped ({e})"),
    };
    let detail = line.clone();
    let _ = windups::base::events::append(a.conn(), "sync", &detail);
    match windups::base::events::compact(a.conn(), &cfg.core.events) {
        Ok(_) => line,
        Err(e) => format!("{line}; event log: {e}"),
    }
}

/// The end-of-run sync (opens jack.db itself).
pub fn sync_line(cfg: &Config) -> String {
    match Archive::open(&cfg.archive) {
        Ok(mut a) => sync_with(&mut a, cfg),
        Err(e) => format!("warning: archive: {e}"),
    }
}

impl Windup for Jack {
    fn name(&self) -> &str {
        "jack"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn layout(&self) -> &Layout {
        &self.layout
    }
    fn core_cfg(&self) -> &windups::base::config::BaseCfg {
        &self.cfg.core
    }
    fn conn(&mut self) -> &mut rusqlite::Connection {
        self.archive.conn()
    }
    fn status(&mut self) -> Result<String, String> {
        let (all, left) = self.archive.counts()?;
        let target = if self.cfg.store.enabled {
            self.cfg.archive.sync_target.as_str()
        } else {
            "none ([store] enabled = false)"
        };
        Ok(format!(
            "records: {all} kept, {left} not yet synced (sync target: {target})\n"
        ))
    }
    fn sync(&mut self) -> Result<String, String> {
        Ok(sync_with(&mut self.archive, &self.cfg))
    }
    /// Every record as handoff JSON v1, one per line: `-` (or nothing) = stdout, else a folder.
    fn export(&mut self, dest: Option<&str>) -> Result<String, String> {
        let (mut w, to): (Box<dyn std::io::Write>, String) = match dest {
            None | Some("-") => (Box::new(std::io::stdout().lock()), "stdout".into()),
            Some(dir) => {
                std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
                let p = Path::new(dir).join(format!("jack-export-{}.jsonl", crate::archive::now()));
                let f = std::fs::File::create(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                (
                    Box::new(std::io::BufWriter::new(f)),
                    p.display().to_string(),
                )
            }
        };
        let (mut after, mut n) = (0, 0);
        loop {
            let page = self
                .archive
                .page(after, self.cfg.archive.sync_batch.max(1))?;
            let Some(last) = page.last() else { break };
            after = last.seq;
            for r in &page {
                writeln!(w, "{}", r.handoff).map_err(|e| e.to_string())?;
                n += 1;
            }
        }
        w.flush().map_err(|e| e.to_string())?;
        Ok(format!("exported {n} record(s) to {to}"))
    }
    fn actions(&self) -> Vec<Action> {
        vec![Action {
            name: "tag",
            keywords: &["tag", "label", "identify", "metadata", "fetch"],
            help: "fetch, verify and tag files (tag PATH...; same as jack PATH...)",
        }]
    }
    fn action(&mut self, name: &str, rest: &str) -> Result<String, String> {
        if name != "tag" {
            return Err(format!("I don't know how to {name}"));
        }
        let paths: Vec<&str> = rest.split_whitespace().collect();
        if paths.is_empty() {
            return Err("tag what? give file or folder paths".into());
        }
        let mut argv = vec!["jack".to_string()];
        argv.extend(paths.iter().map(|p| p.to_string()));
        if let Some(c) = &self.config_arg {
            argv.push("--config".into());
            argv.push(c.to_string_lossy().into_owned());
        }
        use clap::Parser;
        let mut cli = crate::cli::Cli::try_parse_from(argv).map_err(|e| e.to_string())?;
        crate::secrets::resolve_secrets(&mut cli, &mut crate::secrets::read_secret)?;
        crate::run::run(&cli, &crate::providers::Ureq).map(|(s, _, _)| s)
    }
}

pub fn main(args: &[String]) -> i32 {
    let (config_arg, rest) = split_config(args);
    let Some(cmd) = commands::parse(&rest) else {
        return 2;
    };
    let to_stdout = matches!(&cmd, Command::Export(None))
        || matches!(&cmd, Command::Export(Some(d)) if d == "-");
    let talk = matches!(cmd, Command::Talk(_));
    let r = Jack::open(config_arg.as_deref()).and_then(|mut j| {
        let hooks = Hooks {
            helper: &windups::base::talk::NoHelper,
            off_device: &windups::base::snapshot::NoOffDevice,
        };
        commands::dispatch(&mut j, cmd, &hooks)
    });
    match r {
        Ok(r) => {
            if talk {
                eprintln!("ran: jack {}", r.ran);
            }
            let text = r.text.trim_end();
            if to_stdout {
                eprintln!("{text}");
            } else {
                println!("{text}");
            }
            0
        }
        Err(e) => {
            eprintln!("jack: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windups::base::snapshot::NoOffDevice;
    use windups::base::talk::NoHelper;

    const H: Hooks = Hooks {
        helper: &NoHelper,
        off_device: &NoOffDevice,
    };

    fn jack(d: &tempfile::TempDir) -> Jack {
        let c = d.path().join("config.toml");
        let backups = d.path().join("backups");
        std::fs::write(
            &c,
            format!(
                "[store]\nenabled = false\n[core.snapshots]\ndir = {:?}\n[providers.tmdb]\nread_token = \"sekrit\"\n",
                backups.to_str().unwrap()
            ),
        )
        .unwrap();
        Jack::open(Some(&c)).unwrap()
    }

    fn run(j: &mut Jack, args: &[&str]) -> commands::Reply {
        let a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        commands::dispatch(j, commands::parse(&a).unwrap(), &H).unwrap()
    }

    #[test]
    fn jack_answers_the_universal_commands_from_his_own_folder() {
        let d = tempfile::tempdir().unwrap();
        let mut j = jack(&d);
        assert_eq!(j.layout.db, d.path().join("jack.db"));
        assert_eq!(
            std::path::PathBuf::from(&j.cfg.archive.database),
            j.layout.db
        );
        j.archive
            .write(&crate::archive::tests::handoff("Heat"))
            .unwrap();
        let s = run(&mut j, &["status"]).text;
        assert!(
            s.contains("schema v3") && s.contains("records: 1 kept, 1 not yet synced"),
            "{s}"
        );
        assert!(run(&mut j, &["version"]).text.starts_with("jack 0."));
        let b = run(&mut j, &["backup"]).text;
        assert!(b.contains("checked ok") && b.contains("backups"), "{b}");
        assert!(
            run(&mut j, &["health"])
                .text
                .contains("integrity: ok; schema v3; backups: ok")
        );
        let c = run(&mut j, &["config"]).text;
        assert!(c.contains("<hidden>") && !c.contains("sekrit"), "{c}");
        let out = d.path().join("out");
        let e = run(&mut j, &["export", out.to_str().unwrap()]).text;
        assert!(e.starts_with("exported 1 record(s)"), "{e}");
        let s = run(&mut j, &["sync"]).text;
        assert!(s.contains("no sync target configured"), "{s}");
        let log = run(&mut j, &["log"]).text;
        assert!(
            log.contains(" record jack-") && log.contains(" backup "),
            "{log}"
        );
        let e = commands::dispatch(&mut j, Command::Talk("please identify".into()), &H)
            .err()
            .unwrap();
        assert!(e.contains("tag what?"), "talk found Jack's own action: {e}");
        assert!(
            run(&mut j, &["log"])
                .text
                .contains("\"please identify\" -> tag")
        );
    }

    #[test]
    fn a_real_path_named_like_a_command_is_a_path() {
        assert!(is_command(&["talk".into(), "hi".into()]));
        assert!(!is_command(&["movie.mkv".into()]));
        let (c, rest) = split_config(&[
            "log".into(),
            "--config".into(),
            "/x.toml".into(),
            "5".into(),
        ]);
        assert_eq!(
            (c, rest),
            (
                Some(PathBuf::from("/x.toml")),
                vec!["log".into(), "5".into()]
            )
        );
        let (c, _) = split_config(&["status".into(), "--config=/y.toml".into()]);
        assert_eq!(c, Some(PathBuf::from("/y.toml")));
    }
}
