fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", lauren::PROFILE),
        _ => {
            eprintln!("lauren: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
