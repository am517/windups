fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(as_json) = windups::base::help::requested(&args) {
        let commands = [(
            "hash",
            "print each file's content hash, partial by default (hash [--full] [-j] FILE...)",
        )];
        let v = env!("CARGO_PKG_VERSION");
        if as_json {
            println!(
                "{}",
                windups::base::help::json(larry::PROFILE, v, &commands)
            );
        } else {
            print!(
                "{}",
                windups::base::help::human(larry::PROFILE, v, larry::cli::COMMAND_LINE, &commands)
            );
        }
        return;
    }
    match args.first().map(String::as_str) {
        Some("hash") => {
            let r = match larry::cli::parse(&args[1..]) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("larry: {e}");
                    std::process::exit(2);
                }
            };
            let user = windups::paths::windup_config("larry");
            let cfg = match larry::Config::load(Some(&user)) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("larry: {e}");
                    std::process::exit(1);
                }
            };
            let results = larry::cli::hash_all(&r, &cfg.hash);
            if r.json {
                print!("{}", larry::cli::json(&results));
            } else {
                print!("{}", larry::cli::human(&results));
            }
            if results.iter().any(|(_, h)| h.is_err()) {
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("{}", larry::cli::usage());
            std::process::exit(2);
        }
    }
}
