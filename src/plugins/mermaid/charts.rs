//! Mermaid pie charts and Gantt charts as text: a pie's slices as bars
//! with their shares; a Gantt chart's tasks, by section, as bars on one
//! timeline with their dates.

use chrono::{Duration, NaiveDate};

fn unquote(text: &str) -> &str {
    text.trim().trim_matches('"')
}

/// A pie chart (`pie [title …]`, then `"label" : value` lines) as a bar
/// per slice with its share, `width` columns wide.
pub fn pie(lines: &[String], width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let title = lines
        .first()
        .and_then(|l| l.trim().strip_prefix("pie"))
        .map(|rest| rest.trim().trim_start_matches("showData").trim())
        .and_then(|rest| rest.strip_prefix("title"))
        .map(|t| t.trim().to_string());
    let mut slices: Vec<(String, f64)> = Vec::new();
    for line in lines.iter().skip(1) {
        let line = line.trim();
        if let Some(t) = line.strip_prefix("title") {
            out.push(t.trim().to_string());
            continue;
        }
        if let Some((label, value)) = line.rsplit_once(':')
            && let Ok(value) = value.trim().parse::<f64>()
        {
            slices.push((unquote(label).to_string(), value));
        }
    }
    if let Some(title) = title {
        out.insert(0, title);
    }
    let total: f64 = slices.iter().map(|s| s.1).sum();
    if total <= 0.0 {
        return out;
    }
    let label_w = slices
        .iter()
        .map(|s| s.0.chars().count())
        .max()
        .unwrap_or(0);
    let room = width.saturating_sub(label_w + 2 + 5).max(1);
    for (label, value) in &slices {
        let share = value / total;
        let bar = "█".repeat((share * room as f64).round() as usize);
        let percent = format!("{:>3}%", (share * 100.0).round() as i64);
        out.push(format!(
            "{label:<label_w$}  {bar:<room$} {percent}",
            bar = bar
        ));
    }
    out
}

/// A Gantt chart's task: its name, first and last day.
struct Task {
    name: String,
    start: NaiveDate,
    end: NaiveDate,
}

/// A duration like `10d` or `2w`, in days.
fn days(text: &str) -> Option<i64> {
    let text = text.trim();
    let (n, unit) = text.split_at(text.len().checked_sub(1)?);
    let n: i64 = n.parse().ok()?;
    match unit {
        "d" => Some(n),
        "w" => Some(n * 7),
        _ => None,
    }
}

/// A Gantt chart (`section`s of `Task :[id,] start|after id, 10d|end`) as
/// a timeline: each task a bar on one scale, with its dates, `width`
/// columns wide.
pub fn gantt(lines: &[String], width: usize) -> Vec<String> {
    let mut title = None;
    // (section name, tasks); tasks by id for `after`.
    let mut sections: Vec<(String, Vec<Task>)> = Vec::new();
    let mut ids: Vec<(String, NaiveDate)> = Vec::new();
    let mut last_end: Option<NaiveDate> = None;
    for line in lines.iter().skip(1) {
        let line = line.trim();
        let word = line.split_whitespace().next().unwrap_or_default();
        match word {
            "" | "dateFormat" | "axisFormat" | "excludes" | "todayMarker" => continue,
            _ if line.starts_with("%%") => continue,
            "title" => {
                title = Some(line[5..].trim().to_string());
                continue;
            }
            "section" => {
                sections.push((line[7..].trim().to_string(), Vec::new()));
                continue;
            }
            _ => {}
        }
        let Some((name, spec)) = line.split_once(':') else {
            continue;
        };
        let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
        let mut id = None;
        let mut start = None;
        let mut end = None;
        for part in &parts {
            if let Some(after) = part.strip_prefix("after ") {
                start = ids
                    .iter()
                    .find(|(i, _)| i == after.trim())
                    .map(|(_, e)| *e + Duration::days(1));
            } else if let Ok(date) = NaiveDate::parse_from_str(part, "%Y-%m-%d") {
                if start.is_none() {
                    start = Some(date);
                } else {
                    end = Some(date);
                }
            } else if let Some(n) = days(part) {
                if let Some(s) = start.or(last_end.map(|e| e + Duration::days(1))) {
                    start = Some(s);
                    end = Some(s + Duration::days(n.max(1) - 1));
                }
            } else if !matches!(*part, "done" | "active" | "crit" | "milestone") && !part.is_empty()
            {
                id = Some(part.to_string());
            }
        }
        let Some(start) = start.or(last_end.map(|e| e + Duration::days(1))) else {
            continue;
        };
        let end = end.unwrap_or(start).max(start);
        last_end = Some(end);
        if let Some(id) = id {
            ids.push((id, end));
        }
        if sections.is_empty() {
            sections.push((String::new(), Vec::new()));
        }
        let task = Task {
            name: name.trim().to_string(),
            start,
            end,
        };
        sections.last_mut().expect("a section").1.push(task);
    }
    let all: Vec<&Task> = sections.iter().flat_map(|s| &s.1).collect();
    let mut out: Vec<String> = title.into_iter().collect();
    let (Some(first), Some(last)) = (
        all.iter().map(|t| t.start).min(),
        all.iter().map(|t| t.end).max(),
    ) else {
        return out;
    };
    let name_w = all
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(0)
        + 2;
    let dates_w = 24;
    let room = width.saturating_sub(name_w + 2 + dates_w).max(10);
    let span = (last - first).num_days().max(0) + 1;
    let col = |d: NaiveDate| ((d - first).num_days() * room as i64 / span) as usize;
    for (section, tasks) in &sections {
        if !section.is_empty() {
            out.push(section.clone());
        }
        for t in tasks {
            let (a, b) = (
                col(t.start),
                col(t.end + Duration::days(1)).max(col(t.start) + 1),
            );
            let bar = format!("{}{}", " ".repeat(a), "█".repeat(b - a));
            let dates = if t.start == t.end {
                t.start.to_string()
            } else {
                format!("{} … {}", t.start, t.end)
            };
            out.push(
                format!("  {:<name_w$}{bar:<room$}  {dates}", t.name)
                    .trim_end()
                    .to_string(),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(source: &str) -> Vec<String> {
        source.lines().map(String::from).collect()
    }

    #[test]
    fn a_pie_is_bars_with_shares() {
        let chart = pie(
            &lines("pie title Pets\n  \"Dogs\" : 30\n  \"Cats\" : 10"),
            40,
        );
        assert_eq!(
            chart,
            [
                "Pets",
                "Dogs  ██████████████████████         75%",
                "Cats  ███████                        25%",
            ]
        );
    }

    #[test]
    fn a_gantt_chart_is_a_timeline() {
        let chart = gantt(
            &lines(
                "gantt\n  title Plan\n  dateFormat YYYY-MM-DD\n  section Build\n  Design :a1, 2026-01-01, 10d\n  Code :after a1, 10d\n  section Ship\n  Release :2026-01-21, 1d",
            ),
            50,
        );
        assert_eq!(
            chart,
            [
                "Plan",
                "Build",
                "  Design   ███████          2026-01-01 … 2026-01-10",
                "  Code            ███████   2026-01-11 … 2026-01-20",
                "Ship",
                "  Release                █  2026-01-21",
            ]
        );
    }
}
