//! A small, allocation-free Rust lexer. It is the shared foundation of ember (lint)
//! and forge (comment re-attachment): it yields tokens *and* comments with exact
//! byte offsets, and runs at memory-bandwidth-ish speed because it never builds a tree.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ident,
    Lifetime,
    Literal,
    Punct,
    /// `//` or `/* */` that is not a doc comment.
    Comment,
    /// `///`, `//!`, `/** */`, `/*! */`
    DocComment,
}

#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    /// 1-based line of `start`.
    pub line: u32,
    /// 1-based column (in bytes) of `start`.
    pub col: u32,
}

impl Token {
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        &src[self.start..self.end]
    }
}

fn is_ident_start(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphabetic() || b >= 0x80
}

fn is_ident_continue(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric() || b >= 0x80
}

/// Tokenize `src`. Whitespace is dropped; comments are kept as tokens.
/// Never fails: malformed input degrades to punctuation tokens.
pub fn lex(src: &str) -> Vec<Token> {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len() / 4);
    let (mut i, mut line, mut line_start) = (0usize, 1u32, 0usize);

    macro_rules! push {
        ($kind:expr, $start:expr, $end:expr, $line:expr, $ls:expr) => {
            out.push(Token { kind : $kind, start : $start, end : $end, line : $line, col
            : ($start - $ls + 1) as u32, })
        };
    }

    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            line_start = i;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let (start, tok_line, tok_ls) = (i, line, line_start);

        // Comments
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            let mut j = i + 2;
            while j < b.len() && b[j] != b'\n' {
                j += 1;
            }
            let doc = (b.get(i + 2) == Some(&b'/') && b.get(i + 3) != Some(&b'/'))
                || b.get(i + 2) == Some(&b'!');
            push!(
                if doc { Kind::DocComment } else { Kind::Comment }, start, j, tok_line,
                tok_ls
            );
            i = j;
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            let mut depth = 1;
            let mut j = i + 2;
            while j < b.len() && depth > 0 {
                if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                    depth += 1;
                    j += 2;
                } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                    depth -= 1;
                    j += 2;
                } else {
                    if b[j] == b'\n' {
                        line += 1;
                        line_start = j + 1;
                    }
                    j += 1;
                }
            }
            let doc = (b.get(i + 2) == Some(&b'*') && b.get(i + 3) != Some(&b'*')
                && b.get(i + 3) != Some(&b'/')) || b.get(i + 2) == Some(&b'!');
            push!(
                if doc { Kind::DocComment } else { Kind::Comment }, start, j, tok_line,
                tok_ls
            );
            i = j;
            continue;
        }

        // Raw strings / raw idents / byte & c strings
        if c == b'r' || c == b'b' || c == b'c' {
            let mut j = i;
            if (c == b'b' || c == b'c') && b.get(j + 1) == Some(&b'r') {
                j += 1;
            }
            if b[j] == b'r' {
                let mut k = j + 1;
                let mut hashes = 0;
                while b.get(k) == Some(&b'#') {
                    hashes += 1;
                    k += 1;
                }
                if b.get(k) == Some(&b'"') {
                    k += 1;
                    'raw: while k < b.len() {
                        if b[k] == b'\n' {
                            line += 1;
                            line_start = k + 1;
                        }
                        if b[k] == b'"'
                            && (0..hashes).all(|h| b.get(k + 1 + h) == Some(&b'#'))
                        {
                            k += 1 + hashes;
                            break 'raw;
                        }
                        k += 1;
                    }
                    push!(Kind::Literal, start, k, tok_line, tok_ls);
                    i = k;
                    continue;
                }
                if c == b'r' && hashes == 1
                    && b.get(k).is_some_and(|&x| is_ident_start(x))
                {
                    // raw identifier r#foo
                    while k < b.len() && is_ident_continue(b[k]) {
                        k += 1;
                    }
                    push!(Kind::Ident, start, k, tok_line, tok_ls);
                    i = k;
                    continue;
                }
            }
        }

        // Strings (with optional b/c prefix)
        let is_str = c == b'"'
            || ((c == b'b' || c == b'c') && b.get(i + 1) == Some(&b'"'));
        if is_str {
            let mut j = if c == b'"' { i + 1 } else { i + 2 };
            while j < b.len() && b[j] != b'"' {
                if b[j] == b'\\' {
                    j += 1;
                }
                if j < b.len() && b[j] == b'\n' {
                    line += 1;
                    line_start = j + 1;
                }
                j += 1;
            }
            j = (j + 1).min(b.len());
            push!(Kind::Literal, start, j, tok_line, tok_ls);
            i = j;
            continue;
        }

        // Char / byte-char literal vs lifetime
        if c == b'\'' || (c == b'b' && b.get(i + 1) == Some(&b'\'')) {
            let q = if c == b'b' { i + 1 } else { i };
            let mut j = q + 1;
            let is_char = if b.get(j) == Some(&b'\\') {
                true
            } else {
                // 'x' where the char (possibly multibyte) is followed by a quote
                let mut e = j + 1;
                while e < b.len() && (b[e] & 0xC0) == 0x80 {
                    e += 1;
                }
                b.get(e) == Some(&b'\'')
            };
            if is_char {
                if b.get(j) == Some(&b'\\') {
                    j += 2;
                }
                while j < b.len() && b[j] != b'\'' && b[j] != b'\n' {
                    j += 1;
                }
                j = (j + 1).min(b.len());
                push!(Kind::Literal, start, j, tok_line, tok_ls);
            } else {
                while j < b.len() && is_ident_continue(b[j]) {
                    j += 1;
                }
                push!(Kind::Lifetime, start, j, tok_line, tok_ls);
            }
            i = j;
            continue;
        }

        if is_ident_start(c) {
            let mut j = i + 1;
            while j < b.len() && is_ident_continue(b[j]) {
                j += 1;
            }
            push!(Kind::Ident, start, j, tok_line, tok_ls);
            i = j;
            continue;
        }

        if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < b.len() {
                let d = b[j];
                let frac = d == b'.' && b.get(j + 1).is_some_and(|x| x.is_ascii_digit());
                let exp_sign = (d == b'+' || d == b'-')
                    && matches!(b[j - 1], b'e' | b'E') && !src[i..j].starts_with("0x");
                if !(is_ident_continue(d) || frac || exp_sign) {
                    break;
                }
                j += 1;
            }
            push!(Kind::Literal, start, j, tok_line, tok_ls);
            i = j;
            continue;
        }

        // Punctuation: join the multi-char operators ember/forge care about.
        let two = |a: u8, bb: u8| c == a && b.get(i + 1) == Some(&bb);
        let len = if two(b'=', b'=') || two(b'!', b'=') || two(b'-', b'>')
            || two(b'=', b'>') || two(b':', b':') || two(b'<', b'=') || two(b'>', b'=')
            || two(b'&', b'&') || two(b'|', b'|')
        {
            2
        } else {
            // advance one full char
            1 + b[i + 1..].iter().take_while(|&&x| (x & 0xC0) == 0x80).count()
        };
        push!(Kind::Punct, start, i + len, tok_line, tok_ls);
        i += len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<String> {
        lex(src).iter().map(|t| t.text(src).to_string()).collect()
    }

    #[test]
    fn basics_and_positions() {
        let src = "fn f() {\n    x.unwrap();\n}";
        assert_eq!(
            texts(src), ["fn", "f", "(", ")", "{", "x", ".", "unwrap", "(", ")", ";",
            "}"]
        );
        let t = lex(src).into_iter().find(|t| t.text(src) == "unwrap").unwrap();
        assert_eq!((t.line, t.col), (2, 7));
    }

    #[test]
    fn comments_and_docs() {
        let src = "// a\n/// d\n//! i\n/* b /* n */ */ x //// not doc";
        let k: Vec<_> = lex(src).iter().map(|t| t.kind).collect();
        assert_eq!(
            k, [Kind::Comment, Kind::DocComment, Kind::DocComment, Kind::Comment,
            Kind::Ident, Kind::Comment]
        );
    }

    #[test]
    fn strings_hide_comment_markers() {
        let src = r##"let s = "// no"; let r = r#"/* no "# "quote"; 'c'; 'a: loop {} b'x' b"y""##;
        assert!(
            lex(src).iter().all(| t | ! matches!(t.kind, Kind::Comment |
            Kind::DocComment))
        );
        assert!(texts(src).contains(& "'a".to_string()));
        assert!(texts(src).contains(& "'c'".to_string()));
    }

    #[test]
    fn numbers_and_ops() {
        assert_eq!(texts("1.5e-3 == 0x1f..2"), ["1.5e-3", "==", "0x1f", ".", ".", "2"]);
    }
}
