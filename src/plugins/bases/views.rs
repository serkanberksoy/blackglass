//! Running a base's view and drawing it (BA-40 … BA-50): the notes that
//! pass the filters, sorted, limited and grouped; shown as a table (with
//! summaries), a list, cards or a kanban board, under a line naming the
//! views and the result count.

use chrono::NaiveDateTime;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::expr::{self, Expr, Scope, Value};
use super::syntax::{Base, Filter, Kind, View};
use crate::plugins::dataview::index::Page;

const DIM: Style = Style::new().fg(Color::DarkGray);
/// The view shown, headings: the theme's accent.
fn accent() -> Style {
    Style::new()
        .fg(crate::ui::theme::paint(crate::ui::theme::ACCENT))
        .add_modifier(Modifier::BOLD)
}
const ERROR: Style = Style::new().fg(Color::LightRed);

/// A drawn line and its action (`open:<path>`).
pub type Row = (Line<'static>, Option<String>);

/// What a view runs on.
pub struct Input<'a> {
    pub base: &'a Base,
    pub view: usize,
    pub pages: &'a [Page],
    pub this: Option<usize>,
    pub now: NaiveDateTime,
    /// Only rows with a shown value containing this (BA-53), any case.
    pub search: Option<&'a str>,
    /// A board's selected card: (column, card), on a `.base` page.
    pub selected: Option<(usize, usize)>,
    /// A board's collapsed columns (their labels).
    pub collapsed: &'a [String],
}

/// A view's results: the notes, and their column values.
pub struct Results {
    /// Page indexes, in order.
    pub rows: Vec<usize>,
    /// The columns: property and header.
    pub columns: Vec<(String, String)>,
    /// `cells[r][c]`.
    pub cells: Vec<Vec<Value>>,
    /// Each row's group (`None` without `groupBy`).
    pub groups: Vec<Option<Value>>,
}

fn scope<'a>(input: &'a Input, row: usize) -> Scope<'a> {
    Scope {
        pages: input.pages,
        row: Some(row),
        this: input.this,
        formulas: &input.base.formulas,
        locals: Vec::new(),
        now: input.now,
        depth: 0,
    }
}

fn passes(f: &Filter, s: &mut Scope) -> Result<bool, String> {
    Ok(match f {
        Filter::Expr(e) => expr::eval(e, s)?.truthy(),
        Filter::And(all) => {
            for f in all {
                if !passes(f, s)? {
                    return Ok(false);
                }
            }
            true
        }
        Filter::Or(any) => {
            for f in any {
                if passes(f, s)? {
                    return Ok(true);
                }
            }
            false
        }
        Filter::Not(none) => {
            for f in none {
                if passes(f, s)? {
                    return Ok(false);
                }
            }
            true
        }
    })
}

