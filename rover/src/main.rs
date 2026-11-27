fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(windups::base::help::stub_main(
        env!("CARGO_PKG_VERSION"),
        rover::PROFILE,
        &args,
    ));
}
