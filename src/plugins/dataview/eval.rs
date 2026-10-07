//! Runs a parsed query over the index: picks the pages `FROM` names (or
//! their tasks, for `TASK`), runs the data commands in the order written
//! (`WHERE`, `SORT`, `LIMIT`, `GROUP BY`, `FLATTEN`), and computes what
//! each result row shows.

use std::cmp::Ordering;
use std::rc::Rc;

use chrono::{Duration, Local, NaiveDate, NaiveTime};

use super::index::{Index, Page, Task};
use super::query::{Command, Expr, Kind, Op, Query, Source, is_date_word};
use super::value::{Dur, Value, parse_date};

/// A page in results: its name (shown) and its path in the vault (to open
/// it from the result); a group's key has no path (`""`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRef {
    pub name: String,
    pub rel: String,
}

impl PageRef {
    fn of(page: &Page) -> Self {
        PageRef {
            name: page.name.clone(),
            rel: page.rel.clone(),
        }
    }

    /// A group's key, as a row's id.
    fn key(key: &Value) -> Self {
        PageRef {
            name: key.display(),
            rel: String::new(),
        }
    }
}

impl std::fmt::Display for PageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// What a query found.
#[derive(Debug, Clone, PartialEq)]
pub enum Results {
    /// Each row: its page (or group), with its value if the list has one;
    /// `WITHOUT ID` shows only the values.
    List {
        without_id: bool,
        rows: Vec<(PageRef, Option<Value>)>,
    },
    Table {
        /// The id column's header (`None`: `WITHOUT ID`; `"File"`: the
        /// pages', named by the settings), then the columns'.
        id: Option<String>,
        headers: Vec<String>,
        /// Each row: the page (or group), then the cells.
        rows: Vec<(PageRef, Vec<Value>)>,
    },
    /// Tasks under a heading (their page, or their group's key), each with
    /// its page's path.
    Tasks(Vec<(PageRef, Vec<(String, Task)>)>),
    /// Pages on their days, in date order (`CALENDAR`).
    Calendar(Vec<(NaiveDate, PageRef)>),
}

/// A row the commands work on: a page (and a task in it, for `TASK`),
/// with the names `FLATTEN` bound, or a group of rows.
#[derive(Debug, Clone)]
pub struct Row<'a> {
    page: Option<&'a Page>,
    task: Option<&'a Task>,
    vars: Vec<(String, Value)>,
    group: Option<Rc<Group<'a>>>,
}

/// `GROUP BY`'s rows with one key.
#[derive(Debug)]
pub struct Group<'a> {
    key: Value,
    name: Option<String>,
    rows: Vec<Row<'a>>,
}

impl<'a> Row<'a> {
    fn page(page: &'a Page) -> Self {
        Row {
            page: Some(page),
            task: None,
            vars: Vec::new(),
            group: None,
        }
    }
}

/// Where fields are looked up: a page (none for a group), maybe a task in
/// it, the names bound by `FLATTEN` and lambdas, the group, and the page
/// the query is in (`this`).
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    pub index: &'a Index,
    pub page: Option<&'a Page>,
    pub task: Option<&'a Task>,
    pub this: Option<&'a Page>,
    pub vars: &'a [(String, Value)],
    pub group: Option<&'a Group<'a>>,
}

impl<'a> Scope<'a> {
    /// A page's own scope.
    pub fn of_page(index: &'a Index, page: &'a Page, this: Option<&'a Page>) -> Self {
        Scope {
            index,
            page: Some(page),
            task: None,
            this,
            vars: &[],
            group: None,
        }
    }

    fn of_row(index: &'a Index, row: &'a Row<'a>, this: Option<&'a Page>) -> Self {
        Scope {
            index,
            page: row.page,
            task: row.task,
            this,
            vars: &row.vars,
            group: row.group.as_deref(),
        }
    }
}

/// Runs `query`, written in the note `this` (if it's in the vault).
pub fn run(query: &Query, index: &Index, this: Option<&Page>) -> Result<Results, String> {
    let mut pages: Vec<&Page> = index
        .pages
        .iter()
        .filter(|p| {
            query
                .from
                .as_ref()
                .is_none_or(|s| matches(s, p, index, this))
        })
        .collect();
    pages.sort_by(|a, b| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()));
    let task_query = query.kind == Kind::Task;
    let mut rows: Vec<Row> = if task_query {
        pages
            .iter()
            .flat_map(|&p| {
                p.tasks.iter().map(move |t| Row {
                    task: Some(t),
                    ..Row::page(p)
                })
            })
            .collect()
    } else {
        pages.iter().map(|&p| Row::page(p)).collect()
    };
    let mut grouped = None;
    for command in &query.commands {
        rows = match command {
            Command::Where(e) => {
                let mut kept = Vec::with_capacity(rows.len());
                for row in rows {
                    if eval(e, Scope::of_row(index, &row, this))?.truthy() {
                        kept.push(row);
                    }
                }
                kept
            }
            Command::Sort(keys) => {
                let mut keyed: Vec<(Vec<Value>, Row)> = rows
                    .into_iter()
                    .map(|row| {
                        let values = keys
                            .iter()
                            .map(|(k, _)| {
                                eval(k, Scope::of_row(index, &row, this)).unwrap_or(Value::Null)
                            })
                            .collect();
                        (values, row)
                    })
                    .collect();
                keyed.sort_by(|(a, _), (b, _)| {
                    for ((x, y), (_, descending)) in a.iter().zip(b).zip(keys) {
                        let order = x.sort_order(y);
                        let order = if *descending { order.reverse() } else { order };
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                    Ordering::Equal
                });
                keyed.into_iter().map(|(_, row)| row).collect()
            }
            Command::Limit(n) => {
                rows.truncate(*n);
                rows
            }
            Command::GroupBy(e, name) => {
                let mut groups: Vec<(Value, Vec<Row>)> = Vec::new();
                for row in rows {
                    let key = eval(e, Scope::of_row(index, &row, this))?;
                    match groups
                        .iter_mut()
                        .find(|(k, _)| k.equals(&key) && k.kind_eq(&key))
                    {
                        Some((_, list)) => list.push(row),
                        None => groups.push((key, vec![row])),
                    }
                }
                groups.sort_by(|(a, _), (b, _)| a.sort_order(b));
                grouped = Some(name.clone().unwrap_or_else(|| "Group".into()));
                groups
                    .into_iter()
                    .map(|(key, rows)| Row {
                        page: None,
                        task: None,
                        vars: Vec::new(),
                        group: Some(Rc::new(Group {
                            key,
                            name: name.clone(),
                            rows,
                        })),
                    })
                    .collect()
            }
            Command::Flatten(e, name) => {
                let mut out = Vec::new();
                for row in rows {
                    let value = eval(e, Scope::of_row(index, &row, this))?;
                    let items = match value {
                        Value::List(items) => items,
                        other => vec![other],
                    };
                    for item in items {
                        let mut r = row.clone();
                        r.vars.push((name.clone(), item));
                        out.push(r);
                    }
                }
                out
            }
        };
    }
    let row_ref = |row: &Row| match (&row.group, row.page) {
        (Some(g), _) => PageRef::key(&g.key),
        (None, Some(p)) => PageRef::of(p),
        (None, None) => PageRef::key(&Value::Null),
    };
    match &query.kind {
        Kind::List { value, without_id } => {
            let mut out = Vec::new();
            for row in &rows {
                let v = match (value, &row.group) {
                    (Some(e), _) => Some(eval(e, Scope::of_row(index, row, this))?),
                    // A group without a value: its rows.
                    (None, Some(g)) => {
                        Some(Value::List(g.rows.iter().map(|r| row_link(r)).collect()))
                    }
                    (None, None) => None,
                };
                out.push((row_ref(row), v));
            }
            Ok(Results::List {
                without_id: *without_id,
                rows: out,
            })
        }
        Kind::Table {
            columns,
            without_id,
        } => {
            let mut out = Vec::new();
            for row in &rows {
                let scope = Scope::of_row(index, row, this);
                let cells = columns
                    .iter()
                    .map(|(e, _)| eval(e, scope))
                    .collect::<Result<_, _>>()?;
                out.push((row_ref(row), cells));
            }
            Ok(Results::Table {
                id: (!without_id).then(|| grouped.clone().unwrap_or_else(|| "File".into())),
                headers: columns.iter().map(|(_, h)| h.clone()).collect(),
                rows: out,
            })
        }
        Kind::Task => {
            let mut groups: Vec<(PageRef, Vec<(String, Task)>)> = Vec::new();
            let mut add = |heading: PageRef, row: &Row| {
                let (Some(task), Some(page)) = (row.task, row.page) else {
                    return;
                };
                let entry = (page.rel.clone(), task.clone());
                match groups.iter_mut().find(|(h, _)| *h == heading) {
                    Some((_, list)) => list.push(entry),
                    None => groups.push((heading, vec![entry])),
                }
            };
            for row in &rows {
                match &row.group {
                    // Grouped: under the key.
                    Some(g) => {
                        for r in &g.rows {
                            add(PageRef::key(&g.key), r);
                        }
                    }
                    // Else under their pages, in the order they come.
                    None => {
                        if let Some(p) = row.page {
                            add(PageRef::of(p), row);
                        }
                    }
                }
            }
            Ok(Results::Tasks(groups))
        }
        Kind::Calendar(e) => {
            let mut days = Vec::new();
            for row in &rows {
                if let Value::Date(d) = eval(e, Scope::of_row(index, row, this))? {
                    days.push((d.date(), row_ref(row)));
                }
            }
            days.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
            Ok(Results::Calendar(days))
        }
    }
}

