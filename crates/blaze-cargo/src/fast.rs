//! Freshness fast path for `build`, `check` and `run`.
//!
//! After a successful cargo run we record every file that influenced it (rustc dep-info for
//! local crates, manifests, the lockfile, build-script `rerun-if-*` inputs and env vars) with
//! mtime and length. The next invocation just `stat`s that list: if nothing changed and the
//! outputs still exist, cargo is not started at all. `run` then execs the built binary directly.
//!
//! Registry dependencies are not tracked file by file: they are immutable for a given
//! `Cargo.lock`, which is tracked. Set `BLAZE_NO_FAST=1` to always go through cargo.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Build,
    Check,
    Run,
}

impl Kind {
    fn cargo(self) -> &'static str {
        match self {
            Kind::Build | Kind::Run => "build",
            Kind::Check => "check",
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Bin {
    name: String,
    exe: String,
    dir: String,
    /// `bin` or `example`.
    #[serde(default)]
    kind: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Record {
    inputs: Vec<(String, u64, u64)>,
    env: Vec<(String, Option<String>)>,
    outputs: Vec<String>,
    bins: Vec<Bin>,
}

/// Arguments split into cargo flags and, for `run`, the program's own arguments after `--`.
pub struct Invocation {
    pub kind: Kind,
    pub cargo_args: Vec<String>,
    pub program_args: Vec<String>,
}

impl Invocation {
    pub fn parse(kind: Kind, args: &[String]) -> Self {
        match args.iter().position(|a| a == "--") {
            Some(i) if kind == Kind::Run => {
                Invocation {
                    kind,
                    cargo_args: args[..i].to_vec(),
                    program_args: args[i + 1..].to_vec(),
                }
            }
            _ => {
                Invocation {
                    kind,
                    cargo_args: args.to_vec(),
                    program_args: Vec::new(),
                }
            }
        }
    }

    /// Every flag is part of the cache key, so any invocation can take the fast path except the
    /// ones that change what cargo prints or writes in ways we do not track.
    fn eligible(&self) -> bool {
        self.cargo_args
            .iter()
            .all(|a| {
                let a = a.as_str();
                let flag = a.split('=').next().unwrap_or(a);
                !(matches!(
                    flag, "--manifest-path" | "--message-format" | "--build-plan" |
                    "--timings" | "--future-incompat-report" | "--unit-graph" |
                    "--out-dir" | "--artifact-dir" | "-h" | "--help" | "-V" | "--version"
                    | "--list"
                ) || a.starts_with("-Z"))
            })
    }

    fn quiet(&self) -> bool {
        self.cargo_args.iter().any(|a| a == "-q" || a == "--quiet")
    }

    /// `--bin NAME` or `--example NAME`: `(name, is_example)`.
    fn target(&self) -> Option<(String, bool)> {
        let mut it = self.cargo_args.iter();
        while let Some(a) = it.next() {
            for (flag, example) in [("--bin", false), ("--example", true)] {
                if a == flag {
                    return it.next().map(|n| (n.clone(), example));
                }
                if let Some(v) = a.strip_prefix(&format!("{flag}=")) {
                    return Some((v.to_string(), example));
                }
            }
        }
        None
    }
}

/// Opt-in shared target directory: `[build] shared_target = true` in blaze.toml or
/// `BLAZE_SHARED_TARGET=1`. Dependencies compiled for one project are then reused by every other
/// project that uses the same versions, features and toolchain.
pub fn shared_target_dir() -> Option<PathBuf> {
    if std::env::var_os("CARGO_TARGET_DIR").is_some() {
        return None; // the user already chose
    }
    let enabled = std::env::var_os("BLAZE_SHARED_TARGET").is_some()
        || std::fs::read_to_string("blaze.toml")
            .ok()
            .and_then(|t| t.parse::<toml_edit::DocumentMut>().ok())
            .and_then(|d| d.get("build")?.get("shared_target")?.as_bool())
            .unwrap_or(false);
    if !enabled {
        return None;
    }
    std::env::var_os("BLAZE_TARGET_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".cache").join("blaze").join("target"))
        })
}

fn apply_target_dir(cmd: &mut Command) {
    if let Some(d) = shared_target_dir() {
        cmd.env("CARGO_TARGET_DIR", d);
    }
}

fn stat(p: &Path) -> (u64, u64) {
    std::fs::metadata(p)
        .ok()
        .and_then(|m| Some((
            m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos() as u64,
            m.len(),
        )))
        .unwrap_or((0, 0))
}

fn cache_key(inv: &Invocation) -> String {
    let mut h = blake3::Hasher::new();
    h.update(inv.kind.cargo().as_bytes());
    h.update(inv.cargo_args.join("\u{1}").as_bytes());
    if let Some(d) = shared_target_dir() {
        h.update(d.to_string_lossy().as_bytes());
    }
    h.update(std::env::current_dir().unwrap_or_default().to_string_lossy().as_bytes());
    let mut env: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| {
            k.starts_with("CARGO_") || k.starts_with("RUSTC") || k == "RUSTFLAGS"
                || k == "RUSTUP_TOOLCHAIN"
        })
        .collect();
    env.sort();
    for (k, v) in env {
        h.update(format!("{k}={v}\u{1}").as_bytes());
    }
    h.finalize().to_hex()[..16].to_string()
}

fn record_path(key: &str) -> PathBuf {
    PathBuf::from(".blaze_cache").join(format!("build-{key}.json"))
}

fn is_fresh(r: &Record) -> bool {
    !r.inputs.is_empty()
        && r.inputs.iter().all(|(p, m, l)| stat(Path::new(p)) == (*m, *l))
        && r.env.iter().all(|(k, v)| std::env::var(k).ok() == *v)
        && r.outputs.iter().all(|o| Path::new(o).exists())
}

/// Entry point for `build`, `check` and `run`. Returns the process exit code.
pub fn execute(kind: Kind, args: &[String]) -> Result<i32> {
    let inv = Invocation::parse(kind, args);
    if std::env::var_os("BLAZE_NO_FAST").is_some() || !inv.eligible() {
        return delegate(&inv);
    }
    let key = cache_key(&inv);
    let t0 = Instant::now();
    let existing: Option<Record> = std::fs::read(record_path(&key))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());