/// Runs the view.
pub fn run(input: &Input) -> Result<Results, String> {
    let view = &input.base.views[input.view];
    let parse = |p: &str| expr::parse(p).map_err(|e| format!("property {p}: {e}"));
    let columns: Vec<(String, Expr)> = view
        .order
        .iter()
        .map(|p| Ok((p.clone(), parse(p)?)))
        .collect::<Result<_, String>>()?;
    let sorts: Vec<(Expr, bool)> = view
        .sort
        .iter()
        .map(|(p, desc)| Ok((parse(p)?, *desc)))
        .collect::<Result<_, String>>()?;
    let group = match &view.group_by {
        Some((p, desc)) => Some((parse(p)?, *desc)),
        None => None,
    };
    let mut rows = Vec::new();
    for i in 0..input.pages.len() {
        let mut s = scope(input, i);
        let base_ok = match &input.base.filters {
            Some(f) => passes(f, &mut s)?,
            None => true,
        };
        let view_ok = match &view.filters {
            Some(f) => passes(f, &mut s)?,
            None => true,
        };
        if base_ok && view_ok {
            rows.push(i);
        }
    }
    let value = |e: &Expr, i: usize| expr::eval(e, &mut scope(input, i));
    // Sort keys first (errors surface), then the order.
    let mut keyed: Vec<(usize, Vec<Value>, Option<Value>)> = Vec::new();
    for &i in &rows {
        let mut keys = Vec::new();
        for (e, _) in &sorts {
            keys.push(value(e, i)?);
        }
        let g = match &group {
            Some((e, _)) => Some(value(e, i)?),
            None => None,
        };
        keyed.push((i, keys, g));
    }
    let pages = input.pages;
    keyed.sort_by(|a, b| {
        if let (Some(ga), Some(gb), Some((_, desc))) = (&a.2, &b.2, &group) {
            let o = ga.compare(gb, pages);
            let o = if *desc { o.reverse() } else { o };
            if o.is_ne() {
                return o;
            }
        }
        for (k, (_, desc)) in sorts.iter().enumerate() {
            let o = a.1[k].compare(&b.1[k], pages);
            let o = if *desc { o.reverse() } else { o };
            if o.is_ne() {
                return o;
            }
        }
        pages[a.0]
            .name
            .to_lowercase()
            .cmp(&pages[b.0].name.to_lowercase())
    });
    if let Some(limit) = view.limit {
        keyed.truncate(limit);
    }
    let mut cells = Vec::new();
    for (i, _, _) in &keyed {
        let mut row = Vec::new();
        for (_, e) in &columns {
            row.push(value(e, *i)?);
        }
        cells.push(row);
    }
    if let Some(q) = input
        .search
        .map(str::to_lowercase)
        .filter(|q| !q.is_empty())
    {
        let hit = |row: &Vec<Value>, page: usize| {
            pages[page].name.to_lowercase().contains(&q)
                || row
                    .iter()
                    .any(|v| v.display(pages).to_lowercase().contains(&q))
        };
        let keep: Vec<bool> = cells
            .iter()
            .zip(&keyed)
            .map(|(row, (page, _, _))| hit(row, *page))
            .collect();
        let mut k = keep.iter();
        cells.retain(|_| *k.next().expect("one flag per row"));
        let mut k = keep.iter();
        keyed.retain(|_| *k.next().expect("one flag per row"));
    }
    Ok(Results {
        rows: keyed.iter().map(|k| k.0).collect(),
        groups: keyed.into_iter().map(|k| k.2).collect(),
        columns: columns
            .into_iter()
            .map(|(p, _)| {
                let header = input.base.display_name(&p);
                (p, header)
            })
            .collect(),
        cells,
    })
}

/// The error lines for `message`.
pub fn error(message: &str) -> Vec<Row> {
    vec![(Line::styled(format!("Bases: {message}"), ERROR), None)]
}

/// Draws the view in `width` columns.
pub fn draw(input: &Input, width: usize) -> Vec<Row> {
    let results = match run(input) {
        Ok(r) => r,
        Err(e) => return error(&e),
    };
    let view = &input.base.views[input.view];
    let mut out = vec![header(
        input.base,
        input.view,
        results.rows.len(),
        input.search,
    )];
    let body = match view.kind {
        Kind::Table => table(input, view, &results, width),
        Kind::List => list(input, view, &results),
        Kind::Cards => cards(input, view, &results, width),
        Kind::Kanban => kanban(input, view, &results, width),
    };
    match body {
        Ok(lines) => out.extend(lines),
        Err(e) => out.extend(error(&e)),
    }
    out
}

/// The views' names (the shown one highlighted) and the count.
fn header(base: &Base, shown: usize, count: usize, search: Option<&str>) -> Row {
    let mut spans = Vec::new();
    for (i, v) in base.views.iter().enumerate() {
        // An embed of one view names only it.
        if base.only.is_some() && i != shown {
            continue;
        }
        if i > 0 && base.only.is_none() {
            spans.push(Span::styled(" · ", DIM));
        }
        let style = if i == shown { accent() } else { DIM };
        spans.push(Span::styled(v.name.clone(), style));
    }
    let noun = if count == 1 { "result" } else { "results" };
    spans.push(Span::styled(format!("   {count} {noun}"), DIM));
    if let Some(q) = search.filter(|q| !q.is_empty()) {
        spans.push(Span::styled(format!("   search: {q}"), DIM));
    }
    (Line::from(spans), None)
}

