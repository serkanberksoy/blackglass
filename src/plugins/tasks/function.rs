//! Tasks' JavaScript functions (TK-46): `filter by function`, `sort by
//! function` and `group by function`. Every function of a query is worked
//! out for every task in one run of the sandbox (`plugins::js`), with a
//! `task` object like the original's: `description`, `status` (`symbol`,
//! `name`, `type`), `isDone`, `priorityName`, `priorityNumber`, `urgency`,
//! `tags`, `file` (`path`, `filename`, `filenameWithoutExtension`,
//! `folder`, `root`), `heading`, the dates (`due`, `scheduled`, `start`,
//! `created`, `done`, `cancelled`, `happens`: each with `moment`,
//! `formatAsDate()`, `category`), `isRecurring`, `recurrenceRule`, `id`,
//! `dependsOn`, `lineNumber`, `originalMarkdown`; and `moment()`.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::NaiveDate;
use serde_json::{Value, json};

use super::task::{DateField, Priority, Statuses, Task, Type};

/// Each task's function values, in the query's order of functions.
#[derive(Debug, Default)]
pub struct Computed {
    values: HashMap<(PathBuf, usize), Vec<Value>>,
}

impl Computed {
    /// Function `i`'s value for task `t` (`null` when it has none).
    pub fn get(&self, t: &Task, i: usize) -> &Value {
        self.values
            .get(&(t.path.clone(), t.line))
            .and_then(|v| v.get(i))
            .unwrap_or(&Value::Null)
    }
}

/// Whether a function's value counts as true (JavaScript's truthiness).
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// A function's value as a group heading (or several, from a list).
pub fn headings(v: &Value) -> Vec<String> {
    match v {
        Value::Null => vec![String::new()],
        Value::String(s) => vec![s.clone()],
        Value::Array(items) => items.iter().flat_map(headings).collect(),
        other => vec![other.to_string()],
    }
}

/// Orders two function values: numbers by size, text by text, nothing
/// last.
pub fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Greater,
        (_, Value::Null) => Ordering::Less,
        (Value::Number(x), Value::Number(y)) => x
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&y.as_f64().unwrap_or(0.0)),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (x, y) => text(x).to_lowercase().cmp(&text(y).to_lowercase()),
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The task as the functions see it.
fn object(t: &Task, today: NaiveDate, statuses: &Statuses) -> Value {
    let status = statuses.get(t.status);
    let date = |d: Option<NaiveDate>| match d {
        Some(d) => {
            let category = match d.cmp(&today) {
                std::cmp::Ordering::Less => ("Overdue", 1),
                std::cmp::Ordering::Equal => ("Today", 2),
                std::cmp::Ordering::Greater => ("Future", 3),
            };
            json!({ "iso": d.format("%Y-%m-%d").to_string(), "category": category.0, "order": category.1 })
        }
        None => json!({ "iso": null, "category": "Undated", "order": 4 }),
    };
    let path = t.path.to_string_lossy().replace('\\', "/");
    let filename = path.rsplit('/').next().unwrap_or(&path).to_string();
    let folder = match path.rsplit_once('/') {
        Some((f, _)) => format!("{f}/"),
        None => "/".into(),
    };
    let root = match path.split_once('/') {
        Some((r, _)) => format!("{r}/"),
        None => "/".into(),
    };
    let happens = [DateField::Start, DateField::Scheduled, DateField::Due]
        .into_iter()
        .filter_map(|f| t.date(f))
        .min();
    let priority_number = match t.priority {
        Priority::Highest => 0,
        Priority::High => 1,
        Priority::Medium => 2,
        Priority::None => 3,
        Priority::Low => 4,
        Priority::Lowest => 5,
    };
    let mut dates = serde_json::Map::new();
    for (field, _, name, _) in DateField::ALL {
        dates.insert(name.into(), date(t.date(field)));
    }
    dates.insert("happens".into(), date(happens));
    json!({
        "description": t.description,
        "status": { "symbol": t.status.to_string(), "name": status.name, "type": status.kind.name() },
        "isDone": matches!(status.kind, Type::Done | Type::Cancelled),
        "priorityName": t.priority.name(),
        "priorityNumber": priority_number,
        "urgency": (t.urgency(today) * 100.0).round() / 100.0,
        "tags": t.tags,
        "file": {
            "path": path,
            "filename": filename,
            "filenameWithoutExtension": filename.strip_suffix(".md").unwrap_or(&filename),
            "folder": folder,
            "root": root,
        },
        "heading": t.heading,
        "dates": dates,
        "isRecurring": t.recurrence.is_some(),
        "recurrenceRule": t.recurrence.clone().unwrap_or_default(),
        "id": t.id.clone().unwrap_or_default(),
        "dependsOn": t.depends,
        "lineNumber": t.line,
        "originalMarkdown": t.raw,
    })
}

/// Works out `functions` (JavaScript expressions over `task`) for every
/// one of `tasks`; `Err` names the first that fails.
pub fn compute(
    functions: &[String],
    tasks: &[Task],
    today: NaiveDate,
    statuses: &Statuses,
) -> Result<Computed, String> {
    if functions.is_empty() {
        return Ok(Computed::default());
    }
    let data: Vec<Value> = tasks.iter().map(|t| object(t, today, statuses)).collect();
    let fns: Vec<String> = functions
        .iter()
        .map(|code| format!("(task) => ({code})"))
        .collect();
    let script = format!(
        r#"{moment}
const __fns = [{fns}];
function __date(d) {{
  const m = d.iso ? moment(d.iso) : null;
  return {{
    moment: m,
    category: {{ name: d.category, sortOrder: d.order }},
    formatAsDate(fallback = "") {{ return d.iso || fallback; }},
    formatAsDateAndTime(fallback = "") {{ return d.iso ? d.iso + " 00:00" : fallback; }},
    toISOString() {{ return d.iso; }},
    toString() {{ return d.iso || ""; }},
  }};
}}
function __task(t) {{
  const task = Object.assign({{}}, t);
  for (const [name, d] of Object.entries(t.dates)) task[name] = __date(d);
  task.descriptionWithoutTags = t.tags.reduce((s, g) => s.split(g).join(""), t.description).trim();
  return task;
}}
function __plain(v) {{
  if (v === undefined || v === null) return null;
  if (v.__iso !== undefined) return v.__iso;
  if (typeof v === "object" && typeof v.toISOString === "function" && v.moment !== undefined) return v.toISOString();
  return v;
}}
const __tasks = JSON.parse({data});
globalThis.__result = JSON.stringify(__tasks.map(t => {{
  const task = __task(t);
  return __fns.map((f, i) => {{
    try {{ return __plain(f(task)); }}
    catch (e) {{ throw new Error("function " + (i + 1) + ": " + String(e)); }}
  }});
}}));"#,
        moment = crate::plugins::js::MOMENT,
        fns = fns.join(",\n"),
        data = crate::plugins::js::literal(&Value::Array(data).to_string()),
    );
    let out = crate::plugins::js::run(&script, &[]).map_err(|e| {
        // Without the engine's place in the generated script.
        let e = e.split(" in Cover").next().unwrap_or(&e);
        let e = e.split(" at line ").next().unwrap_or(e).trim();
        if e.contains("function") {
            e.to_string()
        } else {
            format!("function: {e}")
        }
    })?;
    let rows: Vec<Vec<Value>> = serde_json::from_str(&out).map_err(|e| e.to_string())?;
    let values = tasks
        .iter()
        .zip(rows)
        .map(|(t, v)| ((t.path.clone(), t.line), v))
        .collect();
    Ok(Computed { values })
}
