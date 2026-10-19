use super::*;

fn codes(src: &str) -> Vec<&'static str> {
    lint_selected(src, &Selection::all()).into_iter().map(|d| d.code).collect()
}

/// Apply every fix, safe and unsafe, so each rule's rewrite is tested.
fn fixed(src: &str) -> String {
    fix_source(src, &Selection::all(), true).0
}

#[test]
fn original_rules_still_fire() {
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
    assert!(
        codes("fn f(x: Option<u8>) { x.unwrap(); } // blaze:ignore").is_empty(),
    );
    assert!(
        codes("fn f(x: Option<u8>) { x.unwrap(); } // blaze:ignore B001").is_empty(),
    );
    assert_eq!(
        codes("fn f(x: Option<u8>) { x.unwrap(); } // blaze:ignore B002"),
        ["B001"],
    );
    assert!(
        codes("fn f(x: Option<u8>) { x.unwrap(); } // blaze:ignore unwrap-used")
            .is_empty(),
    );
    assert!(
        codes("// blaze:ignore-file\nfn f(x: Option<u8>) { x.unwrap(); }").is_empty(),
    );
}

#[test]
fn return_not_at_tail_is_fine() {
    assert!(codes("fn f(c: bool) -> u8 { if c { return 1; } 2 }").is_empty());
    assert!(
        codes("fn f() { let c = || { return 1; }; let _ = [0; 3]; }").is_empty(),
    );
    assert!(codes("trait T { fn f(&self); fn g(&self) -> u8 { 1 } }").is_empty());
}

#[test]
fn strings_and_comments_are_not_code() {
    assert!(
        codes("fn f() { let s = \".unwrap()\"; /* x.unwrap() */ }").is_empty(),
    );
}

#[test]
fn skips_cfg_test_items() {
    assert!(
        codes("#[cfg(test)] mod t { fn f(x: Option<u8>) { x.unwrap(); } }").is_empty(),
    );
    assert!(
        codes("#[cfg(test)] fn helper(x: Option<u8>) { x.unwrap(); }").is_empty(),
    );
    assert_eq!(
        codes("#[cfg(test)] mod t {} fn f(x: Option<u8>) { x.unwrap(); }"),
        ["B001"],
    );
}

#[test]
fn reports_position() {
    let d = &lint_source("fn f() {\n    dbg!(1);\n}")[0];
    assert_eq!((d.line, d.column), (2, 5));
}

#[test]
fn selection_defaults_select_and_ignore() {
    let src = "fn f(x: Option<u8>) { x.expect(\"e\"); x.unwrap(); }";
    assert_eq!(lint_source(src).len(), 1); // expect-used is off by default
    let only_expect = Selection::resolve(&["B007".into()], &[]).unwrap();
    assert_eq!(
        lint_selected(src, &only_expect).iter().map(|d| d.code).collect::<Vec<_>>(),
        ["B007"],
    );
    let no_unwrap = Selection::resolve(&[], &["unwrap-used".into()]).unwrap();
    assert!(lint_selected(src, &no_unwrap).is_empty());
    assert!(Selection::resolve(&["B999".into()], &[]).is_err());
}

// ---- new rules: each has a positive case, a fix expectation where fixable, and a negative case ----

#[test]
fn len_zero_variants_and_fix() {
    assert_eq!(
        fixed("fn f(v: Vec<u8>) -> bool { v.len() == 0 }"),
        "fn f(v: Vec<u8>) -> bool { v.is_empty() }",
    );
    assert_eq!(
        fixed("fn f(v: Vec<u8>) -> bool { v.len() > 0 }"),
        "fn f(v: Vec<u8>) -> bool { !v.is_empty() }",
    );
    assert_eq!(
        fixed("fn f(s: S) -> bool { s.items.len() != 0 }"),
        "fn f(s: S) -> bool { !s.items.is_empty() }",
    );
    assert_eq!(
        fixed("fn f(v: Vec<u8>) -> bool { v.len() >= 1 }"),
        "fn f(v: Vec<u8>) -> bool { !v.is_empty() }",
    );
    assert!(codes("fn f(v: Vec<u8>) -> bool { v.len() == 2 }").is_empty());
}

