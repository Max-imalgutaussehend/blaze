//! Quality gate for forge on a real corpus: `eval <file-with-one-path-per-line>`.
//! Reports how many files format, how many are unsupported (and why), and how many change
//! again on a second pass (non-idempotent).
use std::collections::BTreeMap;
use std::sync::Mutex;

use blaze_forge::{FormatError, format_source};
use rayon::prelude::*;

fn main() {
    let list = std::fs::read_to_string(
            std::env::args().nth(1).expect("usage: eval <list>"),
        )
        .unwrap();
    let files: Vec<&str> = list.split_whitespace().collect();
    std::panic::set_hook(Box::new(|_| {}));
    let counts: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());
    let bad: Mutex<Vec<String>> = Mutex::new(Vec::new());
    files
        .par_iter()
        .for_each(|f| {
            let Ok(src) = std::fs::read_to_string(f) else { return };
            let key = match format_source(&src) {
                Ok(out) => {
                    match format_source(&out) {
                        Ok(again) if again == out => "ok".to_string(),
                        Ok(_) => {
                            bad.lock()
                                .unwrap()
                                .push(format!("not idempotent: {f}"));
                            "NOT IDEMPOTENT".to_string()
                        }
                        Err(_) => "second pass failed".to_string(),
                    }
                }
                Err(FormatError::Unsupported(why)) => {
                    bad.lock().unwrap().push(format!("unsupported ({why}): {f}"));
                    format!("unsupported: {why}")
                }
                Err(FormatError::Syntax(_)) => "syntax error (invalid input)".to_string(),
            };
            *counts.lock().unwrap().entry(key).or_default() += 1;
        });
    let total = files.len();
    for (k, v) in counts.lock().unwrap().iter() {
        println!("{v:>6} {:>5.1}%  {k}", *v as f64 * 100.0 / total as f64);
    }
    for b in bad.lock().unwrap().iter().take(40) {
        println!("  {b}");
    }
}
