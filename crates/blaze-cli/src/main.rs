#[global_allocator]
static GLOBAL: blaze::MiMalloc = blaze::MiMalloc;

use std::process::{Command, ExitCode};

use blaze::{CargoArgs, EmberArgs, ForgeArgs, SparkArgs, WardArgs, exit, exit_code};
use blaze_cargo::fast::{Kind, execute};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "blaze",
    version,
    about = "A fast front-end for cargo, plus ember, forge, spark and ward"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build; skips cargo entirely when nothing changed
    #[command(disable_help_flag = true)]
    Build(CargoArgs),
    /// Build and run; execs the binary directly when it is up to date
    #[command(disable_help_flag = true)]
    Run(CargoArgs),
    /// Type-check; skips cargo entirely when nothing changed
    #[command(disable_help_flag = true)]
    Check(CargoArgs),
    /// Add dependencies (resolved from the crates.io sparse index)
    #[command(disable_help_flag = true)]
    Add(CargoArgs),
    /// Remove dependencies
    #[command(disable_help_flag = true)]
    Remove(CargoArgs),
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
    /// forge --check + ember + ward, for CI and pre-commit
    Ci,
    /// Anything else (new, init, clean, publish, ...) is forwarded to cargo
    #[command(external_subcommand)]
    Cargo(Vec<String>),
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Cmd::Build(a) => exit_code(execute(Kind::Build, &a.args)),
        Cmd::Run(a) => exit_code(execute(Kind::Run, &a.args)),
        Cmd::Check(a) => exit_code(execute(Kind::Check, &a.args)),
        Cmd::Add(a) => exit_code(blaze_cargo::add::add(&a.args)),
        Cmd::Remove(a) => exit_code(blaze_cargo::add::remove(&a.args)),
        Cmd::Ember(a) => exit(blaze::ember(&a)),
        Cmd::Forge(a) => exit(blaze::forge(&a)),
        Cmd::Spark(a) => exit(blaze::spark(&a)),
        Cmd::Ward(a) => exit(blaze::ward(&a)),
        Cmd::Ci => {
            exit(
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
                })(),
            )
        }
        Cmd::Cargo(args) => {
            exit_code(
                Command::new("cargo")
                    .args(args)
                    .status()
                    .map(|s| s.code().unwrap_or(1))
                    .map_err(Into::into),
            )
        }
    }
}
