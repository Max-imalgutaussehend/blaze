//! Native `add` and `remove`: edit `Cargo.toml` in place (comments and formatting preserved)
//! and resolve versions from the crates.io sparse index, one request per crate, in parallel.
//!
//! Unsupported flags (`--git`, `--path`, `--rename`, `-p`, ...) and virtual workspaces are
//! forwarded to `cargo add` / `cargo remove`. `Cargo.lock` is not touched; cargo refreshes it
//! on the next build.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use semver::{Version, VersionReq};
use serde::Deserialize;
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value};

#[derive(Debug, Default, PartialEq)]
pub struct AddOpts {
    pub crates: Vec<(String, Option<String>)>,
    pub table: &'static str,
    pub features: Vec<String>,
    pub no_default: bool,
    pub optional: bool,
}

fn table_for(dev: bool, build: bool) -> &'static str {
    match (dev, build) {
        (true, _) => "dev-dependencies",
        (_, true) => "build-dependencies",
        _ => "dependencies",
    }
}

/// `None` means "use cargo": some flag or shape is not handled natively.
pub fn parse_add(args: &[String]) -> Option<AddOpts> {
    let mut o = AddOpts {
        table: "dependencies",
        ..Default::default()
    };
    let (mut dev, mut build) = (false, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-D" | "--dev" => dev = true,
            "-B" | "--build" => build = true,
            "--no-default-features" => o.no_default = true,
            "--optional" => o.optional = true,
            "-F" | "--features" => {
                o.features
                    .extend(
                        it
                            .next()?
                            .split([',', ' '])
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                    )
            }
            s if s.starts_with("--features=") => {
                o.features
                    .extend(
                        s["--features=".len()..]
                            .split([',', ' '])
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                    )
            }
            s if s.starts_with('-') => return None,
            s => {
                let (n, v) = match s.split_once('@') {
                    Some((n, v)) => (n, Some(v.to_string())),
                    None => (s, None),
                };
                o.crates.push((n.to_string(), v));
            }
        }
    }
    o.table = table_for(dev, build);
    (!o.crates.is_empty() && !(dev && build)).then_some(o)
}

#[derive(Deserialize)]
struct IndexLine {
    vers: String,
    #[serde(default)]
    yanked: bool,
    #[serde(default)]
    features: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    features2: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    deps: Vec<IndexDep>,
}

#[derive(Deserialize)]
struct IndexDep {
    name: String,
    #[serde(default)]
    optional: bool,
}

pub fn index_path(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    match n.len() {
        1 => format!("1/{n}"),
        2 => format!("2/{n}"),
        3 => format!("3/{}/{n}", & n[..1]),
        _ => format!("{}/{}/{n}", & n[..2], & n[2..4]),
    }
}

struct Resolved {
    version: String,
    valid_features: BTreeSet<String>,
}

/// Sparse-index lookups are cached on disk with their ETag, so a repeat only costs a 304.
fn cache_file(rel: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("blaze").join("index").join(rel.replace('/', "_")))
}

fn fetch_index(name: &str) -> Result<String> {
    let rel = index_path(name);
    let url = format!("https://index.crates.io/{rel}");
    let cached = cache_file(&rel).and_then(|p| std::fs::read_to_string(p).ok());
    // cached file layout: first line = etag, rest = body
    let (etag, cached_body) = cached
        .as_deref()
        .and_then(|c| c.split_once('\n'))
        .map_or((None, None), |(e, b)| (Some(e.to_string()), Some(b.to_string())));
    let mut req = ureq::get(&url).header("User-Agent", "blaze");
    if let Some(e) = etag.as_deref().filter(|e| !e.is_empty()) {
        req = req.header("If-None-Match", e);
    }
    let mut resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(404)) => {
            bail!("the crate `{name}` could not be found in the crates.io index")
        }
        Err(e) => return Err(e).with_context(|| format!("fetching {url}")),
    };
    if resp.status().as_u16() == 304 && let Some(b) = cached_body {
        return Ok(b);
    }
    let new_etag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = resp.body_mut().read_to_string()?;
    if let (Some(path), Some(e)) = (cache_file(&rel), new_etag) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, format!("{e}\n{body}"));
    }
    Ok(body)
}

