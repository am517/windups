fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(as_json) = windups::base::help::requested(&args) {
        let commands = [
            (
                "ingest",
                "load handoff JSON files (or - for stdin) into media.db (ingest [--db PATH] FILE... | -)",
            ),
            ("accept", "same as ingest"),
            (
                "serve",
                "take synced records over a Unix socket (serve [--socket PATH] [--db PATH])",
            ),
        ];
        let v = env!("CARGO_PKG_VERSION");
        if as_json {
            println!(
                "{}",
                windups::base::help::json(daniel::PROFILE, v, &commands)
            );
        } else {
            let usage = "daniel ingest|accept [--db PATH] FILE...|- | daniel serve [--socket PATH] [--db PATH] | daniel -h [-j]";
            print!(
                "{}",
                windups::base::help::human(daniel::PROFILE, v, usage, &commands)
            );
        }
        return;
    }
    match args.first().map(String::as_str) {
        Some("serve") => {
            if let Err(e) = daniel::serve::run(&args[1..]) {
                eprintln!("daniel: {e}");
                std::process::exit(if e.starts_with("usage:") { 2 } else { 1 });
            }
        }
        Some("ingest" | "accept") => match daniel::ingest::run(&args[1..]) {
            Ok((s, 0)) => print!("{s}"),
            Ok((s, n)) => {
                print!("{s}");
                eprintln!("daniel: {n} failed");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("daniel: {e}");
                std::process::exit(if e.starts_with("usage:") { 2 } else { 1 });
            }
        },
        _ => {
            eprintln!("{}", daniel::ingest::usage());
            std::process::exit(2);
        }
    }
}
