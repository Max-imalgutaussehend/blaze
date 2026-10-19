//! Ember: token-level Rust linter. No syntax tree is built; every rule is a pattern over
//! the token stream produced by `blaze_lexer`, so throughput is limited by memory
//! bandwidth rather than parsing. The trade-off: rules cannot use type information, so
//! each rule only fires on shapes that are unambiguous without types.
//!
//! Many rules carry an automatic fix. Fixes are local token rewrites, applied together only
//! if the result keeps the bracket structure of the file intact.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use blaze_lexer::{Kind, Token, lex};

mod rules;
pub use rules::{RULES, Rule, UNSAFE_FIXES};

/// A replacement of `start..end` (byte offsets) with `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub line: usize,
    pub column: usize,
    /// Automatic fix, if the rule has one for this occurrence.
    pub fix: Option<Vec<Edit>>,
    /// False when the fix depends on types ember cannot see; applied only with unsafe fixes enabled.
    pub safe: bool,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match (&self.fix, self.safe) {
            (None, _) => "",
            (Some(_), true) => " [fix]",
            (Some(_), false) => " [unsafe fix]",
        };
        write!(
            f,
            "{}:{} {} {}{tag}",
            self.line,
            self.column,
            self.code,
            self.message,
        )
    }
}

/// Which rules are active.
#[derive(Debug, Clone)]
pub struct Selection {
    enabled: HashSet<&'static str>,
}

impl Selection {
    /// Default rules, with `select` (codes, names or `ALL`) replacing the default set when
    /// non-empty, and `ignore` removed afterwards. Unknown entries are an error.
    pub fn resolve(select: &[String], ignore: &[String]) -> Result<Selection, String> {
        let lookup = |s: &str| {
            RULES
                .iter()
                .find(|r| r.code.eq_ignore_ascii_case(s) || r.name == s)
                .map(|r| r.code)
        };
        let mut enabled: HashSet<&'static str> = if select.is_empty() {
            RULES.iter().filter(|r| r.default_on).map(|r| r.code).collect()
        } else if select.iter().any(|s| s == "ALL") {
            RULES.iter().map(|r| r.code).collect()
        } else {
            select
                .iter()
                .map(|s| {
                    lookup(s)
                        .ok_or_else(|| format!(
                            "unknown rule `{s}` (see `ember --rules`)",
                        ))
                })
                .collect::<Result<_, _>>()?
        };
        for i in ignore {
            let code = lookup(i)
                .ok_or_else(|| format!(
                    "unknown rule `{i}` (see `ember --rules`)",
                ))?;
            enabled.remove(code);
        }
        Ok(Selection { enabled })
    }

    pub fn all() -> Selection {
        Selection {
            enabled: RULES.iter().map(|r| r.code).collect(),
        }
    }

    pub fn default_rules() -> Selection {
        Selection::resolve(&[], &[]).unwrap_or_else(|_| Selection::all())
    }

    /// The selection without `codes`.
    pub fn minus(&self, codes: &[&str]) -> Selection {
        Selection {
            enabled: self
                .enabled
                .iter()
                .copied()
                .filter(|c| !codes.contains(c))
                .collect(),
        }
    }

    pub fn contains(&self, code: &str) -> bool {
        self.enabled.contains(code)
    }

    /// Stable text used to key caches.
    pub fn fingerprint(&self) -> String {
        let mut v: Vec<&str> = self.enabled.iter().copied().collect();
        v.sort_unstable();
        v.join(",")
    }
}

/// Everything the rules need, computed once per file.
pub(crate) struct Ctx<'a> {
    pub src: &'a str,
    /// Code tokens (comments removed).
    pub c: Vec<Token>,
    /// Comment tokens (doc comments excluded).
    pub comments: Vec<Token>,
    /// For every opening bracket the index of its closing bracket (`usize::MAX` otherwise).
    pub close: Vec<usize>,
}