/// A row as a link (a group's rows, listed).
/// A row as one value (each of a group's `rows`): what FLATTEN bound,
/// `file`, then the page's fields, as Dataview gives them; a group's
/// row (a group of groups) is its key.
fn row_object(row: &Row, index: &Index) -> Value {
    let Some(page) = row.page else {
        return row_link(row);
    };
    let mut fields: Vec<(String, Value)> = row.vars.iter().rev().cloned().collect();
    fields.push(("file".into(), file_object(page, index)));
    fields.extend(page.fields.iter().cloned());
    Value::Object(fields)
}

fn row_link(row: &Row) -> Value {
    match row.page {
        Some(p) => Value::Link(p.rel.trim_end_matches(".md").to_string()),
        None => row.group.as_ref().map_or(Value::Null, |g| g.key.clone()),
    }
}

/// The page called `name` (a link's target: a name or a path), if any.
pub fn page_named<'a>(index: &'a Index, name: &str) -> Option<&'a Page> {
    let name = name.split('#').next().unwrap_or_default();
    let name = name.trim().trim_end_matches(".md").to_lowercase();
    index
        .pages
        .iter()
        .filter(|p| {
            p.name.to_lowercase() == name || p.rel.trim_end_matches(".md").to_lowercase() == name
        })
        .min_by_key(|p| p.rel.len())
}

/// Whether `page` is in `source` (`this`: the note the query is in).
pub fn matches(source: &Source, page: &Page, index: &Index, this: Option<&Page>) -> bool {
    match source {
        Source::Tag(tag) => {
            let tag = format!("#{}", tag.to_lowercase());
            page.tags.iter().any(|t| {
                let t = t.to_lowercase();
                t == tag || t.strip_prefix(&tag).is_some_and(|r| r.starts_with('/'))
            })
        }
        Source::Folder(folder) => {
            folder.is_empty()
                || page.folder == *folder
                || page.folder.starts_with(&format!("{folder}/"))
                || page.rel == *folder
                || page.rel == format!("{folder}.md")
        }
        Source::LinksTo(name) => {
            let name = if name.is_empty() {
                match this {
                    Some(t) => t.name.clone(),
                    None => return false,
                }
            } else {
                name.clone()
            };
            let name = name.rsplit('/').next().unwrap_or(&name).to_lowercase();
            page.links_to(&name)
        }
        Source::Outgoing(name) => {
            let from = if name.is_empty() {
                this
            } else {
                page_named(index, name)
            };
            from.is_some_and(|f| f.links_to(&page.name.to_lowercase()))
        }
        Source::And(a, b) => matches(a, page, index, this) && matches(b, page, index, this),
        Source::Or(a, b) => matches(a, page, index, this) || matches(b, page, index, this),
        Source::Not(a) => !matches(a, page, index, this),
    }
}

/// The value of `expr` in `scope`.
pub fn eval(expr: &Expr, scope: Scope) -> Result<Value, String> {
    Ok(match expr {
        Expr::Value(v) => v.clone(),
        Expr::Field(path) => field(path, scope),
        Expr::Not(e) => Value::Bool(!eval(e, scope)?.truthy()),
        Expr::Neg(e) => match eval(e, scope)? {
            Value::Number(n) => Value::Number(-n),
            Value::Duration(d) => Value::Duration(d.times(-1.0)),
            Value::Null => Value::Null,
            other => return Err(format!("can't negate {}", other.display())),
        },
        Expr::Binary(Op::And, a, b) => {
            Value::Bool(eval(a, scope)?.truthy() && eval(b, scope)?.truthy())
        }
        Expr::Binary(Op::Or, a, b) => {
            Value::Bool(eval(a, scope)?.truthy() || eval(b, scope)?.truthy())
        }
        Expr::Binary(op, a, b) => binary(*op, eval(a, scope)?, eval(b, scope)?)?,
        Expr::Call(name, args) => call(name, args, scope)?,
        Expr::List(items) => Value::List(
            items
                .iter()
                .map(|e| eval(e, scope))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|(k, e)| Ok((k.clone(), eval(e, scope)?)))
                .collect::<Result<_, String>>()?,
        ),
        Expr::Index(base, key) => index_value(eval(base, scope)?, &eval(key, scope)?, scope),
        Expr::Lambda(..) => {
            return Err("a lambda ((x) => …) goes in map, filter, reduce, sort …".into());
        }
    })
}

/// `base[key]`: an object's key, a list's item (or each item's key), a
/// linked page's field, a date's or a duration's part.
pub fn index_value(base: Value, key: &Value, scope: Scope) -> Value {
    match (base, key) {
        (Value::Object(o), k) => {
            let k = k.display();
            o.into_iter()
                .find(|(name, _)| *name == k)
                .map_or(Value::Null, |(_, v)| v)
        }
        (Value::List(items), Value::Number(n)) => {
            let i = if *n < 0.0 { items.len() as f64 + n } else { *n };
            items.get(i as usize).cloned().unwrap_or(Value::Null)
        }
        // `list.key`: each item's, lists flattened (Dataview's swizzling).
        (Value::List(items), k) => {
            let mut out = Vec::new();
            for item in items {
                match index_value(item, k, scope) {
                    Value::List(inner) => out.extend(inner),
                    v => out.push(v),
                }
            }
            Value::List(out)
        }
        (Value::Link(name), Value::Text(k)) => match page_named(scope.index, &name) {
            Some(page) => field(
                &k.split('.').map(str::to_string).collect::<Vec<_>>(),
                Scope {
                    page: Some(page),
                    task: None,
                    vars: &[],
                    group: None,
                    ..scope
                },
            ),
            None => Value::Null,
        },
        (Value::Date(d), Value::Text(k)) => {
            use chrono::{Datelike, Timelike};
            let n = match k.as_str() {
                "year" => d.year() as f64,
                "month" => d.month() as f64,
                "day" => d.day() as f64,
                "hour" => d.hour() as f64,
                "minute" => d.minute() as f64,
                "second" => d.second() as f64,
                "weekday" => d.weekday().number_from_monday() as f64,
                "week" | "weekNumber" => d.iso_week().week() as f64,
                _ => return Value::Null,
            };
            Value::Number(n)
        }
        (Value::Duration(d), Value::Text(k)) => {
            let ms = d.approx_millis() as f64;
            let n = match k.as_str() {
                "years" => ms / (365.0 * 86_400_000.0),
                "months" => ms / (30.0 * 86_400_000.0),
                "weeks" => ms / (7.0 * 86_400_000.0),
                "days" => ms / 86_400_000.0,
                "hours" => ms / 3_600_000.0,
                "minutes" => ms / 60_000.0,
                "seconds" => ms / 1000.0,
                "milliseconds" => ms,
                _ => return Value::Null,
            };
            Value::Number(n)
        }
        (Value::Text(t), Value::Number(n)) => t
            .chars()
            .nth(*n as usize)
            .map_or(Value::Null, |c| Value::Text(c.to_string())),
        _ => Value::Null,
    }
}

fn binary(op: Op, a: Value, b: Value) -> Result<Value, String> {
    use Value::{Date, Duration as Dv, List, Null, Number, Text};
    let cmp = a.compare(&b);
    Ok(match op {
        Op::Eq => Value::Bool(a.equals(&b)),
        Op::Ne => Value::Bool(!a.equals(&b)),
        Op::Lt => Value::Bool(cmp == Some(Ordering::Less)),
        Op::Gt => Value::Bool(cmp == Some(Ordering::Greater)),
        Op::Le => Value::Bool(matches!(cmp, Some(Ordering::Less | Ordering::Equal))),
        Op::Ge => Value::Bool(matches!(cmp, Some(Ordering::Greater | Ordering::Equal))),
        Op::And | Op::Or => unreachable!("evaluated lazily in eval"),
        _ => match (op, a, b) {
            (_, Null, _) | (_, _, Null) => Null,
            (Op::Add, Number(x), Number(y)) => Number(x + y),
            (Op::Sub, Number(x), Number(y)) => Number(x - y),
            (Op::Mul, Number(x), Number(y)) => Number(x * y),
            (Op::Div | Op::Mod, Number(_), Number(0.0)) => Null,
            (Op::Div, Number(x), Number(y)) => Number(x / y),
            (Op::Mod, Number(x), Number(y)) => Number(x % y),
            (Op::Add, Text(x), y) => Text(format!("{x}{}", y.display())),
            (Op::Add, x, Text(y)) => Text(format!("{}{y}", x.display())),
            (Op::Add, List(mut x), List(y)) => {
                x.extend(y);
                List(x)
            }
            // Dates and durations.
            (Op::Sub, Date(x), Date(y)) => Dv(Dur {
                months: 0,
                millis: (x - y).num_milliseconds(),
            }),
            (Op::Add, Date(d), Dv(u)) | (Op::Add, Dv(u), Date(d)) => {
                u.shift(d, false).map_or(Null, Date)
            }
            (Op::Sub, Date(d), Dv(u)) => u.shift(d, true).map_or(Null, Date),
            (Op::Add, Dv(x), Dv(y)) => Dv(x.plus(y)),
            (Op::Sub, Dv(x), Dv(y)) => Dv(x.plus(y.times(-1.0))),
            (Op::Mul, Dv(x), Number(n)) | (Op::Mul, Number(n), Dv(x)) => Dv(x.times(n)),
            (Op::Div, Dv(x), Number(n)) if n != 0.0 => Dv(x.times(1.0 / n)),
            // A date plus or minus days (as before durations).
            (Op::Add, Date(d), Number(n)) => Date(d + Duration::days(n as i64)),
            (Op::Sub, Date(d), Number(n)) => Date(d - Duration::days(n as i64)),
            (op, x, y) => {
                return Err(format!(
                    "can't use {op:?} on {} and {}",
                    x.display(),
                    y.display()
                ));
            }
        },
    })
}

