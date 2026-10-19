//! Safety gate for ember's autofix on a real corpus: `fixcheck <file-with-one-path-per-line>`.
//! For every file that parses before fixing it must still parse after, and fixing must be
//! idempotent. Also prints how often each rule fires, with a few sample locations.
use std::collections::BTreeMap;
use std::sync::Mutex;

use blaze_ember::{Selection, fix_source, lint_selected};
use rayon::prelude::*;

/// (findings, fixable findings, sample locations)
type Stats = (usize, usize, Vec<String>);

fn main() {
    let list = std::fs::read_to_string(
            std::env::args().nth(1).expect("usage: fixcheck <list>"),
        )
        .unwrap();
    let files: Vec<&str> = list.split_whitespace().collect();
    let sel = Selection::all();
    let counts: Mutex<BTreeMap<&'static str, Stats>> = Mutex::new(BTreeMap::new());
    let broken: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let unstable: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let fixed_files = Mutex::new(0usize);
    files
        .par_iter()
        .for_each(|f| {
            let Ok(src) = std::fs::read_to_string(f) else { return };
            for d in lint_selected(&src, &sel) {
                let mut g = counts.lock().unwrap();
                let e = g.entry(d.code).or_default();
                e.0 += 1;
                e.1 += usize::from(d.fix.is_some());
                if e.2.len() < 3 {
                    e.2.push(format!("{f}:{}:{}", d.line, d.column));
                }
            }
            let parsed_before = syn::parse_file(&src).is_ok();
            let (out, n, _) = fix_source(&src, &sel, true);
            if n == 0 {
                return;
            }
            *fixed_files.lock().unwrap() += 1;
            if parsed_before && syn::parse_file(&out).is_err() {
                broken.lock().unwrap().push(f.to_string());
            }
            if fix_source(&out, &sel, true).1 != 0 {
                unstable.lock().unwrap().push(f.to_string());
            }
        });
    println!(
        "{} files, {} with fixes applied",
        files.len(),
        fixed_files.lock().unwrap(),
    );
    for (code, (n, fixable, samples)) in counts.lock().unwrap().iter() {
        println!(
            "{code}  {n:>6} findings, {fixable:>6} fixable   e.g. {}",
            samples.join("  "),
        );
    }
    println!(
        "BROKEN after fix (parsed before, not after): {}",
        broken.lock().unwrap().len(),
    );
    for b in broken.lock().unwrap().iter().take(10) {
        println!("  {b}");
    }
    println!("not idempotent: {}", unstable.lock().unwrap().len());
}
