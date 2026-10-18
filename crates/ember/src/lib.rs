//! Ember: token-level Rust linter. No syntax tree is built; every rule is a pattern over
//! the token stream produced by `blaze_core::lexer`, so throughput is limited by memory
//! bandwidth rather than parsing. The trade-off: rules cannot use type information.

use std::fmt;
use std::path::{Path, PathBuf};

use blaze_core::lexer::{Kind, Token, lex};
use rayon::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub line: usize,
    pub column: usize,
}

pub struct Rule {
    pub code: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
}

pub const RULES: &[Rule] = &[
    Rule {
        code: "B001",
        name: "unwrap-used",
        summary: "`.unwrap()` can panic; propagate the error or use `expect`",
    },
    Rule {
        code: "B002",
        name: "dbg-macro",
        summary: "leftover `dbg!` invocation",
    },
    Rule {
        code: "B003",
        name: "todo-macro",
        summary: "`todo!`/`unimplemented!` left in code",
    },
    Rule {
        code: "B004",
        name: "len-zero",
        summary: "`.len() == 0` should be `.is_empty()`",
    },
    Rule {
        code: "B005",
        name: "needless-return",
        summary: "`return` as the last statement of a function",
    },
    Rule {
        code: "B006",
        name: "bool-comparison",
        summary: "comparison against a boolean literal",
    },
];

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} {} {}", self.line, self.column, self.code, self.message)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FrameKind {
    Fn,
    Other,
}

struct Frame {
    kind: FrameKind,
    cur_start: usize,
    last_start: usize,
}