#[test]
fn needless_return_fix() {
    assert_eq!(fixed("fn f() -> u8 { return 1; }"), "fn f() -> u8 { 1 }");
    assert_eq!(fixed("fn f() { work(); return; }"), "fn f() { work(); }");
}

#[test]
fn dbg_fix_statement_and_expression() {
    assert_eq!(fixed("fn f() { dbg!(x); work(); }"), "fn f() {  work(); }");
    assert_eq!(
        fixed("fn f() -> u8 { let a = dbg!(1 + 2); a }"),
        "fn f() -> u8 { let a = 1 + 2; a }",
    );
}

#[test]
fn mem_replace_default() {
    assert_eq!(
        fixed(
            "fn f(x: &mut V) { let old = mem::replace(&mut x.a, Default::default()); }",
        ),
        "fn f(x: &mut V) { let old = mem::take(&mut x.a); }",
    );
    assert!(
        codes("fn f(x: &mut V) { let old = mem::replace(&mut x.a, 3); }").is_empty(),
    );
}

#[test]
fn identity_map_and_nth_zero() {
    assert_eq!(
        fixed("fn f(i: I) { let v = i.iter().map(|x| x).count_all(); }"),
        "fn f(i: I) { let v = i.iter().count_all(); }",
    );
    assert_eq!(fixed("fn f(i: I) { i.nth(0); }"), "fn f(i: I) { i.next(); }");
    assert!(codes("fn f(i: I) { i.map(|x| y); }").is_empty());
}

#[test]
fn empty_println() {
    assert_eq!(fixed("fn f() { println!(\"\"); }"), "fn f() { println!(); }");
    assert_eq!(fixed("fn f() { eprintln!(\"\"); }"), "fn f() { eprintln!(); }");
}

#[test]
fn get_first_is_reported_not_fixed() {
    let d = lint_selected("fn f(v: Vec<u8>) { v.get(0); }", &Selection::all());
    assert_eq!(d.len(), 1);
    assert!(d[0].fix.is_none());
    assert!(codes("fn f(v: Vec<u8>) { v.get(1); }").is_empty());
}

#[test]
fn filter_next_and_skip_next() {
    assert_eq!(
        fixed("fn f(i: I) { i.filter(|x| x.ok()).next(); }"),
        "fn f(i: I) { i.find(|x| x.ok()); }",
    );
    assert_eq!(
        fixed("fn f(i: I) { i.skip(2).next(); }"),
        "fn f(i: I) { i.nth(2); }",
    );
    assert!(codes("fn f(i: I) { i.filter(|x| x.ok()).count(); }").is_empty());
}

#[test]
fn unwrap_or_default_and_or_fun_call() {
    assert_eq!(
        fixed("fn f(o: Option<Vec<u8>>) { o.unwrap_or(Vec::new()); }"),
        "fn f(o: Option<Vec<u8>>) { o.unwrap_or_default(); }",
    );
    assert_eq!(
        fixed("fn f(o: Option<String>) { o.unwrap_or(Default::default()); }"),
        "fn f(o: Option<String>) { o.unwrap_or_default(); }",
    );
    assert_eq!(
        codes("fn f(o: Option<String>) { o.unwrap_or(compute(1)); }"),
        ["B020"],
    );
    assert_eq!(
        codes("fn f(o: Option<String>) { o.unwrap_or(format!(\"{}\", 1)); }"),
        ["B020"],
    );
    assert!(
        codes(
                "fn f(o: Option<u8>) { o.unwrap_or(0); o.unwrap_or(Some(1)); o.unwrap_or(CONST); o.unwrap_or(now()); }",
            )
            .is_empty(),
    );
}

