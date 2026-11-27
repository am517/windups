const USAGE: &str = "walter [note | pending | patrol [--all] [--wake CAPABILITY]] [--db PATH] [--config PATH] | audit | status | health | backup | log | help | talk WORDS... | -h [-j]";

/// Walter's own commands, after the universal ones.
const OWN: [(&str, &str); 4] = [
    (
        "pending",
        "list unfinished work; read-only, nobody is called",
    ),
    (
        "patrol",
        "discover windups, wake what waits, call Jack with what is due (patrol [--all] [--wake CAPABILITY])",
    ),
    ("note", "same as patrol"),
    ("audit", "read-only checks of every windup's database"),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(as_json) = windups::base::help::requested(&args) {
        let commands: Vec<(&str, &str)> = windups::base::commands::UNIVERSAL
            .iter()
            .chain(OWN.iter())
            .copied()
            .collect();
        let v = env!("CARGO_PKG_VERSION");
        if as_json {
            println!(
                "{}",
                windups::base::help::json(walter::PROFILE, v, &commands)
            );
        } else {
            print!(
                "{}",
                windups::base::help::human(walter::PROFILE, v, USAGE, &commands)
            );
        }
        return;
    }
    if walter::universal::is_command(&args) {
        std::process::exit(walter::universal::main(&args));
    }
    let r = match args.first().map(String::as_str) {
        Some(cmd @ ("pending" | "patrol" | "note")) => {
            walter::run(cmd, &args[1..], &walter::caller::Process)
        }
        _ => Err(format!("usage: {USAGE}")),
    };
    match r {
        Ok(s) => print!("{s}"),
        Err(e) => {
            eprintln!("walter: {e}");
            std::process::exit(1);
        }
    }
}
