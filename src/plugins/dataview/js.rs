//! DataviewJS: ```` ```dataviewjs ```` blocks run JavaScript with Dataview's
//! `dv` object, in the plugin sandbox ([`crate::plugins::js`]):
//!
//! - data: `dv.pages(source)` (the same sources as `FROM`: `"#tag"`,
//!   `'"Folder"'`, `"[[Note]]"`, with `and`, `or`, `-`), `dv.current()`,
//!   `dv.page(name)`, `dv.date(text)`, `dv.func.dateformat(date, format)`;
//! - output: `dv.header(level, text)`, `dv.paragraph(text, style)`,
//!   `dv.span`, `dv.el(tag, text)`, `dv.list(items)`,
//!   `dv.table(headers, rows)`, `dv.taskList(tasks)`, `console.log`.
//!
//! Page lists have Dataview's array helpers: `where`, `filter`, `map`,
//! `flatMap`, `sort(key, "asc" | "desc")`, `groupBy`, `limit`, `slice`,
//! `first`, `last`, `distinct`, `sum`, `avg`, `min`, `max`, `array`.
//! Dates are text (`2026-08-09`), so they sort and compare as expected.
//! `dv.paragraph(text, {color: "#8ab4f8", bold: true})` colors its text (a
//! blackglass extension, for charts).

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{Context, JsNativeError, JsResult, JsValue};
use chrono::NaiveTime;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::{Map, Value as Json, json};

use super::eval;
use super::index::{Index, Page};
use super::query::{self, Source};
use super::render::{self, DIM, Display, LINK};
use super::value::Value;
use crate::plugins::js;

thread_local! {
    /// The index `dv.pages` filters, while a block runs.
    static INDEX: RefCell<Option<Rc<Index>>> = const { RefCell::new(None) };
}

/// The `dv` API, in JavaScript. `__pages` (the vault's pages) and `__this`
/// (the current page's index, or -1) are set before it.
const PRELUDE: &str = r#"
const __out = [];
function __str(v) {
  if (v === null || v === undefined) return "-";
  if (Array.isArray(v)) return v.map(__str).join(", ");
  if (typeof v === "object") return v.file ? v.file.name : JSON.stringify(v);
  return String(v);
}
// A list item: its text, and its page's path if it is one (to open it).
function __item(v) {
  return { text: __str(v), path: v && typeof v === "object" && v.file ? v.file.path : null };
}
function __key(f) { return typeof f === "function" ? f : (x => x); }
function __cmp(a, b) { return a === b ? 0 : (a === null || a === undefined) ? -1 : (b === null || b === undefined) ? 1 : a < b ? -1 : 1; }
function __da(items) {
  const a = Array.from(items);
  const methods = {
    where(f) { return __da(Array.prototype.filter.call(this, f)); },
    filter(f) { return __da(Array.prototype.filter.call(this, f)); },
    map(f) { return __da(Array.prototype.map.call(this, f)); },
    flatMap(f) { return __da(Array.prototype.flatMap.call(this, f)); },
    sort(key, dir) {
      const k = __key(key), sign = dir === "desc" ? -1 : 1;
      return __da(Array.from(this).sort((x, y) => sign * __cmp(k(x), k(y))));
    },
    groupBy(key) {
      const k = __key(key), groups = new Map();
      for (const x of this) { const g = k(x); if (!groups.has(g)) groups.set(g, []); groups.get(g).push(x); }
      return __da(Array.from(groups, ([key, rows]) => ({ key, rows: __da(rows) })));
    },
    limit(n) { return __da(Array.prototype.slice.call(this, 0, n)); },
    slice(a, b) { return __da(Array.prototype.slice.call(this, a, b)); },
    first() { return this[0]; },
    last() { return this[this.length - 1]; },
    distinct(key) {
      const k = __key(key), seen = new Set();
      return __da(Array.prototype.filter.call(this, x => { const v = k(x); if (seen.has(v)) return false; seen.add(v); return true; }));
    },
    sum() { return Array.prototype.reduce.call(this, (s, x) => s + Number(x || 0), 0); },
    avg() { return this.length ? this.sum() / this.length : null; },
    min() { return this.length ? Math.min(...this) : null; },
    max() { return this.length ? Math.max(...this) : null; },
    array() { return Array.from(this); },
  };
  for (const [name, f] of Object.entries(methods)) Object.defineProperty(a, name, { value: f });
  return a;
}
const dv = {
  pages(source) { return __da(JSON.parse(__source(source || "")).map(i => __pages[i])); },
  current() { return __this >= 0 ? __pages[__this] : null; },
  page(name) {
    const n = String(name).replace(/\.md$/, "").toLowerCase();
    return __pages.find(p => p.file.name.toLowerCase() === n || p.file.path.replace(/\.md$/, "").toLowerCase() === n) || null;
  },
  fileLink(path) {
    const p = dv.page(path);
    if (p) return p;
    const name = String(path).replace(/\.md$/, "");
    return { file: { name: name.split("/").pop(), path: name + ".md" } };
  },
  date(text) {
    const t = String(text).toLowerCase();
    const now = __now();
    if (t === "now") return now.slice(0, 16).replace("T", " ");
    if (t === "today") return now.slice(0, 10);
    if (t === "tomorrow") return __shift(now, "P1D").slice(0, 10);
    if (t === "yesterday") return __shift(now, "-P1D").slice(0, 10);
    return String(text);
  },
  func: {
    dateformat(date, format) { return __fmt(String(date), format); },
    lower(t) { return String(t).toLowerCase(); },
    upper(t) { return String(t).toUpperCase(); },
  },
  header(level, text) { __out.push({ t: "h", level, text: __str(text) }); },
  paragraph(text, style) { __out.push({ t: "p", text: __str(text), color: style && style.color, bold: !!(style && style.bold) }); },
  span(text, style) { dv.paragraph(text, style); },
  el(tag, text, style) {
    const m = /^h([1-6])$/.exec(tag);
    if (m) dv.header(Number(m[1]), text); else dv.paragraph(text, style);
  },
  list(items) { __out.push({ t: "list", items: Array.from(items || [], __item) }); },
  table(headers, rows) {
    __out.push({
      t: "table",
      headers: Array.from(headers, String),
      rows: Array.from(rows || [], r => {
        const page = Array.from(r).find(c => c && typeof c === "object" && c.file);
        return { cells: Array.from(r, __str), path: page ? page.file.path : null };
      }),
    });
  },
  taskList(tasks) {
    const groups = [];
    for (const task of tasks || []) {
      const page = task.page || "";
      let g = groups.find(g => g.page === page);
      if (!g) groups.push(g = { page, path: task.path || null, tasks: [] });
      g.tasks.push({ text: task.text, status: task.status, line: task.line, path: task.path || null });
    }
    __out.push({ t: "tasks", groups });
  },
};
const console = { log(...args) { __out.push({ t: "p", text: args.map(__str).join(" "), dim: true }); } };
"#;

