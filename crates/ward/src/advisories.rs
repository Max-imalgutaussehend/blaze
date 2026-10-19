//! RustSec advisory checks.
//!
//! The advisory database (github.com/RustSec/advisory-db) is downloaded as one tarball, split
//! into one small JSON file per crate under the cache directory, and refreshed at most once a
//! day with a conditional request. A check then reads only the files of the crates that are in
//! `Cargo.lock`, so it costs a few milliseconds once the database is cached.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::{Config, Package, Violation};

const DB_URL: &str = "https://github.com/RustSec/advisory-db/archive/refs/heads/main.tar.gz";
const STALE_AFTER: Duration = Duration::from_secs(24 * 3600);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Advisory {
    pub id: String,
    pub package: String,
    pub title: String,
    pub url: Option<String>,
    /// `unmaintained`, `unsound` or `notice` for informational advisories; `None` for vulnerabilities.
    pub informational: Option<String>,
    pub withdrawn: bool,
    pub patched: Vec<String>,
    pub unaffected: Vec<String>,
}

pub fn default_db_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("blaze").join("advisory-db")
}

/// Parse one advisory markdown file (TOML front matter in a ```toml fence, then `# Title`).
pub fn parse_advisory(text: &str) -> Option<Advisory> {
    #[derive(Deserialize)]
    struct Front {
        advisory: Meta,
        #[serde(default)]
        versions: Versions,
    }
    #[derive(Deserialize)]
    struct Meta {
        id: String,
        package: String,
        url: Option<String>,
        informational: Option<String>,
        withdrawn: Option<toml::value::Datetime>,
    }
    #[derive(Deserialize, Default)]
    struct Versions {
        #[serde(default)]
        patched: Vec<String>,
        #[serde(default)]
        unaffected: Vec<String>,
    }
    let start = text.find("```toml")? + "```toml".len();
    let end = start + text[start..].find("```")?;
    let front: Front = toml::from_str(&text[start..end]).ok()?;
    let title = text[end + 3..]
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or("")
        .trim()
        .to_string();
    Some(Advisory {
        id: front.advisory.id,
        package: front.advisory.package,
        title,
        url: front.advisory.url,
        informational: front.advisory.informational,
        withdrawn: front.advisory.withdrawn.is_some(),
        patched: front.versions.patched,
        unaffected: front.versions.unaffected,
    })
}

/// Is `version` affected? Affected means: not patched and not unaffected.
pub fn is_affected(a: &Advisory, version: &str) -> bool {
    let Ok(v) = Version::parse(version) else { return false };
    let matches = |reqs: &[String]| {
        reqs.iter().any(|r| VersionReq::parse(r).is_ok_and(|req| req.matches(&v)))
    };
    !matches(&a.patched) && !matches(&a.unaffected)
}

fn is_fresh(db: &Path) -> bool {
    std::fs::metadata(db.join("updated"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < STALE_AFTER)
}

/// Download (conditionally) and index the database. Returns `true` if it changed.
pub fn update(db: &Path) -> Result<bool> {
    let etag_file = db.join("etag");
    let mut req = ureq::get(DB_URL).header("User-Agent", "blaze");
    if let Ok(e) = std::fs::read_to_string(&etag_file) && db.join("crates").exists() {
        req = req.header("If-None-Match", e.trim());
    }
    let mut resp = req.call().context("downloading the RustSec advisory database")?;
    std::fs::create_dir_all(db)?;
    if resp.status().as_u16() == 304 {
        std::fs::write(db.join("updated"), "")?;
        return Ok(false);
    }
    let etag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let mut per_crate: BTreeMap<String, Vec<Advisory>> = BTreeMap::new();
    let reader = flate2::read::GzDecoder::new(resp.body_mut().as_reader());
    for entry in tar::Archive::new(reader).entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let parts: Vec<&str> = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();
        // <root>/crates/<name>/RUSTSEC-YYYY-NNNN.md
        if parts.len() != 4 || parts[1] != "crates" || !parts[3].ends_with(".md") {
            continue;
        }
        let mut text = String::new();
        entry.read_to_string(&mut text)?;
        if let Some(a) = parse_advisory(&text) {
            per_crate.entry(parts[2].to_string()).or_default().push(a);
        }
    }
    if per_crate.is_empty() {
        bail!("the downloaded archive contained no advisories");
    }
    // write into a fresh directory and swap, so a crash never leaves a half-written database
    let tmp = db.join("crates.tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    for (name, list) in &per_crate {
        std::fs::write(
            tmp.join(format!("{name}.json")),
            serde_json::to_vec(list)?,
        )?;
    }
    let live = db.join("crates");
    let _ = std::fs::remove_dir_all(&live);
    std::fs::rename(&tmp, &live)?;
    if let Some(e) = etag {
        std::fs::write(&etag_file, e)?;
    }
    std::fs::write(db.join("updated"), "")?;
    Ok(true)
}

pub struct Options {
    pub offline: bool,
    pub force_update: bool,
}

