fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", john::PROFILE),
        _ => {
            eprintln!("john: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
