use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(name = "spark", version)]
struct Cli {
    #[command(flatten)]
    args: blaze::SparkArgs,
}

fn main() -> ExitCode {
    blaze::exit(blaze::spark(&Cli::parse().args))
}
