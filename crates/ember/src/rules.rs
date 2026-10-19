//! The rule set. Every rule is a pattern over code tokens; see `Ctx` for the helpers.

use blaze_lexer::{Kind, Token};

use crate::{Ctx, Diagnostic, Edit, Selection};

pub struct Rule {
    pub code: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    /// Enabled without `--select`.
    pub default_on: bool,
    pub fixable: bool,
}

macro_rules! rule {
    ($code:expr, $name:expr, $on:expr, $fix:expr, $summary:expr) => {
        Rule { code : $code, name : $name, summary : $summary, default_on : $on, fixable
        : $fix }
    };
}

pub const RULES: &[Rule] = &[
    rule!(
        "B001", "unwrap-used", true, false,
        "`.unwrap()` can panic; propagate the error or use `expect`"
    ),
    rule!("B002", "dbg-macro", true, true, "leftover `dbg!` invocation"),
    rule!("B003", "todo-macro", true, false, "`todo!`/`unimplemented!` left in code"),
    rule!(
        "B004", "len-zero", true, true,
        "comparing `.len()` to 0 or 1 instead of `.is_empty()`"
    ),
    rule!(
        "B005", "needless-return", true, true,
        "`return` as the last statement of a function"
    ),
    rule!(
        "B006", "bool-comparison", true, false, "comparison against a boolean literal"
    ),
    rule!("B007", "expect-used", false, false, "`.expect(..)` can panic"),
    rule!(
        "B008", "print-stdout", false, false,
        "`print!`/`println!` and friends write to the terminal"
    ),
    rule!(
        "B009", "undocumented-unsafe", false, false,
        "`unsafe` block without a `// SAFETY:` comment"
    ),
    rule!("B010", "todo-comment", false, false, "TODO/FIXME/XXX comment"),
    rule!(
        "B011", "explicit-iter-loop", false, false,
        "`for x in y.iter()` can be `for x in &y` (if `&y` is iterable)"
    ),
    rule!(
        "B012", "mem-replace-default", true, true,
        "`mem::replace(&mut x, Default::default())` is `mem::take(&mut x)`"
    ),
    rule!("B013", "identity-map", true, true, "`.map(|x| x)` does nothing"),
    rule!("B014", "empty-println", true, true, "`println!(\"\")` is `println!()`"),
    rule!(
        "B015", "get-first", true, false,
        "`.get(0)` can be `.first()` on slices and vectors"
    ),
    rule!("B016", "nth-zero", true, true, "`.nth(0)` is `.next()`"),
    rule!("B017", "filter-next", true, true, "`.filter(p).next()` is `.find(p)`"),
    rule!("B018", "skip-next", true, true, "`.skip(n).next()` is `.nth(n)`"),
    rule!(
        "B019", "unwrap-or-default", true, true,
        "`unwrap_or(Vec::new())` and friends are `unwrap_or_default()`"
    ),
    rule!(
        "B020", "or-fun-call", false, false,
        "`unwrap_or(f(x))` evaluates `f(x)` eagerly; use `unwrap_or_else` (`|_|` closure for `Result`)"
    ),
    rule!("B021", "assign-op", true, true, "`a = a + b` is `a += b`"),
    rule!(
        "B022", "and-then-some", true, true, "`.and_then(|x| Some(y))` is `.map(|x| y)`"
    ),
    rule!(
        "B023", "empty-string-new", true, true,
        "`String::from(\"\")` and `\"\".to_string()` are `String::new()`"
    ),
    rule!(
        "B024", "partialeq-none", true, false,
        "comparing with `None` using `==`/`!=`; use `.is_none()`/`.is_some()`"
    ),
    rule!(
        "B025", "redundant-pattern-matching", true, false,
        "`if let Some(_) = x` is `x.is_some()`"
    ),
    rule!(
        "B026", "identity-op", true, true,
        "`x + 0`, `x - 0`, `x * 1` and `x / 1` do nothing"
    ),
    rule!(
        "B027", "map-or-bool", true, false,
        "`.map_or(false, f)` is `.is_some_and(f)`; `.map_or(true, f)` is `.is_none_or(f)`"
    ),
    rule!(
        "B028", "ptr-arg", true, false,
        "`&Vec<T>`/`&String`/`&PathBuf` parameters should be `&[T]`/`&str`/`&Path`"
    ),
    rule!(
        "B029", "bool-to-if", true, false, "`if c { true } else { false }` is just `c`"
    ),
    rule!("B030", "redundant-field-names", true, true, "`Foo { a: a }` is `Foo { a }`"),
    rule!(
        "B031", "single-char-pattern", false, false,
        "a one-character string pattern is faster as a `char` (only for `str` methods)"
    ),
];