    let record = match existing.filter(is_fresh) {
        Some(r) => {
            if !inv.quiet() {
                eprintln!(
                    "    blaze: up to date, cargo not started ({:.1?})",
                    t0.elapsed(),
                );
            }
            r
        }
        None => {
            match build_and_record(&inv)? {
                (0, Some(r)) => {
                    let path = record_path(&key);
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                        let ignore = dir.join(".gitignore");
                        if !ignore.exists() {
                            let _ = std::fs::write(ignore, "*\n");
                        }
                    }
                    let _ = std::fs::write(path, serde_json::to_vec(&r)?);
                    r
                }
                (code, _) => return Ok(code),
            }
        }
    };

    if kind != Kind::Run {
        return Ok(0);
    }
    let only_bins: Vec<&Bin> = record
        .bins
        .iter()
        .filter(|b| b.kind != "example")
        .collect();
    let bin = match inv.target() {
        Some((n, example)) => {
            record.bins.iter().find(|b| b.name == n && (b.kind == "example") == example)
        }
        None if only_bins.len() == 1 => only_bins.first().copied(),
        None => None,
    };
    match bin {
        Some(b) => exec(b, &inv.program_args),
        // several binaries and no --bin: let cargo apply default-run and print its own error
        None => delegate(&inv),
    }
}

fn delegate(inv: &Invocation) -> Result<i32> {
    let mut cmd = Command::new("cargo");
    apply_target_dir(&mut cmd);
    cmd.arg(
            match inv.kind {
                Kind::Build => "build",
                Kind::Check => "check",
                Kind::Run => "run",
            },
        )
        .args(&inv.cargo_args);
    if inv.kind == Kind::Run && !inv.program_args.is_empty() {
        cmd.arg("--").args(&inv.program_args);
    }
    Ok(cmd.status().context("starting cargo")?.code().unwrap_or(1))
}

fn exec(bin: &Bin, args: &[String]) -> Result<i32> {
    let mut cmd = Command::new(&bin.exe);
    cmd.args(args).env("CARGO_MANIFEST_DIR", &bin.dir).env("CARGO_BIN_NAME", &bin.name);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        Err(err).with_context(|| format!("running {}", bin.exe))
    }
    #[cfg(not(unix))]
    {
        Ok(
            cmd
                .status()
                .with_context(|| format!("running {}", bin.exe))?
                .code()
                .unwrap_or(1),
        )
    }
}

fn is_local(package_id: &str) -> bool {
    package_id.starts_with("path+file://") || package_id.contains("(path+file://")
}

