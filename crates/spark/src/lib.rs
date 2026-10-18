//! nextest-style runner: every test runs in its own process, all tests of all
//! binaries are scheduled on one shared thread pool, and failures are summarised.
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
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
    let t0 = Instant::now();
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
        if let (Some(exe), Some(mp), Some(t)) = (
            m.executable,
            m.manifest_path,
            m.target,
        ) {
            bins.push(TestBinary {
                exe,
                package_dir: mp.parent().unwrap_or(root).to_path_buf(),
                label: t.name,
            });
        }
    }
    if std::env::var_os("SPARK_TIMINGS").is_some() {
        eprintln!(
            "spark timing: cargo build check {:?} ({} binaries)", t0.elapsed(), bins
            .len()
        );
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
    stdout.lines().filter_map(|l| l.strip_suffix(": test")).map(str::to_string).collect()
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
        Ok(o) => {
            (
                o.status.success(),
                format!(
                    "{}{}", String::from_utf8_lossy(& o.stdout),
                    String::from_utf8_lossy(& o.stderr)
                ),
            )
        }
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
pub struct Summary {
    pub outcomes: Vec<Outcome>,
    /// Tests skipped because they passed before against an identical binary.
    pub cached: usize,
}

type PassCache = HashMap<String, Vec<String>>;

fn cache_path(root: &Path) -> PathBuf {
    root.join(".blaze_cache").join("spark.json")
}

fn load_cache(root: &Path) -> PassCache {
    std::fs::read(cache_path(root))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Run all tests; `on_done` is invoked from worker threads as each test finishes.
///
/// With `use_cache`, tests that previously passed against a byte-identical test binary are
/// skipped. That is only sound for deterministic tests that read nothing but their binary.
pub fn run(
    root: &Path,
    bins: &[TestBinary],
    filter: Option<&str>,
    use_cache: bool,
    shared: bool,
    on_done: impl Fn(&Outcome) + Sync,
) -> Result<Summary> {
    let timings = std::env::var_os("SPARK_TIMINGS").is_some();
    let t_list = Instant::now();
    let mut cache = if use_cache { load_cache(root) } else { PassCache::new() };
    // List tests of all binaries in parallel. Binaries are only hashed when the cache needs them.
    let listed: Vec<(String, Vec<String>)> = bins
        .par_iter()
        .map(|b| {
            let key = if use_cache {
                format!(
                    "{}:{}", b.label, blake3::hash(& std::fs::read(& b.exe) ?).to_hex()
                )
            } else {
                String::new()
            };
            Ok((key, list_tests(b)?))
        })
        .collect::<Result<_>>()?;
    let mut cached = 0;
    // (binary, cache key, tests to run, whether that is every test of the binary)
    let mut plan: Vec<(&TestBinary, String, Vec<String>, bool)> = Vec::new();
    for (b, (key, tests)) in bins.iter().zip(listed) {
        let passed_before = cache.get(&key).cloned().unwrap_or_default();
        let total = tests.len();
        let mut to_run = Vec::new();
        for t in tests {
            if filter.is_none_or(|f| t.contains(f)) {
                if use_cache && passed_before.contains(&t) {
                    cached += 1;
                } else {
                    to_run.push(t);
                }
            }
        }
        if !to_run.is_empty() {
            let all = to_run.len() == total;
            plan.push((b, key, to_run, all));
        }
    }
    let t_run = Instant::now();
    if timings {
        let to_run: usize = plan.iter().map(|p| p.2.len()).sum();
        let hashed = if use_cache { "+hash" } else { "" };
        eprintln!(
            "spark timing: list{hashed} {:?} ({to_run} tests to run)", t_list.elapsed()
        );
    }
    let outcomes: Vec<(String, Outcome)> = if shared {
        // one process per binary, libtest schedules the tests on its own threads
        plan.par_iter()
            .flat_map(|(b, key, tests, all)| {
                let outs = run_shared(b, tests, *all);
                outs.into_iter()
                    .map(|o| {
                        on_done(&o);
                        (key.clone(), o)
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    } else {
        let jobs: Vec<(&TestBinary, &String, &String)> = plan
            .iter()
            .flat_map(|(b, key, tests, _)| tests.iter().map(move |t| (*b, key, t)))
            .collect();
        jobs.par_iter()
            .map(|(b, key, t)| {
                let o = run_one(b, t);
                on_done(&o);
                ((*key).clone(), o)
            })
            .collect()
    };
    if timings {
        eprintln!("spark timing: run {:?}", t_run.elapsed());
    }
    if use_cache {
        for (key, o) in outcomes.iter().filter(|(_, o)| o.passed) {
            cache.entry(key.clone()).or_default().push(o.name.clone());
        }
        // drop entries for binaries that no longer exist
        let live: Vec<String> = bins
            .iter()
            .filter_map(|b| std::fs::read(&b.exe).ok())
            .map(|x| blake3::hash(&x).to_hex().to_string())
            .collect();
        cache.retain(|k, _| live.iter().any(|h| k.ends_with(h.as_str())));
        std::fs::create_dir_all(cache_path(root).parent().unwrap_or(root))?;
        std::fs::write(cache_path(root), serde_json::to_vec(&cache)?)?;
    }
    Ok(Summary {
        outcomes: outcomes.into_iter().map(|(_, o)| o).collect(),
        cached,
    })
}

/// Run all `tests` of one binary in a single process (libtest's own thread pool).
fn run_shared(bin: &TestBinary, tests: &[String], all: bool) -> Vec<Outcome> {
    let mut cmd = Command::new(&bin.exe);
    cmd.current_dir(&bin.package_dir).env("CARGO_MANIFEST_DIR", &bin.package_dir);
    if !all {
        // explicit names beat a substring filter; huge lists fall back to the whole binary
        if tests.len() > 20_000 {
            return tests.iter().map(|t| run_one(bin, t)).collect();
        }
        cmd.arg("--exact").args(tests);
    }
    let start = Instant::now();
    let res = cmd.output();
    let elapsed = start.elapsed();
    let text = match &res {
        Ok(o) => {
            format!(
                "{}{}", String::from_utf8_lossy(& o.stdout), String::from_utf8_lossy(& o
                .stderr)
            )
        }
        Err(e) => e.to_string(),
    };
    let mut outs = parse_pretty(&text, &bin.label);
    // anything we asked for but never saw a result for means the binary died
    for t in tests {
        if !outs.iter().any(|o| &o.name == t) {
            outs.push(Outcome {
                label: bin.label.clone(),
                name: t.clone(),
                passed: false,
                duration: elapsed,
                output: format!("no result reported; binary output:\n{text}"),
            });
        }
    }
    outs
}

/// Parse libtest's default output: `test NAME ... ok|FAILED|ignored` plus `---- NAME stdout ----` blocks.
pub fn parse_pretty(text: &str, label: &str) -> Vec<Outcome> {
    let mut blocks: HashMap<&str, String> = HashMap::new();
    let mut current: Option<&str> = None;
    for line in text.lines() {
        if let Some(name) = line
            .strip_prefix("---- ")
            .and_then(|l| l.strip_suffix(" stdout ----"))
        {
            current = Some(name);
            blocks.entry(name).or_default();
        } else if line.starts_with("failures:") || line.starts_with("test result:") {
            current = None;
        } else if let Some(c) = current {
            let b = blocks.entry(c).or_default();
            b.push_str(line);
            b.push('\n');
        }
    }
    text.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("test ")?;
            let (name, status) = rest.rsplit_once(" ... ")?;
            if status.starts_with("ignored") {
                return None;
            }
            let passed = status == "ok";
            Some(Outcome {
                label: label.to_string(),
                name: name.to_string(),
                passed,
                duration: Duration::ZERO,
                output: if passed {
                    String::new()
                } else {
                    blocks.get(name).cloned().unwrap_or_default()
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_pretty_output_with_failure_blocks() {
        let out = "running 3 tests\ntest a::ok ... ok\ntest b::bad ... FAILED\ntest c::skip ... ignored\n\nfailures:\n\n---- b::bad stdout ----\nthread panicked at x\nboom\n\nfailures:\n    b::bad\n\ntest result: FAILED.\n";
        let r = parse_pretty(out, "bin");
        assert_eq!(r.len(), 2);
        assert!(r[0].passed && r[0].name == "a::ok");
        assert!(! r[1].passed && r[1].output.contains("boom"));
    }

    #[test]
    fn parses_terse_list() {
        let out = "a::one: test\nb: benchmark\nc::two: test\n\n2 tests, 1 benchmark\n";
        assert_eq!(parse_list(out), ["a::one", "c::two"]);
    }
}