/// Fixes that depend on the receiver's type (method availability, `mut` bindings, operator impls).
/// They are only applied with `--unsafe-fixes`.
pub const UNSAFE_FIXES: &[&str] = &[
    "B004",
    "B016",
    "B017",
    "B018",
    "B019",
    "B021",
    "B022",
    "B026",
];

const KEYWORDS: &[&str] = &[
    "return",
    "in",
    "if",
    "else",
    "match",
    "while",
    "break",
    "let",
    "mut",
    "ref",
    "as",
    "move",
    "loop",
    "for",
    "where",
    "unsafe",
    "continue",
    "yield",
];
const COLLECTIONS: &[&str] = &[
    "Vec",
    "String",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "VecDeque",
    "BinaryHeap",
];
const PATTERN_METHODS: &[&str] = &[
    "split",
    "rsplit",
    "split_terminator",
    "rsplit_terminator",
    "contains",
    "starts_with",
    "ends_with",
    "find",
    "rfind",
    "matches",
    "rmatches",
    "trim_start_matches",
    "trim_end_matches",
    "trim_matches",
    "strip_prefix",
    "strip_suffix",
    "replace",
    "replacen",
    "split_once",
    "rsplit_once",
];

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

struct R<'a, 'o> {
    cx: &'a Ctx<'a>,
    out: &'o mut Vec<Diagnostic>,
}

impl R<'_, '_> {
    fn report(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        at: usize,
        fix: Option<Vec<Edit>>,
    ) {
        let t: &Token = &self.cx.c[at];
        self.out
            .push(Diagnostic {
                code,
                message: message.into(),
                line: t.line as usize,
                column: t.col as usize,
                fix,
                safe: true,
            });
    }
}

