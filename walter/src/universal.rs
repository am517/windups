//! Walter on the shared core: the universal commands (`walter status`, `walter talk ...`) plus his
//! own action, `audit`. His own walter.db holds only the core tables (event log, snapshots).
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use windups::base::commands::{self, Action, Command, Hooks, Windup};
use windups::base::config::BaseCfg;
use windups::base::layout::{Layout, Probe, resolve};

pub struct Walter {
    pub layout: Layout,
    pub core: BaseCfg,
    pub cfg: crate::config::Config,
    pub c: Connection,
}

impl Walter {
    pub fn open(config_arg: Option<&Path>) -> Result<Self, String> {
        let layout = resolve("walter", &Probe::real("walter", config_arg));
        Self::open_at(layout)
    }

    pub fn open_at(layout: Layout) -> Result<Self, String> {
        let text = std::fs::read_to_string(&layout.config).unwrap_or_default();
        let core = BaseCfg::from_toml(&text)?;
        let cfg = crate::config::Config::load(Some(&layout.config))?;
        std::fs::create_dir_all(&layout.dir)
            .map_err(|e| format!("{}: {e}", layout.dir.display()))?;
        let mut c = Connection::open(&layout.db).map_err(|e| e.to_string())?;
        windups::base::schema::migrate(&mut c, "walter.db", windups::base::CORE_V1)?;
        Ok(Walter {
            layout,
            core,
            cfg,
            c,
        })
    }
}

impl Windup for Walter {
    fn name(&self) -> &str {
        "walter"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn layout(&self) -> &Layout {
        &self.layout
    }
    fn core_cfg(&self) -> &BaseCfg {
        &self.core
    }
    fn conn(&mut self) -> &mut Connection {
        &mut self.c
    }
    fn status(&mut self) -> Result<String, String> {
        let roots: Vec<String> = crate::audit::roots(&self.cfg)
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        Ok(format!(
            "auditing windup folders under: {}\n",
            roots.join(", ")
        ))
    }
    fn actions(&self) -> Vec<Action> {
        vec![Action {
            name: "audit",
            keywords: &[
                "audit",
                "everyone",
                "everybody",
                "windups",
                "gaps",
                "check on",
            ],
            help: "read every windup's database (read-only) and say what needs fixing and how",
        }]
    }
    fn action(&mut self, name: &str, _rest: &str) -> Result<String, String> {
        match name {
            "audit" => Ok(crate::audit::report(
                &crate::audit::roots(&self.cfg),
                self.core.snapshots.stale_after_days,
            )),
            other => Err(format!("I don't know how to {other}")),
        }
    }
}

/// `walter <universal command | audit> ... [--config PATH]`.
pub fn is_command(args: &[String]) -> bool {
    commands::parse(args).is_some() || args.first().is_some_and(|a| a == "audit")
}

pub fn main(args: &[String]) -> i32 {
    let mut config: Option<PathBuf> = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--config" {
            config = it.next().map(PathBuf::from);
        } else {
            rest.push(a.clone());
        }
    }
    let cmd = commands::parse(&rest)
        .unwrap_or_else(|| Command::Action(rest[0].clone(), rest[1..].join(" ")));
    let talk = matches!(cmd, Command::Talk(_));
    let r = Walter::open(config.as_deref()).and_then(|mut w| {
        let hooks = Hooks {
            helper: &windups::base::talk::NoHelper,
            off_device: &windups::base::snapshot::NoOffDevice,
        };
        commands::dispatch(&mut w, cmd, &hooks)
    });
    match r {
        Ok(r) => {
            if talk {
                eprintln!("ran: walter {}", r.ran);
            }
            println!("{}", r.text.trim_end());
            0
        }
        Err(e) => {
            eprintln!("walter: {e}");
            1
        }
    }
}