/// A field in scope: names bound by `FLATTEN` and lambdas, a group's `key`
/// and `rows`, `this.…`, task fields, `file.*`, then page fields.
fn field(path: &[String], scope: Scope) -> Value {
    let first = path[0].as_str();
    // A bound name (the longest that starts the path), latest first.
    for n in (1..=path.len()).rev() {
        let name = path[..n].join(".");
        if let Some((_, v)) = scope.vars.iter().rev().find(|(k, _)| *k == name) {
            return rest_of(v.clone(), &path[n..], scope);
        }
    }
    // The note the query is in: in a group too (Dataview's `this`).
    if first == "this" {
        return match scope.this {
            Some(this) if path.len() > 1 => field(
                &path[1..],
                Scope {
                    page: Some(this),
                    task: None,
                    vars: &[],
                    group: None,
                    ..scope
                },
            ),
            Some(this) => Value::Link(this.rel.trim_end_matches(".md").to_string()),
            None => Value::Null,
        };
    }
    if let Some(group) = scope.group {
        if first == "key" || group.name.as_deref() == Some(first) {
            return rest_of(group.key.clone(), &path[1..], scope);
        }
        if first == "rows" {
            let rest = &path[1..];
            let mut out = Vec::new();
            for row in &group.rows {
                if rest.is_empty() {
                    out.push(row_object(row, scope.index));
                    continue;
                }
                match field(rest, Scope::of_row(scope.index, row, scope.this)) {
                    // Swizzled lists are flattened, as Dataview does.
                    Value::List(inner) => out.extend(inner),
                    v => out.push(v),
                }
            }
            return Value::List(out);
        }
        return Value::Null;
    }
    if let Some(task) = scope.task
        && let Some(v) = task_field(task, scope, path)
    {
        return v;
    }
    let Some(page) = scope.page else {
        return Value::Null;
    };
    if first == "file" {
        let all = file_object(page, scope.index);
        return rest_of(all, &path[1..], scope);
    }
    // A key may itself contain dots; else the first key, then indexed.
    if let Some(v) = page.field(&path.join(".")) {
        return v.clone();
    }
    match page.field(first) {
        Some(v) => rest_of(v.clone(), &path[1..], scope),
        None => Value::Null,
    }
}

/// `value` indexed by the rest of a path.
fn rest_of(value: Value, rest: &[String], scope: Scope) -> Value {
    rest.iter().fold(value, |v, key| {
        index_value(v, &Value::Text(key.clone()), scope)
    })
}

/// A task's field (`None`: not a task field, look in the page).
fn task_field(task: &Task, scope: Scope, path: &[String]) -> Option<Value> {
    let page = scope.page?;
    let v = match path[0].as_str() {
        "text" => Value::Text(task.text.clone()),
        "visual" => Value::Text(task.text.clone()),
        "completed" | "checked" => Value::Bool(task.completed()),
        "fullyCompleted" | "fullycompleted" => Value::Bool(fully_completed(page, task)),
        "status" => Value::Text(task.status.to_string()),
        "line" => Value::Number(task.line as f64),
        "lineCount" | "linecount" => Value::Number(1.0),
        "path" => Value::Text(page.rel.clone()),
        "link" => Value::Link(page.rel.trim_end_matches(".md").to_string()),
        "section" => section(page, task),
        "tags" => Value::List(task.tags.iter().cloned().map(Value::Text).collect()),
        "outlinks" => Value::List(task.outlinks.iter().cloned().map(Value::Link).collect()),
        "blockId" | "blockid" => task.block_id.clone().map_or(Value::Null, Value::Text),
        "annotated" => Value::Bool(task.annotated),
        "parent" => task.parent.map_or(Value::Null, |l| Value::Number(l as f64)),
        "children" | "subtasks" => Value::List(
            page.lists
                .iter()
                .filter(|c| c.parent == Some(task.line))
                .map(|c| item_object(page, c))
                .collect(),
        ),
        "task" => Value::Bool(task.is_task),
        _ => {
            // Its own fields (`[due:: …]`, emoji dates), then the page's.
            let name = path[0].as_str();
            let (_, v) = task
                .fields
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))?;
            v.clone()
        }
    };
    Some(rest_of(v, &path[1..], scope))
}

/// Whether a task and every task under it are done.
fn fully_completed(page: &Page, task: &Task) -> bool {
    task.completed()
        && page
            .lists
            .iter()
            .filter(|c| c.parent == Some(task.line) && c.is_task)
            .all(|c| fully_completed(page, c))
}

/// A list item (or task) as an object, for `file.lists`, `file.tasks` and
/// `children`.
/// A list item's section: a link to the heading above it (or its page).
fn section(page: &Page, item: &Task) -> Value {
    let note = page.rel.trim_end_matches(".md");
    match &item.section {
        Some(h) => Value::Link(format!("{note}#{h}")),
        None => Value::Link(note.to_string()),
    }
}

fn item_object(page: &Page, item: &Task) -> Value {
    let mut o = vec![
        ("text".to_string(), Value::Text(item.text.clone())),
        ("line".into(), Value::Number(item.line as f64)),
        ("task".into(), Value::Bool(item.is_task)),
        ("status".into(), Value::Text(item.status.to_string())),
        ("completed".into(), Value::Bool(item.completed())),
        (
            "fullyCompleted".into(),
            Value::Bool(fully_completed(page, item)),
        ),
        ("path".into(), Value::Text(page.rel.clone())),
        (
            "link".into(),
            Value::Link(page.rel.trim_end_matches(".md").to_string()),
        ),
        (
            "tags".into(),
            Value::List(item.tags.iter().cloned().map(Value::Text).collect()),
        ),
        (
            "outlinks".into(),
            Value::List(item.outlinks.iter().cloned().map(Value::Link).collect()),
        ),
        ("section".into(), section(page, item)),
        (
            "parent".into(),
            item.parent.map_or(Value::Null, |l| Value::Number(l as f64)),
        ),
        (
            "blockId".into(),
            item.block_id.clone().map_or(Value::Null, Value::Text),
        ),
        (
            "children".into(),
            Value::List(
                page.lists
                    .iter()
                    .filter(|c| c.parent == Some(item.line))
                    .map(|c| item_object(page, c))
                    .collect(),
            ),
        ),
    ];
    for (k, v) in &item.fields {
        if !o.iter().any(|(name, _)| name == k) {
            o.push((k.clone(), v.clone()));
        }
    }
    Value::Object(o)
}

/// `file`: the page's file data, as an object.
pub(crate) fn file_object(page: &Page, index: &Index) -> Value {
    let date = |d: Option<chrono::NaiveDateTime>| d.map_or(Value::Null, Value::Date);
    let link = Value::Link(page.rel.trim_end_matches(".md").to_string());
    Value::Object(vec![
        ("name".into(), Value::Text(page.name.clone())),
        ("path".into(), Value::Text(page.rel.clone())),
        ("folder".into(), Value::Text(page.folder.clone())),
        ("link".into(), link),
        ("ext".into(), Value::Text("md".into())),
        ("size".into(), Value::Number(page.size as f64)),
        ("mtime".into(), date(page.mtime)),
        (
            "mday".into(),
            date(page.mtime.map(|d| d.date().and_time(NaiveTime::MIN))),
        ),
        ("ctime".into(), date(page.ctime)),
        (
            "cday".into(),
            date(page.ctime.map(|d| d.date().and_time(NaiveTime::MIN))),
        ),
        ("day".into(), date(page.day)),
        (
            "tags".into(),
            Value::List(page.tags.iter().cloned().map(Value::Text).collect()),
        ),
        (
            "etags".into(),
            Value::List(page.tags.iter().cloned().map(Value::Text).collect()),
        ),
        (
            "outlinks".into(),
            Value::List(page.outlinks.iter().cloned().map(Value::Link).collect()),
        ),
        (
            "inlinks".into(),
            Value::List(
                index
                    .inlinks(page)
                    .map(|p| Value::Link(p.name.clone()))
                    .collect(),
            ),
        ),
        (
            "aliases".into(),
            Value::List(page.aliases.iter().cloned().map(Value::Text).collect()),
        ),
        ("starred".into(), Value::Bool(page.starred)),
        (
            "frontmatter".into(),
            Value::Object(page.frontmatter.clone()),
        ),
        (
            "tasks".into(),
            Value::List(page.tasks.iter().map(|t| item_object(page, t)).collect()),
        ),
        (
            "lists".into(),
            Value::List(page.lists.iter().map(|t| item_object(page, t)).collect()),
        ),
    ])
}