impl<'a> Ctx<'a> {
    fn new(src: &'a str) -> Self {
        let all = lex(src);
        let mut c = Vec::with_capacity(all.len());
        let mut comments = Vec::new();
        for t in all {
            match t.kind {
                Kind::Comment => comments.push(t),
                Kind::DocComment => {}
                _ => c.push(t),
            }
        }
        let mut close = vec![usize::MAX; c.len()];
        let mut stack: Vec<usize> = Vec::new();
        for (i, t) in c.iter().enumerate() {
            if t.kind != Kind::Punct {
                continue;
            }
            match t.text(src) {
                "(" | "[" | "{" => stack.push(i),
                ")" | "]" | "}" => {
                    if let Some(o) = stack.pop() {
                        close[o] = i;
                    }
                }
                _ => {}
            }
        }
        Ctx { src, c, comments, close }
    }

    /// Text of code token `i`, or `""` past the end.
    pub fn s(&self, i: usize) -> &'a str {
        self.c.get(i).map_or("", |t| t.text(self.src))
    }

    pub fn is_ident(&self, i: usize) -> bool {
        self.c.get(i).is_some_and(|t| t.kind == Kind::Ident)
    }

    pub fn end_of(&self, i: usize) -> usize {
        self.c[i].end
    }

    pub fn start_of(&self, i: usize) -> usize {
        self.c[i].start
    }

    pub fn edit(&self, from: usize, to: usize, text: impl Into<String>) -> Edit {
        Edit {
            start: from,
            end: to,
            text: text.into(),
        }
    }

    /// Delete code tokens `a..=b` (byte range from the start of `a` to the end of `b`).
    pub fn delete(&self, a: usize, b: usize) -> Edit {
        self.edit(self.start_of(a), self.end_of(b), "")
    }

    /// Start of a simple receiver chain (`a.b.c`, `self.0`, `a::b`) ending just before the `.` at `dot`.
    pub fn chain_before(&self, dot: usize) -> Option<usize> {
        let mut q = dot.checked_sub(1)?;
        loop {
            let ok = self.is_ident(q)
                || self.c[q].kind == Kind::Literal
                    && self.s(q).bytes().all(|b| b.is_ascii_digit());
            if !ok {
                return None;
            }
            if q >= 2 && matches!(self.s(q - 1), "." | "::") {
                q -= 2;
                continue;
            }
            return Some(q);
        }
    }
}

/// Per-line suppressions: `// blaze:ignore` (all rules) or `// blaze:ignore B001,unwrap-used`.
struct Suppress {
    /// (line, None = everything, Some(list) = only these)
    lines: Vec<(u32, Option<Vec<String>>)>,
    file: bool,
}

impl Suppress {
    fn parse(ctx: &Ctx) -> Suppress {
        let mut lines = Vec::new();
        let mut file = false;
        for t in &ctx.comments {
            let text = t.text(ctx.src);
            let Some(i) = text.find("blaze:ignore") else { continue };
            let rest = &text[i + "blaze:ignore".len()..];
            if rest.starts_with("-file") {
                file = true;
                continue;
            }
            let list: Vec<String> = rest
                .trim()
                .trim_end_matches("*/")
                .trim()
                .split([',', ' '])
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            lines.push((t.line, if list.is_empty() { None } else { Some(list) }));
        }
        Suppress { lines, file }
    }

    fn hides(&self, line: u32, code: &str) -> bool {
        self.file
            || self
                .lines
                .iter()
                .any(|(l, only)| {
                    *l == line
                        && only
                            .as_ref()
                            .is_none_or(|v| {
                                v.iter()
                                    .any(|x| {
                                        x.eq_ignore_ascii_case(code)
                                            || RULES
                                                .iter()
                                                .any(|r| r.code == code && r.name == x.as_str())
                                    })
                            })
                })
    }
}

/// Lint a source string with the default rule set.
pub fn lint_source(src: &str) -> Vec<Diagnostic> {
    lint_selected(src, &Selection::default_rules())
}

/// Lint with an explicit selection. `// blaze:ignore` comments suppress findings on their line,
/// and `#[cfg(test)]` items are skipped.
pub fn lint_selected(src: &str, sel: &Selection) -> Vec<Diagnostic> {
    let ctx = Ctx::new(src);
    let sup = Suppress::parse(&ctx);
    let mut raw = Vec::new();
    rules::run(&ctx, sel, &mut raw);
    raw.retain(|d| sel.contains(d.code) && !sup.hides(d.line as u32, d.code));
    for d in &mut raw {
        d.safe = !UNSAFE_FIXES.contains(&d.code);
    }
    raw.sort_by_key(|d| (d.line, d.column));
    raw
}

