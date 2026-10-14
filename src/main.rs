fn main() {
    if let Err(err) = calc::ui::run() {
        eprintln!("calc: {err}");
        std::process::exit(1);
    }
}