/// Run cargo with JSON messages, forwarding diagnostics, and derive the fingerprint record.
fn build_and_record(inv: &Invocation) -> Result<(i32, Option<Record>)> {
    let mut cmd = Command::new("cargo");
    apply_target_dir(&mut cmd);
    let mut child = cmd
        .arg(inv.kind.cargo())
        .arg("--message-format=json-render-diagnostics")
        .args(&inv.cargo_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("starting cargo")?;
    let stdout = child.stdout.take().context("no stdout")?;

    let cwd = std::env::current_dir()?;
    let ws = cwd
        .ancestors()
        .find(|d| d.join("Cargo.lock").exists())
        .unwrap_or(&cwd)
        .to_path_buf();
    let mut inputs: BTreeSet<PathBuf> = BTreeSet::new();
    let mut outputs: BTreeSet<String> = BTreeSet::new();
    let mut env_names: BTreeSet<String> = BTreeSet::new();
    let mut bins = Vec::new();
    let mut complete = true;

    inputs.insert(ws.join("Cargo.lock"));
    for d in cwd.ancestors() {
        inputs
            .extend([
                d.join(".cargo/config.toml"),
                d.join(".cargo/config"),
                d.join("rust-toolchain"),
                d.join("rust-toolchain.toml"),
            ]);
    }
    if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
        inputs.extend([h.join(".cargo/config.toml"), h.join(".rustup/settings.toml")]);
    }

    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let local = v["package_id"].as_str().is_some_and(is_local);
        match v["reason"].as_str() {
            Some("compiler-artifact") if local => {
                if let Some(m) = v["manifest_path"].as_str() {
                    inputs.insert(PathBuf::from(m));
                }
                for f in v["filenames"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    outputs.insert(f.to_string());
                    match dep_info_for(Path::new(f)) {
                        Some(text) => {
                            inputs
                                .extend(
                                    parse_depinfo(&text)
                                        .into_iter()
                                        .filter_map(|p| resolve(&p, &ws, &cwd)),
                                )
                        }
                        None if !f.ends_with(".rmeta") && !f.ends_with(".rlib")
                            && !f.contains(".dSYM") => complete = false,
                        None => {}
                    }
                }
                let kind_of = |k: &str| {
                    v["target"]["kind"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|x| x == k))
                };
                let kind = if kind_of("bin") {
                    "bin"
                } else if kind_of("example") {
                    "example"
                } else {
                    ""
                };
                if let (Some(exe), true) = (v["executable"].as_str(), !kind.is_empty()) {
                    bins.push(Bin {
                        kind: kind.to_string(),
                        name: v["target"]["name"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        exe: exe.to_string(),
                        dir: Path::new(v["manifest_path"].as_str().unwrap_or_default())
                            .parent()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    });
                }
            }
            Some("build-script-executed") if local => {
                let Some(out_dir) = v["out_dir"].as_str() else { continue };
                let pkg_dir = v["package_id"]
                    .as_str()
                    .and_then(|id| id.split("path+file://").nth(1))
                    .map(|s| {
                        s.split('#')
                            .next()
                            .unwrap_or(s)
                            .trim_end_matches(')')
                            .to_string()
                    });
                match std::fs::read_to_string(
                    Path::new(out_dir).with_file_name("output"),
                ) {
                    Ok(text) => {
                        let (changed, envs, any_rerun_if) = parse_build_output(&text);
                        env_names.extend(envs);
                        match (&pkg_dir, any_rerun_if) {
                            (Some(dir), true) => {
                                inputs
                                    .extend(changed.into_iter().map(|c| Path::new(dir).join(c)))
                            }
                            // no rerun-if-changed: cargo reruns on any file change in the package
                            (Some(dir), false) => walk(Path::new(dir), &mut inputs),
                            _ => complete = false,
                        }
                    }
                    Err(_) => complete = false,
                }
            }
            _ => {}
        }
    }
    let status = child.wait()?;
    let code = status.code().unwrap_or(1);
    if code != 0 || !complete || outputs.is_empty() {
        return Ok((code, None));
    }
    let record = Record {
        inputs: inputs
            .iter()
            .map(|p| {
                let (m, l) = stat(p);
                (p.to_string_lossy().into_owned(), m, l)
            })
            .collect(),
        env: env_names
            .into_iter()
            .map(|k| {
                let v = std::env::var(&k).ok();
                (k, v)
            })
            .collect(),
        outputs: outputs.into_iter().collect(),
        bins,
    };
    Ok((0, Some(record)))
}

fn dep_info_for(artifact: &Path) -> Option<String> {
    let dir = artifact.parent()?;
    let stem = artifact.file_stem()?.to_str()?;
    if stem == "build-script-build" {
        // the build script's dep-info is named `build_script_build-<hash>.d` next to it
        let found = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| {
                        n.starts_with("build_script_build-") && n.ends_with(".d")
                    })
            })?;
        return std::fs::read_to_string(found).ok();
    }
    for s in [stem, stem.strip_prefix("lib").unwrap_or(stem)] {
        if let Ok(t) = std::fs::read_to_string(dir.join(format!("{s}.d"))) {
            return Some(t);
        }
    }
    None
}

