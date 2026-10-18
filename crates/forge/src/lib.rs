//! Parallel, cached Rust formatter built on `syn` + `prettyplease`.
//!
//! Comments and blank lines are re-attached by [`reattach`]; files where that cannot be
//! verified are left untouched. Output follows prettyplease's style, which is close to but
//! not identical to rustfmt's.

use std::path::{Path, PathBuf};

use anyhow::Result;
use blaze_core::{Cache, Stamp, hash_bytes};
use blaze_lexer::{Kind, lex};
use rayon::prelude::*;

pub mod reattach;

#[derive(Debug, Default)]
pub struct Report {
    /// Files that differ from formatted output (written when not in check mode).
    pub changed: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, String)>,
    pub skipped_cached: usize,
    /// Files left untouched because the result could not be verified to be safe.
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

/// Files at least this large are split at top-level item boundaries and parsed in parallel,
/// so one huge generated file does not become the critical path of a run.
const CHUNK_MIN: usize = 192 * 1024;
const CHUNK_TARGET: usize = 96 * 1024;

fn unparse_whole(src: &str) -> Result<String, FormatError> {
    let file = syn::parse_file(src).map_err(|e| FormatError::Syntax(e.to_string()))?;
    // prettyplease panics (`not implemented`) on syntax it cannot print, e.g. unstable items.
    std::panic::catch_unwind(|| prettyplease::unparse(&file))
        .map_err(|_| FormatError::Unsupported("syntax the printer does not support"))
}

/// Byte offsets just after top-level items, roughly `CHUNK_TARGET` apart.
pub fn split_points(src: &str) -> Vec<usize> {
    let toks = lex(src);
    let mut depth = 0i32;
    let mut last = 0usize;
    let mut cuts = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if matches!(t.kind, Kind::Comment | Kind::DocComment) {
            continue;
        }
        match t.text(src) {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" => depth -= 1,
            "}" => {
                depth -= 1;
                if depth == 0 && t.end - last >= CHUNK_TARGET {
                    // only cut when the next token plausibly starts a new item
                    let next = toks[i + 1..]
                        .iter()
                        .find(|n| !matches!(n.kind, Kind::Comment | Kind::DocComment));
                    let ok = next
                        .is_none_or(|n| {
                            let s = n.text(src);
                            s == "#"
                                || (n.kind == Kind::Ident
                                    && !matches!(s, "as" | "else" | "where"))
                        });
                    if ok {
                        cuts.push(t.end);
                        last = t.end;
                    }
                }
            }
            ";" if depth == 0 && t.end - last >= CHUNK_TARGET => {
                cuts.push(t.end);
                last = t.end;
            }
            _ => {}
        }
    }
    cuts
}

fn unparse_chunked(src: &str) -> Result<String, FormatError> {
    let cuts = split_points(src);
    if cuts.is_empty() {
        return unparse_whole(src);
    }
    let mut bounds = vec![0];
    bounds.extend(cuts);
    bounds.push(src.len());
    let parts: Vec<Result<String, FormatError>> = bounds
        .windows(2)
        .collect::<Vec<_>>()
        .par_iter()
        .map(|w| unparse_whole(&src[w[0]..w[1]]))
        .collect();
    // any failure: fall back to the authoritative whole-file path (and its error reporting)
    let mut out = String::with_capacity(src.len());
    for p in parts {
        match p {
            Ok(s) => out.push_str(&s),
            Err(_) => return unparse_whole(src),
        }
    }
    Ok(out)
}

