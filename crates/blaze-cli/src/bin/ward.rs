use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(name = "ward", version)]
struct Cli {
    #[command(flatten)]
    args: blaze::WardArgs,
}

fn main() -> ExitCode {
    blaze::exit(blaze::ward(&Cli::parse().args))
}