#[test]
fn assign_op() {
    assert_eq!(fixed("fn f() { x = x + 1; }"), "fn f() { x += 1; }");
    assert_eq!(
        fixed("fn f() { self.total = self.total * rate; }"),
        "fn f() { self.total *= rate; }",
    );
    assert_eq!(
        fixed("fn f() { x = x - foo(1, 2); }"),
        "fn f() { x -= foo(1, 2); }",
    );
    // operator in the rest of the expression: not simple, no report
    assert!(codes("fn f() { x = x + y * 2; }").is_empty());
    assert!(codes("fn f() { x = y + 1; }").is_empty());
    assert!(codes("fn f() { x += 1; }").is_empty());
}

#[test]
fn and_then_some() {
    assert_eq!(
        fixed("fn f(o: Option<u8>) { o.and_then(|x| Some(x + 1)); }"),
        "fn f(o: Option<u8>) { o.map(|x| x + 1); }",
    );
    assert_eq!(
        fixed("fn f(o: R) { o.and_then(|x| Ok(x + 1)); }"),
        "fn f(o: R) { o.map(|x| x + 1); }",
    );
    // the identity map this leaves behind is removed in the next fix round
    assert_eq!(
        fixed("fn f(o: R) { o.and_then(|x| Ok(x)); }"),
        "fn f(o: R) { o; }",
    );
    assert!(codes("fn f(o: Option<u8>) { o.and_then(|x| g(x)); }").is_empty());
    assert!(
        codes("fn f(o: Option<u8>) { o.and_then(|x| Some(x)).foo(); }").len() == 1,
    );
}

#[test]
fn test_and_example_paths_are_relaxed() {
    use std::path::Path;
    assert!(is_test_path(Path::new("crates/x/tests/it.rs")));
    assert!(is_test_path(Path::new("crates/x/examples/demo.rs")));
    assert!(is_test_path(Path::new("crates/x/src/tests.rs")));
    assert!(!is_test_path(Path::new("crates/x/src/lib.rs")));
    assert!(!is_test_path(Path::new("src/contests/lib.rs")));
}

#[test]
fn unsafe_fixes_need_opt_in() {
    let src = "fn f(v: Vec<u8>) -> bool { v.len() == 0 }";
    let (out, n, _) = fix_source(src, &Selection::all(), false);
    assert_eq!((out.as_str(), n), (src, 0));
    let d = lint_source(src);
    assert!(d[0].fix.is_some() && !d[0].safe);
    // a type-independent fix is applied by default
    assert_eq!(fix_source("fn f() { dbg!(x); }", &Selection::all(), false).1, 1);
}

#[test]
fn and_then_with_question_mark_is_left_alone() {
    assert!(
        codes("fn f(o: Option<&str>) { o.and_then(|s| Some(g(s.strip_prefix('-')?))); }")
            .is_empty(),
    );
    assert!(codes("fn f(v: &Vec<'a, T>) {}").is_empty());
}

#[test]
fn empty_string_new() {
    assert_eq!(
        fixed("fn f() -> String { String::from(\"\") }"),
        "fn f() -> String { String::new() }",
    );
    assert_eq!(
        fixed("fn f() -> String { \"\".to_string() }"),
        "fn f() -> String { String::new() }",
    );
    assert!(codes("fn f() -> String { String::from(\"a\") }").is_empty());
}

#[test]
fn partialeq_none_and_pattern_matching() {
    assert_eq!(codes("fn f(x: Option<u8>) -> bool { x == None }"), ["B024"]);
    assert_eq!(
        codes("fn f(x: Option<u8>) { if let Some(_) = x { go(); } }"),
        ["B025"],
    );
    assert_eq!(codes("fn f(x: Option<u8>) { while let Ok(_) = x {} }"), ["B025"]);
    assert!(
        codes("fn f(x: Option<u8>) { if let Some(v) = x { go(v); } }").is_empty(),
    );
}

