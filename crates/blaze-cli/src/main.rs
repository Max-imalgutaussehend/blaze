use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Instant;
#[derive(Parser)]
#[command(name = "blaze", version, about = "Fast, unified Rust tooling")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    /// Lint Rust files (single-pass AST linter)
    Lint {
        #[arg(default_value = ".")]
        paths: Vec<PathBuf>,
    },
    /// List available lint rules
    Rules,
    /// Format Rust files in place
    Fmt {
        /// Only check; exit 1 if any file would change
        #[arg(long)]
        check: bool,
        #[arg(default_value = ".")]
        paths: Vec<PathBuf>,
    },
    /// Run tests, one process per test, in parallel
    Test {
        /// Only run tests whose name contains this string
        filter: Option<String>,
    },
    /// Check dependencies against the [deny] policy in blaze.toml
    Deny,
    /// fmt --check + lint + deny
    Check,
    /// Anything else (build, add, new, ...) is forwarded to cargo
    #[command(external_subcommand)]
    Cargo(Vec<String>),
}
fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
fn run(cli: Cli) -> Result<bool> {
    match cli.command {
        Cmd::Lint { paths } => lint(&paths),
        Cmd::Rules => {
            for r in blaze_lint::RULES {
                println!("{}  {:<16} {}", r.code, r.name, r.summary);
            }
            Ok(true)
        }
        Cmd::Fmt { check, paths } => fmt(&paths, check),
        Cmd::Test { filter } => test(filter.as_deref()),
        Cmd::Deny => deny(),
        Cmd::Check => Ok(fmt(&[".".into()], true)? & lint(&[".".into()])? & deny()?),
        Cmd::Cargo(args) => Ok(Command::new("cargo").args(args).status()?.success()),
    }
}
fn lint(paths: &[PathBuf]) -> Result<bool> {
    let start = Instant::now();
    let files = blaze_core::discover_rust_files(paths);
    let results = blaze_lint::lint_files(&files);
    let (mut diags, mut errs) = (0, 0);
    for r in &results {
        if let Some(e) = &r.error {
            errs += 1;
            eprintln!("{}: {e}", r.path.display());
        }
        for d in &r.diagnostics {
            diags += 1;
            println!("{}:{d}", r.path.display());
        }
    }
    eprintln!(
        "lint: {} files, {diags} issues, {errs} errors in {:.0?}",
        files.len(),
        start.elapsed()
    );
    Ok(diags == 0 && errs == 0)
}
fn fmt(paths: &[PathBuf], check: bool) -> Result<bool> {
    let start = Instant::now();
    let files = blaze_core::discover_rust_files(paths);
    let report = blaze_fmt::run(&PathBuf::from("."), &files, check)?;
    for p in &report.changed {
        println!(
            "{} {}",
            if check {
                "would reformat"
            } else {
                "reformatted"
            },
            p.display()
        );
    }
    for (p, e) in &report.errors {
        eprintln!("{}: {e}", p.display());
    }
    eprintln!(
        "fmt: {} files ({} cached), {} changed, {} errors in {:.0?}",
        report.total,
        report.skipped_cached,
        report.changed.len(),
        report.errors.len(),
        start.elapsed()
    );
    Ok(report.errors.is_empty() && !(check && !report.changed.is_empty()))
}
fn test(filter: Option<&str>) -> Result<bool> {
    let root = PathBuf::from(".");
    let bins = blaze_test::build(&root, &[])?;
    let start = Instant::now();
    let outcomes = blaze_test::run(&bins, filter, |o| {
        println!(
            "{} {:>6.2?}  {}::{}",
            if o.passed { "PASS" } else { "FAIL" },
            o.duration,
            o.label,
            o.name
        );
    })?;
    let failed: Vec<_> = outcomes.iter().filter(|o| !o.passed).collect();
    for f in &failed {
        println!("\n--- {}::{} ---\n{}", f.label, f.name, f.output);
    }
    eprintln!(
        "test: {} run, {} failed in {:.2?}",
        outcomes.len(),
        failed.len(),
        start.elapsed()
    );
    Ok(failed.is_empty())
}
fn deny() -> Result<bool> {
    let root = PathBuf::from(".");
    let cfg = blaze_deny::Config::load(&root)?;
    let violations = blaze_deny::check(&blaze_deny::load_packages(&root)?, &cfg);
    for v in &violations {
        println!("[{}] {}", v.kind, v.message);
    }
    eprintln!("deny: {} violations", violations.len());
    Ok(violations.is_empty())
}
