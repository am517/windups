fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", flora::PROFILE),
        _ => {
            eprintln!("flora: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
