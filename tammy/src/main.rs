fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", tammy::PROFILE),
        _ => {
            eprintln!("tammy: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
