//! Comment and blank-line re-attachment.
//!
//! prettyplease prints a syntax tree and therefore loses comments and blank lines. We
//! tokenize the original and the formatted text with the same lexer, align the two code
//! token streams, and re-insert every comment next to the token it was attached to.
//! The result is verified (code tokens unchanged, comments identical and in order);
//! if anything is off we return `None` and the caller leaves the file untouched.

use blaze_lexer::{Kind, Token, lex};

fn is_comment(k: Kind) -> bool {
    k == Kind::Comment
}

fn text<'a>(t: &Token, s: &'a str) -> &'a str {
    t.text(s)
}

fn text_at<'a>(t: &[Token], s: &'a str, i: usize) -> &'a str {
    t.get(i).map_or("", |x| x.text(s))
}

/// If the punctuation tokens starting at `from` concatenate exactly to `want`, return how many.
fn punct_run(t: &[Token], s: &str, from: usize, want: &str) -> Option<usize> {
    let mut acc = String::new();
    for (k, tok) in t.iter().enumerate().skip(from) {
        if tok.kind != Kind::Punct || acc.len() >= want.len() {
            break;
        }
        acc.push_str(tok.text(s));
        if acc == want {
            return Some(k - from + 1);
        }
        if !want.starts_with(&acc) {
            break;
        }
    }
    None
}

/// Maps each original code token to a formatted code token index.
fn align(o: &[Token], os: &str, f: &[Token], fs: &str) -> Option<Vec<usize>> {
    let (mut i, mut j) = (0, 0);
    let mut map = vec![0; o.len()];
    while i < o.len() {
        let Some(ft) = f.get(j) else {
            // formatted ran out: only removable tokens may remain
            if matches!(text(& o[i], os), "," | "(" | ")" | "{" | "}" | ";") {
                map[i] = f.len().saturating_sub(1);
                i += 1;
                continue;
            }
            if std::env::var_os("FORGE_DEBUG").is_some() {
                eprintln!(
                    "align mismatch: formatted text ended, orig line {} `{}` left over",
                    o[i].line,
                    text(&o[i], os),
                );
            }
            return None;
        };
        // `#[doc = "x"]` is printed as `/// x` by the formatter
        if ft.kind == Kind::DocComment && text(&o[i], os) == "#" {
            let inner = text(o.get(i + 1).unwrap_or(&o[i]), os) == "!";
            let b0 = i + 1 + usize::from(inner);
            let is_doc_attr = text_at(o, os, b0) == "["
                && text_at(o, os, b0 + 1) == "doc" && text_at(o, os, b0 + 2) == "="
                && o.get(b0 + 3).is_some_and(|t| t.kind == Kind::Literal)
                && text_at(o, os, b0 + 4) == "]";
            if is_doc_attr {
                for m in &mut map[i..b0 + 5] {
                    *m = j;
                }
                i = b0 + 5;
                j += 1;
                continue;
            }
        }
        // the reverse: a doc comment printed as `#[doc = "x"]`
        if o[i].kind == Kind::DocComment && text(ft, fs) == "#" {
            let inner = text_at(f, fs, j + 1) == "!";
            let b0 = j + 1 + usize::from(inner);
            if text_at(f, fs, b0) == "[" && text_at(f, fs, b0 + 1) == "doc"
                && text_at(f, fs, b0 + 2) == "="
                && f.get(b0 + 3).is_some_and(|t| t.kind == Kind::Literal)
                && text_at(f, fs, b0 + 4) == "]"
            {
                map[i] = j;
                i += 1;
                j = b0 + 5;
                continue;
            }
        }
        let (a, b) = (text(&o[i], os), text(ft, fs));
        // the same punctuation split differently, e.g. `=>` vs `=` `>`
        if a != b && o[i].kind == Kind::Punct && ft.kind == Kind::Punct {
            if let Some(k) = punct_run(f, fs, j, a) {
                map[i] = j;
                i += 1;
                j += k;
                continue;
            }
            if let Some(k) = punct_run(o, os, i, b) {
                for m in &mut map[i..i + k] {
                    *m = j;
                }
                i += k;
                j += 1;
                continue;
            }
        }
        let both_doc = o[i].kind == Kind::DocComment && ft.kind == Kind::DocComment;
        if a == b || both_doc {
            map[i] = j;
            i += 1;
            j += 1;
        } else if matches!(b, "," | "(" | ")" | "{" | "}" | ";") {
            j += 1; // token inserted by the formatter
        } else if matches!(a, "," | "(" | ")" | "{" | "}" | ";") {
            map[i] = j; // token removed by the formatter
            i += 1;
        } else {
            if std::env::var_os("FORGE_DEBUG").is_some() {
                eprintln!(
                    "align mismatch: orig line {} `{a}` vs fmt line {} `{b}`",
                    o[i].line,
                    ft.line,
                );
            }
            return None;
        }
    }
    Some(map)
}