pub fn format_source(src: &str) -> Result<String, FormatError> {
    let formatted = if src.len() >= CHUNK_MIN {
        unparse_chunked(src)?
    } else {
        unparse_whole(src)?
    };
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

struct Outcome {
    /// Hash of the file's content after this run (empty when answered from a stat match).
    hash: String,
    status: Status,
    /// Answered from the cache without formatting.
    cached: bool,
    /// Stamp to record: taken before reading, or after our own write.
    stamp: Option<Stamp>,
    /// Answered by `stat` alone, entry is already up to date.
    quick: bool,
}

fn process(path: &Path, check: bool, cache: &Cache) -> Result<Outcome, String> {
    let stamp = Stamp::of(path);
    if let Some(st) = stamp.and_then(|s| cache.quick(path, s)).and_then(Status::from_tag)
    {
        // a cached "dirty" file must still be rewritten when not checking
        if st != Status::Dirty || check {
            return Ok(Outcome {
                hash: String::new(),
                status: st,
                cached: true,
                stamp,
                quick: true,
            });
        }
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let hash = hash_bytes(&bytes);
    if let Some(st) = cache.lookup(path, &hash).and_then(Status::from_tag)
        && (st != Status::Dirty || check)
    {
        return Ok(Outcome {
            hash,
            status: st,
            cached: true,
            stamp,
            quick: false,
        });
    }
    let src = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let out = match format_source(&src) {
        Ok(o) => o,
        Err(FormatError::Unsupported(_)) => {
            return Ok(Outcome {
                hash,
                status: Status::Unsupported,
                cached: false,
                stamp,
                quick: false,
            });
        }
        Err(FormatError::Syntax(e)) => return Err(e),
    };
    if out == src {
        return Ok(Outcome {
            hash,
            status: Status::Clean,
            cached: false,
            stamp,
            quick: false,
        });
    }
    if check {
        return Ok(Outcome {
            hash,
            status: Status::Dirty,
            cached: false,
            stamp,
            quick: false,
        });
    }
    std::fs::write(path, &out).map_err(|e| e.to_string())?;
    // reported as dirty (it changed), but the *new* content is what we remember
    Ok(Outcome {
        hash: hash_bytes(out.as_bytes()),
        status: Status::Dirty,
        cached: false,
        stamp: Stamp::of(path),
        quick: false,
    })
}

pub fn run(root: &Path, files: &[PathBuf], check: bool) -> Result<Report> {
    let mut cache = Cache::load(root, "fmt", env!("CARGO_PKG_VERSION"));
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    // Largest files first: they dominate the critical path, so start them early.
    let mut order: Vec<(&PathBuf, u64)> = files
        .iter()
        .map(|p| (p, std::fs::metadata(p).map_or(0, |m| m.len())))
        .collect();
    order.sort_by_key(|&(_, len)| std::cmp::Reverse(len));
    let results: Vec<_> = order
        .par_iter()
        .map(|&(path, _)| {
            process(path, check, &cache)
                .map(|o| (path, o))
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
            Ok((path, o)) => {
                report.skipped_cached += usize::from(o.cached);
                match o.status {
                    Status::Unsupported => report.unsupported.push(path.clone()),
                    Status::Dirty => report.changed.push(path.clone()),
                    Status::Clean => {}
                }
                if o.quick {
                    continue;
                }
                // in write mode a Dirty file now holds the formatted content, i.e. it is clean
                let tag = if o.status == Status::Dirty && !check {
                    "clean"
                } else {
                    o.status.tag()
                };
                cache.mark_stamped(path, o.stamp, &o.hash, tag);
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
        assert!(matches!(format_source("fn ("), Err(FormatError::Syntax(_))));
    }

    /// A big file with items that end in `}` and `;` and tricky `use a::{b};` shapes.
    fn big_source() -> String {
        let mut s = String::from(
            "//! crate docs\n#![allow(dead_code)]\nuse std::{fmt, io};\n",
        );
        for i in 0..3000 {
            s.push_str(
                &format!(
                    "/// item {i}\npub struct S{i} {{ pub a: u32, pub b: [u8; 4] }}\nconst C{i}: S{i} = S{i} {{ a: {i}, b: [0; 4] }};\nimpl S{i} {{ pub fn get(&self) -> u32 {{ // trailing\n        self.a + {i} }} }}\nuse self::S{i} as T{i};\n"
                ),
            );
        }
        s
    }

    #[test]
    fn chunked_formatting_equals_whole_file_formatting() {
        let src = big_source();
        assert!(src.len() > CHUNK_MIN, "test source must exceed the chunking threshold");
        assert!(! split_points(& src).is_empty());
        let whole = reattach::reattach(&src, &unparse_whole(&src).unwrap());
        let chunked = format_source(&src).ok();
        assert!(whole.is_some() && whole == chunked);
    }

    #[test]
    fn split_points_never_cut_inside_braces_or_before_semicolons() {
        let src = big_source();
        for c in split_points(&src) {
            let before = src[..c].trim_end();
            assert!(before.ends_with('}') || before.ends_with(';'));
            assert!(! src[c..].trim_start().starts_with(';'));
        }
    }
}