fn fetch(name: &str, req: Option<&str>) -> Result<Resolved> {
    let body = fetch_index(name)?;
    let lines: Vec<IndexLine> = body
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let req = req
        .map(|r| {
            VersionReq::parse(r)
                .with_context(|| format!("invalid version requirement `{r}`"))
        })
        .transpose()?;
    let versions = lines
        .iter()
        .filter(|l| !l.yanked)
        .filter_map(|l| Version::parse(&l.vers).ok().map(|v| (v, l)));
    let pick = |stable_only: bool| {
        versions
            .clone()
            .filter(|(v, _)| {
                (!stable_only || v.pre.is_empty())
                    && req.as_ref().is_none_or(|r| r.matches(v))
            })
            .max_by(|a, b| a.0.cmp(&b.0))
    };
    let (v, line) = pick(true)
        .or_else(|| pick(false))
        .with_context(|| format!("no matching version of `{name}` found"))?;
    let mut valid: BTreeSet<String> = line
        .features
        .keys()
        .chain(line.features2.keys())
        .cloned()
        .collect();
    valid.extend(line.deps.iter().filter(|d| d.optional).map(|d| d.name.clone()));
    Ok(Resolved {
        version: v.to_string(),
        valid_features: valid,
    })
}

pub struct Spec {
    pub name: String,
    /// What gets written: the requirement the user gave, or the resolved version.
    pub version: String,
    pub table: &'static str,
    pub features: Vec<String>,
    pub no_default: bool,
    pub optional: bool,
}

/// Insert or update a dependency, preserving comments and formatting.
pub fn edit_add(text: &str, s: &Spec) -> Result<String> {
    let mut doc: DocumentMut = text.parse().context("parsing Cargo.toml")?;
    let table = doc
        .entry(s.table)
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .with_context(|| format!("[{}] is not a table", s.table))?;
    let was_sorted = {
        let keys: Vec<&str> = table.iter().map(|(k, _)| k).collect();
        keys.windows(2).all(|w| w[0] <= w[1])
    };
    let mut features: BTreeSet<String> = s.features.iter().cloned().collect();
    let (mut no_default, mut optional) = (s.no_default, s.optional);
    if let Some(existing) = table.get(&s.name) {
        // keep what is already declared
        if let Some(t) = existing.as_inline_table() {
            features
                .extend(
                    t
                        .get("features")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_string),
                );
            no_default
                |= t.get("default-features").and_then(Value::as_bool) == Some(false);
            optional |= t.get("optional").and_then(Value::as_bool) == Some(true);
        } else if let Some(t) = existing.as_table() {
            features
                .extend(
                    t
                        .get("features")
                        .and_then(Item::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_string),
                );
            no_default
                |= t.get("default-features").and_then(Item::as_bool) == Some(false);
            optional |= t.get("optional").and_then(Item::as_bool) == Some(true);
        }
    }
    let item = if features.is_empty() && !no_default && !optional {
        toml_edit::value(s.version.clone())
    } else {
        let mut t = InlineTable::new();
        t.insert("version", s.version.clone().into());
        if no_default {
            t.insert("default-features", false.into());
        }
        if optional {
            t.insert("optional", true.into());
        }
        if !features.is_empty() {
            t.insert(
                "features",
                Value::Array(
                    features.iter().map(|f| Value::from(f.as_str())).collect::<Array>(),
                ),
            );
        }
        Item::Value(Value::InlineTable(t))
    };
    table.insert(&s.name, item);
    if was_sorted {
        table.sort_values();
    }
    Ok(doc.to_string())
}

/// Remove `name` from the given tables; returns the new text and the tables it was found in.
pub fn edit_remove(
    text: &str,
    name: &str,
    tables: &[&str],
) -> Result<(String, Vec<String>)> {
    let mut doc: DocumentMut = text.parse().context("parsing Cargo.toml")?;
    let mut found = Vec::new();
    for t in tables {
        if let Some(tab) = doc.get_mut(t).and_then(Item::as_table_mut)
            && tab.remove(name).is_some()
        {
            found.push(t.to_string());
        }
    }
    Ok((doc.to_string(), found))
}

fn find_manifest() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()?
        .ancestors()
        .map(|d| d.join("Cargo.toml"))
        .find(|p| p.exists())
}

fn forward(sub: &str, args: &[String]) -> Result<i32> {
    Ok(
        Command::new("cargo")
            .arg(sub)
            .args(args)
            .status()
            .context("starting cargo")?
            .code()
            .unwrap_or(1),
    )
}

