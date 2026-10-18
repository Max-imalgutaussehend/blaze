//! Parallel, cached Rust formatter built on `syn` + `prettyplease`.
//!
//! Comments and blank lines are re-attached by [`reattach`]; files where that cannot be
//! verified are left untouched. Output follows prettyplease's style, which is close to but not identical to rustfmt's.
use anyhow::Result;
use blaze_core::{Cache, hash_bytes};
use rayon::prelude::*;
pub mod reattach;

use std::path::{Path, PathBuf};
#[derive(Debug, Default)]
pub struct Report {
    /// Files that differ from formatted output (written when not in check mode).
    pub changed: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, String)>,
    pub skipped_cached: usize,
    /// Files left untouched because they contain comments the formatter cannot yet preserve.
    pub unsupported: Vec<PathBuf>,
    pub total: usize,
}
#[derive(Debug)]
pub enum FormatError {
    /// The input is not valid Rust.
    Syntax(String),
    /// Valid Rust that forge cannot (yet) format without risking loss of information.
    Unsupported(&'static str),
}

pub fn format_source(src: &str) -> Result<String, FormatError> {
    let file = syn::parse_file(src).map_err(|e| FormatError::Syntax(e.to_string()))?;
    // prettyplease panics (`not implemented`) on syntax it cannot print, e.g. unstable items.
    let formatted = std::panic::catch_unwind(|| prettyplease::unparse(&file))
        .map_err(|_| FormatError::Unsupported("syntax the printer does not support"))?;
    reattach::reattach(src, &formatted)
        .ok_or(FormatError::Unsupported("could not safely preserve comments"))
}
#[derive(Clone, Copy, PartialEq)]
enum Status {
    Clean,
    /// Would change when formatted (cached across `--check` runs).
    Dirty,
    Unsupported,
}

impl Status {
    fn tag(self) -> &'static str {
        match self {
            Status::Clean => "clean",
            Status::Dirty => "dirty",
            Status::Unsupported => "unsupported",
        }
    }

    fn from_tag(t: &str) -> Option<Self> {
        Some(
            match t {
                "clean" => Status::Clean,
                "dirty" => Status::Dirty,
                "unsupported" => Status::Unsupported,
                _ => return None,
            },
        )
    }
}

/// Returns the hash that identifies the file's content *after* this run, the resulting
/// status, and whether the answer came from the cache.
fn process(
    path: &Path,
    check: bool,
    cache: &Cache,
) -> Result<(String, Status, bool), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let hash = hash_bytes(&bytes);
    if let Some(st) = cache.lookup(path, &hash).and_then(Status::from_tag) {
        // a cached "dirty" file must still be rewritten when not checking
        if st != Status::Dirty || check {
            return Ok((hash, st, true));
        }
    }
    let src = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let out = match format_source(&src) {
        Ok(o) => o,
        Err(FormatError::Unsupported(_)) => return Ok((hash, Status::Unsupported, false)),
        Err(FormatError::Syntax(e)) => return Err(e),
    };
    if out == src {
        return Ok((hash, Status::Clean, false));
    }
    if check {
        return Ok((hash, Status::Dirty, false));
    }
    std::fs::write(path, &out).map_err(|e| e.to_string())?;
    // report as dirty (it changed) but remember the *new* content as clean
    Ok((hash_bytes(out.as_bytes()), Status::Dirty, false))
}

pub fn run(root: &Path, files: &[PathBuf], check: bool) -> Result<Report> {
    let mut cache = Cache::load(root, "fmt", env!("CARGO_PKG_VERSION"));
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    // Largest files first: they dominate the critical path, so start them early.
    let mut order: Vec<&PathBuf> = files.iter().collect();
    order
        .sort_by_cached_key(|p| std::cmp::Reverse(
            std::fs::metadata(p).map_or(0, |m| m.len()),
        ));
    let results: Vec<_> = order
        .par_iter()
        .map(|p| *p)
        .map(|path| {
            process(path, check, &cache)
                .map(|(h, s, c)| (path, h, s, c))
                .map_err(|e| (path.clone(), e))
        })
        .collect();
    std::panic::set_hook(prev_hook);

    let mut report = Report {
        total: files.len(),
        ..Default::default()
    };
    for r in results {
        match r {
            Ok((path, hash, status, cached)) => {
                report.skipped_cached += usize::from(cached);
                match status {
                    Status::Unsupported => report.unsupported.push(path.clone()),
                    Status::Dirty => {
                        report.changed.push(path.clone());
                        // in write mode the file now holds the formatted content
                        cache.mark(path, &hash, if check { "dirty" } else { "clean" });
                    }
                    Status::Clean => cache.mark(path, &hash, "clean"),
                }
                if status == Status::Unsupported {
                    cache.mark(path, &hash, status.tag());
                }
            }
            Err(e) => report.errors.push(e),
        }
    }
    cache.save()?;
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_and_is_idempotent() {
        let once = format_source("fn  main( ){let x=1;}").unwrap();
        assert_eq!(once, "fn main() {\n    let x = 1;\n}\n");
        assert_eq!(format_source(& once).unwrap(), once);
    }
    #[test]
    fn preserves_comments_and_blank_lines() {
        let src = "// head\nfn  main(){\n    let x=1; // one\n\n    // about y\n    let y=2;\n}\n";
        let out = format_source(src).unwrap();
        assert_eq!(
            out,
            "// head\nfn main() {\n    let x = 1; // one\n\n    // about y\n    let y = 2;\n}\n"
        );
        assert_eq!(format_source(& out).unwrap(), out);
    }

    #[test]
    fn block_and_trailing_file_comments() {
        let src = "fn f() { /* inner */ }\n// end\n";
        let out = format_source(src).unwrap();
        assert!(out.contains("/* inner */") && out.contains("// end"));
    }

    #[test]
    fn unsupported_syntax_is_an_error_not_a_panic() {
        let src = "#[eii(x)] pub fn f(d: &mut [u8]) -> Result<(), ()>;";
        let _ = format_source(src); // must not unwind
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(format_source("fn (").is_err());
    }
}
