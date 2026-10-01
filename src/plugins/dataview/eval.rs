//! Runs a parsed query over the index: picks the pages `FROM` names,
//! filters them with `WHERE`, sorts and limits them, and computes what
//! each result row shows.

use std::cmp::Ordering;

use chrono::{Duration, Local, NaiveTime};

use super::index::{Index, Page, Task};
use super::query::{Expr, Kind, Op, Query, Source, is_date_word};
use super::value::{Value, parse_date};

/// A page in results: its name (shown) and its path in the vault (to open
/// it from the result).
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
}

impl std::fmt::Display for PageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// What a query found.
#[derive(Debug, Clone, PartialEq)]
pub enum Results {
    /// Each page, with its value if the list has one.
    List(Vec<(PageRef, Option<Value>)>),
    Table {
        /// The page column's header (`None`: `WITHOUT ID`), then the
        /// columns'.
        id: Option<String>,
        headers: Vec<String>,
        /// Each row: the page, then the cells.
        rows: Vec<(PageRef, Vec<Value>)>,
    },
    /// Tasks grouped by page, pages in order.
    Tasks(Vec<(PageRef, Vec<Task>)>),
}

/// Where fields are looked up: a page, maybe a task in it, and the page
/// the query is in (`this`).
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    pub index: &'a Index,
    pub page: &'a Page,
    pub task: Option<&'a Task>,
    pub this: Option<&'a Page>,
}

/// Runs `query`, written in the note `this` (if it's in the vault).
pub fn run(query: &Query, index: &Index, this: Option<&Page>) -> Result<Results, String> {
    let mut pages: Vec<&Page> = index
        .pages
        .iter()
        .filter(|p| query.from.as_ref().is_none_or(|s| matches(s, p)))
        .collect();
    pages.sort_by(|a, b| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()));
    let scope = |page, task| Scope {
        index,
        page,
        task,
        this,
    };
    let keep = |s: Scope| -> Result<bool, String> {
        for f in &query.filters {
            if !eval(f, s)?.truthy() {
                return Ok(false);
            }
        }
        Ok(true)
    };
    let sort = |a: Scope, b: Scope| -> Ordering {
        for (key, descending) in &query.sort {
            let (x, y) = (
                eval(key, a).unwrap_or(Value::Null),
                eval(key, b).unwrap_or(Value::Null),
            );
            let order = x.sort_order(&y);
            let order = if *descending { order.reverse() } else { order };
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    };
    let limit = query.limit.unwrap_or(usize::MAX);

    if query.kind == Kind::Task {
        let mut tasks: Vec<Scope> = Vec::new();
        for page in pages {
            for task in &page.tasks {
                let s = scope(page, Some(task));
                if keep(s)? {
                    tasks.push(s);
                }
            }
        }
        tasks.sort_by(|a, b| sort(*a, *b));
        tasks.truncate(limit);
        // Grouped by page, in the order the pages first appear.
        let mut groups: Vec<(PageRef, Vec<Task>)> = Vec::new();
        for s in tasks {
            let task = s.task.expect("task scopes have a task").clone();
            match groups.iter_mut().find(|(page, _)| page.rel == s.page.rel) {
                Some((_, list)) => list.push(task),
                None => groups.push((PageRef::of(s.page), vec![task])),
            }
        }
        return Ok(Results::Tasks(groups));
    }

    let mut kept = Vec::new();
    for page in pages {
        let s = scope(page, None);
        if keep(s)? {
            kept.push(s);
        }
    }
    kept.sort_by(|a, b| sort(*a, *b));
    kept.truncate(limit);
    match &query.kind {
        Kind::List(value) => Ok(Results::List(
            kept.into_iter()
                .map(|s| {
                    let v = value.as_ref().map(|e| eval(e, s)).transpose()?;
                    Ok((PageRef::of(s.page), v))
                })
                .collect::<Result<_, String>>()?,
        )),
        Kind::Table {
            columns,
            without_id,
        } => {
            let rows = kept
                .into_iter()
                .map(|s| {
                    let cells = columns
                        .iter()
                        .map(|(e, _)| eval(e, s))
                        .collect::<Result<_, _>>()?;
                    Ok((PageRef::of(s.page), cells))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Results::Table {
                id: (!without_id).then(|| "File".to_string()),
                headers: columns.iter().map(|(_, h)| h.clone()).collect(),
                rows,
            })
        }
        Kind::Task => unreachable!("handled above"),
    }
}

/// Whether `page` is in `source`.
pub fn matches(source: &Source, page: &Page) -> bool {
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
            let name = name.rsplit('/').next().unwrap_or(name).to_lowercase();
            page.links_to(&name)
        }
        Source::And(a, b) => matches(a, page) && matches(b, page),
        Source::Or(a, b) => matches(a, page) || matches(b, page),
        Source::Not(a) => !matches(a, page),
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
    })
}