/// Applies a lambda (or a function's name) to `args`.
fn apply(f: &Expr, args: Vec<Value>, scope: Scope) -> Result<Value, String> {
    match f {
        Expr::Lambda(params, body) => {
            let mut vars = scope.vars.to_vec();
            vars.extend(params.iter().cloned().zip(args));
            eval(
                body,
                Scope {
                    vars: &vars,
                    ..scope
                },
            )
        }
        // `map(list, upper)`: a function by its name.
        Expr::Field(path) if path.len() == 1 => {
            let values: Vec<Expr> = args.into_iter().map(Expr::Value).collect();
            call(&path[0].to_lowercase(), &values, scope)
        }
        _ => Err("expected a lambda: (x) => …".into()),
    }
}

/// The list in `v` (a single value is a list of one; null is empty).
fn as_list(v: Value) -> Vec<Value> {
    match v {
        Value::List(l) => l,
        Value::Null => Vec::new(),
        other => vec![other],
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => Some(*n),
        _ => None,
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::Text(t) | Value::Link(t) => t.clone(),
        Value::Null => String::new(),
        other => other.display(),
    }
}

/// A regular expression from a query (the error says which).
fn regex(pattern: &str) -> Result<regex::Regex, String> {
    regex::Regex::new(pattern).map_err(|e| format!("bad regular expression {pattern}: {e}"))
}