/// Run all rules over the file. Findings of rules that are not selected are discarded by the caller;
/// the expensive opt-in rules are skipped here.
pub(crate) fn run(cx: &Ctx, sel: &Selection, out: &mut Vec<Diagnostic>) {
    let mut r = R { cx, out };
    let n = cx.c.len();

    if sel.contains("B010") {
        for t in &cx.comments {
            let text = t.text(cx.src);
            if ["TODO", "FIXME", "XXX"].iter().any(|w| text.contains(w)) {
                r.out
                    .push(Diagnostic {
                        code: "B010",
                        message: "TODO/FIXME comment".into(),
                        line: t.line as usize,
                        column: t.col as usize,
                        fix: None,
                        safe: true,
                    });
            }
        }
    }

    let mut frames: Vec<Frame> = vec![Frame {
        kind: FrameKind::Other,
        cur_start: 0,
        last_start: usize::MAX,
    }];
    let mut pending_fn: Option<usize> = None;
    let mut openers: Vec<char> = Vec::new();
    let mut i = 0;
    while i < n {
        let t = cx.s(i);
        match t {
            "#" => {
                // #![cfg(test)] disables the whole file; #[cfg(test)] skips the next item.
                let inner = cx.s(i + 1) == "!";
                let b = i + 1 + usize::from(inner);
                if cx.s(b) == "[" && cx.s(b + 1) == "cfg" && cx.s(b + 2) == "("
                    && cx.s(b + 3) == "test" && cx.s(b + 4) == ")" && cx.s(b + 5) == "]"
                {
                    if inner {
                        break;
                    }
                    i = skip_item(cx, b + 6);
                    continue;
                }
            }
            "." if cx.is_ident(i + 1) && cx.s(i + 2) == "(" => method_rules(&mut r, i),
            "dbg" | "todo" | "unimplemented" | "println" | "print" | "eprintln"
            | "eprint" if cx.s(i + 1) == "!"
                && matches!(cx.s(i + 2), "(" | "[" | "{") => macro_rules(&mut r, i),
            "==" | "!=" => {
                if matches!(cx.s(i + 1), "true" | "false")
                    || (i > 0 && matches!(cx.s(i - 1), "true" | "false"))
                {
                    r.report(
                        "B006",
                        "comparison to a boolean literal is redundant",
                        i,
                        None,
                    );
                }
                if cx.s(i + 1) == "None" && cx.s(i + 2) != "(" {
                    r.report(
                        "B024",
                        "compare with `.is_none()`/`.is_some()` instead of `== None`",
                        i,
                        None,
                    );
                }
            }
            "=" => assign_op(&mut r, i),
            "unsafe" if cx.s(i + 1) == "{" && sel.contains("B009") => {
                let line = cx.c[i].line;
                let documented = cx
                    .comments
                    .iter()
                    .any(|c| {
                        c.line + 3 > line && c.line <= line
                            && c.text(cx.src).contains("SAFETY:")
                    });
                if !documented {
                    r.report(
                        "B009",
                        "`unsafe` block without a `// SAFETY:` comment",
                        i,
                        None,
                    );
                }
            }
            "for" if sel.contains("B011") => explicit_iter_loop(&mut r, i),
            "fn" if cx.is_ident(i + 1) => {
                pending_fn = Some(frames.len());
                ptr_arg(&mut r, i);
            }
            "else" if i >= 3 && cx.s(i + 1) == "{" && cx.s(i + 3) == "}"
                && cx.s(i - 1) == "}" => {
                let (a, b) = (cx.s(i - 2), cx.s(i + 2));
                if cx.s(i - 3) == "{"
                    && ((a == "true" && b == "false") || (a == "false" && b == "true"))
                {
                    r.report(
                        "B029",
                        "`if c { true } else { false }` is just `c`",
                        i,
                        None,
                    );
                }
            }
            "let" if matches!(cx.s(i.wrapping_sub(1)), "if" | "while") && i > 0 => {
                let pat = cx.s(i + 1);
                if matches!(pat, "Some" | "Ok" | "Err") && cx.s(i + 2) == "("
                    && cx.s(i + 3) == "_" && cx.s(i + 4) == ")" && cx.s(i + 5) == "="
                {
                    let m = match pat {
                        "Some" => "is_some()",
                        "Ok" => "is_ok()",
                        _ => "is_err()",
                    };
                    r.report(
                        "B025",
                        format!("use `.{m}` instead of matching `{pat}(_)`"),
                        i,
                        None,
                    );
                } else if pat == "None" && cx.s(i + 2) == "=" {
                    r.report(
                        "B025",
                        "use `.is_none()` instead of matching `None`",
                        i,
                        None,
                    );
                }
            }
            "String" if cx.s(i + 1) == "::" && cx.s(i + 2) == "from"
                && cx.s(i + 3) == "(" && cx.s(i + 4) == "\"\"" && cx.s(i + 5) == ")" => {
                let fix = vec![
                    cx.edit(cx.start_of(i), cx.end_of(i + 5), "String::new()"),
                ];
                r.report("B023", "use `String::new()`", i, Some(fix));
            }
            "replace" if i >= 2 && cx.s(i - 1) == "::" && cx.s(i - 2) == "mem"
                && cx.s(i + 1) == "(" => mem_replace(&mut r, i),
            "\"\"" if cx.s(i + 1) == "."
                && matches!(cx.s(i + 2), "to_string" | "to_owned") && cx.s(i + 3) == "("
                && cx.s(i + 4) == ")" => {
                let fix = vec![
                    cx.edit(cx.start_of(i), cx.end_of(i + 4), "String::new()"),
                ];
                r.report("B023", "use `String::new()`", i, Some(fix));
            }
            "0" | "1" => identity_op(&mut r, i),
            s if cx.is_ident(i) && cx.s(i + 1) == ":" && cx.s(i + 2) == s
                && matches!(cx.s(i + 3), "," | "}")
                && matches!(cx.s(i.wrapping_sub(1)), "{" | ",") && i > 0
                && openers.last() == Some(&'{')
                && !s.starts_with(|c: char| c.is_uppercase())
                && !KEYWORDS.contains(&s) => {
                let fix = vec![cx.edit(cx.end_of(i), cx.end_of(i + 2), "")];
                r.report(
                    "B030",
                    format!("field `{s}` can use the shorthand `{s}`"),
                    i,
                    Some(fix),
                );
            }
            "{" => {
                openers.push('{');
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
                openers.push(if t == "(" { '(' } else { '[' });
                frames
                    .push(Frame {
                        kind: FrameKind::Other,
                        cur_start: i + 1,
                        last_start: usize::MAX,
                    });
            }
            "}" | ")" | "]" => {
                openers.pop();
                if frames.len() > 1 {
                    let f = frames
                        .pop()
                        .unwrap_or(Frame {
                            kind: FrameKind::Other,
                            cur_start: 0,
                            last_start: usize::MAX,
                        });
                    if t == "}" && f.kind == FrameKind::Fn && i > 0 && cx.s(i - 1) == ";"
                        && f.last_start != usize::MAX && cx.s(f.last_start) == "return"
                    {
                        let ret = f.last_start;
                        let fix = if cx.s(ret + 1) == ";" {
                            vec![
                                cx
                                    .edit(
                                        cx.end_of(ret.saturating_sub(1)),
                                        cx.end_of(ret + 1),
                                        "",
                                    ),
                            ]
                        } else {
                            vec![
                                cx.edit(cx.start_of(ret), cx.start_of(ret + 1), ""),
                                cx.delete(i - 1, i - 1),
                            ]
                        };
                        r.report("B005", "unneeded `return` statement", ret, Some(fix));
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
}

/// Method-call shaped rules; `dot` is the index of the `.`, the method name is at `dot + 1`, `(` at `dot + 2`.
fn method_rules(r: &mut R, dot: usize) {
    let cx = r.cx;
    let m = cx.s(dot + 1);
    let open = dot + 2;
    let close = cx.close[open];
    if close == usize::MAX {
        return;
    }
    let no_args = close == open + 1;
    match m {
        "unwrap" if no_args => r.report("B001", "used `.unwrap()`", dot + 1, None),
        "expect" => r.report("B007", "used `.expect(..)`", dot + 1, None),
        "len" if no_args => len_zero(r, dot, close),
        "get" if close == open + 2 && cx.s(open + 1) == "0" => {
            r.report("B015", "use `.first()` instead of `.get(0)`", dot + 1, None)
        }
        "nth" if close == open + 2 && cx.s(open + 1) == "0" => {
            let fix = vec![
                cx.edit(cx.start_of(dot + 1), cx.end_of(close), "next()"),
            ];
            r.report("B016", "`.nth(0)` is `.next()`", dot + 1, Some(fix));
        }
        "map" if close == open + 5 && cx.s(open + 1) == "|" && cx.is_ident(open + 2)
            && cx.s(open + 3) == "|" && cx.s(open + 4) == cx.s(open + 2) => {
            r.report(
                "B013",
                "`.map(|x| x)` does nothing",
                dot + 1,
                Some(vec![cx.delete(dot, close)]),
            );
        }
        "filter"
        | "skip" if cx.s(close + 1) == "." && cx.s(close + 2) == "next"
            && cx.s(close + 3) == "(" && cx.s(close + 4) == ")" => {
            let (code, to, msg) = if m == "filter" {
                ("B017", "find", "`.filter(p).next()` is `.find(p)`")
            } else {
                ("B018", "nth", "`.skip(n).next()` is `.nth(n)`")
            };
            let fix = vec![
                cx.edit(cx.start_of(dot + 1), cx.end_of(dot + 1), to),
                cx.delete(close + 1, close + 4),
            ];
            r.report(code, msg, dot + 1, Some(fix));
        }
        "unwrap_or" => unwrap_or_rules(r, dot, close),
        "and_then" => {
            // .and_then(|x| Some(y))
            let (a, b) = (open + 1, open + 4);
            if cx.s(a) == "|" && cx.is_ident(a + 1) && cx.s(a + 2) == "|"
                && matches!(cx.s(b), "Some" | "Ok") && cx.s(b + 1) == "("
            {
                let cs = cx.close[b + 1];
                // `?` or `return` inside would change meaning once the closure no longer returns an Option
                let diverges = (b + 2..cs).any(|k| matches!(cx.s(k), "?" | "return"));
                if cs != usize::MAX && cs + 1 == close && !diverges {
                    let fix = vec![
                        cx.edit(cx.start_of(dot + 1), cx.end_of(dot + 1), "map"),
                        cx.delete(b, b + 1),
                        cx.delete(cs, cs),
                    ];
                    r.report(
                        "B022",
                        format!(
                            "`.and_then(|x| {}(..))` is `.map(|x| ..)`",
                            cx.s(b),
                        ),
                        dot + 1,
                        Some(fix),
                    );
                }
            }
        }
        "map_or" if matches!(cx.s(open + 1), "false" | "true")
            && cx.s(open + 2) == "," => {
            let to = if cx.s(open + 1) == "false" {
                "is_some_and"
            } else {
                "is_none_or"
            };
            r.report(
                "B027",
                format!(
                    "`.map_or({}, f)` is `.{to}(f)` (`is_ok_and` for `Result`)",
                    cx.s(open + 1),
                ),
                dot + 1,
                None,
            );
        }
        _ if PATTERN_METHODS.contains(&m) => {
            let lit = cx.s(open + 1);
            let inner = lit.strip_prefix('"').and_then(|l| l.strip_suffix('"'));
            if let Some(ch) = inner
                .filter(|s| s.chars().count() == 1 && !matches!(* s, "\\" | "'" | "\""))
                && cx.c[open + 1].kind == Kind::Literal
                && matches!(cx.s(open + 2), "," | ")")
            {
                r.report(
                    "B031",
                    format!(
                        "use the char '{ch}' instead of the string \"{ch}\" if the receiver is a `str`",
                    ),
                    open + 1,
                    None,
                );
            }
        }
        _ => {}
    }
}

/// `x.len() == 0` and friends. `dot` is the `.` before `len`, `close` the `)` of `len()`.
fn len_zero(r: &mut R, dot: usize, close: usize) {
    let cx = r.cx;
    let (op, lit) = (cx.s(close + 1), cx.s(close + 2));
    let (empty, negated) = match (op, lit) {
        ("==", "0") | ("<", "1") => (true, false),
        ("!=", "0") | (">", "0") | (">=", "1") => (true, true),
        _ => (false, false),
    };
    if !empty {
        return;
    }
    let fix = cx
        .chain_before(dot)
        .map(|start| {
            let recv = &cx.src[cx.start_of(start)..cx.start_of(dot)];
            vec![
                cx
                    .edit(
                        cx.start_of(start),
                        cx.end_of(close + 2),
                        format!(
                            "{}{recv}.is_empty()",
                            if negated { "!" } else { "" },
                        ),
                    ),
            ]
        });
    r.report(
        "B004",
        "use `.is_empty()` instead of comparing `.len()` to a constant",
        dot + 1,
        fix,
    );
}

fn unwrap_or_rules(r: &mut R, dot: usize, close: usize) {
    let cx = r.cx;
    let open = dot + 2;
    let a = open + 1; // first argument token
    // B019: unwrap_or(Vec::new()) / unwrap_or(Default::default())
    let n = close - a;
    let default_call = n == 5 && cx.s(a + 1) == "::" && cx.s(a + 3) == "("
        && cx.s(a + 4) == ")"
        && ((COLLECTIONS.contains(&cx.s(a)) && cx.s(a + 2) == "new")
            || (cx.s(a) == "Default" && cx.s(a + 2) == "default"));
    if default_call {
        let fix = vec![
            cx.edit(cx.start_of(dot + 1), cx.end_of(close), "unwrap_or_default()"),
        ];
        r.report("B019", "use `unwrap_or_default()`", dot + 1, Some(fix));
        return;
    }
    // B020: the argument is a single call with arguments, or `format!(..)`
    let mut j = a;
    while cx.is_ident(j) && cx.s(j + 1) == "::" {
        j += 2;
    }
    if !cx.is_ident(j) {
        return;
    }
    let (callee, paren) = if cx.s(j + 1) == "!" && cx.s(j) == "format" {
        (j, j + 2)
    } else {
        (j, j + 1)
    };
    if cx.s(paren) != "(" || cx.close[paren] != close - 1 {
        return;
    }
    let is_macro = paren == j + 2;
    let lowercase = cx.s(callee).starts_with(|c: char| c.is_lowercase() || c == '_');
    let has_args = cx.close[paren] > paren + 1;
    if (lowercase && has_args) || is_macro {
        r.report(
            "B020",
            "the argument is evaluated eagerly; consider `unwrap_or_else`",
            dot + 1,
            None,
        );
    }
}

fn macro_rules(r: &mut R, i: usize) {
    let cx = r.cx;
    let name = cx.s(i);
    match name {
        "dbg" => {
            let open = i + 2;
            let close = cx.close[open];
            let mut fix = None;
            if close != usize::MAX {
                let stmt_start = i == 0 || matches!(cx.s(i - 1), ";" | "{" | "}");
                if stmt_start && cx.s(close + 1) == ";" {
                    fix = Some(vec![cx.delete(i, close + 1)]);
                } else if close > open + 1 && !top_level_comma(cx, open + 1, close) {
                    let inner = cx.src[cx.end_of(open)..cx.start_of(close)].trim();
                    fix = Some(
                        vec![cx.edit(cx.start_of(i), cx.end_of(close), inner)],
                    );
                }
            }
            r.report("B002", "`dbg!` macro left in code", i, fix);
        }
        "todo" | "unimplemented" => {
            r.report("B003", format!("`{name}!` left in code"), i, None)
        }
        _ => {
            if matches!(name, "println" | "eprintln") && cx.s(i + 3) == "\"\""
                && cx.s(i + 4) == ")"
            {
                r.report(
                    "B014",
                    format!("`{name}!(\"\")` is `{name}!()`"),
                    i,
                    Some(vec![cx.delete(i + 3, i + 3)]),
                );
            }
            r.report("B008", format!("`{name}!` writes to the terminal"), i, None);
        }
    }
}

fn top_level_comma(cx: &Ctx, from: usize, to: usize) -> bool {
    let mut i = from;
    while i < to {
        match cx.s(i) {
            "(" | "[" | "{" => {
                let c = cx.close[i];
                if c == usize::MAX {
                    return true;
                }
                i = c;
            }
            "," => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

/// `mem::replace(&mut x, Default::default())`; `i` is the `replace` token.
fn mem_replace(r: &mut R, i: usize) {
    let cx = r.cx;
    let close = cx.close[i + 1];
    if close == usize::MAX || close < 6 {
        return;
    }
    let (comma, def) = (close - 6, close - 5);
    if cx.s(comma) == "," && cx.s(def) == "Default" && cx.s(def + 1) == "::"
        && cx.s(def + 2) == "default" && cx.s(def + 3) == "(" && cx.s(def + 4) == ")"
        && cx.s(i + 2) == "&" && cx.s(i + 3) == "mut"
    {
        let fix = vec![
            cx.edit(cx.start_of(i), cx.end_of(i), "take"),
            cx.delete(comma, close - 1),
        ];
        r.report("B012", "use `mem::take(&mut x)`", i, Some(fix));
    }
}

/// `a = a + b;` at statement level. `i` is the `=` token.
fn assign_op(r: &mut R, i: usize) {
    let cx = r.cx;
    if i < 2 || !cx.is_ident(i - 1) {
        return;
    }
    // left-hand chain: a, a.b, a.b.c
    let mut l = i - 1;
    while l >= 2 && matches!(cx.s(l - 1), "." | "::") && cx.is_ident(l - 2) {
        l -= 2;
    }
    if l > 0 && !matches!(cx.s(l - 1), ";" | "{" | "}") {
        return;
    }
    let len = i - l; // tokens in the chain
    let same = (0..len).all(|k| cx.s(l + k) == cx.s(i + 1 + k));
    let op = i + 1 + len;
    if !same || !matches!(cx.s(op), "+" | "-" | "*" | "/" | "%" | "^" | "&" | "|") {
        return;
    }
    // the rest up to `;` must be a single operand (no binary operators at depth 0)
    let mut j = op + 1;
    let mut simple = j < cx.c.len();
    while j < cx.c.len() && cx.s(j) != ";" {
        match cx.s(j) {
            "(" | "[" | "{" => {
                let c = cx.close[j];
                if c == usize::MAX {
                    return;
                }
                j = c;
            }
            ")" | "]" | "}" => return,
            "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<" | ">" | "=" | "!" | "?"
            | "&&" | "||" | "==" | "!=" | "<="
            | ">=" if j > op + 1 || cx.s(j) != "-" => {
                simple = false;
            }
            _ => {}
        }
        j += 1;
    }
    if !simple || j >= cx.c.len() {
        return;
    }
    let o = cx.s(op);
    let fix = vec![cx.edit(cx.start_of(i), cx.end_of(op), format!("{o}="))];
    r.report("B021", format!("use `{o}=`"), i, Some(fix));
}

/// `x + 0`, `x * 1`; `i` is the literal.
fn identity_op(r: &mut R, i: usize) {
    let cx = r.cx;
    if i < 2 || cx.c[i].kind != Kind::Literal {
        return;
    }
    let op = cx.s(i - 1);
    let ok_op = if cx.s(i) == "0" {
        matches!(op, "+" | "-")
    } else {
        matches!(op, "*" | "/")
    };
    if !ok_op {
        return;
    }
    let before = i - 2;
    let operand = (cx.is_ident(before) && !KEYWORDS.contains(&cx.s(before)))
        || cx.c[before].kind == Kind::Literal || matches!(cx.s(before), ")" | "]");
    let after_ok = i + 1 >= cx.c.len()
        || matches!(cx.s(i + 1), ";" | ")" | "]" | "}" | ",");
    if operand && after_ok {
        r.report(
            "B026",
            format!("`{op} {}` does nothing", cx.s(i)),
            i,
            Some(vec![cx.edit(cx.end_of(before), cx.end_of(i), "")]),
        );
    }
}

/// `for x in y.iter() {`; `i` is the `for`.
fn explicit_iter_loop(r: &mut R, i: usize) {
    let cx = r.cx;
    let mut j = i + 1;
    while j < cx.c.len() && cx.s(j) != "in" && cx.s(j) != "{" {
        match cx.s(j) {
            "(" | "[" => {
                let c = cx.close[j];
                if c == usize::MAX {
                    return;
                }
                j = c;
            }
            _ => {}
        }
        j += 1;
    }
    if cx.s(j) != "in" {
        return;
    }
    let mut k = j + 1;
    while cx.is_ident(k) && matches!(cx.s(k + 1), "." | "::") {
        k += 2;
    }
    if cx.is_ident(k) && cx.s(k).starts_with(|c: char| c == '_' || c.is_lowercase())
        && matches!(cx.s(k), "iter" | "iter_mut") && k > j + 1
        && matches!(cx.s(k - 1), ".") && cx.s(k + 1) == "(" && cx.s(k + 2) == ")"
        && cx.s(k + 3) == "{"
    {
        r.report(
            "B011",
            format!(
                "`for .. in x.{}()` can be `for .. in &x` if `&x` is iterable",
                cx.s(k),
            ),
            k,
            None,
        );
    }
}

/// `fn name(.. &Vec<T> ..)`; `i` is the `fn`.
fn ptr_arg(r: &mut R, i: usize) {
    let cx = r.cx;
    let mut j = i + 2;
    let mut angle = 0i32;
    while j < cx.c.len() {
        match cx.s(j) {
            "<" => angle += 1,
            ">" => angle -= 1,
            "(" if angle <= 0 => break,
            "{" | ";" => return,
            _ => {}
        }
        j += 1;
    }
    let close = cx.close.get(j).copied().unwrap_or(usize::MAX);
    if close == usize::MAX {
        return;
    }
    for k in j + 1..close {
        if cx.s(k) == ":" && cx.s(k + 1) == "&" {
            let mut p = k + 2;
            if cx.c.get(p).is_some_and(|t| t.kind == Kind::Lifetime) {
                p += 1;
            }
            let (name, next) = (cx.s(p), cx.s(p + 1));
            let (what, better) = match name {
                "Vec" if next == "<"
                    && cx.c.get(p + 2).is_none_or(|t| t.kind != Kind::Lifetime) => {
                    ("&Vec<T>", "&[T]")
                }
                "String" if matches!(next, "," | ")") => ("&String", "&str"),
                "PathBuf" if matches!(next, "," | ")") => ("&PathBuf", "&Path"),
                _ => continue,
            };
            r.report(
                "B028",
                format!("prefer `{better}` over `{what}` in parameters"),
                k + 1,
                None,
            );
        }
    }
}

/// Index just past the item that an attribute (ending before `i`) annotates.
fn skip_item(cx: &Ctx, mut i: usize) -> usize {
    while cx.s(i) == "#" && cx.s(i + 1) == "[" {
        let c = cx.close[i + 1];
        if c == usize::MAX {
            return cx.c.len();
        }
        i = c + 1;
    }
    let mut depth = 0i32;
    while i < cx.c.len() {
        match cx.s(i) {
            "(" | "[" => depth += 1,
            ")" | "]" => depth -= 1,
            ";" if depth <= 0 => return i + 1,
            "{" if depth <= 0 => {
                let c = cx.close[i];
                return if c == usize::MAX { cx.c.len() } else { c + 1 };
            }
            _ => {}
        }
        i += 1;
    }
    cx.c.len()
}