fn binary(op: Op, a: Value, b: Value) -> Result<Value, String> {
    use Value::{Date, Null, Number, Text};
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
            (Op::Div, Number(_), Number(0.0)) => Null,
            (Op::Div, Number(x), Number(y)) => Number(x / y),
            (Op::Add, Text(x), y) => Text(format!("{x}{}", y.display())),
            (Op::Add, x, Text(y)) => Text(format!("{}{y}", x.display())),
            // Days between dates, and a date plus or minus days.
            (Op::Sub, Date(x), Date(y)) => Number((x - y).num_days() as f64),
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

/// A field in scope: task fields, `file.*`, `this.…`, then page fields.
fn field(path: &[String], scope: Scope) -> Value {
    let first = path[0].as_str();
    if first == "this" {
        return match scope.this {
            Some(this) if path.len() > 1 => field(
                &path[1..],
                Scope {
                    page: this,
                    task: None,
                    ..scope
                },
            ),
            _ => Value::Null,
        };
    }
    if let (Some(task), [name]) = (scope.task, path) {
        match name.as_str() {
            "text" => return Value::Text(task.text.clone()),
            "completed" | "checked" => return Value::Bool(task.completed()),
            "status" => return Value::Text(task.status.to_string()),
            "line" => return Value::Number(task.line as f64),
            _ => {}
        }
    }
    let page = scope.page;
    if first == "file" && path.len() == 2 {
        let date = |d: Option<chrono::NaiveDateTime>| d.map_or(Value::Null, Value::Date);
        return match path[1].as_str() {
            "name" => Value::Text(page.name.clone()),
            "path" => Value::Text(page.rel.clone()),
            "folder" => Value::Text(page.folder.clone()),
            "link" => Value::Link(page.rel.trim_end_matches(".md").to_string()),
            "ext" => Value::Text("md".into()),
            "size" => Value::Number(page.size as f64),
            "mtime" | "mday" => date(page.mtime),
            "ctime" | "cday" => date(page.ctime),
            "day" => date(page.day),
            "tags" | "etags" => Value::List(page.tags.iter().cloned().map(Value::Text).collect()),
            "outlinks" => Value::List(page.outlinks.iter().cloned().map(Value::Link).collect()),
            "inlinks" => Value::List(
                scope
                    .index
                    .inlinks(page)
                    .map(|p| Value::Link(p.name.clone()))
                    .collect(),
            ),
            "tasks" => Value::List(
                page.tasks
                    .iter()
                    .map(|t| Value::Text(t.text.clone()))
                    .collect(),
            ),
            _ => Value::Null,
        };
    }
    // A key may itself contain dots.
    page.field(&path.join(".")).cloned().unwrap_or(Value::Null)
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
    Ok(match name {
        "date" => {
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
        "contains" | "icontains" => {
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
                    _ => i.equals(x),
                }),
                (Value::Text(t), x) => fold(t).contains(&fold(&x.display())),
                (Value::Link(l), x) => fold(l).contains(&fold(&x.display())),
                _ => false,
            })
        }
        "length" => {
            want(1)?;
            match &values()?[0] {
                Value::List(l) => Value::Number(l.len() as f64),
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
        "default" => {
            want(2)?;
            let v = values()?;
            if v[0] == Value::Null {
                v[1].clone()
            } else {
                v[0].clone()
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
        "round" => {
            let v = values()?;
            let digits = match v.get(1) {
                Some(Value::Number(d)) => *d as i32,
                _ => 0,
            };
            match v.first() {
                Some(Value::Number(n)) => {
                    let f = 10f64.powi(digits);
                    Value::Number((n * f).round() / f)
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
        "string" => {
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
        "sum" => {
            want(1)?;
            match &values()?[0] {
                Value::List(l) => Value::Number(
                    l.iter()
                        .filter_map(|v| {
                            if let Value::Number(n) = v {
                                Some(n)
                            } else {
                                None
                            }
                        })
                        .sum(),
                ),
                other => other.clone(),
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
        other => return Err(format!("unknown function {other}()")),
    })
}

/// Formats with Luxon-style tokens as Dataview does (`yyyy-MM-dd`,
/// `dd.MM.yyyy`, `MMMM d`, `EEE`, `HH:mm`).
fn format_date(d: &chrono::NaiveDateTime, format: &str) -> String {
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
    d.format(&out).to_string()
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
            Results::List(rows) => rows
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
            .map(|(p, t)| (p.name, t.into_iter().map(|t| t.text).collect()))
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
        let Results::List(rows) = run(&q, &ix, this).unwrap() else {
            panic!("a list")
        };
        let names: Vec<_> = rows.into_iter().map(|(n, _)| n.name).collect();
        assert_eq!(names, ["2026-08-09"]);
    }
}
