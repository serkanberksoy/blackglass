//! The calendar panel (PN-23): a month with ISO week numbers; days, weeks
//! and the month that have a note are highlighted, today is underlined,
//! the selected day has a background. Keys and clicks open (or create)
//! the daily, weekly or monthly note.
//!
//! ```text
//!  ‹     September 2026     ›
//!  Wk Mo Tu We Th Fr Sa Su
//!  36 31  1  2  3  4  5  6
//! ```

use std::cell::Cell;

use chrono::{Datelike, Duration, Months, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::Period;

/// Days, weeks and months that have a note: the theme's accent.
fn noted() -> Style {
    Style::new()
        .fg(crate::ui::theme::paint(crate::ui::theme::ACCENT))
        .add_modifier(Modifier::BOLD)
}

/// The chosen day's background: the theme's selection.
fn selected_bg() -> Color {
    crate::ui::theme::paint(crate::ui::theme::BG_SELECTED)
}
const OTHER_MONTH: Style = Style::new().fg(Color::DarkGray);
/// Rows in the grid: enough for any month.
const WEEKS: usize = 6;
/// Columns per day: two digits and a space.
const CELL: u16 = 3;
/// The narrowest panel that has room for week numbers.
const WIDE: u16 = 25;

/// What the calendar asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Open (or create) the note of this period and date.
    Open(Period, NaiveDate),
}

#[derive(Debug, Default)]
pub struct Calendar {
    /// The selected day; `None`: today.
    selected: Option<NaiveDate>,
    /// The first day of the month shown; `None`: the selected day's.
    month: Option<NaiveDate>,
    /// The layout of the last drawing, for clicks: whether week numbers
    /// were shown.
    week_numbers: Cell<bool>,
    width: Cell<u16>,
}

fn first_of(day: NaiveDate) -> NaiveDate {
    day.with_day(1).expect("every month has a 1st")
}

impl Calendar {
    pub fn selected(&self, today: NaiveDate) -> NaiveDate {
        self.selected.unwrap_or(today)
    }

    fn month(&self, today: NaiveDate) -> NaiveDate {
        self.month.unwrap_or_else(|| first_of(self.selected(today)))
    }

    /// The Monday the grid starts on.
    fn grid_start(&self, today: NaiveDate) -> NaiveDate {
        Period::Weekly.start(self.month(today))
    }

    /// Where the day columns start.
    fn day_x(&self) -> u16 {
        1 + if self.week_numbers.get() { CELL } else { 0 }
    }

