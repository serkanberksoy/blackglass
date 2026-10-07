//! A query's results as lines (TK-38 … TK-44): filtered, sorted (the
//! query's keys, then Tasks' default order), grouped under headings,
//! limited, each task drawn with the fields the layout shows, and a
//! toggle action per task row.

use std::cmp::Ordering;
use std::collections::HashSet;

use chrono::NaiveDate;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::query::{DateKey, Env, Key, PathPart, Query, path_part};
use super::task::{DateField, Priority, Task, Type};

const DIM: Style = Style::new().fg(Color::DarkGray);
/// Group headings: the theme's accent.
fn heading_style() -> Style {
    Style::new()
        .fg(crate::ui::theme::paint(crate::ui::theme::ACCENT))
        .add_modifier(Modifier::BOLD)
}

/// A result row: the line and the actions on its parts (a task's box
/// checks it, its backlink opens its note there).
pub type Row = mdedit::processor::CellRow;

/// The action a task row runs: toggle it.
pub fn toggle_action(t: &Task) -> String {
    format!(
        "plugin:tasks:toggle:{}:{}",
        t.line,
        t.path.to_string_lossy().replace('\\', "/")
    )
}

/// Runs `query` over `tasks`.
pub fn run(
    query: &Query,
    tasks: &[Task],
    env: &Env,
    remove_tag: Option<&str>,
    width: usize,
) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    if query.explain {
        out.push((
            Line::styled("Explanation of this Tasks query:", heading_style()),
            Vec::new(),
        ));
        for f in &query.filters {
            out.push((Line::styled(format!("  {}", f.explain()), DIM), Vec::new()));
        }
        if query.filters.is_empty() {
            out.push((Line::styled("  No filters: every task", DIM), Vec::new()));
        }
        out.push((Line::default(), Vec::new()));
    }
    let mut found: Vec<&Task> = tasks
        .iter()
        .filter(|t| query.filters.iter().all(|f| f.matches(t, env)))
        .filter(|t| !query.exclude_sub || t.parent.is_none())
        .collect();
    found.sort_by(|a, b| {
        for (key, reverse) in &query.sorts {
            let order = compare(*key, a, b, env);
            let order = if *reverse { order.reverse() } else { order };
            if order != Ordering::Equal {
                return order;
            }
        }
        default_order(a, b, env)
    });
    if let Some(limit) = query.limit {
        found.truncate(limit);
    }
    let total = found.len();
    let tree = !query.hidden.contains("tree");
    if tree {
        // A task under another one shown is drawn with it, not again.
        let shown: HashSet<(&std::path::Path, usize)> =
            found.iter().map(|t| (t.path.as_path(), t.line)).collect();
        found.retain(|t| {
            t.parent
                .is_none_or(|p| !shown.contains(&(t.path.as_path(), p)))
        });
    }
    if let Some(key) = query.columns {
        out.extend(columns(&found, key, query, env, width));
    } else if query.groups.is_empty() {
        let mut shown: Vec<&Task> = found;
        if let Some(n) = query.group_limit {
            shown.truncate(n);
        }
        for t in shown {
            push_task(&mut out, t, tasks, 0, tree, query, env, remove_tag);
        }
    } else {
        grouped(&mut out, &found, &query.groups, 0, query, env, remove_tag);
    }
    if !query.hidden.contains("task count") {
        let noun = if total == 1 { "task" } else { "tasks" };
        out.push((Line::styled(format!("{total} {noun}"), DIM), Vec::new()));
    }
    out
}

