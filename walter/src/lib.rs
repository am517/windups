//! Walter, the patrol windup . Nothing incomplete is forgotten:
//! `pending` only lists (read-only: no patrol, no windups called); `patrol` (or `note`, the same) is a full
//! patrol: it discovers capabilities, schedules returns, and calls Jack with whatever is due. No system timers: windups call windups.
pub mod audit;
pub mod caller;
pub mod config;
pub mod discover;
pub mod patrol;
pub mod universal;

use std::path::PathBuf;

pub const PROFILE: &str = include_str!("../PROFILE.toml");

pub struct Args {
    pub db: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub all: bool,
    pub wake: Vec<String>,
}

pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        db: None,
        config: None,
        all: false,
        wake: vec![],
    };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{x} needs a value"));
        match x.as_str() {
            "--all" => a.all = true,
            "--db" => a.db = Some(val()?.into()),
            "--config" => a.config = Some(val()?.into()),
            "--wake" => a.wake.push(val()?),
            o => return Err(format!("unknown argument: {o}")),
        }
    }
    Ok(a)
}

fn line(p: &windups::returns::Pending) -> String {
    let when = match (&p.next_retry_at, &p.waiting_for) {
        (Some(t), Some(w)) => format!("{t} UTC or when {w} appears"),
        (Some(t), None) => format!("{t} UTC"),
        (None, Some(w)) => format!("when {w} appears"),
        (None, None) => "now".into(),
    };
    let reason = if p.reason.is_empty() { "-" } else { &p.reason };
    format!(
        "#{} {} [{}] {reason} — next: {when} (attempts {})\n",
        p.media_id,
        p.path.as_deref().unwrap_or("(no file)"),
        p.stage,
        p.attempts
    )
}

/// `pending` lists scheduled returns and changes nothing. `patrol` (or `note`, the same) is a full
/// patrol, stamped as `last_patrol` in `windup_state` so callers (Daniel) can count from it.
pub fn run(cmd: &str, args: &[String], caller: &dyn caller::Caller) -> Result<String, String> {
    let a = parse(args)?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let cfg_path = a
        .config
        .clone()
        .or(home.map(|h| h.join(".config/windups/walter/config.toml")));
    let cfg = config::Config::load(cfg_path.as_deref())?;
    let db = match a.db {
        Some(p) => p,
        None => windups::Settings::load()?.database_path(),
    };
    let c = windups::db::open(&db)?;
    let e = |e: rusqlite::Error| e.to_string();
    let mut out = String::new();
    if cmd == "pending" {
        if a.all || !a.wake.is_empty() {
            return Err("pending only lists; use `walter patrol` for --all or --wake".into());
        }
        let all = windups::returns::pending(&c).map_err(e)?;
        out += &format!("{} pending\n", all.len());
        all.iter().for_each(|p| out += &line(p));
        return Ok(out);
    }
    let woke = discover::discover(&c, &cfg, caller, &a.wake).map_err(e)?;
    if !woke.is_empty() {
        out += &format!("woke items waiting for: {}\n", woke.join(", "));
    }
    patrol::sweep(&c, &cfg.retry).map_err(e)?;
    // a data freak: whenever he is called to patrol, Walter patrols the whole database
    let items = if a.all {
        windups::returns::pending(&c)
    } else {
        windups::returns::due(&c)
    }
    .map_err(e)?;
    let files = patrol::files(&items);
    out += &format!("{} due, {} with a file present\n", items.len(), files.len());
    if !files.is_empty() {
        let jack = cfg
            .windups
            .get("jack")
            .ok_or("no command for jack in [windups]")?;
        out += &caller.call(jack, &files)?;
        patrol::touched(&c, &items).map_err(e)?;
        patrol::sweep(&c, &cfg.retry).map_err(e)?;
    }
    c.execute(
        "INSERT INTO windup_state(windup, key, value) VALUES ('walter', 'last_patrol', strftime('%Y-%m-%d %H:%M:%f', 'now'))
         ON CONFLICT(windup, key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        [],
    )
    .map_err(e)?;
    Ok(out)
}