/// Renders a `dataviewjs` block: runs `code` with `dv`, then draws what
/// it wrote; rows for pages and tasks have actions (to open them).
pub fn render(
    code: &str,
    index: &Rc<Index>,
    this: Option<&Page>,
    width: usize,
    d: &Display,
) -> Vec<(Line<'static>, Option<String>)> {
    let pages: Vec<Json> = index.pages.iter().map(|p| page_json(p, index)).collect();
    let this_index = this
        .and_then(|t| index.pages.iter().position(|p| p.path == t.path))
        .map_or(-1, |i| i as i64);
    let script = format!(
        "const __pages = JSON.parse({pages});\nconst __this = {this_index};\n{PRELUDE}\n\
         (async () => {{\n{code}\n}})().then(() => {{ globalThis.__result = JSON.stringify(__out); }}, e => {{ globalThis.__error = String(e); }});",
        pages = js::literal(&Json::Array(pages).to_string()),
    );
    INDEX.with(|i| *i.borrow_mut() = Some(Rc::clone(index)));
    let result = js::run(&script, &[("__source", 1, source)]);
    INDEX.with(|i| *i.borrow_mut() = None);
    match result.and_then(|out| serde_json::from_str::<Vec<Json>>(&out).map_err(|e| e.to_string()))
    {
        Ok(items) if items.is_empty() => {
            vec![(Line::from(Span::styled(d.no_results.clone(), DIM)), None)]
        }
        Ok(items) => items
            .iter()
            .flat_map(|item| output(item, width, d))
            .collect(),
        Err(e) => render::error(&e).into_iter().map(|l| (l, None)).collect(),
    }
}

/// `dv.pages(source)`: the indexes of the pages in a `FROM` source.
fn source(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let text = js::arg(args, 0, ctx)?.unwrap_or_default();
    let from: Option<Source> = if text.trim().is_empty() {
        None
    } else {
        let q = query::parse(&format!("LIST FROM {text}"))
            .map_err(|e| JsNativeError::typ().with_message(format!("dv.pages: {e}")))?;
        q.from
    };
    let found = INDEX.with(|i| {
        let i = i.borrow();
        let index = i.as_ref().expect("the index is set while a block runs");
        let mut pages: Vec<(usize, &Page)> = index
            .pages
            .iter()
            .enumerate()
            .filter(|(_, p)| from.as_ref().is_none_or(|s| eval::matches(s, p)))
            .collect();
        pages.sort_by_key(|(_, p)| p.rel.to_lowercase());
        pages.into_iter().map(|(i, _)| i).collect::<Vec<_>>()
    });
    Ok(js::text(
        &serde_json::to_string(&found).expect("numbers serialize"),
    ))
}

