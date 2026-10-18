use std::process::{Command, ExitCode};

use blaze::{EmberArgs, ForgeArgs, SparkArgs, WardArgs, exit};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "blaze", version, about = "Extremely fast Rust tooling")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Lint (alias: lint)
    #[command(alias = "lint")]
    Ember(EmberArgs),
    /// Format (alias: fmt)
    #[command(alias = "fmt")]
    Forge(ForgeArgs),
    /// Run tests (alias: test)
    #[command(alias = "test")]
    Spark(SparkArgs),
    /// Check dependencies (alias: deny)
    #[command(alias = "deny")]
    Ward(WardArgs),
    /// forge --check + ember + ward
    Check,
    /// Anything else (build, add, new, ...) is forwarded to cargo
    #[command(external_subcommand)]
    Cargo(Vec<String>),
}

fn main() -> ExitCode {
    exit(
        match Cli::parse().command {
            Cmd::Ember(a) => blaze::ember(&a),
            Cmd::Forge(a) => blaze::forge(&a),
            Cmd::Spark(a) => blaze::spark(&a),
            Cmd::Ward(a) => blaze::ward(&a),
            Cmd::Check => {
                (|| {
                    let f = blaze::forge(
                        &ForgeArgs {
                            check: true,
                            paths: vec![".".into()],
                        },
                    )?;
                    let e = blaze::ember(
                        &EmberArgs {
                            paths: vec![".".into()],
                            rules: false,
                            ignore: vec![],
                        },
                    )?;
                    let w = blaze::ward(&WardArgs {})?;
                    Ok(f && e && w)
                })()
            }
            Cmd::Cargo(args) => {
                Command::new("cargo")
                    .args(args)
                    .status()
                    .map(|s| s.success())
                    .map_err(Into::into)
            }
        },
    )
}
