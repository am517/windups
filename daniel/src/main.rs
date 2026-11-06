fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--capabilities") => print!("{}", daniel::PROFILE),
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
