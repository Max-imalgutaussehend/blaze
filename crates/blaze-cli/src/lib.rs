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
    /// Only run these rules (codes, names or ALL); replaces the default set
    #[arg(long, value_delimiter = ',')]
    pub select: Vec<String>,
    /// Rules to skip, e.g. `--ignore B001,len-zero` (adds to `[ember] ignore` in blaze.toml)
    #[arg(long, value_delimiter = ',')]
    pub ignore: Vec<String>,
    /// Apply automatic fixes in place (only fixes that do not depend on types)
    #[arg(long)]
    pub fix: bool,
    /// With --fix, also apply fixes that depend on types ember cannot see (may need manual follow-up)
    #[arg(long)]
    pub unsafe_fixes: bool,
    /// Output format: text or json (one object per line)
    #[arg(long, default_value = "text")]
    pub format: String,
    /// Also run `cargo clippy` for type-aware lints that ember cannot do
    #[arg(long)]
    pub deep: bool,
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
    /// Re-run a failing test up to N times; passing on a retry is reported as flaky
    #[arg(long, default_value_t = 0)]
    pub retries: u32,
    /// Kill a test that runs longer than this many seconds (not with --shared)
    #[arg(long)]
    pub timeout: Option<f64>,
    /// Write a JUnit XML report to this path
    #[arg(long)]
    pub junit: Option<PathBuf>,
    /// Also run documentation tests (through cargo test --doc)
    #[arg(long)]
    pub doc: bool,
}

#[derive(Args)]
pub struct WardArgs {
    /// Do not use the network: check advisories against the cached database only
    #[arg(long)]
    pub offline: bool,
    /// Force a refresh of the advisory database
    #[arg(long)]
    pub update: bool,
    /// Skip the advisory check (licenses, bans, duplicates and sources only)
    #[arg(long)]
    pub no_advisories: bool,
    /// Fail if the advisory database is unavailable instead of skipping that check
    #[arg(long)]
    pub strict: bool,
}

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
        println!(
            "{:<5} {:<27} {:<8} {:<8} summary",
            "code",
            "name",
            "default",
            "fix",
        );
        for r in blaze_ember::RULES {
            println!(
                "{:<5} {:<27} {:<8} {:<8} {}",
                r.code,
                r.name,
                if r.default_on { "on" } else { "off" },
                if r.fixable { "yes" } else { "-" },
                r.summary,
            );
        }
        return Ok(true);
    }
    let start = Instant::now();
    let root = PathBuf::from(".");
    let mut select = blaze_core::config_strings(&root, "ember", "select");
    select.extend(a.select.iter().cloned());
    let mut ignore = blaze_core::config_strings(&root, "ember", "ignore");
    ignore.extend(a.ignore.iter().cloned());
    let sel = blaze_ember::Selection::resolve(&select, &ignore)
        .map_err(anyhow::Error::msg)?;
    let json = match a.format.as_str() {
        "text" => false,
        "json" => true,
        other => anyhow::bail!("unknown --format `{other}` (use text or json)"),
    };
    let files = blaze_core::discover_rust_files(&a.paths);
    // Files whose stamp matches a previous clean run (same rule selection) are skipped unread.
    let mut cache = Cache::load(
        &root,
        "ember",
        &format!("{}:{}", env!("CARGO_PKG_VERSION"), sel.fingerprint()),
    );
    let checked: Vec<(Option<Stamp>, blaze_ember::FileResult)> = files
        .par_iter()
        .filter_map(|p| {
            let stamp = Stamp::of(p);
            if stamp.is_some_and(|s| cache.quick(p, s) == Some("clean")) {
                return None;
            }
            Some((
                stamp,
                blaze_ember::lint_file(p, &sel, a.fix || a.unsafe_fixes, a.unsafe_fixes),
            ))
        })
        .collect();
    let (mut results, mut fixed) = (Vec::new(), 0);
    for (stamp, r) in checked {
        fixed += r.fixed;
        if r.diagnostics.is_empty() && r.error.is_none() {
            // a fixed file has a new stamp; record it as clean
            let stamp = if r.fixed > 0 { Stamp::of(&r.path) } else { stamp };
            cache.mark_stamped(&r.path, stamp, "", "clean");
        } else {
            results.push(r);
        }
    }
    cache.save()?;
    results.sort_by(|x, y| x.path.cmp(&y.path));
    let (mut diags, mut errs) = (0, 0);
    for r in &results {
        if let Some(e) = &r.error {
            errs += 1;
            eprintln!("{}: {e}", r.path.display());
        }
        for d in &r.diagnostics {
            diags += 1;
            if json {
                let name = blaze_ember::RULES
                    .iter()
                    .find(|x| x.code == d.code)
                    .map_or("", |x| x.name);
                println!(
                    "{{\"file\":{:?},\"line\":{},\"column\":{},\"code\":\"{}\",\"name\":\"{name}\",\"message\":{:?},\"fixable\":{}}}",
                    r.path.display().to_string(),
                    d.line,
                    d.column,
                    d.code,
                    d.message,
                    d.fix.is_some(),
                );
            } else {
                println!("{}:{d}", r.path.display());
            }
        }
    }
    let fix_note = if a.fix || a.unsafe_fixes {
        format!(", {fixed} fixed")
    } else {
        String::new()
    };
    eprintln!(
        "ember: {} files, {diags} issues{fix_note}, {errs} errors in {:.0?}",
        files.len(),
        start.elapsed(),
    );
    let mut ok = diags == 0 && errs == 0;
    if a.deep {
        eprintln!("ember: --deep runs cargo clippy for the type-aware lints");
        let status = std::process::Command::new("cargo")
            .args(["clippy", "--all-targets", "--", "-D", "warnings"])
            .status();
        match status {
            Ok(s) => ok &= s.success(),
            Err(e) => {
                eprintln!("ember: could not start cargo clippy: {e}");
                ok = false;
            }
        }
    }
    Ok(ok)
}

