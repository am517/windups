//! The universal commands every windup answers, so one client can talk to any windup.
//! The living list is `windups/COMMANDS.md`.
use super::config::BaseCfg;
use super::layout::Layout;
use super::snapshot::OffDevice;
use super::talk::{Helper, Intent};
use super::{events, schema, snapshot};
use rusqlite::Connection;
use std::fmt::Write as _;

/// (name, what it does). Order is the help order.
pub const UNIVERSAL: [(&str, &str); 10] = [
    ("status", "what I hold and how far along I am"),
    ("health", "integrity check, schema version, backup age"),
    ("sync", "send records the sync target doesn't have yet"),
    ("backup", "take a checked snapshot of my database"),
    ("export", "write my records out (export [-|DIR])"),
    ("version", "my version and schema version"),
    (
        "config",
        "where my config is and what it says (secrets hidden)",
    ),
    ("log", "recent events (log [N])"),
    ("help", "this list, plus what only I can do"),
    (
        "talk",
        "say it in plain words; I say what I ran (talk WORDS...)",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Status,
    Health,
    Sync,
    Backup,
    Export(Option<String>),
    Version,
    Config,
    Log(usize),
    Help,
    Talk(String),
    /// Something only this windup does (name, rest of the words).
    Action(String, String),
}

/// Something only this windup does, findable by `talk`.
#[derive(Debug, Clone, Copy)]
pub struct Action {
    pub name: &'static str,
    pub keywords: &'static [&'static str],
    pub help: &'static str,
}

/// A universal command from argv (after the program name), or None if it isn't one.
pub fn parse(args: &[String]) -> Option<Command> {
    let (first, rest) = args.split_first()?;
    Some(match first.as_str() {
        "status" => Command::Status,
        "health" => Command::Health,
        "sync" => Command::Sync,
        "backup" => Command::Backup,
        "export" => Command::Export(rest.first().cloned()),
        "version" => Command::Version,
        "config" => Command::Config,
        "log" => Command::Log(rest.first().and_then(|n| n.parse().ok()).unwrap_or(20)),
        "help" => Command::Help,
        "talk" => Command::Talk(rest.join(" ")),
        _ => return None,
    })
}