/// A page as the script sees it: `file.*` and the page's fields.
fn page_json(page: &Page, index: &Index) -> Json {
    let date =
        |d: Option<chrono::NaiveDateTime>| d.map_or(Json::Null, |d| value_json(&Value::Date(d)));
    let tasks: Vec<Json> = page
        .tasks
        .iter()
        .map(|t| {
            json!({
                "text": t.text, "status": t.status.to_string(), "completed": t.completed(),
                "line": t.line, "path": page.rel, "page": page.name,
            })
        })
        .collect();
    let mut obj = Map::new();
    obj.insert(
        "file".into(),
        json!({
            "name": page.name, "path": page.rel, "folder": page.folder,
            "link": page.name, "size": page.size, "ctime": date(page.ctime),
            "mtime": date(page.mtime), "day": date(page.day), "tags": page.tags,
            "outlinks": page.outlinks,
            "inlinks": index.inlinks(page).map(|p| p.name.clone()).collect::<Vec<_>>(),
            "tasks": tasks,
        }),
    );
    for (key, value) in &page.fields {
        let v = value_json(value);
        obj.entry(key.clone()).or_insert_with(|| v.clone());
        // Also by Dataview's simplified key: `Due Date` → `due-date`.
        obj.entry(key.trim().to_lowercase().replace(' ', "-"))
            .or_insert(v);
    }
    Json::Object(obj)
}

fn value_json(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Bool(b) => json!(b),
        Value::Number(n) => json!(n),
        Value::Text(t) | Value::Link(t) => json!(t),
        Value::Date(d) if d.time() == NaiveTime::MIN => json!(d.format("%Y-%m-%d").to_string()),
        Value::Date(d) => json!(d.format("%Y-%m-%dT%H:%M").to_string()),
        Value::List(items) => Json::Array(items.iter().map(value_json).collect()),
    }
}

