#[global_allocator]
static GLOBAL: blaze::MiMalloc = blaze::MiMalloc;

use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(name = "forge", version)]
struct Cli {
    #[command(flatten)]
    args: blaze::ForgeArgs,
}

fn main() -> ExitCode {
    blaze::exit(blaze::forge(&Cli::parse().args))
}