    /// The calendar's lines, `width` columns wide. `has_note` says whether
    /// a period's note exists for a date; `weekly` whether weekly notes
    /// are on (then week numbers are shown).
    pub fn lines(
        &self,
        today: NaiveDate,
        width: u16,
        weekly: bool,
        has_note: impl Fn(Period, NaiveDate) -> bool,
    ) -> Vec<Line<'static>> {
        let week_numbers = weekly && width >= WIDE;
        self.week_numbers.set(week_numbers);
        self.width.set(width);
        let month = self.month(today);
        let selected = self.selected(today);
        let inner = usize::from(width.saturating_sub(4));
        let title = month.format("%B %Y").to_string();
        let title_style = if has_note(Period::Monthly, month) {
            noted()
        } else {
            Style::new().add_modifier(Modifier::BOLD)
        };
        let mut lines = vec![Line::from(vec![
            Span::raw(" "),
            Span::styled("‹", OTHER_MONTH),
            Span::styled(format!("{title:^inner$}"), title_style),
            Span::styled("›", OTHER_MONTH),
        ])];
        let mut header = String::from(" ");
        if week_numbers {
            header.push_str("Wk ");
        }
        header.push_str("Mo Tu We Th Fr Sa Su");
        lines.push(Line::from(Span::styled(header, OTHER_MONTH)));
        let start = self.grid_start(today);
        for week in 0..WEEKS {
            let monday = start + Duration::weeks(week as i64);
            let mut spans = vec![Span::raw(" ")];
            if week_numbers {
                let style = if has_note(Period::Weekly, monday) {
                    noted()
                } else {
                    OTHER_MONTH
                };
                spans.push(Span::styled(
                    format!("{:>2}", monday.iso_week().week()),
                    style,
                ));
                spans.push(Span::raw(" "));
            }
            for d in 0..7 {
                let day = monday + Duration::days(d);
                let mut style = if has_note(Period::Daily, day) {
                    noted()
                } else if day.month() != month.month() {
                    OTHER_MONTH
                } else {
                    Style::new()
                };
                if day == today {
                    style = style.add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
                }
                if day == selected {
                    style = style.bg(selected_bg());
                }
                spans.push(Span::styled(format!("{:>2}", day.day()), style));
                spans.push(Span::raw(" "));
            }
            lines.push(Line::from(spans));
        }
        lines
    }

    /// Selects `day` and shows its month.
    fn select(&mut self, day: NaiveDate) {
        self.selected = Some(day);
        self.month = Some(first_of(day));
    }

    /// ←→↑↓ move a day or a week, PgUp / PgDn a month, `t` (or Home) goes
    /// to today; Enter opens the day's note, `w` its week's, `m` its
    /// month's.
    pub fn key(&mut self, key: KeyEvent, today: NaiveDate) -> Option<Action> {
        let day = self.selected(today);
        let months = |n: i32| {
            let span = Months::new(n.unsigned_abs());
            if n >= 0 {
                day.checked_add_months(span)
            } else {
                day.checked_sub_months(span)
            }
        };
        let to = match key.code {
            KeyCode::Left => day.pred_opt(),
            KeyCode::Right => day.succ_opt(),
            KeyCode::Up => day.checked_sub_signed(Duration::weeks(1)),
            KeyCode::Down => day.checked_add_signed(Duration::weeks(1)),
            KeyCode::PageUp => months(-1),
            KeyCode::PageDown => months(1),
            KeyCode::Home | KeyCode::Char('t') => Some(today),
            KeyCode::Enter => return Some(Action::Open(Period::Daily, day)),
            KeyCode::Char('w') => return Some(Action::Open(Period::Weekly, day)),
            KeyCode::Char('m') => return Some(Action::Open(Period::Monthly, day)),
            _ => None,
        };
        if let Some(to) = to {
            self.select(to);
        }
        None
    }

    /// A click at a row and column of the last drawing: ‹ / › change the
    /// month, the title opens the month's note, a week number the week's,
    /// a day the day's.
    pub fn click(&mut self, row: u16, col: u16, today: NaiveDate) -> Option<Action> {
        let month = self.month(today);
        let width = self.width.get();
        if row == 0 {
            let shift = |n: i32| {
                let span = Months::new(n.unsigned_abs());
                if n >= 0 {
                    month.checked_add_months(span)
                } else {
                    month.checked_sub_months(span)
                }
            };
            if col <= 2 {
                self.month = shift(-1);
            } else if col + 3 >= width {
                self.month = shift(1);
            } else {
                return Some(Action::Open(Period::Monthly, month));
            }
            return None;
        }
        let week = usize::from(row.checked_sub(2)?);
        if week >= WEEKS {
            return None;
        }
        let monday = self.grid_start(today) + Duration::weeks(week as i64);
        let x = self.day_x();
        if col < x {
            return self
                .week_numbers
                .get()
                .then_some(Action::Open(Period::Weekly, monday));
        }
        let d = (col - x) / CELL;
        if d >= 7 {
            return None;
        }
        let day = monday + Duration::days(i64::from(d));
        self.selected = Some(day);
        Some(Action::Open(Period::Daily, day))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
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
    fn a_month_with_week_numbers() {
        let c = Calendar::default();
        let today = day(2026, 9, 29);
        let lines = text(&c.lines(today, 30, true, |_, _| false));
        assert_eq!(lines[0], " ‹      September 2026      ›");
        assert_eq!(lines[1], " Wk Mo Tu We Th Fr Sa Su");
        assert_eq!(
            lines[2], " 36 31  1  2  3  4  5  6",
            "starts on the Monday before the 1st"
        );
        assert_eq!(lines[6], " 40 28 29 30  1  2  3  4");
        assert_eq!(lines.len(), 8);
        let narrow = text(&c.lines(today, 22, true, |_, _| false));
        assert_eq!(
            narrow[1], " Mo Tu We Th Fr Sa Su",
            "no room for week numbers"
        );
    }

    #[test]
    fn notes_today_and_the_selection_are_styled() {
        let mut c = Calendar::default();
        let today = day(2026, 9, 29);
        c.select(day(2026, 9, 2));
        let lines = c.lines(today, 30, true, |p, d| {
            p == Period::Daily && d == day(2026, 9, 1)
        });
        let style_of = |row: usize, text: &str| {
            lines[row]
                .spans
                .iter()
                .find(|s| s.content == text)
                .map(|s| s.style)
                .unwrap()
        };
        assert_eq!(style_of(2, " 1"), noted());
        assert_eq!(style_of(2, " 2").bg, Some(selected_bg()));
        assert!(
            style_of(6, "29")
                .add_modifier
                .contains(Modifier::UNDERLINED),
            "today"
        );
        assert_eq!(style_of(2, "31"), OTHER_MONTH, "August");
    }

    #[test]
    fn keys_move_and_open() {
        let mut c = Calendar::default();
        let today = day(2026, 9, 29);
        let key = |c: &mut Calendar, code| c.key(KeyEvent::new(code, KeyModifiers::NONE), today);
        key(&mut c, KeyCode::Down);
        assert_eq!(c.selected(today), day(2026, 10, 6));
        assert_eq!(c.month(today), day(2026, 10, 1), "the month follows");
        key(&mut c, KeyCode::PageUp);
        assert_eq!(c.selected(today), day(2026, 9, 6));
        assert_eq!(
            key(&mut c, KeyCode::Enter),
            Some(Action::Open(Period::Daily, day(2026, 9, 6)))
        );
        assert_eq!(
            key(&mut c, KeyCode::Char('w')),
            Some(Action::Open(Period::Weekly, day(2026, 9, 6)))
        );
        key(&mut c, KeyCode::Char('t'));
        assert_eq!(c.selected(today), today);
    }

    #[test]
    fn clicks_open_days_weeks_and_months() {
        let mut c = Calendar::default();
        let today = day(2026, 9, 29);
        c.lines(today, 30, true, |_, _| false);
        // Row 2 is the first week; day columns start at x 4, three wide.
        assert_eq!(
            c.click(2, 4 + 3 + 1, today),
            Some(Action::Open(Period::Daily, day(2026, 9, 1)))
        );
        assert_eq!(
            c.click(3, 1, today),
            Some(Action::Open(Period::Weekly, day(2026, 9, 7)))
        );
        assert_eq!(
            c.click(0, 12, today),
            Some(Action::Open(Period::Monthly, day(2026, 9, 1)))
        );
        assert_eq!(c.click(0, 1, today), None);
        assert_eq!(c.month(today), day(2026, 8, 1), "‹");
        c.click(0, 29, today);
        assert_eq!(c.month(today), day(2026, 9, 1), "›");
    }
}
