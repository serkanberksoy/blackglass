//! Draws query results as the lines of a block: a bulleted list (a
//! group's rows under its key), a table with a header row, tasks under
//! their page's name (or their group's key), or a month calendar.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use chrono::NaiveTime;

use super::eval::Results;
use super::value::Value;
use crate::plugins::moment;

/// How results look (the plugin's settings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    /// moment.js format for dates without a time.
    pub date_format: String,
    pub datetime_format: String,
    /// Shown when a query finds nothing.
    pub no_results: String,
    /// The header of a table's page column.
    pub id_header: String,
    /// A click or Enter on a task checks it off (else opens it at its
    /// line).
    pub check_tasks: bool,
}

impl Default for Display {
    fn default() -> Self {
        Display {
            date_format: "YYYY-MM-DD".into(),
            datetime_format: "YYYY-MM-DD HH:mm".into(),
            no_results: "No results".into(),
            id_header: "File".into(),
            check_tasks: true,
        }
    }
}

/// A value as a result shows it: dates in the set formats.
fn show(value: &Value, d: &Display) -> String {
    match value {
        Value::Date(t) if t.time() == NaiveTime::MIN => moment::format(t, &d.date_format),
        Value::Date(t) => moment::format(t, &d.datetime_format),
        Value::List(items) => items
            .iter()
            .map(|v| show(v, d))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.display(),
    }
}

/// Page names in results, like links.
/// Links: the editor's own link color (a theme can change it).
pub(super) const LINK: Style = Style::new().fg(Color::LightBlue);
pub(super) const DIM: Style = Style::new().fg(Color::DarkGray);
const ERROR: Style = Style::new().fg(Color::LightRed);

/// A row's action: open a page (its path in the vault).
pub fn open_action(rel: &str) -> String {
    format!("open:{rel}")
}

/// A row's action: open a page at a line (a task's).
pub fn line_action(rel: &str, line: usize) -> String {
    format!("line:{line}:{rel}")
}

/// A task row's action: check it off (the plugin does), or open it.
pub fn task_action(rel: &str, line: usize, d: &Display) -> String {
    if d.check_tasks {
        format!("plugin:dataview:toggle:{line}:{rel}")
    } else {
        line_action(rel, line)
    }
}

/// A value as an inline query shows it.
pub fn inline(value: &Value, d: &Display) -> String {
    show(value, d)
}

/// The lines for `results`, `width` columns wide (tables are fitted).
pub fn results(results: &Results, width: usize, d: &Display) -> Vec<Line<'static>> {
    rows(results, width, d)
        .into_iter()
        .map(|(l, _)| l)
        .collect()
}

