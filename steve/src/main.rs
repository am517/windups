fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", steve::PROFILE),
        _ => {
            eprintln!("steve: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