fn call(name: &str, args: &[Expr], scope: Scope) -> Result<Value, String> {
    let values = || {
        args.iter()
            .map(|a| eval(a, scope))
            .collect::<Result<Vec<_>, _>>()
    };
    let want = |n: usize| {
        if args.len() == n {
            Ok(())
        } else {
            Err(format!(
                "{name}() takes {n} value{}",
                if n == 1 { "" } else { "s" }
            ))
        }
    };
    let at_least = |n: usize| {
        if args.len() >= n {
            Ok(())
        } else {
            Err(format!(
                "{name}() takes at least {n} value{}",
                if n == 1 { "" } else { "s" }
            ))
        }
    };
    // `map(list, (x) => …)` and the others taking a lambda.
    let lambda_list = |want_fn: bool| -> Result<(Vec<Value>, Option<&Expr>), String> {
        if args.is_empty() || (want_fn && args.len() < 2) {
            return Err(format!("{name}() takes a list and a lambda"));
        }
        let list = as_list(eval(&args[0], scope)?);
        Ok((list, args.get(1)))
    };
    Ok(match name {
        "date" => {
            if args.len() == 2 {
                let v = values()?;
                return Ok(match (&v[0], &v[1]) {
                    (Value::Text(t), Value::Text(f)) => {
                        parse_with(t, f).map_or(Value::Null, Value::Date)
                    }
                    _ => Value::Null,
                });
            }
            want(1)?;
            match eval(&args[0], scope)? {
                Value::Text(t) if is_date_word(&t) => {
                    let now = Local::now().naive_local();
                    let today = now.date().and_time(NaiveTime::MIN);
                    match t.to_lowercase().as_str() {
                        "now" => Value::Date(now),
                        "tomorrow" => Value::Date(today + Duration::days(1)),
                        "yesterday" => Value::Date(today - Duration::days(1)),
                        _ => Value::Date(today),
                    }
                }
                Value::Text(t) | Value::Link(t) => {
                    let name = t.rsplit('/').next().unwrap_or(&t);
                    parse_date(name).map_or(Value::Null, Value::Date)
                }
                d @ Value::Date(_) => d,
                _ => Value::Null,
            }
        }
        "dur" => {
            want(1)?;
            match eval(&args[0], scope)? {
                Value::Text(t) => Dur::parse(&t).map_or(Value::Null, Value::Duration),
                d @ Value::Duration(_) => d,
                _ => Value::Null,
            }
        }
        "contains" | "icontains" | "econtains" => {
            want(2)?;
            let v = values()?;
            let fold = |s: &str| {
                if name == "icontains" {
                    s.to_lowercase()
                } else {
                    s.to_string()
                }
            };
            Value::Bool(match (&v[0], &v[1]) {
                (Value::List(items), x) => items.iter().any(|i| match (i, x) {
                    (Value::Text(a), Value::Text(b)) if name == "icontains" => {
                        a.eq_ignore_ascii_case(b)
                    }
                    (Value::Text(a), Value::Text(b)) if name == "econtains" => a == b,
                    // contains() looks inside the items' text too.
                    (Value::Text(a), Value::Text(b)) if name == "contains" => {
                        a == b || a.contains(b.as_str())
                    }
                    _ => i.equals(x),
                }),
                (Value::Object(o), k) => o.iter().any(|(key, _)| *key == text(k)),
                (Value::Text(t), x) => fold(t).contains(&fold(&x.display())),
                (Value::Link(l), x) => fold(l).contains(&fold(&x.display())),
                _ => false,
            })
        }
        "containsword" => {
            want(2)?;
            let v = values()?;
            let word = text(&v[1]).to_lowercase();
            let has = |t: &str| {
                t.to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| w == word)
            };
            match &v[0] {
                Value::List(items) => {
                    Value::List(items.iter().map(|i| Value::Bool(has(&text(i)))).collect())
                }
                other => Value::Bool(has(&text(other))),
            }
        }
        "length" => {
            want(1)?;
            match &values()?[0] {
                Value::List(l) => Value::Number(l.len() as f64),
                Value::Object(o) => Value::Number(o.len() as f64),
                Value::Text(t) => Value::Number(t.chars().count() as f64),
                Value::Null => Value::Number(0.0),
                _ => Value::Number(1.0),
            }
        }
        "lower" | "upper" => {
            want(1)?;
            match &values()?[0] {
                Value::Text(t) if name == "lower" => Value::Text(t.to_lowercase()),
                Value::Text(t) => Value::Text(t.to_uppercase()),
                other => other.clone(),
            }
        }
        "default" | "ldefault" => {
            want(2)?;
            let v = values()?;
            match &v[0] {
                Value::Null => v[1].clone(),
                Value::List(items) if name == "default" => Value::List(
                    items
                        .iter()
                        .map(|i| {
                            if *i == Value::Null {
                                v[1].clone()
                            } else {
                                i.clone()
                            }
                        })
                        .collect(),
                ),
                other => other.clone(),
            }
        }
        "choice" => {
            want(3)?;
            let v = values()?;
            if v[0].truthy() {
                v[1].clone()
            } else {
                v[2].clone()
            }
        }
        "round" | "trunc" | "floor" | "ceil" => {
            let v = values()?;
            let digits = match v.get(1) {
                Some(Value::Number(d)) => *d as i32,
                _ => 0,
            };
            match v.first() {
                Some(Value::Number(n)) => {
                    let f = 10f64.powi(digits);
                    let x = n * f;
                    let x = match name {
                        "trunc" => x.trunc(),
                        "floor" => x.floor(),
                        "ceil" => x.ceil(),
                        _ => x.round(),
                    };
                    Value::Number(x / f)
                }
                _ => Value::Null,
            }
        }
        "number" => {
            want(1)?;
            match &values()?[0] {
                Value::Number(n) => Value::Number(*n),
                Value::Text(t) => {
                    let digits: String = t
                        .chars()
                        .skip_while(|c| !(c.is_ascii_digit() || *c == '-'))
                        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-'))
                        .collect();
                    digits.parse().map_or(Value::Null, Value::Number)
                }
                _ => Value::Null,
            }
        }
        "string" | "display" => {
            want(1)?;
            Value::Text(values()?[0].display())
        }
        "startswith" | "endswith" => {
            want(2)?;
            let v = values()?;
            let (t, p) = (v[0].display(), v[1].display());
            Value::Bool(if name == "startswith" {
                t.starts_with(&p)
            } else {
                t.ends_with(&p)
            })
        }
        "sum" | "product" | "average" => {
            want(1)?;
            let list = as_list(values()?.remove(0));
            if list.iter().any(|v| matches!(v, Value::Duration(_))) && name == "sum" {
                let total = list.iter().fold(Dur::default(), |acc, v| match v {
                    Value::Duration(d) => acc.plus(*d),
                    _ => acc,
                });
                return Ok(Value::Duration(total));
            }
            let numbers: Vec<f64> = list.iter().filter_map(number).collect();
            match name {
                "sum" => Value::Number(numbers.iter().sum()),
                "product" => Value::Number(numbers.iter().product()),
                _ if numbers.is_empty() => Value::Null,
                _ => Value::Number(numbers.iter().sum::<f64>() / numbers.len() as f64),
            }
        }
        "min" | "max" => {
            at_least(1)?;
            let v = values()?;
            let items = if v.len() == 1 {
                as_list(v.into_iter().next().expect("one"))
            } else {
                v
            };
            let pick = items
                .into_iter()
                .filter(|i| *i != Value::Null)
                .reduce(|a, b| {
                    let take_b = match a.sort_order(&b) {
                        Ordering::Greater => name == "min",
                        Ordering::Less => name == "max",
                        Ordering::Equal => false,
                    };
                    if take_b { b } else { a }
                });
            pick.unwrap_or(Value::Null)
        }
        "minby" | "maxby" => {
            let (list, f) = lambda_list(true)?;
            let f = f.expect("checked");
            let mut best: Option<(Value, Value)> = None;
            for item in list {
                let key = apply(f, vec![item.clone()], scope)?;
                let better = match &best {
                    None => true,
                    Some((k, _)) => match key.sort_order(k) {
                        Ordering::Less => name == "minby",
                        Ordering::Greater => name == "maxby",
                        Ordering::Equal => false,
                    },
                };
                if better {
                    best = Some((key, item));
                }
            }
            best.map_or(Value::Null, |(_, v)| v)
        }
        "map" => {
            let (list, f) = lambda_list(true)?;
            let f = f.expect("checked");
            Value::List(
                list.into_iter()
                    .map(|x| apply(f, vec![x], scope))
                    .collect::<Result<_, _>>()?,
            )
        }
        "filter" => {
            let (list, f) = lambda_list(true)?;
            let f = f.expect("checked");
            let mut out = Vec::new();
            for x in list {
                if apply(f, vec![x.clone()], scope)?.truthy() {
                    out.push(x);
                }
            }
            Value::List(out)
        }
        "reduce" => {
            let (list, f) = lambda_list(true)?;
            let f = f.expect("checked");
            let mut items = list.into_iter();
            let Some(mut acc) = items.next() else {
                return Ok(Value::Null);
            };
            for x in items {
                acc = apply(f, vec![acc, x], scope)?;
            }
            acc
        }
        "all" | "any" | "none" => {
            at_least(1)?;
            let (list, f) = if args.len() == 1 {
                (as_list(eval(&args[0], scope)?), None)
            } else {
                lambda_list(true)?
            };
            let mut results = Vec::new();
            for x in list {
                results.push(match f {
                    Some(f) => apply(f, vec![x], scope)?.truthy(),
                    None => x.truthy(),
                });
            }
            Value::Bool(match name {
                "all" => results.iter().all(|b| *b),
                "any" => results.iter().any(|b| *b),
                _ => !results.iter().any(|b| *b),
            })
        }
        "sort" => {
            at_least(1)?;
            let list = as_list(eval(&args[0], scope)?);
            let mut keyed = Vec::new();
            for x in list {
                let key = match args.get(1) {
                    Some(f) => apply(f, vec![x.clone()], scope)?,
                    None => x.clone(),
                };
                keyed.push((key, x));
            }
            keyed.sort_by(|(a, _), (b, _)| a.sort_order(b));
            Value::List(keyed.into_iter().map(|(_, x)| x).collect())
        }
        "reverse" => {
            want(1)?;
            match values()?.remove(0) {
                Value::List(mut l) => {
                    l.reverse();
                    Value::List(l)
                }
                Value::Text(t) => Value::Text(t.chars().rev().collect()),
                other => other,
            }
        }
        "nonnull" => {
            want(1)?;
            Value::List(
                as_list(values()?.remove(0))
                    .into_iter()
                    .filter(|v| *v != Value::Null)
                    .collect(),
            )
        }
        "firstvalue" => {
            want(1)?;
            as_list(values()?.remove(0))
                .into_iter()
                .find(|v| *v != Value::Null)
                .unwrap_or(Value::Null)
        }
        "join" => {
            at_least(1)?;
            let v = values()?;
            let sep = v.get(1).map_or(", ".to_string(), text);
            Value::Text(
                as_list(v[0].clone())
                    .iter()
                    .map(Value::display)
                    .collect::<Vec<_>>()
                    .join(&sep),
            )
        }
        "unique" => {
            want(1)?;
            let mut out: Vec<Value> = Vec::new();
            for x in as_list(values()?.remove(0)) {
                if !out.iter().any(|o| o.equals(&x) && o.kind_eq(&x)) {
                    out.push(x);
                }
            }
            Value::List(out)
        }
        "flat" => {
            at_least(1)?;
            let v = values()?;
            let depth = v.get(1).and_then(number).unwrap_or(1.0) as usize;
            fn flatten(items: Vec<Value>, depth: usize, out: &mut Vec<Value>) {
                for i in items {
                    match i {
                        Value::List(inner) if depth > 0 => flatten(inner, depth - 1, out),
                        other => out.push(other),
                    }
                }
            }
            let mut out = Vec::new();
            flatten(as_list(v[0].clone()), depth, &mut out);
            Value::List(out)
        }
        "slice" => {
            at_least(1)?;
            let v = values()?;
            let list = as_list(v[0].clone());
            let len = list.len() as i64;
            let at = |i: Option<&Value>, default: i64| {
                let i = i.and_then(number).map_or(default, |n| n as i64);
                (if i < 0 { len + i } else { i }).clamp(0, len) as usize
            };
            let (start, end) = (at(v.get(1), 0), at(v.get(2), len));
            Value::List(if start < end {
                list[start..end].to_vec()
            } else {
                Vec::new()
            })
        }
        "extract" => {
            at_least(1)?;
            let v = values()?;
            let keys: Vec<String> = v[1..].iter().map(text).collect();
            match &v[0] {
                Value::Object(o) => Value::Object(
                    o.iter()
                        .filter(|(k, _)| keys.contains(k))
                        .cloned()
                        .collect(),
                ),
                _ => Value::Null,
            }
        }
        "object" => {
            let v = values()?;
            if v.len() % 2 != 0 {
                return Err("object() takes keys and values in pairs".into());
            }
            Value::Object(
                v.chunks(2)
                    .map(|kv| (text(&kv[0]), kv[1].clone()))
                    .collect(),
            )
        }
        "list" | "array" => Value::List(values()?),
        "link" | "embed" => {
            at_least(1)?;
            match values()?.remove(0) {
                Value::Text(t) | Value::Link(t) => Value::Link(super::value::link_name(&t)),
                _ => Value::Null,
            }
        }
        "elink" => {
            at_least(1)?;
            let v = values()?;
            Value::Text(v.get(1).map_or_else(|| text(&v[0]), text))
        }
        "typeof" => {
            want(1)?;
            Value::Text(
                match values()?[0] {
                    Value::Null => "null",
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::Text(_) => "string",
                    Value::Date(_) => "date",
                    Value::Duration(_) => "duration",
                    Value::Link(_) => "link",
                    Value::List(_) => "array",
                    Value::Object(_) => "object",
                }
                .into(),
            )
        }
        "meta" => {
            want(1)?;
            match values()?.remove(0) {
                Value::Link(l) => {
                    let (path, sub) = match l.split_once('#') {
                        Some((p, s)) => (p.to_string(), Value::Text(s.to_string())),
                        None => (l.clone(), Value::Null),
                    };
                    let display = path.rsplit('/').next().unwrap_or(&path).to_string();
                    Value::Object(vec![
                        ("display".into(), Value::Text(display)),
                        ("embed".into(), Value::Bool(false)),
                        ("path".into(), Value::Text(path)),
                        ("subpath".into(), sub),
                        ("type".into(), Value::Text("file".into())),
                    ])
                }
                _ => Value::Null,
            }
        }
        "regextest" | "regexmatch" => {
            want(2)?;
            let v = values()?;
            let pattern = text(&v[0]);
            let pattern = if name == "regexmatch" {
                format!("^(?:{pattern})$")
            } else {
                pattern
            };
            Value::Bool(regex(&pattern)?.is_match(&text(&v[1])))
        }
        "regexreplace" => {
            want(3)?;
            let v = values()?;
            let re = regex(&text(&v[1]))?;
            Value::Text(
                re.replace_all(&text(&v[0]), text(&v[2]).as_str())
                    .into_owned(),
            )
        }
        "replace" => {
            want(3)?;
            let v = values()?;
            Value::Text(text(&v[0]).replace(&text(&v[1]), &text(&v[2])))
        }
        "split" => {
            at_least(2)?;
            let v = values()?;
            let re = regex(&text(&v[1]))?;
            let t = text(&v[0]);
            let parts: Vec<Value> = match v.get(2).and_then(number) {
                Some(n) => re
                    .splitn(&t, n as usize)
                    .map(|s| Value::Text(s.into()))
                    .collect(),
                None => re.split(&t).map(|s| Value::Text(s.into())).collect(),
            };
            Value::List(parts)
        }
        "padleft" | "padright" => {
            at_least(2)?;
            let v = values()?;
            let t = text(&v[0]);
            let width = v.get(1).and_then(number).unwrap_or(0.0) as usize;
            let pad = v.get(2).map_or(" ".to_string(), text);
            let have = t.chars().count();
            if have >= width || pad.is_empty() {
                Value::Text(t)
            } else {
                let fill: String = pad.chars().cycle().take(width - have).collect();
                Value::Text(if name == "padleft" {
                    fill + &t
                } else {
                    t + &fill
                })
            }
        }
        "substring" => {
            at_least(2)?;
            let v = values()?;
            let chars: Vec<char> = text(&v[0]).chars().collect();
            let start = (v.get(1).and_then(number).unwrap_or(0.0) as usize).min(chars.len());
            let end = v
                .get(2)
                .and_then(number)
                .map_or(chars.len(), |n| (n as usize).min(chars.len()))
                .max(start);
            Value::Text(chars[start..end].iter().collect())
        }
        "truncate" => {
            at_least(2)?;
            let v = values()?;
            let t = text(&v[0]);
            let n = v.get(1).and_then(number).unwrap_or(0.0) as usize;
            let suffix = v.get(2).map_or("...".to_string(), text);
            if t.chars().count() <= n {
                Value::Text(t)
            } else {
                let keep = n.saturating_sub(suffix.chars().count());
                Value::Text(t.chars().take(keep).collect::<String>() + &suffix)
            }
        }
        "dateformat" => {
            want(2)?;
            let v = values()?;
            match (&v[0], &v[1]) {
                (Value::Date(d), Value::Text(f)) => Value::Text(format_date(d, f)),
                _ => Value::Null,
            }
        }
        "durationformat" => {
            want(2)?;
            let v = values()?;
            match (&v[0], &v[1]) {
                (Value::Duration(d), Value::Text(f)) => Value::Text(format_duration(*d, f)),
                _ => Value::Null,
            }
        }
        "striptime" => {
            want(1)?;
            match values()?.remove(0) {
                Value::Date(d) => Value::Date(d.date().and_time(NaiveTime::MIN)),
                other => other,
            }
        }
        "localtime" => {
            want(1)?;
            values()?.remove(0)
        }
        "currencyformat" => {
            at_least(1)?;
            let v = values()?;
            let Some(n) = number(&v[0]) else {
                return Ok(Value::Null);
            };
            let code = v.get(1).map_or("USD".to_string(), text).to_uppercase();
            let symbol = match code.as_str() {
                "USD" => "$",
                "EUR" => "€",
                "GBP" => "£",
                "JPY" => "¥",
                "TRY" => "₺",
                "INR" => "₹",
                _ => "",
            };
            let whole = format!("{:.2}", n.abs());
            let (int, frac) = whole.split_once('.').expect("two decimals");
            let mut grouped = String::new();
            for (i, c) in int.chars().enumerate() {
                if i > 0 && (int.len() - i) % 3 == 0 {
                    grouped.push(',');
                }
                grouped.push(c);
            }
            let sign = if n < 0.0 { "-" } else { "" };
            Value::Text(if symbol.is_empty() {
                format!("{sign}{code} {grouped}.{frac}")
            } else {
                format!("{sign}{symbol}{grouped}.{frac}")
            })
        }
        "hash" => {
            at_least(1)?;
            let v = values()?;
            // FNV-1a over the arguments: the same text, the same number.
            let mut h: u32 = 0x811c_9dc5;
            for b in v
                .iter()
                .map(Value::display)
                .collect::<Vec<_>>()
                .join("\u{0}")
                .bytes()
            {
                h ^= u32::from(b);
                h = h.wrapping_mul(0x0100_0193);
            }
            Value::Number(f64::from(h))
        }
        other => return Err(format!("unknown function {other}()")),
    })
}