/// [`results`], each line with its action: a page's row opens it, a
/// task's row opens its note at the task.
pub fn rows(results: &Results, width: usize, d: &Display) -> Vec<(Line<'static>, Option<String>)> {
    let none = |lines: Vec<Line<'static>>| lines.into_iter().map(|l| (l, None)).collect();
    match results {
        Results::List { rows, .. } if rows.is_empty() => none(nothing(d)),
        Results::List { without_id, rows } => {
            let mut lines = Vec::new();
            for (page, value) in rows {
                let action = (!page.rel.is_empty()).then(|| open_action(&page.rel));
                // A group without a value: its key, its rows under it.
                if page.rel.is_empty()
                    && let Some(Value::List(items)) = value
                    && items.iter().all(|i| matches!(i, Value::Link(_)))
                    && !without_id
                {
                    lines.push((
                        Line::from(vec![
                            Span::raw("• "),
                            Span::styled(
                                page.name.clone(),
                                Style::new().add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        None,
                    ));
                    for item in items {
                        let Value::Link(l) = item else { continue };
                        lines.push((
                            Line::from(vec![Span::raw("  • "), Span::styled(item.display(), LINK)]),
                            Some(open_action(&format!("{l}.md"))),
                        ));
                    }
                    continue;
                }
                let line = match (without_id, value) {
                    (true, Some(v)) => Line::from(vec![Span::raw("• "), Span::raw(show(v, d))]),
                    (_, value) => {
                        let style = if page.rel.is_empty() {
                            Style::new()
                        } else {
                            LINK
                        };
                        let mut spans =
                            vec![Span::raw("• "), Span::styled(page.name.clone(), style)];
                        if let Some(v) = value {
                            spans.push(Span::raw(format!(": {}", show(v, d))));
                        }
                        Line::from(spans)
                    }
                };
                lines.push((line, action));
            }
            lines
        }
        Results::Calendar(days) if days.is_empty() => none(nothing(d)),
        Results::Calendar(days) => calendar(days),
        Results::Table { rows, .. } if rows.is_empty() => none(nothing(d)),
        Results::Table { id, headers, rows } => {
            // The pages' column is named by the settings; a group's by it.
            let id = id
                .as_deref()
                .map(|h| if h == "File" { d.id_header.as_str() } else { h });
            let named: Vec<(String, Vec<Value>)> = rows
                .iter()
                .map(|(p, cells)| (p.name.clone(), cells.clone()))
                .collect();
            let lines = table(id, headers, &named, width, d);
            // The header and its rule have no action; each row opens its page.
            let actions = [None, None].into_iter().chain(
                rows.iter()
                    .map(|(p, _)| (!p.rel.is_empty()).then(|| open_action(&p.rel))),
            );
            lines.into_iter().zip(actions).collect()
        }
        Results::Tasks(groups) if groups.is_empty() => none(nothing(d)),
        Results::Tasks(groups) => {
            let mut lines = Vec::new();
            for (page, tasks) in groups {
                let heading = if page.rel.is_empty() {
                    Style::new().add_modifier(Modifier::BOLD)
                } else {
                    LINK.add_modifier(Modifier::BOLD)
                };
                lines.push((
                    Line::from(Span::styled(page.name.clone(), heading)),
                    (!page.rel.is_empty()).then(|| open_action(&page.rel)),
                ));
                for (rel, task) in tasks {
                    let (mark, style) = match task.status {
                        ' ' => ("☐".to_string(), Style::new()),
                        'x' | 'X' => ("☑".to_string(), DIM.add_modifier(Modifier::CROSSED_OUT)),
                        c => (format!("[{c}]"), Style::new()),
                    };
                    lines.push((
                        Line::from(vec![
                            Span::raw(format!("  {mark} ")),
                            Span::styled(task.text.clone(), style),
                        ]),
                        Some(task_action(rel, task.line, d)),
                    ));
                }
            }
            lines
        }
    }
}

/// `CALENDAR`: each month with notes as a grid (Monday first), its days
/// with notes marked, then those notes (a row each, to open).
fn calendar(
    days: &[(chrono::NaiveDate, super::eval::PageRef)],
) -> Vec<(Line<'static>, Option<String>)> {
    use chrono::Datelike;
    let mut lines = Vec::new();
    let mut months: Vec<(i32, u32)> = days.iter().map(|(d, _)| (d.year(), d.month())).collect();
    months.dedup();
    for (year, month) in months {
        let first = chrono::NaiveDate::from_ymd_opt(year, month, 1).expect("a month's first day");
        lines.push((
            Line::from(Span::styled(
                first.format("%B %Y").to_string(),
                Style::new().add_modifier(Modifier::BOLD),
            )),
            None,
        ));
        lines.push((
            Line::from(Span::styled("Mo  Tu  We  Th  Fr  Sa  Su", DIM)),
            None,
        ));
        let lead = first.weekday().num_days_from_monday() as usize;
        let mut week: Vec<Span<'static>> = vec![Span::raw("    ".repeat(lead))];
        let mut day = first;
        while day.month() == month {
            let has = days.iter().any(|(d, _)| *d == day);
            let cell = format!("{:>2}{} ", day.day(), if has { "•" } else { " " });
            week.push(if has {
                Span::styled(cell, LINK.add_modifier(Modifier::BOLD))
            } else {
                Span::raw(cell)
            });
            if day.weekday() == chrono::Weekday::Sun {
                lines.push((Line::from(std::mem::take(&mut week)), None));
            }
            day = day.succ_opt().expect("a next day");
        }
        if !week.is_empty() {
            lines.push((Line::from(week), None));
        }
        for (d, page) in days
            .iter()
            .filter(|(d, _)| (d.year(), d.month()) == (year, month))
        {
            lines.push((
                Line::from(vec![
                    Span::styled(format!("  {} ", d.format("%d")), DIM),
                    Span::styled(page.name.clone(), LINK),
                ]),
                (!page.rel.is_empty()).then(|| open_action(&page.rel)),
            ));
        }
    }
    lines
}

/// A query error, for the block.
pub fn error(message: &str) -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        format!("Dataview: {message}"),
        ERROR,
    ))]
}

fn nothing(d: &Display) -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(d.no_results.clone(), DIM))]
}