pub fn line(c: &Command) -> String {
    match c {
        Command::Export(Some(d)) => format!("export {d}"),
        Command::Export(None) => "export".into(),
        Command::Log(n) => format!("log {n}"),
        Command::Talk(t) => format!("talk {t}"),
        Command::Action(a, r) if r.is_empty() => a.clone(),
        Command::Action(a, r) => format!("{a} {r}"),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// What a windup supplies; everything else comes from the core.
pub trait Windup {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn layout(&self) -> &Layout;
    fn core_cfg(&self) -> &BaseCfg;
    fn conn(&mut self) -> &mut Connection;
    /// Extra status lines.
    fn status(&mut self) -> Result<String, String> {
        Ok(String::new())
    }
    fn sync(&mut self) -> Result<String, String> {
        Ok("nothing to sync".into())
    }
    fn export(&mut self, _dest: Option<&str>) -> Result<String, String> {
        Err("nothing to export".into())
    }
    fn actions(&self) -> Vec<Action> {
        vec![]
    }
    fn action(&mut self, name: &str, _rest: &str) -> Result<String, String> {
        Err(format!("I don't know how to {name}"))
    }
}

pub struct Reply {
    /// The command actually run (for `talk`, what the words became).
    pub ran: String,
    pub text: String,
}

pub struct Hooks<'a> {
    pub helper: &'a dyn Helper,
    pub off_device: &'a dyn OffDevice,
}

fn snapshot_age(c: &Connection) -> Result<String, String> {
    Ok(match snapshot::latest(c)? {
        Some((at, p)) => format!("{} ({p})", events::fmt_utc(at)),
        None => "never".into(),
    })
}

pub fn dispatch(w: &mut dyn Windup, cmd: Command, hooks: &Hooks) -> Result<Reply, String> {
    let ran = line(&cmd);
    let text = match cmd {
        Command::Status => {
            let c = w.conn();
            let v = schema::version(c)?;
            let n: i64 = c
                .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            let snap = snapshot_age(c)?;
            let mut t = format!("schema v{v}; {n} recent event(s); last snapshot: {snap}\n");
            t += &w.status()?;
            t
        }
        Command::Health => health(w)?,
        Command::Sync => logged(w, "sync", |w| w.sync())?,
        Command::Backup => {
            let cfg = w.core_cfg().clone();
            let dir = if cfg.snapshots.dir.is_empty() {
                w.layout().backups.clone()
            } else {
                crate::paths::expand(&cfg.snapshots.dir)
            };
            let name = w.name().to_string();
            let c = w.conn();
            let s = snapshot::take(c, &name, &dir, &cfg.snapshots, hooks.off_device)?;
            let k = events::compact(c, &cfg.events)?;
            let t = format!(
                "snapshot {} ({} bytes, checked ok); events: {} batched, {} summarized",
                s.path.display(),
                s.bytes,
                k.batched,
                k.summarized
            );
            events::append(c, "backup", &s.path.to_string_lossy())?;
            t
        }
        Command::Export(d) => logged(w, "export", |w| w.export(d.as_deref()))?,
        Command::Version => {
            let v = schema::version(w.conn())?;
            format!("{} {} (schema v{v})", w.name(), w.version())
        }
        Command::Config => {
            let p = w.layout().config.clone();
            match std::fs::read_to_string(&p) {
                Ok(t) => format!("{}\n{}", p.display(), super::config::redacted(&t)),
                Err(_) => format!("{} (not there; built-in defaults)", p.display()),
            }
        }
        Command::Log(n) => {
            let mut t = String::new();
            for e in events::recent(w.conn(), n)? {
                let _ = writeln!(
                    t,
                    "#{} {} {} {}",
                    e.seq,
                    events::fmt_utc(e.at),
                    e.kind,
                    e.detail
                );
            }
            t
        }
        Command::Help => {
            let mut t = String::new();
            for (n, h) in UNIVERSAL {
                let _ = writeln!(t, "  {n:<8} {h}");
            }
            for a in w.actions() {
                let _ = writeln!(t, "  {:<8} {}", a.name, a.help);
            }
            t
        }
        Command::Talk(words) => return talk(w, &words, hooks),
        Command::Action(a, rest) => logged(w, "action", |w| w.action(&a, &rest))?,
    };
    Ok(Reply { ran, text })
}

fn logged(
    w: &mut dyn Windup,
    kind: &str,
    f: impl FnOnce(&mut dyn Windup) -> Result<String, String>,
) -> Result<String, String> {
    let r = f(w);
    let detail = match &r {
        Ok(t) => t.lines().next().unwrap_or("").to_string(),
        Err(e) => format!("failed: {e}"),
    };
    events::append(w.conn(), kind, &detail)?;
    r
}

fn health(w: &mut dyn Windup) -> Result<String, String> {
    let stale = w.core_cfg().snapshots.stale_after_days;
    let c = w.conn();
    let ok: String = c
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let v = schema::version(c)?;
    let snap = match snapshot::latest(c)? {
        Some((at, _)) if events::now() - at > (stale * 86_400) as i64 => {
            format!("stale (last {}; run backup)", events::fmt_utc(at))
        }
        Some((at, _)) => format!("ok (last {})", events::fmt_utc(at)),
        None => "none yet (run backup)".into(),
    };
    Ok(format!("integrity: {ok}; schema v{v}; backups: {snap}"))
}

fn talk(w: &mut dyn Windup, words: &str, hooks: &Hooks) -> Result<Reply, String> {
    let actions = w.actions();
    let syn = w.core_cfg().talk.synonyms.clone();
    let intent = super::talk::interpret(words, &actions, &syn);
    let choices = match &intent {
        Intent::Ambiguous(c) => c.clone(),
        _ => UNIVERSAL
            .iter()
            .map(|u| u.0.to_string())
            .chain(actions.iter().map(|a| a.name.to_string()))
            .collect(),
    };
    let cmd = match intent {
        Intent::Run(c) => Some(c),
        _ => hooks.helper.place(words, &choices).and_then(|l| {
            let args: Vec<String> = l.split_whitespace().map(str::to_string).collect();
            parse(&args).or_else(|| {
                let (a, rest) = args.split_first()?;
                actions
                    .iter()
                    .any(|x| x.name == a)
                    .then(|| Command::Action(a.clone(), rest.join(" ")))
            })
        }),
    };
    match cmd {
        Some(Command::Talk(_)) | None => {
            let text = format!(
                "I didn't understand {words:?}. I can: {}",
                choices.join(", ")
            );
            events::append(w.conn(), "talk", &format!("{words:?} -> nothing"))?;
            Ok(Reply {
                ran: "nothing".into(),
                text,
            })
        }
        Some(c) => {
            events::append(w.conn(), "talk", &format!("{words:?} -> {}", line(&c)))?;
            let r = dispatch(w, c, hooks)?;
            Ok(Reply {
                ran: r.ran,
                text: r.text,
            })
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::base::layout::{Probe, resolve};
    use crate::base::snapshot::NoOffDevice;
    use crate::base::talk::NoHelper;

    pub struct Toy {
        pub layout: Layout,
        pub cfg: BaseCfg,
        pub c: Connection,
    }

    impl Toy {
        pub fn new(d: &tempfile::TempDir) -> Self {
            let layout = resolve(
                "toy",
                &Probe {
                    env_home: Some(d.path().join("toy")),
                    home: Some(d.path().to_path_buf()),
                    ..Default::default()
                },
            );
            Toy {
                layout,
                cfg: BaseCfg::default(),
                c: crate::base::events::tests::db(),
            }
        }
    }

    impl Windup for Toy {
        fn name(&self) -> &str {
            "toy"
        }
        fn version(&self) -> &str {
            "0.0.1"
        }
        fn layout(&self) -> &Layout {
            &self.layout
        }
        fn core_cfg(&self) -> &BaseCfg {
            &self.cfg
        }
        fn conn(&mut self) -> &mut Connection {
            &mut self.c
        }
        fn actions(&self) -> Vec<Action> {
            vec![Action {
                name: "wind",
                keywords: &["wind", "crank"],
                help: "wind me up",
            }]
        }
        fn action(&mut self, name: &str, _: &str) -> Result<String, String> {
            Ok(format!("{name}: whirr"))
        }
    }

    const H: Hooks = Hooks {
        helper: &NoHelper,
        off_device: &NoOffDevice,
    };

    fn run(t: &mut Toy, args: &[&str]) -> Reply {
        let a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        dispatch(t, parse(&a).unwrap(), &H).unwrap()
    }

    #[test]
    fn every_universal_command_parses_and_unknown_words_dont() {
        for (n, _) in UNIVERSAL {
            assert!(parse(&[n.to_string()]).is_some(), "{n}");
        }
        assert_eq!(parse(&["movie.mkv".into()]), None);
        assert_eq!(parse(&["log".into(), "5".into()]), Some(Command::Log(5)));
        let md = include_str!("../../COMMANDS.md");
        for (n, _) in UNIVERSAL {
            assert!(md.contains(&format!("| `{n}`")), "COMMANDS.md lacks {n}");
        }
    }

    #[test]
    fn any_windup_answers_the_universal_commands() {
        let d = tempfile::tempdir().unwrap();
        let mut t = Toy::new(&d);
        assert!(
            run(&mut t, &["version"])
                .text
                .contains("toy 0.0.1 (schema v2)")
        );
        assert!(
            run(&mut t, &["health"])
                .text
                .contains("integrity: ok; schema v2; backups: none yet")
        );
        let b = run(&mut t, &["backup"]).text;
        assert!(b.contains("checked ok"), "{b}");
        assert!(run(&mut t, &["health"]).text.contains("backups: ok"));
        assert!(run(&mut t, &["status"]).text.contains("last snapshot: 20"));
        assert_eq!(run(&mut t, &["sync"]).text, "nothing to sync");
        assert!(run(&mut t, &["help"]).text.contains("wind me up"));
        assert!(run(&mut t, &["config"]).text.contains("built-in defaults"));
        let log = run(&mut t, &["log", "10"]).text;
        assert!(
            log.contains(" backup ") && log.contains(" sync nothing to sync"),
            "{log}"
        );
    }

    #[test]
    fn talk_reports_what_it_ran_and_asks_the_helper_when_stuck() {
        let d = tempfile::tempdir().unwrap();
        let mut t = Toy::new(&d);
        let r = run(&mut t, &["talk", "crank", "me"]);
        assert_eq!((r.ran.as_str(), r.text.as_str()), ("wind", "wind: whirr"));
        let r = run(&mut t, &["talk", "make", "tea"]);
        assert_eq!(r.ran, "nothing");
        assert!(r.text.contains("I can: status"), "{}", r.text);
        struct Flora;
        impl Helper for Flora {
            fn place(&self, _: &str, _: &[String]) -> Option<String> {
                Some("version".into())
            }
        }
        let h = Hooks {
            helper: &Flora,
            off_device: &NoOffDevice,
        };
        let r = dispatch(&mut t, Command::Talk("who are you".into()), &h).unwrap();
        assert_eq!(r.ran, "version");
        let log = run(&mut t, &["log"]).text;
        assert!(log.contains("\"who are you\" -> version"), "{log}");
    }
}