/// Lint a source string. Lines carrying a `blaze:ignore` comment are suppressed;
/// `#[cfg(test)]` items are skipped.
pub fn lint_source(src: &str) -> Vec<Diagnostic> {
    let all = lex(src);
    let mut ignored: Vec<u32> = all
        .iter()
        .filter(|t| t.kind == Kind::Comment && t.text(src).contains("blaze:ignore"))
        .map(|t| t.line)
        .collect();
    ignored.sort_unstable();
    let c: Vec<Token> = all
        .into_iter()
        .filter(|t| !matches!(t.kind, Kind::Comment | Kind::DocComment))
        .collect();

    let txt = |i: usize| c.get(i).map_or("", |t| t.text(src));
    let mut out = Vec::new();
    let mut report = |code: &'static str, message: String, t: &Token| {
        if ignored.binary_search(&t.line).is_err() {
            out.push(Diagnostic {
                code,
                message,
                line: t.line as usize,
                column: t.col as usize,
            });
        }
    };

    let mut frames: Vec<Frame> = vec![
        Frame { kind : FrameKind::Other, cur_start : 0, last_start : usize::MAX, }
    ];
    let mut pending_fn: Option<usize> = None;
    let mut i = 0;
    while i < c.len() {
        let t = txt(i);
        match t {
            "#" => {
                // #![cfg(test)] disables the whole file; #[cfg(test)] skips the next item.
                let inner = txt(i + 1) == "!";
                let b = i + 1 + usize::from(inner);
                if txt(b) == "[" && txt(b + 1) == "cfg" && txt(b + 2) == "("
                    && txt(b + 3) == "test" && txt(b + 4) == ")" && txt(b + 5) == "]"
                {
                    if inner {
                        break;
                    }
                    i = skip_item(&c, src, b + 6);
                    continue;
                }
            }
            "." if txt(i + 1) == "unwrap" && txt(i + 2) == "(" && txt(i + 3) == ")" => {
                report("B001", "used `.unwrap()`".into(), &c[i + 1]);
            }
            "." if txt(i + 1) == "len" && txt(i + 2) == "(" && txt(i + 3) == ")"
                && txt(i + 4) == "==" && txt(i + 5) == "0" => {
                report(
                    "B004",
                    "use `.is_empty()` instead of comparing `.len()` to 0".into(),
                    &c[i + 1],
                );
            }
            "dbg" | "todo"
            | "unimplemented" if txt(i + 1) == "!"
                && matches!(txt(i + 2), "(" | "[" | "{") => {
                if t == "dbg" {
                    report("B002", "`dbg!` macro left in code".into(), &c[i]);
                } else {
                    report("B003", format!("`{t}!` left in code"), &c[i]);
                }
            }
            "==" | "!=" => {
                if matches!(txt(i + 1), "true" | "false")
                    || (i > 0 && matches!(txt(i - 1), "true" | "false"))
                {
                    report(
                        "B006",
                        "comparison to a boolean literal is redundant".into(),
                        &c[i],
                    );
                }
            }
            "fn" if c.get(i + 1).is_some_and(|n| n.kind == Kind::Ident) => {
                pending_fn = Some(frames.len());
            }
            "{" => {
                let kind = if pending_fn == Some(frames.len()) {
                    pending_fn = None;
                    FrameKind::Fn
                } else {
                    FrameKind::Other
                };
                frames
                    .push(Frame {
                        kind,
                        cur_start: i + 1,
                        last_start: usize::MAX,
                    });
            }
            "(" | "[" => {
                frames
                    .push(Frame {
                        kind: FrameKind::Other,
                        cur_start: i + 1,
                        last_start: usize::MAX,
                    })
            }
            "}" | ")" | "]" => {
                if frames.len() > 1 {
                    let f = frames
                        .pop()
                        .unwrap_or(Frame {
                            kind: FrameKind::Other,
                            cur_start: 0,
                            last_start: usize::MAX,
                        });
                    if t == "}" && f.kind == FrameKind::Fn && i > 0 && txt(i - 1) == ";"
                        && f.last_start != usize::MAX && txt(f.last_start) == "return"
                    {
                        report(
                            "B005",
                            "unneeded `return` statement".into(),
                            &c[f.last_start],
                        );
                    }
                }
            }
            ";" => {
                if pending_fn == Some(frames.len()) {
                    pending_fn = None;
                }
                if let Some(f) = frames.last_mut() {
                    f.last_start = f.cur_start;
                    f.cur_start = i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out.sort_by_key(|d| (d.line, d.column));
    out
}

/// Starting after an attribute at token index `i`, return the index just past the item it annotates.
fn skip_item(c: &[Token], src: &str, mut i: usize) -> usize {
    let txt = |i: usize| c.get(i).map_or("", |t| t.text(src));
    // further attributes
    while txt(i) == "#" && matches!(txt(i + 1), "[") {
        let mut depth = 0;
        i += 1;
        while i < c.len() {
            match txt(i) {
                "[" => depth += 1,
                "]" => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
    // body: first `{` (skip balanced) or `;` at nesting depth 0
    let mut depth = 0;
    while i < c.len() {
        match txt(i) {
            "(" | "[" => depth += 1,
            ")" | "]" => depth -= 1,
            ";" if depth <= 0 => return i + 1,
            "{" if depth <= 0 => {
                let mut d = 0;
                while i < c.len() {
                    match txt(i) {
                        "{" => d += 1,
                        "}" => {
                            d -= 1;
                            if d == 0 {
                                return i + 1;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    c.len()
}

pub struct FileResult {
    pub path: PathBuf,
    pub diagnostics: Vec<Diagnostic>,
    pub error: Option<String>,
}

pub fn lint_files(files: &[PathBuf]) -> Vec<FileResult> {
    files
        .par_iter()
        .map(|p| lint_file(p))
        .filter(|r| !r.diagnostics.is_empty() || r.error.is_some())
        .collect()
}

fn lint_file(path: &Path) -> FileResult {
    let mut res = FileResult {
        path: path.to_path_buf(),
        diagnostics: Vec::new(),
        error: None,
    };
    match std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
    {
        Ok(src) => res.diagnostics = lint_source(&src),
        Err(e) => res.error = Some(e),
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(src: &str) -> Vec<&'static str> {
        lint_source(src).into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn detects_each_rule() {
        assert_eq!(codes("fn f(x: Option<u8>) { x.unwrap(); }"), ["B001"]);
        assert_eq!(codes("fn f() { dbg!(1); }"), ["B002"]);
        assert_eq!(codes("fn f() { todo!(); }"), ["B003"]);
        assert_eq!(codes("fn f(v: Vec<u8>) -> bool { v.len() == 0 }"), ["B004"]);
        assert_eq!(codes("fn f() -> u8 { return 1; }"), ["B005"]);
        assert_eq!(codes("fn f(b: bool) -> bool { b == true }"), ["B006"]);
    }

    #[test]
    fn clean_code_and_suppression() {
        assert!(codes("fn f(v: &[u8]) -> bool { v.is_empty() }").is_empty());
        assert!(codes("fn f(x: Option<u8>) { x.unwrap(); } // blaze:ignore").is_empty());
    }

    #[test]
    fn return_not_at_tail_is_fine() {
        assert!(codes("fn f(c: bool) -> u8 { if c { return 1; } 2 }").is_empty());
        assert!(
            codes("fn f() { let c = || { return 1; }; let _ = [0; 3]; }").is_empty()
        );
        assert!(codes("trait T { fn f(&self); fn g(&self) -> u8 { 1 } }").is_empty());
    }

    #[test]
    fn strings_and_comments_are_not_code() {
        assert!(codes("fn f() { let s = \".unwrap()\"; /* x.unwrap() */ }").is_empty());
    }

    #[test]
    fn skips_cfg_test_items() {
        assert!(
            codes("#[cfg(test)] mod t { fn f(x: Option<u8>) { x.unwrap(); } }")
            .is_empty()
        );
        assert!(
            codes("#[cfg(test)] fn helper(x: Option<u8>) { x.unwrap(); }").is_empty()
        );
        assert_eq!(
            codes("#[cfg(test)] mod t {} fn f(x: Option<u8>) { x.unwrap(); }"), ["B001"]
        );
    }

    #[test]
    fn reports_position() {
        let d = &lint_source("fn f() {\n    dbg!(1);\n}")[0];
        assert_eq!((d.line, d.column), (2, 5));
    }
}