/// A table: header, rule, rows; columns share `width`, the widest giving
/// way first, and cut cells end in `…`.
pub(super) fn table(
    id: Option<&str>,
    headers: &[String],
    rows: &[(String, Vec<Value>)],
    width: usize,
    d: &Display,
) -> Vec<Line<'static>> {
    let mut head: Vec<String> = Vec::new();
    if let Some(id) = id {
        head.push(format!("{id} ({})", rows.len()));
    }
    head.extend(headers.iter().cloned());
    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|(name, cells)| {
            id.map(|_| name.clone())
                .into_iter()
                .chain(cells.iter().map(|v| show(v, d)))
                .collect()
        })
        .collect();
    let columns = head.len();
    let mut widths: Vec<usize> = (0..columns)
        .map(|c| {
            body.iter()
                .map(|r| r[c].width())
                .chain([head[c].width()])
                .max()
                .unwrap_or(1)
                .max(1)
        })
        .collect();
    // " │ " between columns.
    let room = width
        .saturating_sub(3 * columns.saturating_sub(1))
        .max(columns);
    while widths.iter().sum::<usize>() > room {
        let widest = (0..columns)
            .max_by_key(|&c| widths[c])
            .expect("a table has columns");
        if widths[widest] <= 3 {
            break;
        }
        widths[widest] -= 1;
    }
    let row = |cells: &[String], styles: &dyn Fn(usize) -> Style| {
        let mut spans = Vec::new();
        for (c, cell) in cells.iter().enumerate() {
            if c > 0 {
                spans.push(Span::styled(" │ ", DIM));
            }
            spans.push(Span::styled(pad(cell, widths[c]), styles(c)));
        }
        Line::from(spans)
    };
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let mut lines = vec![row(&head, &|_| bold)];
    let rule: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    lines.push(Line::from(Span::styled(rule.join("─┼─"), DIM)));
    let has_id = id.is_some();
    for cells in &body {
        lines.push(row(cells, &|c| {
            if c == 0 && has_id { LINK } else { Style::new() }
        }));
    }
    lines
}

/// `text` cut or padded to exactly `width` columns.
fn pad(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    if text.width() > width {
        for c in text.chars() {
            let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if used + w + 1 > width {
                break;
            }
            out.push(c);
            used += w;
        }
        out.push('…');
        used += 1;
    } else {
        out.push_str(text);
        used = text.width();
    }
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::dataview::eval::PageRef;
    use crate::plugins::dataview::index::Task;

    fn page(name: &str) -> PageRef {
        PageRef {
            name: name.into(),
            rel: format!("Books/{name}.md"),
        }
    }

    fn text(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
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
    fn lists_and_tasks() {
        let list = Results::List {
            without_id: false,
            rows: vec![
                (page("Dune"), Some(Value::Number(5.0))),
                (page("Emma"), None),
            ],
        };
        assert_eq!(
            text(&results(&list, 40, &Display::default())),
            ["• Dune: 5", "• Emma"]
        );
        let tasks = Results::Tasks(vec![(
            page("Dune"),
            vec![
                (
                    "Books/Dune.md".into(),
                    Task {
                        line: 0,
                        text: "one".into(),
                        status: ' ',
                        is_task: true,
                        ..Task::default()
                    },
                ),
                (
                    "Books/Dune.md".into(),
                    Task {
                        line: 1,
                        text: "two".into(),
                        status: 'x',
                        is_task: true,
                        ..Task::default()
                    },
                ),
                (
                    "Books/Dune.md".into(),
                    Task {
                        line: 2,
                        text: "three".into(),
                        status: '/',
                        is_task: true,
                        ..Task::default()
                    },
                ),
            ],
        )]);
        assert_eq!(
            text(&results(&tasks, 40, &Display::default())),
            ["Dune", "  ☐ one", "  ☑ two", "  [/] three"]
        );
        assert_eq!(
            text(&results(
                &Results::List {
                    without_id: false,
                    rows: Vec::new()
                },
                40,
                &Display::default()
            )),
            ["No results"]
        );
    }

    #[test]
    fn tables_fit_the_width() {
        let t = Results::Table {
            id: Some("File".into()),
            headers: vec!["author".into()],
            rows: vec![(page("Dune"), vec![Value::Text("Frank Herbert".into())])],
        };
        assert_eq!(
            text(&results(&t, 40, &Display::default())),
            [
                "File (1) │ author",
                "─────────┼──────────────",
                "Dune     │ Frank Herbert"
            ]
        );
        assert_eq!(
            text(&results(&t, 16, &Display::default())),
            ["File (… │ author", "────────┼───────", "Dune    │ Frank…"],
            "the widest columns give way first"
        );
    }

    #[test]
    fn a_calendar_month() {
        let day = |d: u32| chrono::NaiveDate::from_ymd_opt(2026, 8, d).unwrap();
        let cal = Results::Calendar(vec![(day(9), page("Dune")), (day(10), page("Emma"))]);
        let rows = rows(&cal, 40, &Display::default());
        let lines = text(&rows.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>());
        assert_eq!(lines[0], "August 2026");
        assert_eq!(lines[1], "Mo  Tu  We  Th  Fr  Sa  Su");
        // August 1st, 2026 is a Saturday.
        assert_eq!(lines[2], "                     1   2");
        assert_eq!(lines[3], " 3   4   5   6   7   8   9•");
        assert!(lines[4].starts_with("10• 11"), "{:?}", lines[4]);
        let notes: Vec<_> = rows
            .iter()
            .filter(|(_, a)| a.is_some())
            .map(|(l, a)| {
                (
                    l.spans
                        .iter()
                        .map(|s| s.content.as_ref())
                        .collect::<String>(),
                    a.clone().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            notes,
            [
                ("  09 Dune".to_string(), "open:Books/Dune.md".to_string()),
                ("  10 Emma".to_string(), "open:Books/Emma.md".to_string())
            ]
        );
    }
}
