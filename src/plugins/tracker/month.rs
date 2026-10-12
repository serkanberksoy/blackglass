//! Tracker's month view: a month's days in weeks, a day "circled" (in the
//! circle color, its number on it) when its value passes the threshold,
//! circled days next to each other joined into a streak, today
//! underlined; in annotation mode a row of each day's annotation (an
//! emoji) under its week. `‹` and `›` go to the months before and after
//! (the title back), the dataset's name goes to the next dataset, and a
//! click on a day selects it (its value under the month); a second click
//! opens its note.

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use mdedit::processor::CellRow;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::chart::{color, default_color};
use super::collect::{Data, add_months, resolve_date};
use super::expr;
use super::params::Opts;
use crate::ui::theme::{self, FAINT, MUTED, TEXT};

/// A day's cell: ` 12 `.
const CELL: usize = 4;

/// What the user changed in a block's month view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    /// Months from the one shown first.
    pub offset: i64,
    /// Which of the datasets shown (in `dataset`'s order).
    pub dataset: usize,
    pub selected: Option<NaiveDate>,
}

fn week_start(opts: &Opts) -> Weekday {
    let start = opts
        .str("startWeekOn")
        .unwrap_or("Sun")
        .trim()
        .to_lowercase();
    match start.get(..3).unwrap_or("sun") {
        "mon" => Weekday::Mon,
        "tue" => Weekday::Tue,
        "wed" => Weekday::Wed,
        "thu" => Weekday::Thu,
        "fri" => Weekday::Fri,
        "sat" => Weekday::Sat,
        _ => Weekday::Sun,
    }
}

/// How strongly a circled day is drawn (by its value or its streak).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Low,
    Mid,
    High,
}