/// A date read with Luxon-style tokens (`dd.MM.yyyy`).
fn parse_with(text: &str, format: &str) -> Option<chrono::NaiveDateTime> {
    let spec = luxon_to_chrono(format);
    chrono::NaiveDateTime::parse_from_str(text, &spec)
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(text, &spec)
                .ok()
                .map(|d| d.and_time(NaiveTime::MIN))
        })
}

/// A duration with Luxon tokens: `y M w d h m s S`, text in single
/// quotes (`d 'days', h 'hours'`); larger units take what they can.
fn format_duration(d: Dur, format: &str) -> String {
    const DAY: i64 = 86_400_000;
    let units: [(char, i64); 8] = [
        ('y', 365 * DAY),
        ('M', 30 * DAY),
        ('w', 7 * DAY),
        ('d', DAY),
        ('h', 3_600_000),
        ('m', 60_000),
        ('s', 1000),
        ('S', 1),
    ];
    // The units the format uses, largest first, take their share.
    let mut left = d.approx_millis();
    let mut amounts = std::collections::HashMap::new();
    let mut quoted = false;
    let used: Vec<char> = format
        .chars()
        .filter(|c| {
            if *c == '\'' {
                quoted = !quoted;
            }
            !quoted && units.iter().any(|(u, _)| u == c)
        })
        .collect();
    for (unit, size) in units {
        if used.contains(&unit) {
            amounts.insert(unit, left / size);
            left %= size;
        }
    }
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            for q in chars.by_ref() {
                if q == '\'' {
                    break;
                }
                out.push(q);
            }
        } else if let Some(n) = amounts.get(&c) {
            let mut width = 1;
            while chars.peek() == Some(&c) {
                chars.next();
                width += 1;
            }
            out.push_str(&format!("{n:0width$}"));
        } else {
            out.push(c);
        }
    }
    out
}

