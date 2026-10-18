//! Dependency policy checks over `cargo metadata`: licenses, duplicate versions,
//! banned crates and unapproved sources. Runs offline; advisory (RustSec) checks
//! are not implemented yet.
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    /// SPDX identifiers that are acceptable. Empty = license check disabled.
    pub allow_licenses: Vec<String>,
    /// Crate names that must not appear in the graph.
    pub ban: Vec<String>,
    /// Flag crates present in multiple versions.
    pub deny_duplicates: bool,
    /// Flag crates from sources other than crates.io (git, alternative registries).
    pub deny_unknown_sources: bool,
}
impl Config {
    /// Load the `[deny]` table from `blaze.toml` if present.
    pub fn load(root: &Path) -> Result<Self> {
        #[derive(Deserialize, Default)]
        struct File {
            deny: Option<Config>,
        }
        let path = root.join("blaze.toml");
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)?;
        let f: File =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(f.deny.unwrap_or_default())
    }
}
#[derive(Debug, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub license: Option<String>,
    pub source: Option<String>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Violation {
    pub kind: &'static str,
    pub message: String,
}
pub fn load_packages(manifest_dir: &Path) -> Result<Vec<Package>> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(manifest_dir)
        .output()
        .context("running cargo metadata")?;
    if !out.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    #[derive(Deserialize)]
    struct Meta {
        packages: Vec<Package>,
    }
    Ok(serde_json::from_slice::<Meta>(&out.stdout)?.packages)
}
/// Evaluate an SPDX expression (`A OR B`, `A AND B`, `A/B`, parentheses, `WITH` exceptions)
/// against the allow-list.
pub fn license_allowed(expr: &str, allowed: &[String]) -> bool {
    let normalized = expr
        .replace('/', " OR ")
        .replace('(', " ( ")
        .replace(')', " ) ");
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let mut pos = 0;
    parse_or(&tokens, &mut pos, allowed)
}
fn parse_or(t: &[&str], pos: &mut usize, allowed: &[String]) -> bool {
    let mut result = parse_and(t, pos, allowed);
    while t.get(*pos).is_some_and(|x| x.eq_ignore_ascii_case("or")) {
        *pos += 1;
        let rhs = parse_and(t, pos, allowed);
        result |= rhs;
    }
    result
}
fn parse_and(t: &[&str], pos: &mut usize, allowed: &[String]) -> bool {
    let mut result = parse_atom(t, pos, allowed);
    while t.get(*pos).is_some_and(|x| x.eq_ignore_ascii_case("and")) {
        *pos += 1;
        let rhs = parse_atom(t, pos, allowed);
        result &= rhs;
    }
    result
}
fn parse_atom(t: &[&str], pos: &mut usize, allowed: &[String]) -> bool {
    match t.get(*pos) {
        Some(&"(") => {
            *pos += 1;
            let r = parse_or(t, pos, allowed);
            if t.get(*pos) == Some(&")") {
                *pos += 1;
            }
            r
        }
        Some(id) => {
            *pos += 1;
            if t.get(*pos).is_some_and(|x| x.eq_ignore_ascii_case("with")) {
                *pos += 2; // exception identifiers only add permissions
            }
            let id = id.trim_end_matches('+');
            allowed.iter().any(|a| a.trim_end_matches('+') == id)
        }
        None => false,
    }
}
pub fn check(packages: &[Package], cfg: &Config) -> Vec<Violation> {
    let mut v = Vec::new();
    let mut versions: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for p in packages {
        versions.entry(&p.name).or_default().push(&p.version);
        if cfg.ban.iter().any(|b| b == &p.name) {
            v.push(Violation {
                kind: "ban",
                message: format!("{} {} is banned", p.name, p.version),
            });
        }
        if !cfg.allow_licenses.is_empty() {
            match &p.license {
                None => v.push(Violation {
                    kind: "license",
                    message: format!("{} {} has no license field", p.name, p.version),
                }),
                Some(l) if !license_allowed(l, &cfg.allow_licenses) => v.push(Violation {
                    kind: "license",
                    message: format!("{} {} license `{l}` is not allowed", p.name, p.version),
                }),
                _ => {}
            }
        }
        if cfg.deny_unknown_sources
            && let Some(src) = &p.source
            && !src.starts_with("registry+https://github.com/rust-lang/crates.io-index")
            && !src.starts_with("sparse+https://index.crates.io")
        {
            v.push(Violation {
                kind: "source",
                message: format!("{} {} comes from `{src}`", p.name, p.version),
            });
        }
    }
    if cfg.deny_duplicates {
        for (name, vs) in versions.iter().filter(|(_, vs)| vs.len() > 1) {
            v.push(Violation {
                kind: "duplicate",
                message: format!("{name} appears in versions {}", vs.join(", ")),
            });
        }
    }
    v
}
#[cfg(test)]
mod tests {
    use super::*;
    fn allow(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn spdx_expressions() {
        let a = allow(&["MIT", "Apache-2.0"]);
        assert!(license_allowed("MIT OR GPL-3.0", &a));
        assert!(license_allowed("MIT/Apache-2.0", &a));
        assert!(!license_allowed("MIT AND GPL-3.0", &a));
        assert!(license_allowed("(MIT OR GPL-3.0) AND Apache-2.0", &a));
        assert!(license_allowed("Apache-2.0 WITH LLVM-exception", &a));
        assert!(!license_allowed("GPL-3.0", &a));
    }
    fn pkg(name: &str, version: &str, license: Option<&str>) -> Package {
        Package {
            name: name.into(),
            version: version.into(),
            license: license.map(Into::into),
            source: None,
        }
    }
    #[test]
    fn detects_violations() {
        let cfg = Config {
            allow_licenses: allow(&["MIT"]),
            ban: vec!["openssl".into()],
            deny_duplicates: true,
            deny_unknown_sources: false,
        };
        let pkgs = [
            pkg("a", "1.0.0", Some("MIT")),
            pkg("a", "2.0.0", Some("MIT")),
            pkg("openssl", "0.1.0", Some("MIT")),
            pkg("b", "1.0.0", Some("GPL-3.0")),
            pkg("c", "1.0.0", None),
        ];
        let kinds: Vec<_> = check(&pkgs, &cfg).iter().map(|v| v.kind).collect();
        assert_eq!(kinds, ["ban", "license", "license", "duplicate"]);
    }
}
