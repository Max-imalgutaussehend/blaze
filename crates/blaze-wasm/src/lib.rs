//! Minimal C-ABI surface (no wasm-bindgen): the host allocates a buffer with `alloc`,
//! writes UTF-8 source into it, calls `lint`/`format`, then reads the result via
//! `result_ptr`/`result_len` and finally frees with `dealloc`.

use std::cell::RefCell;

thread_local! {
    static RESULT : RefCell < String > = const { RefCell::new(String::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` and `len` must come from a previous `alloc(len)` call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len) });
}

fn input<'a>(ptr: *const u8, len: usize) -> &'a str {
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) }).unwrap_or("")
}

fn set(s: String) {
    RESULT.with(|r| *r.borrow_mut() = s);
}

/// Lint: result is JSON `[{"code","message","line","column","fix","safe"}, ...]`.
#[unsafe(no_mangle)]
pub extern "C" fn lint(ptr: *const u8, len: usize) {
    let diags = blaze_ember::lint_source(input(ptr, len));
    let items: Vec<String> = diags
        .iter()
        .map(|d| {
            format!(
                "{{\"code\":\"{}\",\"message\":{:?},\"line\":{},\"column\":{},\"fix\":{},\"safe\":{}}}",
                d.code,
                d.message,
                d.line,
                d.column,
                d.fix.is_some(),
                d.safe,
            )
        })
        .collect();
    set(format!("[{}]", items.join(",")));
}

/// Apply ember's fixes: result is `{"output":..., "applied":n}`. `unsafe_fixes` != 0 also applies
/// the fixes that depend on types.
#[unsafe(no_mangle)]
pub extern "C" fn fix(ptr: *const u8, len: usize, unsafe_fixes: u32) {
    let sel = blaze_ember::Selection::default_rules();
    let (out, applied, _) = blaze_ember::fix_source(
        input(ptr, len),
        &sel,
        unsafe_fixes != 0,
    );
    set(format!("{{\"output\":{out:?},\"applied\":{applied}}}"));
}

/// All lint rules: `[{"code","name","summary","default","fixable","unsafe"}, ...]`.
#[unsafe(no_mangle)]
pub extern "C" fn rules() {
    let items: Vec<String> = blaze_ember::RULES
        .iter()
        .map(|r| {
            format!(
                "{{\"code\":\"{}\",\"name\":\"{}\",\"summary\":{:?},\"default\":{},\"fixable\":{},\"unsafe\":{}}}",
                r.code,
                r.name,
                r.summary,
                r.default_on,
                r.fixable,
                blaze_ember::UNSAFE_FIXES.contains(&r.code),
            )
        })
        .collect();
    set(format!("[{}]", items.join(",")));
}

/// Format: result is `{"ok":true,"output":...}` or `{"ok":false,"error":...}`.
#[unsafe(no_mangle)]
pub extern "C" fn format(ptr: *const u8, len: usize) {
    let src = input(ptr, len);
    let res = std::panic::catch_unwind(|| blaze_forge::format_source(src));
    set(
        match res {
            Ok(Ok(out)) => format!("{{\"ok\":true,\"output\":{out:?}}}"),
            Ok(Err(blaze_forge::FormatError::Syntax(e))) => {
                format!("{{\"ok\":false,\"error\":{e:?}}}")
            }
            Ok(Err(blaze_forge::FormatError::Unsupported(e))) => {
                format!("{{\"ok\":false,\"error\":{e:?}}}")
            }
            Err(_) => "{\"ok\":false,\"error\":\"internal error\"}".into(),
        },
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn result_ptr() -> *const u8 {
    RESULT.with(|r| r.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn result_len() -> usize {
    RESULT.with(|r| r.borrow().len())
}