/// The month view of `data`, `today` ringed; `key` names the block in
/// its actions (`plugin:tracker:m:<key>:…`).
pub fn month(data: &Data, opts: &Opts, today: NaiveDate, state: &State, key: u64) -> Vec<CellRow> {
    // The datasets it can show: `dataset`, else every one drawn.
    let mut list: Vec<usize> = opts
        .nums("dataset")
        .iter()
        .map(|i| *i as usize)
        .filter(|i| *i < data.sets.len())
        .collect();
    if list.is_empty() {
        list = data.shown().map(|(i, _)| i).collect();
    }
    if list.is_empty() {
        return Vec::new();
    }
    let index = list[state.dataset % list.len()];
    let set = &data.sets[index];
    let value_of = |i: usize, day: NaiveDate| -> Option<f64> {
        let d = (day - data.start).num_days();
        (0..data.days as i64)
            .contains(&d)
            .then(|| data.sets[i].values[d as usize])
            .flatten()
    };
    let less = opts
        .str("thresholdType")
        .is_some_and(|t| t.trim().eq_ignore_ascii_case("lessthan"));
    let passes_for = |i: usize, day: NaiveDate| {
        let threshold = opts
            .nth("threshold", i)
            .and_then(|t| t.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        value_of(i, day).is_some_and(|v| if less { v < threshold } else { v > threshold })
    };
    let passes = |day: NaiveDate| passes_for(index, day);
    // The month: `initMonth`, else the last with data; then the user's.
    let last_with_data = (0..data.days)
        .rev()
        .find(|&i| set.values[i].is_some())
        .map_or(data.end(), |i| data.day(i));
    let initial = opts
        .str("initMonth")
        .and_then(|m| {
            resolve_date(&format!("{}-01", m.trim()), "YYYY-MM-DD", today)
                .or_else(|| resolve_date(m, "YYYY-MM", today))
        })
        .unwrap_or(last_with_data);
    let first = add_months(initial.with_day(1).expect("a first day"), state.offset)
        .expect("a month near the data exists");
    let next = add_months(first, 1).expect("the next month exists");
    let start_on = week_start(opts);
    let back = (first.weekday().num_days_from_sunday() + 7 - start_on.num_days_from_sunday()) % 7;
    let mut day = first - Duration::days(back as i64);

    let circle = opts
        .str("circleColor")
        .or(opts.str("color"))
        .and_then(color)
        .unwrap_or_else(|| default_color(0));
    let dim = opts.bool("dimNotInMonth").unwrap_or(true);
    let streaks = opts.bool("showStreak").unwrap_or(true);
    let rings = opts.bool("showTodayRing").unwrap_or(true);
    let show_circle = opts.bool("showCircle").unwrap_or(true);
    let annotate = opts
        .str("mode")
        .is_some_and(|m| m.trim().eq_ignore_ascii_case("annotation"))
        || opts.bool("showAnnotation").unwrap_or(false);
    let all_annotations = opts.bool("showAnnotationOfAllTargets").unwrap_or(false);
    let by_value = opts.bool("circleColorByValue").unwrap_or(false);
    let by_streak = opts.bool("circleColorByStreak").unwrap_or(false);
    let today_color = opts.str("todayRingColor").and_then(color);
    let selected_color = opts
        .str("selectedRingColor")
        .and_then(color)
        .unwrap_or_else(|| color("firebrick").expect("a named color"));
    let show_selected_ring = opts.bool("showSelectedRing").unwrap_or(true);
    // The value range, for colors by value.
    let present: Vec<f64> = set.values.iter().flatten().copied().collect();
    let lo = opts
        .nums("yMin")
        .first()
        .copied()
        .unwrap_or_else(|| present.iter().copied().fold(f64::INFINITY, f64::min));
    let hi = opts
        .nums("yMax")
        .first()
        .copied()
        .unwrap_or_else(|| present.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    let streak_len = |day: NaiveDate| {
        let mut n = 1;
        let mut d = day - Duration::days(1);
        while passes(d) {
            n += 1;
            d -= Duration::days(1);
        }
        let mut d = day + Duration::days(1);
        while passes(d) {
            n += 1;
            d += Duration::days(1);
        }
        n
    };
    let level = |day: NaiveDate| {
        if by_value
            && let Some(v) = value_of(index, day)
            && hi > lo
        {
            let f = (v - lo) / (hi - lo);
            return if f <= 1.0 / 3.0 {
                Level::Low
            } else if f <= 2.0 / 3.0 {
                Level::Mid
            } else {
                Level::High
            };
        }
        if by_streak {
            return match streak_len(day) {
                1 => Level::Low,
                2..=4 => Level::Mid,
                _ => Level::High,
            };
        }
        Level::High
    };
    let action = |what: &str| format!("plugin:tracker:m:{key}:{what}");

    let mut out: Vec<CellRow> = Vec::new();
    // The header: ‹ Month Year ›, each a button.
    let month_style = opts
        .str("headerMonthColor")
        .and_then(color)
        .map_or_else(
            || Style::default().fg(theme::paint(TEXT)),
            |c| Style::default().fg(c),
        )
        .add_modifier(Modifier::BOLD);
    let year_style = opts
        .str("headerYearColor")
        .and_then(color)
        .map_or(month_style, |c| month_style.fg(c));
    let name = first.format("%B").to_string();
    let year = first.year().to_string();
    let title_w = name.chars().count() + 1 + year.len();
    let left = ((CELL * 7).saturating_sub(title_w) / 2).max(3);
    let arrows = Style::default().fg(theme::paint(MUTED));
    let spans = vec![
        Span::styled(format!(" {} ", theme::glyph("‹", "<")), arrows),
        Span::raw(" ".repeat(left - 3)),
        Span::styled(name, month_style),
        Span::raw(" "),
        Span::styled(year, year_style),
        Span::raw(" ".repeat((CELL * 7 - 3).saturating_sub(left + title_w))),
        Span::styled(format!(" {} ", theme::glyph("›", ">")), arrows),
    ];
    out.push((
        Line::from(spans),
        vec![
            (0, 3, action("prev")),
            (3, CELL * 7 - 3, action("reset")),
            (CELL * 7 - 3, CELL * 7, action("next")),
        ],
    ));
    // The dataset's name: a click shows the next.
    if list.len() > 1 {
        let label = format!("{} {}", theme::glyph("●", "*"), set.name);
        let w = label.chars().count();
        let pad = (CELL * 7).saturating_sub(w) / 2;
        out.push((
            Line::from(vec![
                Span::raw(" ".repeat(pad)),
                Span::styled(label, Style::default().fg(circle)),
            ]),
            vec![(pad, pad + w, action("dataset"))],
        ));
    }
    let line_color = opts
        .str("dividingLineColor")
        .and_then(color)
        .unwrap_or(circle);
    out.push((
        Line::styled(
            theme::glyph("─", "-").repeat(CELL * 7),
            Style::default().fg(line_color),
        ),
        Vec::new(),
    ));
    let names: String = (0..7)
        .map(|k| {
            let wd = (0..k).fold(start_on, |w, _| w.succ());
            let name = match wd {
                Weekday::Sun => "Su",
                Weekday::Mon => "Mo",
                Weekday::Tue => "Tu",
                Weekday::Wed => "We",
                Weekday::Thu => "Th",
                Weekday::Fri => "Fr",
                Weekday::Sat => "Sa",
            };
            format!(" {name} ")
        })
        .collect();
    out.push((
        Line::styled(names, Style::default().fg(theme::paint(MUTED))),
        Vec::new(),
    ));
    let ascii = theme::glyph("x", "*") == "*";
    while day < next {
        let mut spans = Vec::new();
        let mut parts = Vec::new();
        let mut notes_row = Vec::new();
        for k in 0..7 {
            let in_month = day >= first && day < next;
            let circled = show_circle && passes(day);
            let joined = |other: NaiveDate| streaks && passes(other);
            let base = if in_month || !dim {
                Style::default().fg(theme::paint(TEXT))
            } else {
                Style::default().fg(theme::paint(FAINT))
            };
            let lvl = level(day);
            let on = match lvl {
                Level::Low => Style::default().fg(circle).add_modifier(Modifier::BOLD),
                Level::Mid => Style::default().fg(circle).add_modifier(Modifier::REVERSED),
                Level::High => Style::default()
                    .fg(circle)
                    .add_modifier(Modifier::REVERSED | Modifier::BOLD),
            };
            let selected = state.selected == Some(day) && show_selected_ring;
            let pad = |joined: bool, ring: &'static str| {
                if selected {
                    Span::styled(
                        ring,
                        Style::default()
                            .fg(selected_color)
                            .add_modifier(Modifier::BOLD),
                    )
                } else if circled && joined && lvl != Level::Low {
                    Span::styled(" ", on)
                } else {
                    Span::raw(" ")
                }
            };
            let mut digits = if circled { on } else { base };
            if rings && day == today {
                digits = digits.add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
                if let (Some(c), false) = (today_color, circled) {
                    digits = digits.fg(c);
                }
            }
            let number = if ascii && circled {
                format!("{:>2}", day.day()).replace(' ', "*")
            } else {
                format!("{:>2}", day.day())
            };
            spans.push(pad(k > 0 && joined(day - Duration::days(1)), "["));
            spans.push(Span::styled(number, digits));
            spans.push(pad(k < 6 && joined(day + Duration::days(1)), "]"));
            let rel = data.notes.get(&day).map_or("", String::as_str);
            parts.push((
                k * CELL,
                k * CELL + CELL,
                action(&format!("day:{day}:{rel}")),
            ));
            if annotate {
                let text: String = if all_annotations {
                    list.iter()
                        .filter(|i| passes_for(**i, day))
                        .filter_map(|i| opts.nth("annotation", *i))
                        .collect()
                } else if passes(day) {
                    opts.nth("annotation", index)
                        .unwrap_or_default()
                        .to_string()
                } else {
                    String::new()
                };
                // What fits in the cell (wide emoji count two).
                let mut fit = String::new();
                for c in text.chars() {
                    let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                    if unicode_width::UnicodeWidthStr::width(fit.as_str()) + w > CELL - 1 {
                        break;
                    }
                    fit.push(c);
                }
                let w = unicode_width::UnicodeWidthStr::width(fit.as_str());
                notes_row.push(Span::raw(format!(" {fit}{}", " ".repeat(CELL - 1 - w))));
            }
            day += Duration::days(1);
        }
        out.push((Line::from(spans), parts));
        if annotate {
            out.push((Line::from(notes_row), Vec::new()));
        }
    }
    // The selected day's value (`showSelectedValue`).
    if let Some(sel) = state.selected
        && opts.bool("showSelectedValue").unwrap_or(true)
    {
        let value = value_of(index, sel).map_or_else(|| "–".to_string(), expr::plain);
        let mut text = format!("{}: {} {value}", data.format(sel), set.name);
        if data.notes.contains_key(&sel) {
            text.push_str(" (click again to open)");
        }
        out.push((
            Line::styled(text, Style::default().fg(theme::paint(MUTED))),
            Vec::new(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::tracker::collect::Dataset;
    use crate::plugins::tracker::params::Params;

    fn day(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    fn data() -> Data {
        let mut notes = std::collections::BTreeMap::new();
        notes.insert(day(10, 2), "D/2026-10-02.md".to_string());
        let set = |name: &str, values: Vec<Option<f64>>| Dataset {
            name: name.into(),
            found: 4,
            values,
            is_x: false,
        };
        Data {
            start: day(10, 1),
            days: 5,
            sets: vec![
                set("a", vec![Some(1.0), Some(2.0), None, Some(0.0), Some(3.0)]),
                set("b", vec![None, None, Some(9.0), None, None]),
            ],
            notes,
            date_format: "YYYY-MM-DD".into(),
        }
    }

    fn rows_with(block: &str, state: &State) -> Vec<CellRow> {
        let p = Params::parse(&format!(
            "searchType: tag\nsearchTarget: a, b\ndatasetName: a, b\n{block}"
        ))
        .unwrap();
        month(&data(), &p.outputs[0].opts, day(10, 5), state, 7)
    }

    /// Only the first dataset (no dataset row).
    fn rows(block: &str) -> Vec<CellRow> {
        rows_with(&format!("{block}  dataset: 0\n"), &State::default())
    }

    fn text(row: &CellRow) -> String {
        row.0.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn style_of(row: &CellRow, digits: &str) -> Style {
        row.0
            .spans
            .iter()
            .find(|s| s.content.trim() == digits)
            .map(|s| s.style)
            .unwrap_or_default()
    }

    fn reversed(row: &CellRow, digits: &str) -> bool {
        style_of(row, digits)
            .add_modifier
            .contains(Modifier::REVERSED)
    }

    #[test]
    fn a_month_of_weeks_with_circled_days() {
        let r = rows("month:\n");
        assert_eq!(text(&r[0]), " ‹      October 2026      › ");
        assert_eq!(text(&r[2]), " Su  Mo  Tu  We  Th  Fr  Sa ");
        // October 2026 starts on a Thursday: Sep 27 … Oct 3 first.
        assert_eq!(text(&r[3]), " 27  28  29  30   1   2   3 ");
        assert!(
            reversed(&r[3], "1") && reversed(&r[3], "2"),
            "values above 0"
        );
        assert!(!reversed(&r[3], "3"), "no value");
        assert!(!reversed(&r[4], "4"), "0 isn't above the threshold");
        assert!(reversed(&r[4], "5"));
        // A streak: the space between the 1st and the 2nd is filled.
        let between = r[3].0.spans[14].clone();
        assert!(
            between.style.add_modifier.contains(Modifier::REVERSED),
            "{:?}",
            r[3].0.spans
        );
        // Buttons: the months before and after, each day.
        assert_eq!(r[0].1[0].2, "plugin:tracker:m:7:prev");
        assert_eq!(r[0].1[2].2, "plugin:tracker:m:7:next");
        assert_eq!(
            r[3].1[5],
            (
                20,
                24,
                "plugin:tracker:m:7:day:2026-10-02:D/2026-10-02.md".to_string()
            )
        );
        assert_eq!(r[3].1[6].2, "plugin:tracker:m:7:day:2026-10-03:");
        assert_eq!(r.len(), 3 + 5, "five weeks");
    }

    #[test]
    fn weeks_start_where_asked_thresholds_and_annotations() {
        let r = rows(
            "month:\n  startWeekOn: Mon\n  threshold: 1.5\n  mode: annotation\n  annotation: 🏃\n  initMonth: 2026-10\n",
        );
        assert_eq!(text(&r[2]), " Mo  Tu  We  Th  Fr  Sa  Su ");
        assert_eq!(text(&r[3]), " 28  29  30   1   2   3   4 ");
        assert!(
            !reversed(&r[3], "1") && reversed(&r[3], "2"),
            "above 1.5 only"
        );
        assert!(text(&r[4]).contains("🏃"), "{:?}", text(&r[4]));
        assert_eq!(text(&r[4]).matches('🏃').count(), 1);
    }

    #[test]
    fn months_datasets_and_the_day_selected() {
        let r = rows_with(
            "month:\n  annotation: x, y\n  mode: annotation\n  showAnnotationOfAllTargets: true\n",
            &State::default(),
        );
        assert!(
            text(&r[1]).contains("● a"),
            "two datasets: {:?}",
            text(&r[1])
        );
        assert_eq!(r[1].1[0].2, "plugin:tracker:m:7:dataset");
        let notes = text(&r[5]);
        assert!(
            notes.contains('x') && notes.contains('y'),
            "all of them: {notes:?}"
        );
        // The next dataset, the month before, a day selected.
        let state = State {
            offset: -1,
            dataset: 1,
            selected: Some(day(10, 3)),
        };
        let r = rows_with("month:\n", &state);
        assert!(text(&r[0]).contains("September 2026"), "{:?}", text(&r[0]));
        assert!(text(&r[1]).contains("● b"));
        let r = rows_with("month:\n", &State { offset: 0, ..state });
        assert!(text(&r[4]).contains("[ 3]"), "a ring: {:?}", text(&r[4]));
        assert_eq!(text(r.last().unwrap()), "2026-10-03: b 9");
    }

    #[test]
    fn circles_by_value_or_streak() {
        let r = rows("month:\n  circleColorByValue: true\n");
        assert!(!reversed(&r[3], "1"), "low");
        assert!(style_of(&r[3], "1").add_modifier.contains(Modifier::BOLD));
        assert!(
            reversed(&r[3], "2") && !style_of(&r[3], "2").add_modifier.contains(Modifier::BOLD),
            "middle"
        );
        assert!(
            reversed(&r[4], "5") && style_of(&r[4], "5").add_modifier.contains(Modifier::BOLD),
            "high"
        );
        let r = rows("month:\n  circleColorByStreak: true\n");
        assert!(reversed(&r[3], "1"), "two days in a row");
        assert!(!reversed(&r[4], "5"), "a day alone");
    }
}
