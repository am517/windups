fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", suzan::PROFILE),
        _ => {
            eprintln!("suzan: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
