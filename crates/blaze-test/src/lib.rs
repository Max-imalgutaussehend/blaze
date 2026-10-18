//! nextest-style runner: every test runs in its own process, all tests of all
//! binaries are scheduled on one shared thread pool, and failures are summarised.
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
pub struct TestBinary {
    pub exe: PathBuf,
    pub package_dir: PathBuf,
    pub label: String,
}
#[derive(Debug)]
pub struct Outcome {
    pub label: String,
    pub name: String,
    pub passed: bool,
    pub duration: Duration,
    pub output: String,
}
/// Build all test binaries via cargo and return them.
pub fn build(root: &Path, extra_args: &[String]) -> Result<Vec<TestBinary>> {
    #[derive(Deserialize)]
    struct Msg {
        reason: String,
        executable: Option<PathBuf>,
        manifest_path: Option<PathBuf>,
        target: Option<Target>,
        profile: Option<Profile>,
    }
    #[derive(Deserialize)]
    struct Target {
        name: String,
    }
    #[derive(Deserialize)]
    struct Profile {
        test: bool,
    }
    let out = Command::new("cargo")
        .args([
            "test",
            "--no-run",
            "--workspace",
            "--message-format=json-render-diagnostics",
        ])
        .args(extra_args)
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .context("running cargo test --no-run")?;
    if !out.status.success() {
        bail!("build failed");
    }
    let mut bins = Vec::new();
    for line in out.stdout.split(|b| *b == b'\n') {
        let Ok(m) = serde_json::from_slice::<Msg>(line) else {
            continue;
        };
        if m.reason != "compiler-artifact" || !m.profile.is_some_and(|p| p.test) {
            continue;
        }
        if let (Some(exe), Some(mp), Some(t)) = (m.executable, m.manifest_path, m.target) {
            bins.push(TestBinary {
                exe,
                package_dir: mp.parent().unwrap_or(root).to_path_buf(),
                label: t.name,
            });
        }
    }
    Ok(bins)
}
fn list_tests(bin: &TestBinary) -> Result<Vec<String>> {
    let out = Command::new(&bin.exe)
        .args(["--list", "--format", "terse"])
        .current_dir(&bin.package_dir)
        .output()?;
    Ok(parse_list(&String::from_utf8_lossy(&out.stdout)))
}
/// Parse libtest `--list --format terse` output; ignores benchmarks.
pub fn parse_list(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .map(str::to_string)
        .collect()
}
fn run_one(bin: &TestBinary, name: &str) -> Outcome {
    let start = Instant::now();
    let res = Command::new(&bin.exe)
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .current_dir(&bin.package_dir)
        .env("CARGO_MANIFEST_DIR", &bin.package_dir)
        .output();
    let duration = start.elapsed();
    let (passed, output) = match res {
        Ok(o) => (
            o.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
        ),
        Err(e) => (false, e.to_string()),
    };
    Outcome {
        label: bin.label.clone(),
        name: name.to_string(),
        passed,
        duration,
        output,
    }
}
/// Run all tests; `on_done` is invoked from worker threads as each test finishes.
pub fn run(
    bins: &[TestBinary],
    filter: Option<&str>,
    on_done: impl Fn(&Outcome) + Sync,
) -> Result<Vec<Outcome>> {
    let mut jobs = Vec::new();
    for b in bins {
        for t in list_tests(b)? {
            if filter.is_none_or(|f| t.contains(f)) {
                jobs.push((b, t));
            }
        }
    }
    Ok(jobs
        .par_iter()
        .map(|(b, t)| {
            let o = run_one(b, t);
            on_done(&o);
            o
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_terse_list() {
        let out = "a::one: test\nb: benchmark\nc::two: test\n\n2 tests, 1 benchmark\n";
        assert_eq!(parse_list(out), ["a::one", "c::two"]);
    }
}
