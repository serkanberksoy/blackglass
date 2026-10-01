//! A JavaScript sandbox for plugins (DataviewJS blocks, Templater's
//! `<%* %>` commands), on the pure-Rust Boa engine. Scripts see only what
//! the plugin gives them: no files, no network, no timers. Loops and
//! recursion are limited, so a runaway script in a note fails with an
//! error instead of freezing the editor.
//!
//! Besides the plugin's own prelude, every script has:
//! - `__fmt(iso, format)`: a date (`YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`) in
//!   moment.js `format`;
//! - `__shift(iso, offset)`: a date moved by a number of days or an ISO
//!   8601 duration (`P1M`), or `null`;
//! - `__parse(text, format)`: `text` read as a date in `format`, or `null`;
//! - `__now()`: the local date and time;
//! - `__clipboard()`: the clipboard's text.

use boa_engine::{Context, JsResult, JsValue, NativeFunction, Source, js_string};
use chrono::{Local, NaiveDateTime};

use super::moment;

/// The most loop iterations a script may run.
const LOOP_LIMIT: u64 = 5_000_000;
/// The deepest a script may recurse.
const RECURSION_LIMIT: usize = 512;

/// A native function a plugin adds to its scripts: name, number of
/// arguments, function.
pub type Native = (
    &'static str,
    usize,
    fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
);

/// Runs `script` (which should set `globalThis.__result`, a string, or
/// `globalThis.__error`), finishing any `async` work, with the plugin's
/// own `natives` besides the shared ones. Returns `__result`, or the
/// script's error.
pub fn run(script: &str, natives: &[Native]) -> Result<String, String> {
    let mut ctx = Context::default();
    ctx.runtime_limits_mut()
        .set_loop_iteration_limit(LOOP_LIMIT);
    ctx.runtime_limits_mut()
        .set_recursion_limit(RECURSION_LIMIT);
    let shared: [Native; 5] = [
        ("__fmt", 2, fmt),
        ("__shift", 2, shift),
        ("__parse", 2, parse),
        ("__now", 0, now),
        ("__clipboard", 0, clipboard),
    ];
    for &(name, arity, f) in shared.iter().chain(natives) {
        ctx.register_global_callable(js_string!(name), arity, NativeFunction::from_fn_ptr(f))
            .map_err(|e| e.to_string())?;
    }
    ctx.eval(Source::from_bytes(script))
        .map_err(|e| clean(&e.to_string()))?;
    let _ = ctx.run_jobs();
    let global = |name: &str, ctx: &mut Context| -> Option<String> {
        let v = ctx.global_object().get(js_string!(name), ctx).ok()?;
        if v.is_undefined() {
            return None;
        }
        Some(v.to_string(ctx).ok()?.to_std_string_escaped())
    };
    if let Some(error) = global("__error", &mut ctx) {
        return Err(clean(&error));
    }
    global("__result", &mut ctx).ok_or_else(|| "the script ended without a result".into())
}

/// A JavaScript string literal for `text` (for putting data in a script).
pub fn literal(text: &str) -> String {
    serde_json::to_string(text).expect("a string serializes")
}

/// An error message without Boa's position noise.
fn clean(error: &str) -> String {
    error.lines().next().unwrap_or(error).trim().to_string()
}

/// Argument `i` as text (`None` if missing, null or undefined).
pub fn arg(args: &[JsValue], i: usize, ctx: &mut Context) -> JsResult<Option<String>> {
    match args.get(i) {
        None => Ok(None),
        Some(v) if v.is_null_or_undefined() => Ok(None),
        Some(v) => Ok(Some(v.to_string(ctx)?.to_std_string_escaped())),
    }
}

/// A JavaScript string.
pub fn text(s: &str) -> JsValue {
    JsValue::from(js_string!(s))
}

/// Reads `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM[:SS]` (what these helpers pass
/// around).
pub fn read_iso(iso: &str) -> Option<NaiveDateTime> {
    crate::plugins::dataview::value::parse_date(iso)
}

/// Writes a date the way these helpers pass it around.
pub fn write_iso(date: &NaiveDateTime) -> String {
    date.format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn fmt(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (Some(iso), format) = (arg(args, 0, ctx)?, arg(args, 1, ctx)?) else {
        return Ok(JsValue::null());
    };
    Ok(match read_iso(&iso) {
        Some(d) => text(&moment::format(
            &d,
            format.as_deref().unwrap_or("YYYY-MM-DD"),
        )),
        None => JsValue::null(),
    })
}

fn shift(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (Some(iso), Some(offset)) = (arg(args, 0, ctx)?, arg(args, 1, ctx)?) else {
        return Ok(JsValue::null());
    };
    Ok(read_iso(&iso)
        .and_then(|d| moment::shift(d, &offset))
        .map_or(JsValue::null(), |d| text(&write_iso(&d))))
}

fn parse(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (Some(t), Some(format)) = (arg(args, 0, ctx)?, arg(args, 1, ctx)?) else {
        return Ok(JsValue::null());
    };
    Ok(moment::parse(&t, &format).map_or(JsValue::null(), |d| text(&write_iso(&d))))
}

fn now(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    Ok(text(&write_iso(&Local::now().naive_local())))
}

fn clipboard(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    use mdedit::clipboard::Clipboard;
    let t = mdedit::clipboard::SystemClipboard
        .read()
        .unwrap_or_default();
    Ok(text(&t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_errors_and_async() {
        assert_eq!(
            run("globalThis.__result = String(1 + 2)", &[]),
            Ok("3".into())
        );
        let r = run(
            "(async () => { const x = await Promise.resolve(4); globalThis.__result = 'x' + x; })()",
            &[],
        );
        assert_eq!(r, Ok("x4".into()));
        let e = run("nope()", &[]).unwrap_err();
        assert!(e.contains("nope"), "{e}");
        let e = run("globalThis.__error = 'custom'", &[]).unwrap_err();
        assert_eq!(e, "custom");
        assert_eq!(
            run("1", &[]).unwrap_err(),
            "the script ended without a result"
        );
    }

    #[test]
    fn runaway_scripts_stop() {
        let e = run("while (true) {}", &[]).unwrap_err();
        assert!(e.contains("loop"), "{e}");
        let e = run("function f() { return f(); } f()", &[]).unwrap_err();
        assert!(e.contains("recursive calls"), "{e}");
    }

    #[test]
    fn date_helpers() {
        let r = run(
            "globalThis.__result = [__fmt('2026-08-09', 'dddd Do'), __shift('2026-01-31', 'P1M'), __parse('9.8.2026', 'D.M.YYYY'), typeof __now()].join('|')",
            &[],
        );
        assert_eq!(
            r,
            Ok("Sunday 9th|2026-02-28T00:00:00|2026-08-09T00:00:00|string".into())
        );
        assert_eq!(literal("a\"b\n"), "\"a\\\"b\\n\"");
    }
}
