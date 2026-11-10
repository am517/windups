fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", sally::PROFILE),
        _ => {
            eprintln!("sally: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