pub fn forge(a: &ForgeArgs) -> Result<bool> {
    let start = Instant::now();
    let files = blaze_core::discover_rust_files(&a.paths);
    let report = blaze_forge::run(&PathBuf::from("."), &files, a.check)?;
    for p in &report.changed {
        println!(
            "{} {}",
            if a.check { "would reformat" } else { "reformatted" },
            p.display(),
        );
    }
    for (p, e) in &report.errors {
        eprintln!("{}: {e}", p.display());
    }
    eprintln!(
        "forge: {} files ({} cached, {} unsupported), {} changed, {} errors in {:.0?}",
        report.total,
        report.skipped_cached,
        report.unsupported.len(),
        report.changed.len(),
        report.errors.len(),
        start.elapsed(),
    );
    Ok(report.errors.is_empty() && !(a.check && !report.changed.is_empty()))
}

pub fn spark(a: &SparkArgs) -> Result<bool> {
    let root = PathBuf::from(".");
    let bins = blaze_spark::build(&root, &[])?;
    let start = Instant::now();
    let opts = blaze_spark::Options {
        cached: a.cached,
        shared: a.shared,
        retries: a.retries,
        timeout: a.timeout.map(std::time::Duration::from_secs_f64),
    };
    let summary = blaze_spark::run(
        &root,
        &bins,
        a.filter.as_deref(),
        &opts,
        |o| {
            let t = if o.duration.is_zero() {
                "     -".to_string()
            } else {
                format!("{:>6.2?}", o.duration)
            };
            println!(
                "{} {t}  {}::{}",
                if o.passed { "PASS" } else { "FAIL" },
                o.label,
                o.name,
            );
        },
    )?;
    for f in summary.outcomes.iter().filter(|o| !o.passed) {
        println!("\n--- {}::{} ---\n{}", f.label, f.name, f.output);
    }
    for f in summary.outcomes.iter().filter(|o| o.flaky) {
        println!(
            "FLAKY {}::{} (passed after {} retr{})",
            f.label,
            f.name,
            f.retries_used,
            if f.retries_used == 1 { "y" } else { "ies" },
        );
    }
    if let Some(path) = &a.junit {
        std::fs::write(path, blaze_spark::junit_xml(&summary.outcomes))?;
    }
    let failed = summary.outcomes.iter().filter(|o| !o.passed).count();
    let flaky = summary.outcomes.iter().filter(|o| o.flaky).count();
    let timed_out = summary.outcomes.iter().filter(|o| o.timed_out && !o.passed).count();
    let mut notes = String::new();
    if flaky > 0 {
        notes.push_str(&format!(", {flaky} flaky"));
    }
    if timed_out > 0 {
        notes.push_str(&format!(", {timed_out} timed out"));
    }
    eprintln!(
        "spark: {} run, {} cached, {failed} failed{notes} in {:.2?}",
        summary.outcomes.len(),
        summary.cached,
        start.elapsed(),
    );
    let mut ok = failed == 0;
    if a.doc {
        let (doc_ok, output) = blaze_spark::run_doc_tests(&root)?;
        let result = output
            .lines()
            .filter(|l| l.starts_with("test result:"))
            .collect::<Vec<_>>()
            .join("\n");
        println!(
            "{}",
            if result.is_empty() {
                output.trim_end().to_string()
            } else {
                format!("doc-tests:\n{result}")
            },
        );
        if !doc_ok {
            println!("\n--- doc-tests ---\n{output}");
        }
        ok &= doc_ok;
    }
    Ok(ok)
}

pub fn ward(a: &WardArgs) -> Result<bool> {
    let start = Instant::now();
    let root = PathBuf::from(".");
    let cfg = blaze_ward::Config::load(&root)?;
    let packages = blaze_ward::load_packages(&root)?;
    let mut violations = blaze_ward::check(&packages, &cfg);
    if cfg.advisories.unwrap_or(true) && !a.no_advisories {
        let db = blaze_ward::advisories::default_db_dir();
        let opts = blaze_ward::advisories::Options {
            offline: a.offline,
            force_update: a.update,
        };
        match blaze_ward::advisories::ensure_db(&db, &opts) {
            Ok(()) => {
                violations
                    .extend(blaze_ward::advisories::check_with_db(&packages, &cfg, &db))
            }
            Err(note) if a.strict => anyhow::bail!("{note}"),
            Err(note) => eprintln!("ward: advisory check skipped: {note}"),
        }
    }
    let (mut failed, mut warned) = (0, 0);
    for v in &violations {
        if v.warning {
            warned += 1;
            println!("[warn {}] {}", v.kind, v.message);
        } else {
            failed += 1;
            println!("[{}] {}", v.kind, v.message);
        }
    }
    eprintln!(
        "ward: {failed} violations, {warned} warnings in {:.0?}",
        start.elapsed(),
    );
    Ok(failed == 0)
}