#[test]
fn identity_op() {
    assert_eq!(fixed("fn f(x: u8) -> u8 { x + 0 }"), "fn f(x: u8) -> u8 { x }");
    assert_eq!(
        fixed("fn f(x: u8) -> u8 { g(x * 1, 2) }"),
        "fn f(x: u8) -> u8 { g(x, 2) }",
    );
    assert!(codes("fn f(x: f64) -> f64 { x * 1.0 }").is_empty());
    assert!(codes("fn f(x: u8) -> u8 { x * 1 + 2 }").is_empty()); // not followed by a terminator: left alone
    assert!(codes("fn f() -> i8 { -0 }").is_empty());
}

#[test]
fn map_or_bool_ptr_arg_bool_to_if() {
    assert_eq!(
        codes("fn f(o: Option<u8>) -> bool { o.map_or(false, |x| x > 1) }"),
        ["B027"],
    );
    assert_eq!(codes("fn f(v: &Vec<u8>) {}"), ["B028"]);
    assert_eq!(codes("fn f(s: &String, p: &PathBuf) {}"), ["B028", "B028"]);
    assert!(codes("fn f(v: &mut Vec<u8>) {}").is_empty());
    assert_eq!(
        codes("fn f(c: bool) -> bool { if c { true } else { false } }"),
        ["B029"],
    );
}

#[test]
fn redundant_field_names() {
    assert_eq!(
        fixed("fn f(a: u8) -> S { S { a: a, b: 1 } }"),
        "fn f(a: u8) -> S { S { a, b: 1 } }",
    );
    assert_eq!(
        fixed("fn f(a: u8) -> S { S { b: 1, a: a } }"),
        "fn f(a: u8) -> S { S { b: 1, a } }",
    );
    // parameters and generics with the same name are not struct fields
    assert!(codes("fn f(a: a) {}").is_empty());
}

#[test]
fn single_char_pattern() {
    // report only: `OsStr`, `Path` and custom traits have methods of the same name that take `&str`
    let d = lint_selected("fn f(s: &str) { s.split(\",\"); }", &Selection::all());
    assert_eq!(d.iter().map(|d| d.code).collect::<Vec<_>>(), ["B031"]);
    assert!(d[0].fix.is_none());
    assert!(
        codes("fn f(s: &str) { s.split(\"ab\"); s.contains(\"\\n\"); }").is_empty(),
    );
}

#[test]
fn opt_in_rules() {
    assert_eq!(codes("fn f(x: Option<u8>) { x.expect(\"x\"); }"), ["B007"]);
    assert_eq!(codes("fn f() { println!(\"hi\"); }"), ["B008"]);
    assert_eq!(codes("fn f() { unsafe { g() }; }"), ["B009"]);
    assert!(
        codes("fn f() {\n    // SAFETY: g is fine\n    unsafe { g() };\n}").is_empty(),
    );
    assert_eq!(codes("// TODO: later\nfn f() {}"), ["B010"]);
    assert_eq!(codes("fn f(v: V) { for x in v.items.iter() {} }"), ["B011"]);
    assert!(codes("fn f(v: V) { for x in v.iter().rev() {} }").is_empty());
}

#[test]
fn fixes_keep_bracket_balance_and_reach_a_fixed_point() {
    let src = "fn f(v: Vec<u8>, o: Option<Vec<u8>>) -> bool {\n    let a = o.unwrap_or(Vec::new());\n    println!(\"\");\n    v.len() == 0 && a.len() > 0\n}\n";
    let (out, n, rest) = fix_source(src, &Selection::default_rules(), true);
    assert!(n >= 3, "{out}");
    assert!(rest.iter().all(|d| d.fix.is_none()), "{rest:?}");
    // a second run changes nothing
    assert_eq!(fix_source(&out, &Selection::default_rules(), true).0, out);
}

#[test]
fn overlapping_fixes_do_not_corrupt_the_file() {
    // `.map(|x| x)` inside the argument of another fixed call
    let src = "fn f(o: Option<Vec<u8>>) { o.unwrap_or(Vec::new()).iter().map(|x| x).count(); }";
    let out = fixed(src);
    assert_eq!(
        out,
        "fn f(o: Option<Vec<u8>>) { o.unwrap_or_default().iter().count(); }",
    );
}
