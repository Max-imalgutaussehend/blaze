//! Ruff-style single-pass AST linter for Rust. Files are parsed once and every
//! rule runs in the same visitor traversal; files are linted in parallel.
use rayon::prelude::*;
use std::fmt;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
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
        write!(
            f,
            "{}:{} {} {}",
            self.line, self.column, self.code, self.message
        )
    }
}
struct Linter<'a> {
    out: Vec<Diagnostic>,
    ignored_lines: &'a [bool],
}
impl Linter<'_> {
    fn report(&mut self, code: &'static str, message: impl Into<String>, span: proc_macro2::Span) {
        let start = span.start();
        let suppressed = self
            .ignored_lines
            .get(start.line.wrapping_sub(1))
            .copied()
            .unwrap_or(false);
        if !suppressed {
            self.out.push(Diagnostic {
                code,
                message: message.into(),
                line: start.line,
                column: start.column + 1,
            });
        }
    }
    fn check_tail_return(&mut self, block: &syn::Block) {
        if let Some(syn::Stmt::Expr(syn::Expr::Return(r), Some(_))) = block.stmts.last() {
            self.report("B005", "unneeded `return` statement", r.span());
        }
    }
}
impl<'ast> Visit<'ast> for Linter<'_> {
    fn visit_expr_method_call(&mut self, e: &'ast syn::ExprMethodCall) {
        if e.method == "unwrap" && e.args.is_empty() {
            self.report("B001", "used `.unwrap()`", e.method.span());
        }
        visit::visit_expr_method_call(self, e);
    }
    fn visit_expr_binary(&mut self, e: &'ast syn::ExprBinary) {
        if matches!(e.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) {
            let is_len_call = |x: &syn::Expr| {
                matches!(
                    x, syn::Expr::MethodCall(m) if m.method == "len" && m.args.is_empty()
                )
            };
            let is_zero = |x: &syn::Expr| {
                matches!(
                    x, syn::Expr::Lit(l) if matches!(& l.lit, syn::Lit::Int(i) if i
                    .base10_digits() == "0")
                )
            };
            let is_bool = |x: &syn::Expr| matches!(x, syn::Expr::Lit(l) if matches!(l.lit, syn::Lit::Bool(_)));
            if matches!(e.op, syn::BinOp::Eq(_)) && is_len_call(&e.left) && is_zero(&e.right) {
                self.report(
                    "B004",
                    "use `.is_empty()` instead of comparing `.len()` to 0",
                    e.span(),
                );
            }
            if is_bool(&e.left) || is_bool(&e.right) {
                self.report(
                    "B006",
                    "comparison to a boolean literal is redundant",
                    e.span(),
                );
            }
        }
        visit::visit_expr_binary(self, e);
    }
    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        if let Some(seg) = m.path.segments.last() {
            match seg.ident.to_string().as_str() {
                "dbg" => self.report("B002", "`dbg!` macro left in code", m.span()),
                "todo" | "unimplemented" => {
                    self.report("B003", format!("`{}!` left in code", seg.ident), m.span())
                }
                _ => {}
            }
        }
        visit::visit_macro(self, m);
    }
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        let is_cfg_test = m.attrs.iter().any(|a| {
            a.path().is_ident("cfg")
                && a.meta
                    .require_list()
                    .is_ok_and(|l| l.tokens.to_string() == "test")
        });
        if !is_cfg_test {
            visit::visit_item_mod(self, m);
        }
    }
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        self.check_tail_return(&f.block);
        visit::visit_item_fn(self, f);
    }
    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        self.check_tail_return(&f.block);
        visit::visit_impl_item_fn(self, f);
    }
}
/// Lint a source string. Lines containing `blaze:ignore` are suppressed.
pub fn lint_source(src: &str) -> Result<Vec<Diagnostic>, syn::Error> {
    let file = syn::parse_file(src)?;
    let ignored: Vec<bool> = src.lines().map(|l| l.contains("blaze:ignore")).collect();
    let mut linter = Linter {
        out: Vec::new(),
        ignored_lines: &ignored,
    };
    linter.visit_file(&file);
    linter.out.sort_by_key(|d| (d.line, d.column));
    Ok(linter.out)
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
    match std::fs::read_to_string(path) {
        Err(e) => res.error = Some(e.to_string()),
        Ok(src) => match lint_source(&src) {
            Ok(d) => res.diagnostics = d,
            Err(e) => res.error = Some(format!("syntax error: {e}")),
        },
    }
    res
}
#[cfg(test)]
mod tests {
    use super::*;
    fn codes(src: &str) -> Vec<&'static str> {
        lint_source(src)
            .unwrap()
            .into_iter()
            .map(|d| d.code)
            .collect()
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
    fn skips_cfg_test_modules() {
        assert!(codes("#[cfg(test)] mod t { fn f(x: Option<u8>) { x.unwrap(); } }").is_empty());
    }
    #[test]
    fn reports_position() {
        let d = &lint_source("fn f() {\n    dbg!(1);\n}").unwrap()[0];
        assert_eq!((d.line, d.column), (2, 5));
    }
}
