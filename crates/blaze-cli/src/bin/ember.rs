#[global_allocator]
static GLOBAL: blaze::MiMalloc = blaze::MiMalloc;

use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(name = "ember", version)]
struct Cli {
    #[command(flatten)]
    args: blaze::EmberArgs,
}

fn main() -> ExitCode {
    blaze::exit(blaze::ember(&Cli::parse().args))
}