fn resolve(p: &str, ws: &Path, cwd: &Path) -> Option<PathBuf> {
    let p = Path::new(p);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    [ws, cwd].iter().map(|b| b.join(p)).find(|c| c.exists())
}

fn walk(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        if matches!(
            name.to_str(), Some("target" | ".git" | ".blaze_cache" | "node_modules")
        ) {
            continue;
        }
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.insert(p);
        }
    }
}

/// Parse a Makefile-style rustc dep-info file into the list of source files.
pub fn parse_depinfo(text: &str) -> Vec<String> {
    let joined = text.replace("\\\n", " ");
    let Some(first) = joined.lines().next() else { return Vec::new() };
    let Some((_, deps)) = first.split_once(": ") else { return Vec::new() };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = deps.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&' ') => {
                cur.push(' ');
                chars.next();
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `(rerun-if-changed paths, rerun-if-env-changed names, saw any rerun-if-changed)`.
pub fn parse_build_output(text: &str) -> (Vec<String>, Vec<String>, bool) {
    let (mut changed, mut envs) = (Vec::new(), Vec::new());
    for l in text.lines() {
        let l = l
            .strip_prefix("cargo::")
            .or_else(|| l.strip_prefix("cargo:"))
            .unwrap_or("");
        if let Some(p) = l.strip_prefix("rerun-if-changed=") {
            changed.push(p.to_string());
        } else if let Some(e) = l.strip_prefix("rerun-if-env-changed=") {
            envs.push(e.to_string());
        }
    }
    let any = !changed.is_empty();
    (changed, envs, any)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn depinfo_parsing_handles_continuations_and_escaped_spaces() {
        let d = "target/debug/deps/x-1.d: src/main.rs src/a\\ b.rs \\\n src/c.rs\n\nsrc/main.rs:\nsrc/c.rs:\n";
        assert_eq!(parse_depinfo(d), ["src/main.rs", "src/a b.rs", "src/c.rs"]);
    }

    #[test]
    fn build_script_output_lines() {
        let (c, e, any) = parse_build_output(
            "cargo:rustc-cfg=x\ncargo::rerun-if-changed=build.rs\ncargo:rerun-if-env-changed=FOO\n",
        );
        assert_eq!(
            (c, e, any),
            (vec!["build.rs".to_string()], vec!["FOO".to_string()], true),
        );
        assert!(!parse_build_output("cargo:rustc-cfg=x\n").2);
    }

    #[test]
    fn eligibility_and_run_args_split() {
        let i = Invocation::parse(
            Kind::Run,
            &args(&["--release", "--bin", "x", "--", "--flag", "v"]),
        );
        assert!(i.eligible());
        assert_eq!(i.program_args, ["--flag", "v"]);
        assert_eq!(i.target(), Some(("x".to_string(), false)));
        let e = Invocation::parse(Kind::Run, &args(&["--example=demo"]));
        assert_eq!(e.target(), Some(("demo".to_string(), true)));
        // feature and profile flags are part of the cache key, so they are fine
        assert!(
            Invocation::parse(
                    Kind::Build,
                    &args(&["--features", "a", "--all-targets", "--profile", "dev"]),
                )
                .eligible(),
        );
        // flags that change what cargo prints or writes are not
        assert!(
            !Invocation::parse(Kind::Build, &args(&["--message-format=json"])).eligible(),
        );
        assert!(!Invocation::parse(Kind::Build, &args(&["--timings"])).eligible());
        assert!(
            !Invocation::parse(Kind::Build, &args(&["-Zunstable-options"])).eligible(),
        );
        assert!(
            !Invocation::parse(Kind::Build, &args(&["--manifest-path", "x/Cargo.toml"]))
                .eligible(),
        );
    }

    #[test]
    fn stale_when_a_stamped_file_changes() {
        let dir = std::env::temp_dir()
            .join(format!("blaze-fast-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.rs");
        std::fs::write(&f, "1").unwrap();
        let (m, l) = stat(&f);
        let rec = Record {
            inputs: vec![(f.to_string_lossy().into_owned(), m, l)],
            outputs: vec![f.to_string_lossy().into_owned()],
            ..Default::default()
        };
        assert!(is_fresh(&rec));
        std::fs::write(&f, "22").unwrap();
        assert!(!is_fresh(&rec));
        std::fs::remove_dir_all(dir).ok();
    }
}