/// The tasks under headings, by `groups[level]` and deeper.
fn grouped(
    out: &mut Vec<Row>,
    tasks: &[&Task],
    groups: &[(Key, bool)],
    level: usize,
    query: &Query,
    env: &Env,
    remove_tag: Option<&str>,
) {
    let Some(&(key, reverse)) = groups.get(level) else {
        let limit = query.group_limit.unwrap_or(usize::MAX);
        out.extend(
            tasks
                .iter()
                .take(limit)
                .map(|t| task_row(t, query, env, remove_tag, 0)),
        );
        return;
    };
    // (sort key, heading) → tasks, in first-seen order within a group.
    let mut names: Vec<(String, String)> = Vec::new();
    let mut members: Vec<Vec<&Task>> = Vec::new();
    for t in tasks {
        for name in group_names(key, t, env) {
            match names.iter().position(|n| *n == name) {
                Some(i) => members[i].push(t),
                None => {
                    names.push(name);
                    members.push(vec![t]);
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..names.len()).collect();
    order.sort_by(|&a, &b| {
        let o = names[a].0.cmp(&names[b].0);
        if reverse { o.reverse() } else { o }
    });
    for i in order {
        let heading = format!("{}{}", "  ".repeat(level), names[i].1);
        out.push((Line::styled(heading, heading_style()), Vec::new()));
        grouped(out, &members[i], groups, level + 1, query, env, remove_tag);
    }
}

/// The groups a task is in for `key`: (sort key, heading). Tags put it
/// in one group per tag.
fn group_names(key: Key, t: &Task, env: &Env) -> Vec<(String, String)> {
    let status = env.statuses.get(t.status);
    let one = |sort: String, name: String| vec![(sort, name)];
    match key {
        // A function's text, or each of its list's.
        Key::Function(i) => env
            .functions
            .map(|c| super::function::headings(c.get(t, i)))
            .unwrap_or_default()
            .into_iter()
            .map(|h| (h.to_lowercase(), h))
            .collect(),
        Key::Status => {
            let done = matches!(status.kind, Type::Done | Type::Cancelled | Type::NonTask);
            if done {
                one("2".into(), "Done".into())
            } else {
                one("1".into(), "Todo".into())
            }
        }
        Key::StatusName => one(status.name.clone(), status.name),
        Key::StatusType => one(
            format!("{}", status.kind as u8),
            status.kind.name().to_string(),
        ),
        Key::Priority => {
            let name = match t.priority {
                Priority::None => "Normal".to_string(),
                p => capitalize(p.name()),
            };
            one(format!("{}", t.priority as u8), format!("{name} priority"))
        }
        Key::Urgency => {
            let u = t.urgency(env.today);
            one(format!("{:012.2}", 1000.0 - u), format!("{u:.2}"))
        }
        Key::Date(k) => match date_of(k, t) {
            Some(d) => one(d.to_string(), d.format("%Y-%m-%d %A").to_string()),
            None => one("~".into(), format!("No {} date", date_name(k))),
        },
        Key::Description => one(t.description.to_lowercase(), t.description.clone()),
        Key::Path => {
            let p = path_part(t, PathPart::Path);
            let p = p.strip_suffix(".md").unwrap_or(&p).to_string();
            one(p.to_lowercase(), p)
        }
        Key::Root => part(t, PathPart::Root),
        Key::Folder => part(t, PathPart::Folder),
        Key::Filename => part(t, PathPart::Filename),
        Key::Heading => match &t.heading {
            Some(h) => one(h.to_lowercase(), h.clone()),
            None => one("~".into(), "(No heading)".into()),
        },
        Key::Backlink => {
            let name = backlink(t);
            one(name.to_lowercase(), name)
        }
        Key::Tags if t.tags.is_empty() => one("~".into(), "(No tags)".into()),
        Key::Tags => t
            .tags
            .iter()
            .map(|g| (g.to_lowercase(), g.clone()))
            .collect(),
        Key::Recurring => match t.recurrence {
            Some(_) => one("1".into(), "Recurring".into()),
            None => one("2".into(), "Not Recurring".into()),
        },
        Key::Recurrence => match &t.recurrence {
            Some(r) => one(r.clone(), r.clone()),
            None => one("~".into(), "None".into()),
        },
        Key::Id => match &t.id {
            Some(id) => one(id.clone(), id.clone()),
            None => one("~".into(), "No id".into()),
        },
    }
}

fn part(t: &Task, p: PathPart) -> Vec<(String, String)> {
    let name = path_part(t, p);
    vec![(name.to_lowercase(), name)]
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn date_of(k: DateKey, t: &Task) -> Option<NaiveDate> {
    match k {
        DateKey::Field(f) => t.date(f),
        DateKey::Happens => t.happens(),
    }
}

fn date_name(k: DateKey) -> &'static str {
    match k {
        DateKey::Field(f) => f.name(),
        DateKey::Happens => "happens",
    }
}

/// `Note > Heading`, or the note alone.
fn backlink(t: &Task) -> String {
    match &t.heading {
        Some(h) => format!("{} > {h}", t.filename()),
        None => t.filename(),
    }
}

fn compare(key: Key, a: &Task, b: &Task, env: &Env) -> Ordering {
    let status = |t: &Task| env.statuses.get(t.status);
    // Tasks without the date go last.
    let dates = |x: Option<NaiveDate>, y: Option<NaiveDate>| match (x, y) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    match key {
        Key::Function(i) => env.functions.map_or(Ordering::Equal, |c| {
            super::function::compare(c.get(a, i), c.get(b, i))
        }),
        Key::Status => {
            let done =
                |t: &Task| matches!(status(t).kind, Type::Done | Type::Cancelled | Type::NonTask);
            done(a).cmp(&done(b))
        }
        Key::StatusName => status(a).name.cmp(&status(b).name),
        Key::StatusType => status(a).kind.cmp(&status(b).kind),
        Key::Priority => a.priority.cmp(&b.priority),
        Key::Urgency => b.urgency(env.today).total_cmp(&a.urgency(env.today)),
        Key::Date(k) => dates(date_of(k, a), date_of(k, b)),
        Key::Description => a
            .description
            .to_lowercase()
            .cmp(&b.description.to_lowercase()),
        Key::Path | Key::Root | Key::Folder | Key::Backlink => {
            a.path.cmp(&b.path).then(a.line.cmp(&b.line))
        }
        Key::Filename => a
            .filename()
            .to_lowercase()
            .cmp(&b.filename().to_lowercase()),
        Key::Heading => a.heading.cmp(&b.heading),
        Key::Tags => a.tags.first().cmp(&b.tags.first()),
        Key::Recurring => b.recurrence.is_some().cmp(&a.recurrence.is_some()),
        Key::Recurrence => a.recurrence.cmp(&b.recurrence),
        Key::Id => a.id.cmp(&b.id),
    }
}

/// Tasks' default order: status type, urgency, due, priority, path.
fn default_order(a: &Task, b: &Task, env: &Env) -> Ordering {
    let kind = |t: &Task| env.statuses.get(t.status).kind;
    kind(a)
        .cmp(&kind(b))
        .then_with(|| compare(Key::Urgency, a, b, env))
        .then_with(|| compare(Key::Date(DateKey::Field(DateField::Due)), a, b, env))
        .then_with(|| a.priority.cmp(&b.priority))
        .then_with(|| a.path.cmp(&b.path))
        .then_with(|| a.line.cmp(&b.line))
}

/// A task, and with `tree` the tasks nested under it (from all of
/// `tasks`, shown or not), indented.
#[expect(
    clippy::too_many_arguments,
    reason = "the drawing context, passed down"
)]
fn push_task(
    out: &mut Vec<Row>,
    t: &Task,
    tasks: &[Task],
    depth: usize,
    tree: bool,
    query: &Query,
    env: &Env,
    remove_tag: Option<&str>,
) {
    out.push(task_row(t, query, env, remove_tag, depth));
    if !tree || depth > 8 {
        return;
    }
    for child in tasks
        .iter()
        .filter(|c| c.path == t.path && c.parent == Some(t.line))
    {
        push_task(out, child, tasks, depth + 1, tree, query, env, remove_tag);
    }
}

/// The tasks in columns side by side, one per group of `key` (TK-47).
fn columns(found: &[&Task], key: Key, query: &Query, env: &Env, width: usize) -> Vec<Row> {
    let mut names: Vec<(String, String)> = Vec::new();
    let mut members: Vec<Vec<&Task>> = Vec::new();
    for t in found {
        for name in group_names(key, t, env) {
            match names.iter().position(|n| *n == name) {
                Some(i) => members[i].push(t),
                None => {
                    names.push(name);
                    members.push(vec![t]);
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..names.len()).collect();
    order.sort_by(|&a, &b| names[a].0.cmp(&names[b].0));
    if order.is_empty() {
        return Vec::new();
    }
    let n = order.len();
    let w = (width.saturating_sub(3 * (n - 1)) / n).clamp(6, 30);
    let fit = |s: &str| {
        let mut out: String = s.chars().take(w).collect();
        if s.chars().count() > w {
            out.pop();
            out.push('…');
        }
        let pad = w.saturating_sub(unicode_width::UnicodeWidthStr::width(out.as_str()));
        format!("{out}{}", " ".repeat(pad))
    };
    let drawn: Vec<Vec<String>> = order
        .iter()
        .map(|&i| {
            let mut col = vec![
                fit(&format!("{} ({})", names[i].1, members[i].len())),
                "─".repeat(w),
            ];
            for t in &members[i] {
                let row = task_row(t, query, env, None, 0).0;
                let text: String = row.spans.iter().map(|s| s.content.as_ref()).collect();
                col.push(fit(&text));
            }
            col
        })
        .collect();
    let height = drawn.iter().map(Vec::len).max().unwrap_or(0);
    (0..height)
        .map(|r| {
            let parts: Vec<String> = drawn
                .iter()
                .map(|c| c.get(r).cloned().unwrap_or_else(|| " ".repeat(w)))
                .collect();
            let style = if r == 0 {
                heading_style()
            } else {
                Style::new()
            };
            (
                Line::styled(parts.join(" │ ").trim_end().to_string(), style),
                Vec::new(),
            )
        })
        .collect()
}

/// One task: its checkbox, description and the fields the layout shows,
/// indented `depth` levels (a tree).
fn task_row(t: &Task, query: &Query, env: &Env, remove_tag: Option<&str>, depth: usize) -> Row {
    let status = env.statuses.get(t.status);
    let shown = |what: &str| !query.hidden.contains(what);
    let done = matches!(status.kind, Type::Done | Type::Cancelled);
    let glyph = match t.status {
        ' ' => "☐".to_string(),
        'x' | 'X' => "☑".to_string(),
        '-' => "☒".to_string(),
        '/' => "◐".to_string(),
        c => format!("[{c}]"),
    };
    let text_style = if done {
        DIM.add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::new()
    };
    let mut description = t.description.clone();
    if let Some(tag) = remove_tag {
        description = description
            .split_whitespace()
            .filter(|w| *w != tag)
            .collect::<Vec<_>>()
            .join(" ");
    }
    if !shown("tag") {
        description = description
            .split_whitespace()
            .filter(|w| !w.starts_with('#'))
            .collect::<Vec<_>>()
            .join(" ");
    }
    let mut spans = vec![
        Span::raw(format!("{}{glyph} ", "  ".repeat(depth))),
        Span::styled(description, text_style),
    ];
    let mut field = |text: String| spans.push(Span::styled(format!(" {text}"), DIM));
    if t.priority != Priority::None && shown("priority") {
        field(t.priority.emoji().to_string());
    }
    if let Some(r) = &t.recurrence
        && shown("recurrence rule")
    {
        field(if query.short {
            "🔁".into()
        } else {
            format!("🔁 {r}")
        });
    }
    for (f, _, name, _) in DateField::ALL {
        if let Some(d) = t.date(f)
            && shown(&format!("{name} date"))
        {
            field(if query.short {
                f.emoji().to_string()
            } else {
                format!("{} {d}", f.emoji())
            });
        }
    }
    if let Some(id) = &t.id
        && shown("id")
    {
        field(format!("🆔 {id}"));
    }
    if !t.depends.is_empty() && shown("depends on") {
        field(format!("⛔ {}", t.depends.join(",")));
    }
    if let Some(c) = &t.on_completion
        && shown("on completion")
    {
        field(format!("🏁 {c}"));
    }
    // The box checks the task; the backlink opens its note at it.
    let width = |spans: &[Span]| spans.iter().map(|s| s.content.width()).sum::<usize>();
    let box_from = depth * 2;
    let mut parts = vec![(box_from, box_from + glyph.width(), toggle_action(t))];
    if shown("backlink") {
        let before = width(&spans) + 1;
        let shown = format!("({})", backlink(t));
        parts.push((
            before,
            before + shown.width(),
            crate::plugins::dataview::render::line_action(
                &t.path.to_string_lossy().replace('\\', "/"),
                t.line,
            ),
        ));
        spans.push(Span::styled(format!(" {shown}"), DIM));
    }
    if !query.hidden.contains("urgency") {
        spans.push(Span::styled(format!(" ⚡{:.2}", t.urgency(env.today)), DIM));
    }
    (Line::from(spans), parts)
}

/// Ids of open tasks, and ids open tasks depend on (for "blocked" and
/// "blocking").
pub fn dependencies(
    tasks: &[Task],
    env_statuses: &super::task::Statuses,
) -> (HashSet<String>, HashSet<String>) {
    let open = |t: &&Task| {
        !matches!(
            env_statuses.get(t.status).kind,
            Type::Done | Type::Cancelled | Type::NonTask
        )
    };
    let open_ids = tasks
        .iter()
        .filter(open)
        .filter_map(|t| t.id.clone())
        .collect();
    let depended_on = tasks
        .iter()
        .filter(open)
        .flat_map(|t| t.depends.iter().cloned())
        .collect();
    (open_ids, depended_on)
}