/// The lines for one thing the script wrote, each with its action: a
/// page's row opens it, a task's row opens its note at the task.
fn output(item: &Json, width: usize, d: &Display) -> Vec<(Line<'static>, Option<String>)> {
    let text = |v: &Json, key: &str| {
        v.get(key)
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let open = |v: &Json| {
        v.get("path")
            .and_then(Json::as_str)
            .map(render::open_action)
    };
    let strings = |v: Option<&Json>| -> Vec<String> {
        v.and_then(Json::as_array)
            .map(|a| {
                a.iter()
                    .map(|x| x.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    let list = |v: Option<&Json>| v.and_then(Json::as_array).cloned().unwrap_or_default();
    match item.get("t").and_then(Json::as_str).unwrap_or_default() {
        "h" => {
            let level = item.get("level").and_then(Json::as_u64).unwrap_or(1);
            let style = Style::new().add_modifier(Modifier::BOLD);
            let style = if level <= 2 { style.patch(LINK) } else { style };
            vec![(Line::from(Span::styled(text(item, "text"), style)), None)]
        }
        "p" => {
            let mut style = Style::new();
            if let Some(color) = item
                .get("color")
                .and_then(Json::as_str)
                .and_then(mdedit::config::parse_color)
            {
                style = style.fg(color);
            }
            if item.get("bold").and_then(Json::as_bool) == Some(true) {
                style = style.add_modifier(Modifier::BOLD);
            }
            if item.get("dim").and_then(Json::as_bool) == Some(true) {
                style = style.patch(DIM);
            }
            text(item, "text")
                .split('\n')
                .map(|l| (Line::from(Span::styled(l.to_string(), style)), None))
                .collect()
        }
        "list" => list(item.get("items"))
            .iter()
            .map(|i| {
                let shown = Span::styled(
                    text(i, "text"),
                    if open(i).is_some() {
                        LINK
                    } else {
                        Style::new()
                    },
                );
                (Line::from(vec![Span::raw("• "), shown]), open(i))
            })
            .collect(),
        "table" => {
            let headers = strings(item.get("headers"));
            let rows = list(item.get("rows"));
            if rows.is_empty() {
                return vec![(Line::from(Span::styled(d.no_results.clone(), DIM)), None)];
            }
            let cells: Vec<(String, Vec<Value>)> = rows
                .iter()
                .map(|r| {
                    (
                        String::new(),
                        strings(r.get("cells"))
                            .into_iter()
                            .map(Value::Text)
                            .collect(),
                    )
                })
                .collect();
            let lines = render::table(None, &headers, &cells, width, d);
            let actions = [None, None].into_iter().chain(rows.iter().map(open));
            lines.into_iter().zip(actions).collect()
        }
        "tasks" => {
            let mut lines = Vec::new();
            for group in list(item.get("groups")) {
                let page = text(&group, "page");
                if !page.is_empty() {
                    lines.push((
                        Line::from(Span::styled(page, LINK.add_modifier(Modifier::BOLD))),
                        open(&group),
                    ));
                }
                for task in list(group.get("tasks")) {
                    let (mark, style) =
                        match task.get("status").and_then(Json::as_str).unwrap_or(" ") {
                            "x" | "X" => ("☑".to_string(), DIM.add_modifier(Modifier::CROSSED_OUT)),
                            " " => ("☐".to_string(), Style::new()),
                            c => (format!("[{c}]"), Style::new()),
                        };
                    let action = task.get("path").and_then(Json::as_str).map(|p| {
                        let line = task.get("line").and_then(Json::as_u64).unwrap_or(0);
                        render::line_action(p, line as usize)
                    });
                    lines.push((
                        Line::from(vec![
                            Span::raw(format!("  {mark} ")),
                            Span::styled(text(&task, "text"), style),
                        ]),
                        action,
                    ));
                }
            }
            lines
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use crate::vault::tests::{scratch, write};

    /// One vault for every test here (tests run in parallel); the index is
    /// built from it per test.
    fn index() -> Rc<Index> {
        static VAULT: std::sync::OnceLock<Vault> = std::sync::OnceLock::new();
        let vault = VAULT.get_or_init(|| {
            let dir = scratch("dataviewjs");
            write(
                &dir,
                &[
                    (
                        "Books/Dune.md",
                        "---\nauthor: Frank Herbert\nrating: 5\ntags: [reading]\n---\n- [ ] finish",
                    ),
                    (
                        "Books/Emma.md",
                        "---\nauthor: Jane Austen\nrating: 3\n---\n#reading",
                    ),
                    ("Journal/2026-08-09.md", "waistline:: 138\nRead [[Dune]]"),
                    ("Journal/2026-08-10.md", "waistline:: 137"),
                ],
            );
            Vault::open(&dir).unwrap()
        });
        Rc::new(Index::build(vault))
    }

    fn text(code: &str) -> Vec<String> {
        let ix = index();
        let this = ix.pages.iter().find(|p| p.name == "Dune");
        render(code, &ix, this, 60, &Display::default())
            .iter()
            .map(|(l, _)| l)
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn pages_sources_and_helpers() {
        assert_eq!(
            text(
                "dv.list(dv.pages('#reading').sort(p => p.rating, 'desc').map(p => p.file.name + ' ' + p.rating))"
            ),
            ["• Dune 5", "• Emma 3"]
        );
        assert_eq!(
            text("dv.paragraph(dv.pages('\"Journal\"').map(p => p.waistline).avg())"),
            ["137.5"]
        );
        assert_eq!(
            text("dv.paragraph(dv.current().file.inlinks.join())"),
            ["2026-08-09"]
        );
        assert_eq!(
            text("dv.paragraph(dv.page('Emma').author)"),
            ["Jane Austen"]
        );
        assert_eq!(
            text(
                "dv.list(dv.pages().groupBy(p => p.file.folder).map(g => g.key + ': ' + g.rows.length))"
            ),
            ["• Books: 2", "• Journal: 2"]
        );
    }

    #[test]
    fn output_kinds() {
        assert_eq!(
            text(
                "dv.header(2, 'Books'); dv.table(['Book', 'Stars'], dv.pages('\"Books\"').map(p => [p.file.link, '★'.repeat(p.rating)]))"
            ),
            [
                "Books",
                "Book │ Stars",
                "─────┼──────",
                "Dune │ ★★★★★",
                "Emma │ ★★★"
            ]
        );
        assert_eq!(
            text("dv.taskList(dv.pages().flatMap(p => p.file.tasks).where(t => !t.completed))"),
            ["Dune", "  ☐ finish"]
        );
        assert_eq!(
            text("console.log('n =', 2); dv.paragraph('a\\nb')"),
            ["n = 2", "a", "b"]
        );
        assert_eq!(text("const x = 1;"), ["No results"], "nothing written");
    }

    #[test]
    fn errors_are_shown_in_the_block() {
        let e = text("dv.nope()");
        assert!(
            e[0].starts_with("Dataview: TypeError: not a callable"),
            "{e:?}"
        );
        let e = text("dv.pages('3')");
        assert!(e[0].contains("FROM takes"), "{e:?}");
    }
}