/// Make sure a usable database exists. Returns a note when the check had to be degraded.
pub fn ensure_db(db: &Path, opts: &Options) -> std::result::Result<(), String> {
    let have = db.join("crates").exists();
    if opts.offline {
        return if have {
            Ok(())
        } else {
            Err("no advisory database cached and --offline given".into())
        };
    }
    if have && is_fresh(db) && !opts.force_update {
        return Ok(());
    }
    match update(db) {
        Ok(_) => Ok(()),
        Err(e) if have => {
            eprintln!(
                "ward: could not refresh the advisory database ({e:#}); using the cached copy",
            );
            Ok(())
        }
        Err(e) => Err(format!("advisory database unavailable ({e:#})")),
    }
}

/// Check `packages` against the advisories stored in `db`.
pub fn check_with_db(packages: &[Package], cfg: &Config, db: &Path) -> Vec<Violation> {
    let mut out = Vec::new();
    for p in packages {
        let Ok(bytes) = std::fs::read(
            db.join("crates").join(format!("{}.json", p.name)),
        ) else { continue };
        let Ok(list) = serde_json::from_slice::<Vec<Advisory>>(&bytes) else { continue };
        for a in list {
            if a.withdrawn || cfg.ignore_advisories.iter().any(|i| i == &a.id)
                || !is_affected(&a, &p.version)
            {
                continue;
            }
            let (kind, denied) = match a.informational.as_deref() {
                None => ("advisory", true),
                Some("unmaintained") => ("unmaintained", cfg.deny_unmaintained),
                Some("unsound") => ("unsound", cfg.deny_unsound),
                Some(_) => ("notice", false),
            };
            let fixed = if a.patched.is_empty() {
                "no fixed version".to_string()
            } else {
                format!("fixed in {}", a.patched.join(" or "))
            };
            let url = a
                .url
                .as_deref()
                .map(|u| format!(" {u}"))
                .unwrap_or_default();
            out.push(Violation {
                kind,
                warning: !denied,
                message: format!(
                    "{} {} is affected by {}: {} ({fixed}){url}",
                    p.name,
                    p.version,
                    a.id,
                    a.title,
                ),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "```toml\n[advisory]\nid = \"RUSTSEC-2020-0071\"\npackage = \"time\"\ndate = \"2020-11-18\"\nurl = \"https://github.com/time-rs/time/issues/293\"\n\n[versions]\npatched = [\">= 0.2.23\"]\nunaffected = [\"< 0.2.7\"]\n```\n\n# Potential segfault in the time crate\n\nBody text.\n";

    #[test]
    fn parses_front_matter_and_title() {
        let a = parse_advisory(SAMPLE).unwrap();
        assert_eq!(a.id, "RUSTSEC-2020-0071");
        assert_eq!(a.package, "time");
        assert_eq!(a.title, "Potential segfault in the time crate");
        assert_eq!(a.patched, [">= 0.2.23"]);
        assert!(a.informational.is_none() && !a.withdrawn);
    }

    #[test]
    fn affected_means_neither_patched_nor_unaffected() {
        let a = parse_advisory(SAMPLE).unwrap();
        assert!(is_affected(&a, "0.2.10"));
        assert!(!is_affected(&a, "0.2.23"));
        assert!(!is_affected(&a, "0.3.0"));
        assert!(!is_affected(&a, "0.2.6")); // unaffected range
        assert!(!is_affected(&a, "not-a-version"));
    }

    fn pkg(n: &str, v: &str) -> Package {
        Package {
            name: n.into(),
            version: v.into(),
            license: None,
            source: None,
        }
    }

    fn fake_db(entries: &[(&str, &[Advisory])]) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("ward-db-{}-{}", std::process::id(), entries.len()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("crates")).unwrap();
        for (name, list) in entries {
            std::fs::write(
                    dir.join("crates").join(format!("{name}.json")),
                    serde_json::to_vec(list).unwrap(),
                )
                .unwrap();
        }
        dir
    }

    #[test]
    fn check_reports_only_affected_versions_and_honours_ignores() {
        let a = parse_advisory(SAMPLE).unwrap();
        let mut unmaintained = a.clone();
        unmaintained.id = "RUSTSEC-2021-0001".into();
        unmaintained.informational = Some("unmaintained".into());
        let db = fake_db(&[("time", &[a.clone(), unmaintained])]);
        let cfg = Config::default();
        let v = check_with_db(
            &[pkg("time", "0.2.10"), pkg("time", "0.3.0"), pkg("other", "1.0.0")],
            &cfg,
            &db,
        );
        assert_eq!(
            v.iter().map(|x| (x.kind, x.warning)).collect::<Vec<_>>(),
            [("advisory", false), ("unmaintained", true)],
        );
        let ignoring = Config {
            ignore_advisories: vec!["RUSTSEC-2020-0071".into()],
            ..Config::default()
        };
        assert_eq!(
            check_with_db(&[pkg("time", "0.2.10")], &ignoring, &db).len(),
            1,
        );
        let strict = Config {
            deny_unmaintained: true,
            ..Config::default()
        };
        assert!(
            check_with_db(&[pkg("time", "0.2.10")], &strict, &db)
                .iter()
                .all(|x| !x.warning),
        );
        let _ = std::fs::remove_dir_all(db);
    }

    #[test]
    fn offline_without_a_database_degrades_with_a_message() {
        let dir = std::env::temp_dir()
            .join(format!("ward-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            ensure_db(
                    &dir,
                    &Options {
                        offline: true,
                        force_update: false,
                    },
                )
                .is_err(),
        );
    }
}