/// A row's action: its menu, with the view's note properties to change.
fn open(page: &Page, props: &str) -> Option<String> {
    Some(format!("plugin:bases:row:{props}:{}", page.rel))
}

/// The view's note properties a row's menu can change (not file or
/// formula ones), the grouped one first.
pub fn editable(view: &View) -> Vec<String> {
    let mut props: Vec<String> = Vec::new();
    for p in view.group_by.iter().map(|(p, _)| p).chain(&view.order) {
        if p.starts_with("file.") || p.starts_with("formula.") {
            continue;
        }
        let p = p.strip_prefix("note.").unwrap_or(p);
        let plain = p
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        if plain && !props.iter().any(|q| q == p) {
            props.push(p.to_string());
        }
    }
    props
}

fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return format!("{text}{}", " ".repeat(width - text.width()));
    }
    let mut out = String::new();
    for c in text.chars() {
        if out.width() + c.to_string().width() + 1 > width {
            break;
        }
        out.push(c);
    }
    out.push('…');
    format!("{out}{}", " ".repeat(width.saturating_sub(out.width())))
}

/// The row indexes in each group, in order: (group value, rows).
pub(super) fn grouped(results: &Results) -> Vec<(Option<Value>, Vec<usize>)> {
    let mut out: Vec<(Option<Value>, Vec<usize>)> = Vec::new();
    for (r, g) in results.groups.iter().enumerate() {
        match out.last_mut() {
            Some((last, rows)) if last == g => rows.push(r),
            _ => out.push((g.clone(), vec![r])),
        }
    }
    if out.is_empty() {
        out.push((None, Vec::new()));
    }
    out
}

fn group_name(g: &Value, pages: &[Page]) -> String {
    match g.display(pages) {
        s if s.is_empty() => "(empty)".into(),
        s => s,
    }
}

/// A built-in or custom summary of `values`.
fn summary(input: &Input, name: &str, values: &[Value]) -> Result<String, String> {
    let pages = input.pages;
    let nums: Vec<f64> = values
        .iter()
        .filter_map(|v| match v {
            Value::Number(n) => Some(*n),
            _ => None,
        })
        .collect();
    let dates: Vec<NaiveDateTime> = values
        .iter()
        .filter_map(|v| match v {
            Value::Date(d) => Some(*d),
            _ => None,
        })
        .collect();
    let show = |n: Option<f64>| {
        n.map(|n| expr::number((n * 1000.0).round() / 1000.0))
            .unwrap_or_default()
    };
    let day = |d: Option<&NaiveDateTime>| {
        d.map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default()
    };
    Ok(match name {
        "Average" => show(expr::stat("mean", &nums)),
        "Sum" => show(expr::stat("sum", &nums)),
        "Median" => show(expr::stat("median", &nums)),
        "Stddev" => show(expr::stat("stddev", &nums)),
        "Min" => show(nums.iter().copied().reduce(f64::min)),
        "Max" => show(nums.iter().copied().reduce(f64::max)),
        "Range" if !dates.is_empty() => {
            let (a, b) = (dates.iter().min(), dates.iter().max());
            match (a, b) {
                (Some(a), Some(b)) => format!("{} days", (*b - *a).num_days()),
                _ => String::new(),
            }
        }
        "Range" => {
            let min = nums.iter().copied().reduce(f64::min);
            let max = nums.iter().copied().reduce(f64::max);
            show(max.zip(min).map(|(a, b)| a - b))
        }
        "Earliest" => day(dates.iter().min()),
        "Latest" => day(dates.iter().max()),
        "Checked" => values
            .iter()
            .filter(|v| **v == Value::Bool(true))
            .count()
            .to_string(),
        "Unchecked" => values
            .iter()
            .filter(|v| **v == Value::Bool(false))
            .count()
            .to_string(),
        "Empty" => values.iter().filter(|v| v.is_empty()).count().to_string(),
        "Filled" => values.iter().filter(|v| !v.is_empty()).count().to_string(),
        "Unique" => {
            let mut seen: Vec<String> = values.iter().map(|v| v.display(pages)).collect();
            seen.sort();
            seen.dedup();
            seen.len().to_string()
        }
        custom => {
            let Some((_, e)) = input.base.summaries.iter().find(|(n, _)| n == custom) else {
                return Err(format!("no summary {custom}"));
            };
            let mut s = Scope {
                pages,
                row: None,
                this: input.this,
                formulas: &input.base.formulas,
                locals: vec![("values".into(), Value::List(values.to_vec()))],
                now: input.now,
                depth: 0,
            };
            expr::eval(e, &mut s)?.display(pages)
        }
    })
}

