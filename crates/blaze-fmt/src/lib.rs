//! Parallel, cached Rust formatter built on `syn` + `prettyplease`.
//!
//! Output follows prettyplease's style, which is close to but not identical to rustfmt's.
use anyhow::{Context, Result};
use blaze_core::{Cache, hash_bytes};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
#[derive(Debug, Default)]
pub struct Report {
    /// Files that differ from formatted output (written when not in check mode).
    pub changed: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, String)>,
    pub skipped_cached: usize,
    /// Files left untouched because they contain comments the formatter cannot yet preserve.
    pub skipped_comments: Vec<PathBuf>,
    pub total: usize,
}
/// prettyplease drops `//` and `/* */` comments. Rather than destroy them we refuse to
/// format such files. Deliberately conservative: a `//` inside a string literal also
/// triggers a skip, which is safe.
pub fn has_unpreservable_comments(src: &str) -> bool {
    src.lines().any(|l| {
        let t = l.trim_start();
        let is_doc = t.starts_with("///") && !t.starts_with("////") || t.starts_with("//!");
        !is_doc && (l.contains("//") || l.contains("/*"))
    })
}

pub fn format_source(src: &str) -> Result<String> {
    let file = syn::parse_file(src).context("parse error")?;
    // prettyplease panics (`not implemented`) on syntax it cannot print, e.g. unstable items.
    std::panic::catch_unwind(|| prettyplease::unparse(&file))
        .map_err(|_| anyhow::anyhow!("unsupported syntax (formatter limitation)"))
}
enum Status {
    Cached,
    Unchanged,
    Changed,
    SkippedComments,
}

/// Returns the hash of the file's resulting content and what happened to it.
fn process(path: &Path, check: bool, cache: &Cache) -> Result<(String, Status), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let hash = hash_bytes(&bytes);
    if cache.is_clean(path, &hash) {
        return Ok((hash, Status::Cached));
    }
    let src = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    if has_unpreservable_comments(&src) {
        return Ok((hash, Status::SkippedComments));
    }
    let out = format_source(&src).map_err(|e| format!("{e:#}"))?;
    if out == src {
        return Ok((hash, Status::Unchanged));
    }
    if !check {
        std::fs::write(path, &out).map_err(|e| e.to_string())?;
    }
    Ok((hash_bytes(out.as_bytes()), Status::Changed))
}
pub fn run(root: &Path, files: &[PathBuf], check: bool) -> Result<Report> {
    let mut cache = Cache::load(root, "fmt", env!("CARGO_PKG_VERSION"));
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let results: Vec<_> = files
        .par_iter()
        .map(|path| {
            process(path, check, &cache)
                .map(|(h, s)| (path, h, s))
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
            Ok((path, hash, status)) => match status {
                Status::Cached => report.skipped_cached += 1,
                Status::Unchanged => cache.mark_clean(path, hash),
                Status::Changed => {
                    report.changed.push(path.clone());
                    if !check {
                        cache.mark_clean(path, hash);
                    }
                }
                Status::SkippedComments => report.skipped_comments.push(path.clone()),
            },
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
        assert_eq!(format_source(&once).unwrap(), once);
    }
    #[test]
    fn detects_comments_but_not_doc_comments() {
        assert!(has_unpreservable_comments("// hi\nfn f() {}"));
        assert!(has_unpreservable_comments("fn f() { /* x */ }"));
        assert!(has_unpreservable_comments("fn f() {} // trailing"));
        assert!(!has_unpreservable_comments("/// doc\n//! inner\nfn f() {}"));
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
