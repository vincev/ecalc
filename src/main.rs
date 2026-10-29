use clap::Parser;

#[derive(Parser)]
#[command(name = "ec", version, about)]
struct Cli {
    /// Evaluate an RPN string and print the top of the stack.
    ///
    /// Whitespace separates words. `2 20 ^ d g` prints `1,048,576`.
    #[arg(short, long, value_name = "STRING")]
    eval: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    if let Some(program) = cli.eval {
        match ecalc::eval(&program) {
            Ok(value) => println!("{value}"),
            Err(err) => {
                eprintln!("ec: {err}");
                std::process::exit(1);
            }
        }
        return;
    }
    if let Err(err) = ecalc::ui::run() {
        eprintln!("ec: {err}");
        std::process::exit(1);
    }
}