/// The summaries row for `rows`, or `None` without summaries.
fn summaries(
    input: &Input,
    view: &View,
    results: &Results,
    rows: &[usize],
) -> Result<Option<Vec<String>>, String> {
    if view.summaries.is_empty() {
        return Ok(None);
    }
    let mut out = Vec::new();
    for (c, (property, _)) in results.columns.iter().enumerate() {
        let Some((_, name)) = view.summaries.iter().find(|(p, _)| p == property) else {
            out.push(String::new());
            continue;
        };
        let values: Vec<Value> = rows.iter().map(|&r| results.cells[r][c].clone()).collect();
        out.push(format!("{name} {}", summary(input, name, &values)?));
    }
    Ok(Some(out))
}

fn table(input: &Input, view: &View, results: &Results, width: usize) -> Result<Vec<Row>, String> {
    let props = editable(view).join(",");
    let pages = input.pages;
    let text: Vec<Vec<String>> = results
        .cells
        .iter()
        .map(|row| row.iter().map(|v| v.display(pages)).collect())
        .collect();
    let groups = grouped(results);
    let mut sums = Vec::new();
    for (_, rows) in &groups {
        sums.push(summaries(input, view, results, rows)?);
    }
    let all: Vec<usize> = (0..results.rows.len()).collect();
    let total = if groups.len() > 1 {
        summaries(input, view, results, &all)?
    } else {
        None
    };
    let n = results.columns.len();
    let mut widths: Vec<usize> = results.columns.iter().map(|(_, h)| h.width()).collect();
    for row in text.iter().chain(sums.iter().flatten()).chain(total.iter()) {
        for (c, cell) in row.iter().enumerate() {
            widths[c] = widths[c].max(cell.width());
        }
    }
    // Squeeze the widest columns until the table fits.
    let room = width.saturating_sub(3 * n.saturating_sub(1)).max(n);
    while widths.iter().sum::<usize>() > room {
        let (i, w) = widths
            .iter()
            .copied()
            .enumerate()
            .max_by_key(|(_, w)| *w)
            .expect("a table has columns");
        if w <= 3 {
            break;
        }
        widths[i] -= 1;
    }
    let line = |cells: &[String]| {
        cells
            .iter()
            .enumerate()
            .map(|(c, t)| fit(t, widths[c]))
            .collect::<Vec<_>>()
            .join(" │ ")
            .trim_end()
            .to_string()
    };
    let headers: Vec<String> = results.columns.iter().map(|(_, h)| h.clone()).collect();
    let mut out: Vec<Row> = vec![(
        Line::styled(line(&headers), Style::new().add_modifier(Modifier::BOLD)),
        None,
    )];
    let rule: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    out.push((Line::styled(rule.join("─┼─"), DIM), None));
    for ((g, rows), sum) in groups.iter().zip(&sums) {
        if let Some(g) = g {
            let heading = format!("{} ({})", group_name(g, pages), rows.len());
            out.push((Line::styled(heading, accent()), None));
        }
        for &r in rows {
            out.push((
                Line::raw(line(&text[r])),
                open(&pages[results.rows[r]], &props),
            ));
        }
        if let Some(sum) = sum {
            out.push((Line::styled(line(sum), DIM), None));
        }
    }
    if let Some(total) = total {
        out.push((Line::styled(line(&total), DIM), None));
    }
    Ok(out)
}

