fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", beth::PROFILE),
        _ => {
            eprintln!("beth: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
