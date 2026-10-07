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

/// moment.js as scripts use it (Templater's `moment()`, Tasks' dates):
/// made from a date (`YYYY-MM-DD[THH:MM:SS]`), another moment, text and
/// a format, or nothing (now: the script's `__data.now` if it has one);
/// `format`, `add` / `subtract`, comparisons (`isBefore`, `isAfter`,
/// `isSame`, `isSameOrBefore`, `isSameOrAfter`, to a unit: `"day"`,
/// `"month"`, `"year"`), `diff`, `startOf` and the parts of the date.
pub const MOMENT: &str = r#"
function __dur(n, unit) {
  n = Number(n);
  const sign = n < 0 ? "-" : "", a = Math.abs(n), u = String(unit || "days");
  if (u === "M" || /^months?$/i.test(u)) return `${sign}P${a}M`;
  if (/^(y|years?)$/i.test(u)) return `${sign}P${a}Y`;
  if (/^(w|weeks?)$/i.test(u)) return `${sign}P${a}W`;
  if (/^(h|hours?)$/i.test(u)) return `${sign}PT${a}H`;
  if (u === "m" || /^minutes?$/i.test(u)) return `${sign}PT${a}M`;
  if (/^(s|seconds?)$/i.test(u)) return `${sign}PT${a}S`;
  return `${sign}P${a}D`;
}
function __unitLength(unit) {
  const u = String(unit || "");
  if (/^(y|years?)$/i.test(u)) return 4;
  if (u === "M" || /^months?$/i.test(u)) return 7;
  if (/^(d|days?|date)$/i.test(u)) return 10;
  return 19;
}
function __cmp(a, b, unit) {
  const n = __unitLength(unit);
  const x = String(a.__iso || "").padEnd(19, "0").slice(0, n);
  const y = String(moment(b).__iso || "").padEnd(19, "0").slice(0, n);
  return x < y ? -1 : x > y ? 1 : 0;
}
function moment(input, format) {
  // A script's own "now" (`const __data`, a template's) is no property of
  // the global object.
  const now = () => (typeof __data !== "undefined" && __data.now) ? __data.now : __now();
  const iso = input === undefined ? now()
    : input === null ? null
    : (input && input.__iso !== undefined) ? input.__iso
    : format ? __parse(String(input), format) : String(input);
  return {
    __iso: iso,
    format(f = "YYYY-MM-DDTHH:mm:ss") { return __fmt(this.__iso, f); },
    add(n, unit) { return moment(__shift(this.__iso, __dur(n, unit))); },
    subtract(n, unit) { return moment(__shift(this.__iso, __dur(-n, unit))); },
    clone() { return moment(this.__iso); },
    isValid() { return !!this.__iso; },
    isBefore(o, unit) { return __cmp(this, o, unit) < 0; },
    isAfter(o, unit) { return __cmp(this, o, unit) > 0; },
    isSame(o, unit) { return __cmp(this, o, unit) === 0; },
    isSameOrBefore(o, unit) { return __cmp(this, o, unit) <= 0; },
    isSameOrAfter(o, unit) { return __cmp(this, o, unit) >= 0; },
    diff(o, unit) {
      const ms = new Date(this.__iso) - new Date(moment(o).__iso);
      const per = { y: 31536e6, M: 2592e6, w: 6048e5, d: 864e5, h: 36e5, m: 6e4, s: 1e3 };
      const u = String(unit || "ms");
      const k = /^years?$/i.test(u) ? "y" : (u === "M" || /^months?$/i.test(u)) ? "M"
        : /^weeks?$/i.test(u) ? "w" : /^days?$/i.test(u) ? "d" : /^hours?$/i.test(u) ? "h"
        : (u === "m" || /^minutes?$/i.test(u)) ? "m" : /^seconds?$/i.test(u) ? "s" : u.length === 1 ? u : "";
      return per[k] ? Math.trunc(ms / per[k]) : ms;
    },
    startOf(unit) {
      const n = __unitLength(unit);
      return moment((this.__iso || "").slice(0, n) + (n === 4 ? "-01-01" : n === 7 ? "-01" : ""));
    },
    year() { return Number(this.format("YYYY")); },
    month() { return Number(this.format("M")) - 1; },
    date() { return Number(this.format("D")); },
    day() { return Number(this.format("d")); },
    toString() { return this.format(); },
  };
}
"#;

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