pub fn add(args: &[String]) -> Result<i32> {
    let (Some(opts), Some(manifest)) = (parse_add(args), find_manifest()) else {
        return forward("add", args)
    };
    let text = std::fs::read_to_string(&manifest)?;
    // virtual workspaces need `-p`; let cargo handle them
    if !text.parse::<DocumentMut>().is_ok_and(|d| d.contains_key("package")) {
        return forward("add", args);
    }
    let resolved: Vec<Result<Resolved>> = std::thread::scope(|s| {
        let hs: Vec<_> = opts
            .crates
            .iter()
            .map(|(n, r)| s.spawn(move || fetch(n, r.as_deref())))
            .collect();
        hs.into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("fetch thread panicked")))
            })
            .collect()
    });
    let mut text = text;
    for ((name, req), r) in opts.crates.iter().zip(resolved) {
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e:#}");
                return Ok(1);
            }
        };
        if let Some(bad) = opts.features.iter().find(|f| !r.valid_features.contains(*f))
        {
            eprintln!(
                "error: unrecognized feature `{bad}` for crate `{name}`\navailable features: {}",
                r.valid_features.iter().cloned().collect::< Vec < _ >> ().join(", ")
            );
            return Ok(1);
        }
        let spec = Spec {
            name: name.clone(),
            version: req.clone().unwrap_or_else(|| r.version.clone()),
            table: opts.table,
            features: opts.features.clone(),
            no_default: opts.no_default,
            optional: opts.optional,
        };
        text = edit_add(&text, &spec)?;
        let feats = if opts.features.is_empty() {
            String::new()
        } else {
            format!(" with features: {}", opts.features.join(", "))
        };
        eprintln!("      Adding {name} v{} to {}{feats}", r.version, opts.table);
    }
    std::fs::write(&manifest, text)?;
    Ok(0)
}

pub fn remove(args: &[String]) -> Result<i32> {
    let (mut dev, mut build) = (false, false);
    let mut names = Vec::new();
    for a in args {
        match a.as_str() {
            "-D" | "--dev" => dev = true,
            "-B" | "--build" => build = true,
            s if s.starts_with('-') => return forward("remove", args),
            s => names.push(s.to_string()),
        }
    }
    let Some(manifest) = find_manifest().filter(|_| !names.is_empty()) else {
        return forward("remove", args)
    };
    let tables: Vec<&str> = if dev || build {
        vec![table_for(dev, build)]
    } else {
        vec!["dependencies", "dev-dependencies", "build-dependencies"]
    };
    let mut text = std::fs::read_to_string(&manifest)?;
    for n in &names {
        let (t, found) = edit_remove(&text, n, &tables)?;
        if found.is_empty() {
            eprintln!(
                "error: the dependency `{n}` could not be found in {}", tables
                .join(" or ")
            );
            return Ok(1);
        }
        for f in found {
            eprintln!("    Removing {n} from {f}");
        }
        text = t;
    }
    std::fs::write(&manifest, text)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sparse_index_paths() {
        assert_eq!(index_path("a"), "1/a");
        assert_eq!(index_path("ab"), "2/ab");
        assert_eq!(index_path("syn"), "3/s/syn");
        assert_eq!(index_path("Serde"), "se/rd/serde");
    }

    #[test]
    fn parses_supported_flags_and_rejects_the_rest() {
        let o = parse_add(&args(&["serde@1", "-F", "derive,rc", "--optional", "tokio"]))
            .unwrap();
        assert_eq!(
            o.crates, [("serde".into(), Some("1".into())), ("tokio".into(), None)]
        );
        assert_eq!(o.features, ["derive", "rc"]);
        assert!(o.optional);
        assert_eq!(
            parse_add(& args(& ["x", "--dev"])).unwrap().table, "dev-dependencies"
        );
        assert!(parse_add(& args(& ["x", "--git", "u"])).is_none());
        assert!(parse_add(& args(& [])).is_none());
    }

    const MANIFEST: &str = "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n# keep me\n[dependencies]\nanyhow = \"1\"\nzzz = \"2\"\n";

    fn spec(name: &str, features: &[&str]) -> Spec {
        Spec {
            name: name.into(),
            version: "1.0.0".into(),
            table: "dependencies",
            features: features.iter().map(|s| s.to_string()).collect(),
            no_default: false,
            optional: false,
        }
    }

    #[test]
    fn add_keeps_comments_and_sort_order() {
        let out = edit_add(MANIFEST, &spec("serde", &["derive"])).unwrap();
        assert!(out.contains("# keep me"));
        let a = out.find("anyhow").unwrap();
        let s = out.find("serde").unwrap();
        let z = out.find("zzz").unwrap();
        assert!(a < s && s < z, "{out}");
        assert!(
            out.contains("serde = { version = \"1.0.0\", features = [\"derive\"] }"),
            "{out}"
        );
    }

    #[test]
    fn add_merges_features_into_existing_entry() {
        let once = edit_add(MANIFEST, &spec("serde", &["derive"])).unwrap();
        let twice = edit_add(&once, &spec("serde", &["rc"])).unwrap();
        assert!(twice.contains("features = [\"derive\", \"rc\"]"), "{twice}");
    }

    #[test]
    fn remove_reports_where_it_was_found() {
        let (out, found) = edit_remove(
                MANIFEST,
                "zzz",
                &["dependencies", "dev-dependencies"],
            )
            .unwrap();
        assert_eq!(found, ["dependencies"]);
        assert!(! out.contains("zzz") && out.contains("anyhow"));
        assert!(edit_remove(MANIFEST, "nope", & ["dependencies"]).unwrap().1.is_empty());
    }
}