struct Insert {
    blank: bool,
    text: String,
}

pub fn reattach(orig: &str, formatted: &str) -> Option<String> {
    match reattach_dbg(orig, formatted) {
        Ok(s) => Some(s),
        Err(e) => {
            if std::env::var_os("FORGE_DEBUG").is_some() {
                eprintln!("reattach failed: {e}");
            }
            None
        }
    }
}

pub fn reattach_dbg(orig: &str, formatted: &str) -> Result<String, String> {
    let ot = lex(orig);
    let ft: Vec<Token> = lex(formatted);
    let o_code: Vec<Token> = ot
        .iter()
        .copied()
        .filter(|t| !is_comment(t.kind))
        .collect();
    let map = align(&o_code, orig, &ft, formatted).ok_or("align")?;

    // line tables
    let o_lines: Vec<&str> = orig.split('\n').collect();
    let f_lines: Vec<&str> = formatted.split('\n').collect();
    let blank_above = |line: u32| {
        line >= 2 && o_lines.get(line as usize - 2).is_some_and(|l| l.trim().is_empty())
    };
    let first_on_line = |idx: usize, all: &[Token]| {
        idx == 0 || all[idx - 1].line != all[idx].line
    };

    let mut before: Vec<Vec<Insert>> = (0..f_lines.len()).map(|_| Vec::new()).collect();
    let mut after: Vec<Vec<String>> = vec![Vec::new(); f_lines.len()];
    let mut blank_code: Vec<bool> = vec![false; f_lines.len()];
    let mut trailing_eof: Vec<Insert> = Vec::new();

    // blank lines before code tokens that start an original line
    let mut f_first = vec![false; ft.len()];
    for (k, t) in ft.iter().enumerate() {
        f_first[k] = k == 0 || ft[k - 1].line != t.line;
    }
    let code_pos: Vec<usize> = ot
        .iter()
        .enumerate()
        .filter(|(_, t)| !is_comment(t.kind))
        .map(|(p, _)| p)
        .collect();
    for (ci, t) in o_code.iter().enumerate() {
        // first token on its original line, considering comments too
        let pos = code_pos[ci];
        if first_on_line(pos, &ot) && blank_above(t.line) && ci > 0 {
            let j = map[ci];
            if f_first.get(j) == Some(&true) && ft[j].line >= 2 {
                blank_code[ft[j].line as usize - 1] = true;
            }
        }
    }

    // comments
    let mut ci = 0; // number of code tokens seen
    let mut orig_comments = Vec::new();
    for (pos, t) in ot.iter().enumerate() {
        if !is_comment(t.kind) {
            ci += 1;
            continue;
        }
        let ctext = text(t, orig).to_string();
        orig_comments.push(ctext.clone());
        let line_prefix_blank = orig[orig[..t.start]
                .rfind('\n')
                .map_or(0, |p| p + 1)..t.start]
            .trim()
            .is_empty();
        let own_line = line_prefix_blank;
        if own_line {
            let ins = Insert {
                blank: first_on_line(pos, &ot) && blank_above(t.line),
                text: ctext,
            };
            if let Some(&j) = map.get(ci).filter(|_| ci < o_code.len()) {
                let ln = ft.get(j).ok_or("ftget")?.line as usize - 1;
                before[ln].push(ins);
            } else {
                trailing_eof.push(ins);
            }
        } else {
            if ci == 0 {
                return Err("trailing-at-start".into());
            }
            let j = map[ci - 1];
            let tok = ft.get(j).ok_or("ftget2")?;
            let end_line = tok.line as usize - 1
                + text(tok, formatted).matches('\n').count();
            after[end_line].push(ctext);
        }
    }

    // emit
    let mut out: Vec<String> = Vec::with_capacity(f_lines.len() + orig_comments.len());
    let push_blank = |out: &mut Vec<String>| {
        let last = out.last().map(|s| s.trim_end());
        if let Some(l) = last && !l.is_empty() && !l.ends_with(['{', '(', '[']) {
            out.push(String::new());
        }
    };
    // Indentation for a comment placed in front of formatted line `ln`: that line's indent, or one
    // level deeper when the line starts with a closing delimiter (the comment belongs to the block).
    let comment_indent = |ln: usize| -> String {
        let line = f_lines.get(ln).copied().unwrap_or("");
        let indent: String = line.chars().take_while(|c| *c == ' ').collect();
        if line.trim_start().starts_with(['}', ')', ']']) {
            format!("{indent}    ")
        } else {
            indent
        }
    };
    for (ln, line) in f_lines.iter().enumerate() {
        for ins in &before[ln] {
            if ins.blank {
                push_blank(&mut out);
            }
            out.push(format!("{}{}", comment_indent(ln), ins.text));
        }
        let trimmed = line.trim_start();
        if blank_code[ln] && !trimmed.starts_with(['}', ')', ']']) {
            push_blank(&mut out);
        }
        let mut l = (*line).to_string();
        // A `//` comment runs to the end of the line, so once one is placed inline, further
        // trailing comments of this line move to their own lines right below it.
        let mut extra: Vec<&String> = Vec::new();
        let mut line_open = true;
        for c in &after[ln] {
            if line_open {
                l.push(' ');
                l.push_str(c);
                line_open = !c.starts_with("//");
            } else {
                extra.push(c);
            }
        }
        out.push(l);
        if !extra.is_empty() {
            // like an own-line comment, attach to the next non-empty formatted line
            let next = (ln + 1..f_lines.len())
                .find(|&n| !f_lines[n].trim().is_empty())
                .unwrap_or(ln);
            let ind = comment_indent(next);
            for c in extra {
                out.push(format!("{ind}{c}"));
            }
        }
    }
    // formatted text ends with "\n" so the last element is empty: put EOF comments before it
    if !trailing_eof.is_empty() {
        if out.last().is_some_and(|s| s.is_empty()) {
            out.pop();
        }
        for ins in trailing_eof {
            if ins.blank {
                push_blank(&mut out);
            }
            out.push(ins.text);
        }
        out.push(String::new());
    }
    let result = out.join("\n");

    // verification: stream both token lists, no intermediate vectors
    let rt = lex(&result);
    let mut expected = ft.iter();
    let mut comments = orig_comments.iter();
    for t in &rt {
        if is_comment(t.kind) {
            let want = comments.next().map(String::as_str);
            if want != Some(text(t, &result)) {
                debug_mismatch(
                    "comment",
                    t.line,
                    want.unwrap_or("<none>"),
                    text(t, &result),
                );
                return Err("verify".into());
            }
        } else {
            let want = expected.next().map(|f| text(f, formatted));
            if want != Some(text(t, &result)) {
                debug_mismatch(
                    "code",
                    t.line,
                    want.unwrap_or("<none>"),
                    text(t, &result),
                );
                return Err("verify".into());
            }
        }
    }
    if expected.next().is_some() || comments.next().is_some() {
        return Err("verify".into());
    }
    Ok(result)
}

fn debug_mismatch(kind: &str, line: u32, want: &str, got: &str) {
    if std::env::var_os("FORGE_DEBUG").is_some() {
        let cut = |s: &str| s.chars().take(40).collect::<String>();
        eprintln!(
            "verify mismatch ({kind}) at result line {line}: want `{}` got `{}`",
            cut(want),
            cut(got),
        );
    }
}