/// The row's values after the first (the title), as text.
fn details(results: &Results, r: usize, pages: &[Page]) -> Vec<(String, String)> {
    results
        .columns
        .iter()
        .zip(&results.cells[r])
        .skip(1)
        .map(|((_, h), v)| (h.clone(), v.display(pages)))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

fn title(results: &Results, r: usize, pages: &[Page]) -> String {
    match results.cells[r].first() {
        Some(v) if !v.display(pages).is_empty() => v.display(pages),
        _ => pages[results.rows[r]].name.clone(),
    }
}

fn list(input: &Input, view: &View, results: &Results) -> Result<Vec<Row>, String> {
    let props = editable(view).join(",");
    let pages = input.pages;
    let separator = view.option("separator").unwrap_or(", ");
    let indent = view.option("indentProperties") == Some("true");
    let mut out = Vec::new();
    for (g, rows) in grouped(results) {
        if let Some(g) = &g {
            out.push((
                Line::styled(
                    format!("{} ({})", group_name(g, pages), rows.len()),
                    accent(),
                ),
                None,
            ));
        }
        for (n, &r) in rows.iter().enumerate() {
            let marker = match view.option("markers") {
                Some("number") => format!("{}. ", n + 1),
                Some("none") => String::new(),
                _ => "• ".into(),
            };
            let action = open(&pages[results.rows[r]], &props);
            let values: Vec<String> = details(results, r, pages)
                .into_iter()
                .map(|(_, v)| v)
                .collect();
            if indent || values.is_empty() {
                out.push((
                    Line::raw(format!("{marker}{}", title(results, r, pages))),
                    action.clone(),
                ));
                if indent {
                    for (h, v) in details(results, r, pages) {
                        out.push((Line::styled(format!("    {h}: {v}"), DIM), action.clone()));
                    }
                }
            } else {
                let line = Line::from(vec![
                    Span::raw(format!("{marker}{}", title(results, r, pages))),
                    Span::styled(format!("{separator}{}", values.join(separator)), DIM),
                ]);
                out.push((line, action));
            }
        }
    }
    Ok(out)
}

/// A card's lines, `w` columns wide inside its border.
fn card(results: &Results, r: usize, pages: &[Page], w: usize) -> Vec<String> {
    let mut lines = vec![format!("╭{}╮", "─".repeat(w + 2))];
    lines.push(format!("│ {} │", fit(&title(results, r, pages), w)));
    for (h, v) in details(results, r, pages) {
        lines.push(format!("│ {} │", fit(&format!("{h}: {v}"), w)));
    }
    lines.push(format!("╰{}╯", "─".repeat(w + 2)));
    lines
}

fn cards(input: &Input, view: &View, results: &Results, width: usize) -> Result<Vec<Row>, String> {
    let props = editable(view).join(",");
    let pages = input.pages;
    let size: usize = view
        .option("cardSize")
        .and_then(|s| s.parse().ok())
        .unwrap_or(24)
        .clamp(8, width.saturating_sub(4).max(8));
    let per_line = (width / (size + 5)).max(1);
    let mut out = Vec::new();
    for (g, rows) in grouped(results) {
        if let Some(g) = &g {
            out.push((
                Line::styled(
                    format!("{} ({})", group_name(g, pages), rows.len()),
                    accent(),
                ),
                None,
            ));
        }
        for chunk in rows.chunks(per_line) {
            let drawn: Vec<Vec<String>> = chunk
                .iter()
                .map(|&r| card(results, r, pages, size))
                .collect();
            let height = drawn.iter().map(Vec::len).max().unwrap_or(0);
            // One card per line: its lines open it.
            let action = (chunk.len() == 1)
                .then(|| open(&pages[results.rows[chunk[0]]], &props))
                .flatten();
            for i in 0..height {
                let parts: Vec<String> = drawn
                    .iter()
                    .map(|c| c.get(i).cloned().unwrap_or_else(|| " ".repeat(size + 4)))
                    .collect();
                out.push((
                    Line::raw(parts.join(" ").trim_end().to_string()),
                    action.clone(),
                ));
            }
        }
    }
    Ok(out)
}

fn kanban(input: &Input, view: &View, results: &Results, width: usize) -> Result<Vec<Row>, String> {
    if view.group_by.is_none() {
        return Err("a kanban view needs groupBy (the property its columns are)".into());
    }
    let pages = input.pages;
    let columns = super::board::columns(view, results, pages);
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let n = columns.len();
    let w = view
        .option("columnWidth")
        .and_then(|s| s.parse().ok())
        .unwrap_or(24usize)
        .min((width.saturating_sub(3 * (n - 1)) / n).max(6));
    // `cardColor` (blackglass's): a property or formula naming a color.
    let card_color = match view.option("cardColor") {
        Some(p) => Some(expr::parse(p).map_err(|e| format!("cardColor {p}: {e}"))?),
        None => None,
    };
    let selected_bg = crate::ui::theme::paint(crate::ui::theme::BG_SELECTED);
    let mut drawn: Vec<Vec<Vec<Span<'static>>>> = Vec::new();
    for (c, column) in columns.iter().enumerate() {
        let tint = column.color.map_or_else(accent, |color| {
            Style::new().fg(color).add_modifier(Modifier::BOLD)
        });
        let collapsed = input.collapsed.iter().any(|l| l == column.label());
        let marker = if collapsed { "▸ " } else { "" };
        let mut head = tint;
        if input.selected.is_some_and(|(sc, _)| sc == c) {
            head = head.add_modifier(Modifier::UNDERLINED);
        }
        let heading = format!("{marker}{} ({})", column.label(), column.cards.len());
        let mut lines = vec![
            vec![Span::styled(fit(&heading, w), head)],
            vec![Span::styled(
                "─".repeat(w),
                column.color.map_or(DIM, |c| Style::new().fg(c)),
            )],
        ];
        if collapsed {
            drawn.push(lines);
            continue;
        }
        for (k, &r) in column.cards.iter().enumerate() {
            let bullet = match &card_color {
                Some(e) => expr::eval(e, &mut scope(input, results.rows[r]))
                    .ok()
                    .and_then(|v| super::board::color(&v.display(pages)))
                    .or(column.color),
                None => column.color,
            };
            let bullet_style = bullet.map_or(Style::new(), |c| Style::new().fg(c));
            let mut title_style = Style::new();
            let mut bullet_style = bullet_style;
            if input.selected == Some((c, k)) {
                title_style = title_style.bg(selected_bg).add_modifier(Modifier::BOLD);
                bullet_style = bullet_style.bg(selected_bg);
            }
            lines.push(vec![
                Span::styled("▪ ", bullet_style),
                Span::styled(
                    fit(&title(results, r, pages), w.saturating_sub(2)),
                    title_style,
                ),
            ]);
            for (_, v) in details(results, r, pages) {
                if Some(v.as_str()) != column.group.as_deref() {
                    lines.push(vec![Span::raw(fit(&format!("  {v}"), w))]);
                }
            }
        }
        drawn.push(lines);
    }
    let height = drawn.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..height {
        let mut spans = Vec::new();
        for (c, column) in drawn.iter().enumerate() {
            if c > 0 {
                spans.push(Span::styled(" │ ", DIM));
            }
            match column.get(i) {
                Some(cell) => spans.extend(cell.iter().cloned()),
                None => spans.push(Span::raw(" ".repeat(w))),
            }
        }
        // No trailing blanks.
        while spans
            .last()
            .is_some_and(|s| s.content.trim().is_empty() && s.style.bg.is_none())
        {
            spans.pop();
        }
        if let Some(last) = spans.last_mut().filter(|s| s.style.bg.is_none()) {
            last.content = last.content.trim_end().to_string().into();
        }
        out.push((Line::from(spans), None));
    }
    Ok(out)
}

/// The view's results as CSV (headers first).
pub fn csv(input: &Input) -> Result<String, String> {
    let results = run(input)?;
    let field = |s: String| {
        if s.contains([',', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s
        }
    };
    let mut out = String::new();
    let headers: Vec<String> = results
        .columns
        .iter()
        .map(|(_, h)| field(h.clone()))
        .collect();
    out.push_str(&headers.join(","));
    out.push('\n');
    for row in &results.cells {
        let cells: Vec<String> = row.iter().map(|v| field(v.display(input.pages))).collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    Ok(out)
}
