//! Command implementations shared by the `blaze` umbrella binary and the standalone
//! `ember`, `forge`, `spark` and `ward` binaries.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use anyhow::Result;
use blaze_core::{Cache, Stamp};
use rayon::prelude::*;

/// Faster general-purpose allocator for the allocation-heavy tools.
pub use mimalloc::MiMalloc;
use clap::Args;

#[derive(Args)]
pub struct EmberArgs {
    /// Files or directories to lint
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,
    /// List available rules and exit
    #[arg(long)]
    pub rules: bool,
    /// Rule codes to ignore, e.g. `--ignore B001,B006` (adds to `[ember] ignore` in blaze.toml)
    #[arg(long, value_delimiter = ',')]
    pub ignore: Vec<String>,
}

#[derive(Args)]
pub struct ForgeArgs {
    /// Only check; exit 1 if any file would change
    #[arg(long)]
    pub check: bool,
    /// Files or directories to format
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,
}

#[derive(Args)]
pub struct SparkArgs {
    /// Only run tests whose name contains this string
    pub filter: Option<String>,
    /// Skip tests that passed before with a byte-identical test binary (assumes deterministic tests)
    #[arg(long)]
    pub cached: bool,
    /// Run each test binary once with libtest's own threads (like `cargo test`) instead of one process per test
    #[arg(long)]
    pub shared: bool,
}

#[derive(Args)]
pub struct WardArgs {}

/// Arguments passed through to the cargo front-end untouched.
#[derive(Args)]
pub struct CargoArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Exit with the child's own status code (cargo front-end commands).
pub fn exit_code(r: Result<i32>) -> ExitCode {
    match r {
        Ok(c) => ExitCode::from(c.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

pub fn exit(r: Result<bool>) -> ExitCode {
    match r {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

pub fn ember(a: &EmberArgs) -> Result<bool> {
    if a.rules {
        for r in blaze_ember::RULES {
            println!("{}  {:<16} {}", r.code, r.name, r.summary);
        }
        return Ok(true);
    }
    let start = Instant::now();
    let mut ignore = a.ignore.clone();
    ignore.extend(blaze_core::config_strings(&PathBuf::from("."), "ember", "ignore"));
    let files = blaze_core::discover_rust_files(&a.paths);
    // Files whose stamp matches a previous clean run are skipped without being read.
    let mut cache = Cache::load(&PathBuf::from("."), "ember", env!("CARGO_PKG_VERSION"));
    let checked: Vec<(Option<Stamp>, blaze_ember::FileResult)> = files
        .par_iter()
        .filter_map(|p| {
            let stamp = Stamp::of(p);
            if stamp.is_some_and(|s| cache.quick(p, s) == Some("clean")) {
                return None;
            }
            Some((stamp, blaze_ember::lint_file(p)))
        })
        .collect();
    let mut results = Vec::new();
    for (stamp, r) in checked {
        if r.diagnostics.is_empty() && r.error.is_none() {
            cache.mark_stamped(&r.path, stamp, "", "clean");
        } else {
            results.push(r);
        }
    }
    cache.save()?;
    for r in &mut results {
        r.diagnostics.retain(|d| !ignore.iter().any(|c| c == d.code));
    }
    results.retain(|r| !r.diagnostics.is_empty() || r.error.is_some());
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
        "ember: {} files, {diags} issues, {errs} errors in {:.0?}", files.len(), start
        .elapsed()
    );
    Ok(diags == 0 && errs == 0)
}

pub fn forge(a: &ForgeArgs) -> Result<bool> {
    let start = Instant::now();
    let files = blaze_core::discover_rust_files(&a.paths);
    let report = blaze_forge::run(&PathBuf::from("."), &files, a.check)?;
    for p in &report.changed {
        println!(
            "{} {}", if a.check { "would reformat" } else { "reformatted" }, p.display()
        );
    }
    for (p, e) in &report.errors {
        eprintln!("{}: {e}", p.display());
    }
    eprintln!(
        "forge: {} files ({} cached, {} unsupported), {} changed, {} errors in {:.0?}",
        report.total, report.skipped_cached, report.unsupported.len(), report.changed
        .len(), report.errors.len(), start.elapsed()
    );
    Ok(report.errors.is_empty() && !(a.check && !report.changed.is_empty()))
}

pub fn spark(a: &SparkArgs) -> Result<bool> {
    let root = PathBuf::from(".");
    let bins = blaze_spark::build(&root, &[])?;
    let start = Instant::now();
    let summary = blaze_spark::run(
        &root,
        &bins,
        a.filter.as_deref(),
        a.cached,
        a.shared,
        |o| {
            let t = if o.duration.is_zero() {
                "     -".to_string()
            } else {
                format!("{:>6.2?}", o.duration)
            };
            println!(
                "{} {t}  {}::{}", if o.passed { "PASS" } else { "FAIL" }, o.label, o.name
            );
        },
    )?;
    for f in summary.outcomes.iter().filter(|o| !o.passed) {
        println!("\n--- {}::{} ---\n{}", f.label, f.name, f.output);
    }
    let failed = summary.outcomes.iter().filter(|o| !o.passed).count();
    eprintln!(
        "spark: {} run, {} cached, {failed} failed in {:.2?}", summary.outcomes.len(),
        summary.cached, start.elapsed()
    );
    Ok(failed == 0)
}

pub fn ward(_: &WardArgs) -> Result<bool> {
    let root = PathBuf::from(".");
    let cfg = blaze_ward::Config::load(&root)?;
    let violations = blaze_ward::check(&blaze_ward::load_packages(&root)?, &cfg);
    for v in &violations {
        println!("[{}] {}", v.kind, v.message);
    }
    eprintln!("ward: {} violations", violations.len());
    Ok(violations.is_empty())
}
