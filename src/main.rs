fn main() {
    if let Err(err) = ecalc::ui::run() {
        eprintln!("ec: {err}");
        std::process::exit(1);
    }
}
