//! Readable formatting for common std macros.
//!
//! prettyplease prints macro arguments token by token (`| x |`, `:: < T > ()`). For a whitelist
//! of macros whose arguments are plain comma-separated expressions, we temporarily rewrite the
//! macro call into an ordinary function call, let prettyplease format it like any call, and turn
//! the marker back into `name!(...)` afterwards. Macros that do not parse that way are left alone.

use blaze_lexer::{Kind, lex};
use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};
use syn::{Expr, ExprCall, ExprPath, MacroDelimiter, Stmt, Token};

const PAREN: &str = "__bm_p_";
const BRACKET: &str = "__bm_b_";

/// Macros whose arguments are a comma-separated list of expressions and accept a trailing comma.
const PAREN_MACROS: &[&str] = &[
    "println",
    "print",
    "eprintln",
    "eprint",
    "format",
    "write",
    "writeln",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "todo",
    "unimplemented",
    "unreachable",
    "dbg",
    "format_args",
];
const BRACKET_MACROS: &[&str] = &["vec"];

fn to_call(mac: &syn::Macro, attrs: &[syn::Attribute]) -> Option<Expr> {
    let name = mac.path.get_ident()?.to_string();
    let prefix = match mac.delimiter {
        MacroDelimiter::Paren(_) if PAREN_MACROS.contains(&name.as_str()) => PAREN,
        MacroDelimiter::Bracket(_) if BRACKET_MACROS.contains(&name.as_str()) => BRACKET,
        _ => return None,
    };
    let args = mac
        .parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
        .ok()?;
    let ident = syn::Ident::new(
        &format!("{prefix}{name}"),
        mac.path.segments[0].ident.span(),
    );
    Some(
        Expr::Call(ExprCall {
            attrs: attrs.to_vec(),
            func: Box::new(
                Expr::Path(ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path: ident.into(),
                }),
            ),
            paren_token: Default::default(),
            args,
        }),
    )
}

struct Rewrite {
    changed: bool,
}

impl VisitMut for Rewrite {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        if let Expr::Macro(m) = e && let Some(call) = to_call(&m.mac, &m.attrs) {
            *e = call;
            self.changed = true;
        }
        visit_mut::visit_expr_mut(self, e);
    }

    fn visit_stmt_mut(&mut self, s: &mut Stmt) {
        if let Stmt::Macro(sm) = s && let Some(call) = to_call(&sm.mac, &sm.attrs) {
            *s = Stmt::Expr(call, sm.semi_token);
            self.changed = true;
        }
        visit_mut::visit_stmt_mut(self, s);
    }
}

/// Rewrite eligible macro calls in place. Returns whether anything was rewritten.
pub fn rewrite(file: &mut syn::File) -> bool {
    let mut r = Rewrite { changed: false };
    r.visit_file_mut(file);
    r.changed
}

/// Turn markers back into macro calls in the formatted text.
pub fn restore(text: &str) -> String {
    if !text.contains("__bm_") {
        return text.to_string();
    }
    let toks = lex(text);
    // (start, end, replacement)
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if t.kind != Kind::Ident {
            continue;
        }
        let s = t.text(text);
        if let Some(name) = s.strip_prefix(PAREN) {
            edits.push((t.start, t.end, format!("{name}!")));
        } else if let Some(name) = s.strip_prefix(BRACKET) {
            // `__bm_b_vec(` ... matching `)`  becomes  `vec![` ... `]`
            let Some(open) = toks.get(i + 1).filter(|o| o.text(text) == "(") else {
                continue
            };
            let mut depth = 0i32;
            let mut close = None;
            for c in &toks[i + 1..] {
                match c.text(text) {
                    "(" if c.kind == Kind::Punct => depth += 1,
                    ")" if c.kind == Kind::Punct => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(c);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(c) = close {
                edits.push((t.start, open.end, format!("{name}![")));
                edits.push((c.start, c.end, "]".to_string()));
            }
        }
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = text.to_string();
    for (a, b, r) in edits {
        out.replace_range(a..b, &r);
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::format_source;

    #[test]
    fn formats_macro_arguments_like_calls() {
        let src = "fn f(v: Vec<(u8, usize)>) { println!(\"{} {}\", v.iter().map(|p| p.1).sum::<usize>(), v.len()); }";
        let out = format_source(src).unwrap();
        assert!(
            out
            .contains("println!(\"{} {}\", v.iter().map(|p| p.1).sum::<usize>(), v.len());"),
            "{out}"
        );
        assert!(! out.contains("| p |") && ! out.contains(":: <"));
    }

    #[test]
    fn long_macro_calls_wrap_and_stay_idempotent() {
        let src = "fn f() { assert_eq!(compute_something_long(first_argument, second_argument), expected_value_here, \"message {}\", detail_value); }";
        let once = format_source(src).unwrap();
        assert!(once.contains("assert_eq!(\n"), "{once}");
        assert_eq!(format_source(& once).unwrap(), once);
    }

    #[test]
    fn vec_brackets_are_restored() {
        let out = format_source("fn f() -> Vec<u8> { vec![1, 2, 3] }").unwrap();
        assert!(out.contains("vec![1, 2, 3]"), "{out}");
    }

    #[test]
    fn unparseable_macros_are_left_alone() {
        let src = "fn f(x: u8) -> bool { matches!(x, 1 | 2) }";
        let out = format_source(src).unwrap();
        assert!(out.contains("matches!") && ! out.contains("__bm_"), "{out}");
    }

    #[test]
    fn attributes_on_macro_statements_are_kept() {
        let src = "fn f(x: u8) {\n    #[cfg(debug_assertions)]\n    assert!(x > 0);\n}\n";
        let out = format_source(src).unwrap();
        assert!(
            out.contains("#[cfg(debug_assertions)]") && out.contains("assert!(x > 0);"),
            "{out}"
        );
    }

    #[test]
    fn comments_inside_rewritten_macros_survive() {
        let src = "fn f() {\n    println!(\"a\", // first\n        1);\n}\n";
        let out = format_source(src).unwrap();
        assert!(out.contains("// first"), "{out}");
    }
}