/// Apply the fixes of `diags` to `src`. Overlapping fixes are skipped (re-run to pick them up).
/// Returns the new text and the number of fixes applied. If the bracket structure would change,
/// nothing is applied.
pub fn apply_fixes(
    src: &str,
    diags: &[Diagnostic],
    unsafe_fixes: bool,
) -> (String, usize) {
    let mut taken: Vec<(usize, usize)> = Vec::new();
    let mut edits: Vec<&Edit> = Vec::new();
    let mut applied = 0;
    for d in diags {
        let Some(fix) = d.fix.as_ref().filter(|_| d.safe || unsafe_fixes) else {
            continue
        };
        let overlaps = |e: &Edit| {
            taken
                .iter()
                .any(|&(a, b)| {
                    e.start < b && a < e.end
                        || (e.start == e.end && e.start > a && e.start < b)
                })
        };
        if fix.iter().any(overlaps) {
            continue;
        }
        for e in fix {
            taken.push((e.start, e.end));
            edits.push(e);
        }
        applied += 1;
    }
    if edits.is_empty() {
        return (src.to_string(), 0);
    }
    edits.sort_by_key(|e| std::cmp::Reverse((e.start, e.end)));
    let mut out = src.to_string();
    for e in edits {
        out.replace_range(e.start..e.end, &e.text);
    }
    if bracket_balance(src) != bracket_balance(&out) {
        return (src.to_string(), 0);
    }
    (out, applied)
}

/// Net count of each bracket kind; a fix must not change it.
fn bracket_balance(src: &str) -> [i64; 3] {
    let mut b = [0i64; 3];
    for t in lex(src) {
        if t.kind != Kind::Punct {
            continue;
        }
        match t.text(src) {
            "(" => b[0] += 1,
            ")" => b[0] -= 1,
            "[" => b[1] += 1,
            "]" => b[1] -= 1,
            "{" => b[2] += 1,
            "}" => b[2] -= 1,
            _ => {}
        }
    }
    b
}

/// Fix to a fixed point (at most three rounds). Returns the final text, the number of fixes and
/// the diagnostics that remain.
pub fn fix_source(
    src: &str,
    sel: &Selection,
    unsafe_fixes: bool,
) -> (String, usize, Vec<Diagnostic>) {
    let mut text = src.to_string();
    let mut total = 0;
    for _ in 0..3 {
        let diags = lint_selected(&text, sel);
        let (next, n) = apply_fixes(&text, &diags, unsafe_fixes);
        if n == 0 {
            return (text, total, diags);
        }
        text = next;
        total += n;
    }
    let rest = lint_selected(&text, sel);
    (text, total, rest)
}

pub struct FileResult {
    pub path: PathBuf,
    pub diagnostics: Vec<Diagnostic>,
    /// Fixes applied to the file on disk.
    pub fixed: usize,
    pub error: Option<String>,
}

/// Test and example code may unwrap, expect, print and leave todo!s around.
pub fn is_test_path(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n == "tests.rs" || n == "test.rs")
        || path
            .components()
            .any(|c| {
                matches!(c.as_os_str().to_str(), Some("tests" | "examples" | "benches"))
            })
}

/// Lint (and optionally fix) one file on disk.
pub fn lint_file(
    path: &Path,
    sel: &Selection,
    fix: bool,
    unsafe_fixes: bool,
) -> FileResult {
    let relaxed;
    let sel = if is_test_path(path) {
        relaxed = sel.minus(&["B001", "B003", "B007", "B008"]);
        &relaxed
    } else {
        sel
    };
    let mut res = FileResult {
        path: path.to_path_buf(),
        diagnostics: Vec::new(),
        fixed: 0,
        error: None,
    };
    let src = match std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
    {
        Ok(s) => s,
        Err(e) => {
            res.error = Some(e);
            return res;
        }
    };
    if fix {
        let (text, n, rest) = fix_source(&src, sel, unsafe_fixes);
        if n > 0 && let Err(e) = std::fs::write(path, &text) {
            res.error = Some(e.to_string());
            return res;
        }
        res.fixed = n;
        res.diagnostics = rest;
    } else {
        res.diagnostics = lint_selected(&src, sel);
    }
    res
}

#[cfg(test)]
mod tests;