/// Luxon tokens as chrono's (for reading and writing dates).
fn luxon_to_chrono(format: &str) -> String {
    const TOKENS: [(&str, &str); 14] = [
        ("yyyy", "%Y"),
        ("yy", "%y"),
        ("MMMM", "%B"),
        ("MMM", "%b"),
        ("MM", "%m"),
        ("M", "%-m"),
        ("dd", "%d"),
        ("d", "%-d"),
        ("EEEE", "%A"),
        ("EEE", "%a"),
        ("HH", "%H"),
        ("mm", "%M"),
        ("ss", "%S"),
        ("WW", "%V"),
    ];
    let mut out = String::new();
    let mut rest = format;
    'outer: while !rest.is_empty() {
        for (token, spec) in TOKENS {
            if let Some(r) = rest.strip_prefix(token) {
                out.push_str(spec);
                rest = r;
                continue 'outer;
            }
        }
        let c = rest.chars().next().expect("rest isn't empty");
        if c == '%' {
            out.push_str("%%");
        } else {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Formats with Luxon-style tokens as Dataview does (`yyyy-MM-dd`,
/// `dd.MM.yyyy`, `MMMM d`, `EEE`, `HH:mm`).
fn format_date(d: &chrono::NaiveDateTime, format: &str) -> String {
    d.format(&luxon_to_chrono(format)).to_string()
}

#[cfg(test)]
mod tests {
    use super::super::query::parse;
    use super::*;
    use crate::vault::Vault;
    use crate::vault::tests::{scratch, write};

    fn index(name: &str) -> Index {
        let dir = scratch(name);
        write(
            &dir,
            &[
                (
                    "Books/Dune.md",
                    "---\nauthor: Frank Herbert\nrating: 5\ntags: [reading, scifi]\n---\n- [x] read part one\n- [ ] read part two",
                ),
                (
                    "Books/Emma.md",
                    "---\nauthor: Jane Austen\nrating: 3\n---\n#reading #classic",
                ),
                (
                    "Journal/2026-08-09.md",
                    "Started [[Dune]] again.\nmood:: good\n- [ ] water the garden",
                ),
                ("Journal/2026-08-10.md", "mood:: tired\n- [x] call [[Emma]]"),
                ("Home.md", "welcome"),
            ],
        );
        Index::build(&Vault::open(&dir).unwrap())
    }

    fn run_query(index: &Index, query: &str) -> Results {
        run(&parse(query).unwrap(), index, None).unwrap()
    }

    fn list(index: &Index, query: &str) -> Vec<String> {
        match run_query(index, query) {
            Results::List { rows, .. } => rows
                .into_iter()
                .map(|(page, v)| match v {
                    Some(v) => format!("{page}: {}", v.display()),
                    None => page.name,
                })
                .collect(),
            other => panic!("not a list: {other:?}"),
        }
    }

    #[test]
    fn from_folders_tags_and_links() {
        let ix = index("dv-from");
        assert_eq!(list(&ix, "LIST FROM \"Books\""), ["Dune", "Emma"]);
        assert_eq!(list(&ix, "LIST FROM #reading and -#classic"), ["Dune"]);
        assert_eq!(
            list(&ix, "LIST FROM [[Dune]] or [[Emma]]"),
            ["2026-08-09", "2026-08-10"]
        );
        assert_eq!(list(&ix, "LIST FROM \"Home\""), ["Home"], "a note's path");
        assert_eq!(list(&ix, "LIST").len(), 5);
    }

    #[test]
    fn where_sort_and_limit() {
        assert_eq!(
            list(
                &index("dv-where"),
                "LIST rating FROM \"Books\" WHERE rating > 3"
            ),
            ["Dune: 5"]
        );
        let ix = index("dv-sort");
        assert_eq!(
            list(&ix, "LIST WHERE mood SORT file.name DESC LIMIT 1"),
            ["2026-08-10"]
        );
        assert_eq!(list(&ix, "LIST WHERE mood = \"good\""), ["2026-08-09"]);
        assert_eq!(
            list(
                &ix,
                "LIST file.day FROM \"Journal\" WHERE file.day >= date(2026-08-10)"
            ),
            ["2026-08-10: 2026-08-10"]
        );
        assert_eq!(
            list(&ix, "LIST length(file.inlinks) FROM \"Books\""),
            ["Dune: 1", "Emma: 1"]
        );
    }

    #[test]
    fn tables_have_headers_and_cells() {
        let ix = index("dv-table");
        let Results::Table { id, headers, rows } = run_query(
            &ix,
            "TABLE author AS Author, rating * 2 FROM #reading SORT rating DESC",
        ) else {
            panic!("a table")
        };
        assert_eq!(id.as_deref(), Some("File"));
        assert_eq!(headers, ["Author", "rating * 2"]);
        let cells: Vec<Vec<String>> = rows
            .iter()
            .map(|(n, c)| {
                std::iter::once(n.name.clone())
                    .chain(c.iter().map(Value::display))
                    .collect()
            })
            .collect();
        assert_eq!(
            cells,
            [
                ["Dune", "Frank Herbert", "10"],
                ["Emma", "Jane Austen", "6"]
            ]
        );
    }

    #[test]
    fn tasks_are_filtered_one_by_one_and_grouped_by_page() {
        let ix = index("dv-tasks");
        let Results::Tasks(groups) = run_query(&ix, "TASK WHERE !completed") else {
            panic!("tasks")
        };
        let shown: Vec<(String, Vec<String>)> = groups
            .into_iter()
            .map(|(p, t)| (p.name, t.into_iter().map(|(_, t)| t.text).collect()))
            .collect();
        assert_eq!(
            shown,
            [
                ("Dune".to_string(), vec!["read part two".to_string()]),
                (
                    "2026-08-09".to_string(),
                    vec!["water the garden".to_string()]
                )
            ]
        );
    }

    #[test]
    fn functions() {
        let ix = index("dv-functions");
        assert_eq!(
            list(&ix, "LIST WHERE contains(file.tags, \"#scifi\")"),
            ["Dune"]
        );
        assert_eq!(
            list(&ix, "LIST WHERE icontains(author, \"austen\")"),
            ["Emma"]
        );
        assert_eq!(
            list(
                &ix,
                "LIST upper(default(mood, \"none\")) FROM \"Journal\" or \"Home\""
            ),
            ["Home: NONE", "2026-08-09: GOOD", "2026-08-10: TIRED"]
        );
        assert_eq!(
            list(
                &ix,
                "LIST dateformat(file.day, \"dd.MM.yyyy EEE\") FROM \"Journal\" LIMIT 1"
            ),
            ["2026-08-09: 09.08.2026 Sun"]
        );
        let err = run(&parse("LIST WHERE nope(1)").unwrap(), &ix, None).unwrap_err();
        assert_eq!(err, "unknown function nope()");
    }

    #[test]
    fn this_is_the_note_the_query_is_in() {
        let ix = index("dv-this");
        let this = ix.pages.iter().find(|p| p.name == "Dune");
        let q = parse("LIST WHERE contains(file.outlinks, this.file.link)").unwrap();
        let Results::List { rows, .. } = run(&q, &ix, this).unwrap() else {
            panic!("a list")
        };
        let names: Vec<_> = rows.into_iter().map(|(n, _)| n.name).collect();
        assert_eq!(names, ["2026-08-09"]);
    }

    /// A vault for the newer language: groups, flattening, tasks with
    /// children and dates, file data.
    fn rich(name: &str) -> Index {
        let dir = scratch(name);
        write(
            &dir,
            &[
                (
                    "Books/Dune.md",
                    "---\nauthor: Frank Herbert\nrating: 5\ntags: [reading, scifi]\naliases: [Arrakis]\n---\n# Dune\n## Plan\n- [ ] read part one 📅 2026-08-20\n  - [x] chapter 1\n  - [ ] chapter 2 [pages:: 40] #longread\n- a note about [[Emma]] ^n1",
                ),
                (
                    "Books/Emma.md",
                    "---\nauthor: Jane Austen\nrating: 3\ntags: [reading, classic]\n---\n- [x] done ✅ 2026-08-01",
                ),
                ("Journal/20260809 Monday.md", "Read [[Dune]].\nmood:: good"),
                ("Journal/2026-08-10.md", "mood:: tired"),
                (
                    ".blackglass/plugins/bookmarks/bookmarks.json",
                    "{\"items\": [{\"type\": \"file\", \"path\": \"Books/Emma.md\"}]}",
                ),
            ],
        );
        Index::build(&Vault::open(&dir).unwrap())
    }

    /// A value an expression has on the page at `rel` (`FROM` it alone).
    fn ev(index: &Index, rel: &str, expr: &str) -> String {
        let q = parse(&format!("LIST WITHOUT ID {expr} FROM \"{rel}\"")).unwrap();
        match run(&q, index, None).unwrap() {
            Results::List { rows, .. } => {
                rows[0].1.as_ref().map(Value::display).unwrap_or_default()
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn commands_run_in_the_order_written() {
        let ix = rich("dv-order");
        assert_eq!(
            list(&ix, "LIST FROM \"Books\" SORT rating LIMIT 1"),
            ["Emma"]
        );
        assert_eq!(
            list(&ix, "LIST FROM \"Books\" LIMIT 1 SORT rating"),
            ["Dune"],
            "LIMIT before SORT: the first page only"
        );
        assert_eq!(
            list(
                &ix,
                "LIST FROM \"Books\" WHERE rating > 1 SORT rating DESC WHERE rating < 5"
            ),
            ["Emma"]
        );
    }

    #[test]
    fn a_groups_rows_are_whole_rows() {
        // Each of `rows` has the row's fields: what FLATTEN made, the
        // page's own, and `file` (as in Dataview).
        let ix = rich("dv-group-rows");
        assert_eq!(
            list(
                &ix,
                "LIST length(filter(rows, (r) => r.tag = \"#reading\")) FROM \"Books\" FLATTEN file.tags AS tag GROUP BY file.folder"
            ),
            ["Books: 2"],
            "Dune's and Emma's #reading rows"
        );
        assert_eq!(
            list(
                &ix,
                "LIST map(filter(rows, (r) => r.rating > 4), (r) => r.file.name) FROM \"Books\" GROUP BY file.folder"
            ),
            ["Books: Dune"]
        );
        // `this` (the note the query is in) inside a group, and inside a
        // lambda over its rows.
        let this = ix.pages.iter().find(|p| p.name == "Dune");
        let Results::List { rows, .. } = run(
            &parse("LIST this.rating + length(filter(rows, (r) => r.rating = this.rating)) FROM \"Books\" GROUP BY file.folder").unwrap(),
            &ix,
            this,
        )
        .unwrap() else {
            panic!("a list")
        };
        assert_eq!(rows[0].1, Some(Value::Number(6.0)), "5 + Dune's one row");
    }

    #[test]
    fn groups_and_flattening() {
        let ix = rich("dv-group");
        assert_eq!(
            list(&ix, "LIST rows.file.name GROUP BY file.folder"),
            ["Books: Dune, Emma", "Journal: 2026-08-10, 20260809 Monday"]
        );
        // Without a value: the group's rows, as links.
        let Results::List { rows, .. } = run_query(&ix, "LIST FROM \"Books\" GROUP BY author")
        else {
            panic!("a list")
        };
        assert_eq!(rows[0].0.name, "Frank Herbert");
        assert_eq!(
            rows[0].1,
            Some(Value::List(vec![Value::Link("Books/Dune".into())]))
        );
        let Results::Table { id, rows, .. } = run_query(
            &ix,
            "TABLE length(rows) AS n, sum(rows.rating) AS total GROUP BY file.folder AS folder WHERE length(rows) > 0",
        ) else {
            panic!("a table")
        };
        assert_eq!(
            id.as_deref(),
            Some("folder"),
            "the key's column, by its name"
        );
        let cells: Vec<String> = rows
            .iter()
            .map(|(k, c)| format!("{k} {} {}", c[0].display(), c[1].display()))
            .collect();
        assert_eq!(cells, ["Books 2 8", "Journal 2 0"]);
        // One row per tag; the page stays each row's.
        assert_eq!(
            list(
                &ix,
                "LIST tag FROM \"Books\" FLATTEN file.tags AS tag SORT tag"
            ),
            [
                "Emma: #classic",
                "Dune: #longread",
                "Dune: #reading",
                "Emma: #reading",
                "Dune: #scifi"
            ]
        );
        // An empty list leaves no rows (as Dataview does).
        let Results::List { rows, .. } = run_query(&ix, "LIST x FROM \"Books\" FLATTEN [] AS x")
        else {
            panic!("a list")
        };
        assert!(rows.is_empty(), "{rows:?}");
        // Tasks grouped by a field, under its value.
        let Results::Tasks(groups) = run_query(&ix, "TASK FROM \"Books\" GROUP BY completed")
        else {
            panic!("tasks")
        };
        let heads: Vec<(String, usize)> = groups
            .iter()
            .map(|(h, t)| (h.name.clone(), t.len()))
            .collect();
        assert_eq!(heads, [("false".to_string(), 2), ("true".to_string(), 2)]);
        assert!(
            groups[0].1.iter().all(|(rel, _)| rel == "Books/Dune.md"),
            "each task keeps its page"
        );
    }

    #[test]
    fn sources_of_the_current_note_and_without_id() {
        let ix = rich("dv-sources");
        let this = ix.pages.iter().find(|p| p.name == "Dune");
        let names = |q: &str| -> Vec<String> {
            match run(&parse(q).unwrap(), &ix, this).unwrap() {
                Results::List { rows, .. } => rows.into_iter().map(|(p, _)| p.name).collect(),
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(names("LIST FROM [[]]"), ["20260809 Monday"], "linking here");
        assert_eq!(
            names("LIST FROM outgoing([[]])"),
            ["Emma"],
            "linked from here"
        );
        assert_eq!(names("LIST FROM outgoing([[20260809 Monday]])"), ["Dune"]);
        let Results::List { without_id, rows } = run(
            &parse("LIST WITHOUT ID author FROM \"Books\"").unwrap(),
            &ix,
            None,
        )
        .unwrap() else {
            panic!("a list")
        };
        assert!(without_id);
        assert_eq!(rows[1].1, Some(Value::Text("Jane Austen".into())));
    }

    #[test]
    fn the_expression_language() {
        let ix = rich("dv-exprs");
        let dune = "Books/Dune";
        assert_eq!(ev(&ix, dune, "[1, 2, 3][1]"), "2");
        assert_eq!(ev(&ix, dune, "[1, 2, 3][-1]"), "3");
        assert_eq!(ev(&ix, dune, "{a: 1, \"b c\": 2}[\"b c\"]"), "2");
        assert_eq!(ev(&ix, dune, "{a: {b: 5}}.a.b"), "5");
        assert_eq!(
            ev(&ix, dune, "[[Emma]].author"),
            "Jane Austen",
            "a linked page's field"
        );
        assert_eq!(ev(&ix, dune, "[[Emma]].file.name"), "Emma");
        assert_eq!(
            ev(&ix, dune, "file.tasks.text"),
            "read part one 📅 2026-08-20, chapter 1, chapter 2 [pages:: 40] #longread",
            "swizzled"
        );
        assert_eq!(ev(&ix, dune, "7 % 3"), "1");
        assert_eq!(ev(&ix, dune, "map([1, 2, 3], (x) => x * 2)"), "2, 4, 6");
        assert_eq!(
            ev(&ix, dune, "filter(file.tasks, t => t.completed).text"),
            "chapter 1"
        );
        assert_eq!(ev(&ix, dune, "reduce([1, 2, 3, 4], (a, b) => a + b)"), "10");
        assert_eq!(ev(&ix, dune, "sort([3, 1, 2], (x) => -x)"), "3, 2, 1");
        assert_eq!(
            ev(&ix, dune, "any([1, 0], (x) => x > 0) and !all([1, 0])"),
            "true"
        );
        assert_eq!(ev(&ix, dune, "none([0, 0])"), "true");
        assert_eq!(
            ev(&ix, dune, "maxby(file.tasks, (t) => t.line).text"),
            "chapter 2 [pages:: 40] #longread"
        );
        assert_eq!(
            ev(&ix, dune, "map([\"a\", \"b\"], upper)"),
            "A, B",
            "a function by name"
        );
        let err = run(&parse("LIST (x) => x").unwrap(), &ix, None).unwrap_err();
        assert!(err.contains("lambda"), "{err}");
    }

    #[test]
    fn text_number_and_list_functions() {
        let ix = rich("dv-functions-more");
        let d = "Books/Dune";
        let cases = [
            ("regextest(\"\\\\d+\", \"part 12\")", "true"),
            ("regexmatch(\"\\\\d+\", \"part 12\")", "false"),
            ("regexreplace(\"a-b-c\", \"-\", \"+\")", "a+b+c"),
            ("replace(\"a.b.c\", \".\", \"/\")", "a/b/c"),
            ("split(\"a, b,c\", \",\\\\s*\")", "a, b, c"),
            ("padleft(\"7\", 3, \"0\")", "007"),
            ("padright(\"ab\", 4)", "ab  "),
            ("substring(\"blackglass\", 5)", "glass"),
            ("truncate(\"a long title\", 7)", "a lo..."),
            ("containsword(\"the slip box\", \"Slip\")", "true"),
            ("econtains([\"ab\"], \"a\")", "false"),
            ("contains({a: 1}, \"a\")", "true"),
            ("trunc(-2.7) + floor(2.7) + ceil(2.1)", "3"),
            ("min(3, 1, 2) + max([4, 9, 2])", "10"),
            ("product([2, 3, 4])", "24"),
            ("average([1, 2, 3, 6])", "3"),
            ("unique([1, 1, 2, \"1\"])", "1, 2, 1"),
            // `[[` is always a link: nested lists start with a space.
            ("flat([ [1, 2], [3, [4]] ])", "1, 2, 3, 4"),
            ("slice([1, 2, 3, 4], 1, -1)", "2, 3"),
            ("join(reverse([1, 2, 3]), \"-\")", "3-2-1"),
            ("nonnull([null, 1])", "1"),
            ("firstvalue([null, 2, 3])", "2"),
            ("extract({a: 1, b: 2, c: 3}, \"a\", \"c\")", "a: 1, c: 3"),
            ("object(\"k\", 5).k", "5"),
            (
                "typeof(dur(1 day)) + \" \" + typeof([1]) + \" \" + typeof([[Emma]])",
                "duration array link",
            ),
            ("meta([[Emma#Plot]]).subpath", "Plot"),
            ("currencyformat(1234.5, \"EUR\")", "€1,234.50"),
            (
                "hash(\"seed\", \"text\") = hash(\"seed\", \"text\")",
                "true",
            ),
            ("display([[Emma]])", "Emma"),
            ("default(null, 5)", "5"),
        ];
        for (expr, want) in cases {
            assert_eq!(ev(&ix, d, expr), want, "{expr}");
        }
    }

    #[test]
    fn durations_and_dates() {
        let ix = rich("dv-durations");
        let d = "Books/Dune";
        assert_eq!(ev(&ix, d, "dur(1 day 2 hours)"), "1 day, 2 hours");
        assert_eq!(ev(&ix, d, "date(2026-08-09) + dur(1 month)"), "2026-09-09");
        assert_eq!(ev(&ix, d, "date(2026-08-09) - dur(2 weeks)"), "2026-07-26");
        assert_eq!(ev(&ix, d, "date(2026-08-09) - date(2026-08-01)"), "8 days");
        assert_eq!(
            ev(&ix, d, "(date(2026-08-09) - date(2026-08-01)).days"),
            "8"
        );
        assert_eq!(ev(&ix, d, "dur(3 days) > dur(2 days)"), "true");
        assert_eq!(
            ev(
                &ix,
                d,
                "durationformat(dur(1 day 3 hours), \"d 'd', h 'h'\")"
            ),
            "1 d, 3 h"
        );
        assert_eq!(
            ev(&ix, d, "date(\"09.08.2026\", \"dd.MM.yyyy\").month"),
            "8"
        );
        assert_eq!(
            ev(&ix, d, "striptime(date(2026-08-09T10:30))"),
            "2026-08-09"
        );
        assert_eq!(
            ev(&ix, d, "sum([dur(1 hour), dur(30 minutes)])"),
            "1 hour, 30 minutes"
        );
    }

    #[test]
    fn list_items_task_fields_and_file_data() {
        let ix = rich("dv-fields");
        let d = "Books/Dune";
        assert_eq!(
            ev(&ix, d, "length(file.lists)"),
            "4",
            "tasks and plain items"
        );
        assert_eq!(
            ev(&ix, d, "file.lists[3].text"),
            "a note about [[Emma]] ^n1"
        );
        assert_eq!(ev(&ix, d, "file.lists[3].blockId"), "n1");
        assert_eq!(ev(&ix, d, "file.lists[3].task"), "false");
        assert_eq!(
            ev(&ix, d, "meta(file.lists[3].section).subpath"),
            "Plan",
            "a plain item knows its heading too"
        );
        assert_eq!(
            ev(&ix, d, "file.tasks[0].children.text"),
            "chapter 1, chapter 2 [pages:: 40] #longread"
        );
        assert_eq!(
            ev(&ix, d, "file.tasks[1].parent"),
            "8",
            "the line of the item it's under"
        );
        assert_eq!(ev(&ix, d, "file.tasks[0].due"), "2026-08-20", "📅");
        assert_eq!(
            ev(&ix, "Books/Emma", "file.tasks[0].completion"),
            "2026-08-01",
            "✅"
        );
        assert_eq!(ev(&ix, d, "file.tasks[2].pages"), "40");
        assert_eq!(ev(&ix, d, "file.tasks[2].tags"), "#longread");
        assert_eq!(ev(&ix, d, "file.tasks[0].fullyCompleted"), "false");
        assert_eq!(ev(&ix, d, "file.aliases"), "Arrakis");
        assert_eq!(ev(&ix, d, "file.frontmatter.author"), "Frank Herbert");
        assert_eq!(ev(&ix, d, "file.starred"), "false");
        assert_eq!(ev(&ix, "Books/Emma", "file.starred"), "true", "bookmarked");
        assert_eq!(
            ev(&ix, "Journal/20260809 Monday", "file.day"),
            "2026-08-09",
            "yyyymmdd"
        );
        // TASK queries see each task's own fields and section.
        let Results::Tasks(groups) = run_query(&ix, "TASK WHERE due AND !completed") else {
            panic!("tasks")
        };
        assert_eq!(groups[0].1.len(), 1);
        assert_eq!(groups[0].1[0].1.text, "read part one 📅 2026-08-20");
        let Results::Tasks(groups) = run_query(
            &ix,
            "TASK WHERE section = [[Books/Dune#Plan]] AND annotated",
        ) else {
            panic!("tasks")
        };
        assert_eq!(groups[0].1[0].1.text, "chapter 2 [pages:: 40] #longread");
    }

    #[test]
    fn calendars_put_pages_on_their_days() {
        let ix = rich("dv-calendar");
        let Results::Calendar(days) = run_query(&ix, "CALENDAR file.day FROM \"Journal\"") else {
            panic!("a calendar")
        };
        let shown: Vec<String> = days.iter().map(|(d, p)| format!("{d} {p}")).collect();
        assert_eq!(
            shown,
            ["2026-08-09 20260809 Monday", "2026-08-10 2026-08-10"]
        );
        assert!(parse("CALENDAR").unwrap_err().contains("needs a date"));
    }
}
