//! Dates the way Obsidian plugins write them (Templater, Periodic Notes,
//! Dataview's settings): moment.js format tokens
//! (`YYYY-MM-DD`, `dddd, MMMM Do`, `[week] WW`), ISO 8601 offsets
//! (`P1W`, `-P1M`), and reading a date back with a format.

use chrono::{Datelike, Duration, Months, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday};

/// moment.js tokens, longest first so `MMMM` wins over `MM`.
const TOKENS: [&str; 40] = [
    "YYYY", "YY", "Q", "MMMM", "MMM", "MM", "Mo", "M", "DDDD", "DDD", "DD", "Do", "D", "dddd",
    "ddd", "dd", "d", "E", "e", "GGGG", "GG", "gggg", "gg", "WW", "Wo", "W", "ww", "w", "HH", "H",
    "hh", "h", "mm", "m", "ss", "s", "A", "a", "X", "x",
];

/// How weeks are counted for the locale tokens (`gggg`, `ww`, `w`):
/// the day they start on, and how many days of January the first week
/// has at least (ISO: Monday, 4; the US: Sunday, 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Week {
    pub start: Weekday,
    pub min_days: u32,
}

impl Week {
    pub const ISO: Week = Week {
        start: Weekday::Mon,
        min_days: 4,
    };

    /// Weeks starting on `start`: ISO for Monday, else the first week is
    /// the one with January 1st.
    pub fn starting(start: Weekday) -> Week {
        match start {
            Weekday::Mon => Week::ISO,
            start => Week { start, min_days: 1 },
        }
    }

    /// The first day of the week `day` is in.
    pub fn start_of(self, day: NaiveDate) -> NaiveDate {
        let back =
            (day.weekday().num_days_from_monday() + 7 - self.start.num_days_from_monday()) % 7;
        day - Duration::days(i64::from(back))
    }
}

/// The week-year and week number of `day`, weeks counted as `week` says.
pub fn week_of(day: NaiveDate, week: Week) -> (i32, u32) {
    let first = |year: i32| {
        let jan =
            NaiveDate::from_ymd_opt(year, 1, week.min_days.clamp(1, 7)).expect("a day in January");
        week.start_of(jan)
    };
    let start = week.start_of(day);
    let year = if start >= first(day.year() + 1) {
        day.year() + 1
    } else if start < first(day.year()) {
        day.year() - 1
    } else {
        day.year()
    };
    let n = (start - first(year)).num_days() / 7 + 1;
    (year, u32::try_from(n).expect("a week number is positive"))
}

/// `date` in the moment.js `format`. Text in `[brackets]` is kept as is.
/// Locale weeks (`gggg`, `ww`) are ISO weeks.
pub fn format(date: &NaiveDateTime, format: &str) -> String {
    format_in(date, format, Week::ISO)
}

/// `date` in the moment.js `format`, locale weeks (`gggg`, `ww`, `w`,
/// `e`) counted as `week` says.
pub fn format_in(date: &NaiveDateTime, format: &str, week: Week) -> String {
    let mut out = String::new();
    let mut rest = format;
    'outer: while let Some(c) = rest.chars().next() {
        if c == '[' {
            let end = rest.find(']').unwrap_or(rest.len());
            out.push_str(&rest[1..end]);
            rest = rest.get(end + 1..).unwrap_or("");
            continue;
        }
        for token in TOKENS {
            if let Some(r) = rest.strip_prefix(token) {
                out.push_str(&token_value(date, token, week));
                rest = r;
                continue 'outer;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn token_value(d: &NaiveDateTime, token: &str, week: Week) -> String {
    let hour12 = match d.hour() % 12 {
        0 => 12,
        h => h,
    };
    let iso = d.iso_week();
    let (week_year, week_number) = week_of(d.date(), week);
    match token {
        "YYYY" => format!("{:04}", d.year()),
        "YY" => format!("{:02}", d.year().rem_euclid(100)),
        "Q" => ((d.month() - 1) / 3 + 1).to_string(),
        "MMMM" => d.format("%B").to_string(),
        "MMM" => d.format("%b").to_string(),
        "MM" => format!("{:02}", d.month()),
        "Mo" => ordinal(d.month()),
        "M" => d.month().to_string(),
        "DDDD" => format!("{:03}", d.ordinal()),
        "DDD" => d.ordinal().to_string(),
        "DD" => format!("{:02}", d.day()),
        "Do" => ordinal(d.day()),
        "D" => d.day().to_string(),
        "dddd" => d.format("%A").to_string(),
        "ddd" => d.format("%a").to_string(),
        "dd" => d.format("%a").to_string()[..2].to_string(),
        // Day of the week, Sunday = 0 (moment's `d`).
        "d" => d.weekday().num_days_from_sunday().to_string(),
        // Day of the locale's week, its first day = 0 (Sunday for ISO, as
        // moment's English locale).
        "e" if week == Week::ISO => d.weekday().num_days_from_sunday().to_string(),
        "e" => ((d.weekday().num_days_from_monday() + 7 - week.start.num_days_from_monday()) % 7)
            .to_string(),
        // ISO day of the week, Monday = 1.
        "E" => d.weekday().number_from_monday().to_string(),
        "GGGG" => format!("{:04}", iso.year()),
        "GG" => format!("{:02}", iso.year().rem_euclid(100)),
        "WW" => format!("{:02}", iso.week()),
        "Wo" => ordinal(iso.week()),
        "W" => iso.week().to_string(),
        "gggg" => format!("{week_year:04}"),
        "gg" => format!("{:02}", week_year.rem_euclid(100)),
        "ww" => format!("{week_number:02}"),
        "w" => week_number.to_string(),
        "HH" => format!("{:02}", d.hour()),
        "H" => d.hour().to_string(),
        "hh" => format!("{hour12:02}"),
        "h" => hour12.to_string(),
        "mm" => format!("{:02}", d.minute()),
        "m" => d.minute().to_string(),
        "ss" => format!("{:02}", d.second()),
        "s" => d.second().to_string(),
        "A" => if d.hour() < 12 { "AM" } else { "PM" }.into(),
        "a" => if d.hour() < 12 { "am" } else { "pm" }.into(),
        "X" => d.and_utc().timestamp().to_string(),
        "x" => d.and_utc().timestamp_millis().to_string(),
        other => unreachable!("{other} is in TOKENS"),
    }
}

/// `1st`, `2nd`, `3rd`, `4th`, `11th`, `21st` …
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Reads `text` as a date in moment `format` (numeric tokens, month names,
/// ordinals like `9th`; other text must match). `None` if it doesn't fit.
pub fn parse(text: &str, format: &str) -> Option<NaiveDateTime> {
    let (mut year, mut month, mut day) = (None, None, None);
    let (mut hour, mut minute, mut second) = (0, 0, 0);
    let mut pm = None;
    let mut rest = text;
    let mut fmt = format;
    let number = |rest: &mut &str, max_len: usize| -> Option<u32> {
        let len = rest
            .chars()
            .take(max_len)
            .take_while(char::is_ascii_digit)
            .count();
        let n = rest[..len].parse().ok()?;
        *rest = &rest[len..];
        Some(n)
    };
    'outer: while let Some(c) = fmt.chars().next() {
        if c == '[' {
            let end = fmt.find(']').unwrap_or(fmt.len());
            rest = rest.strip_prefix(&fmt[1..end])?;
            fmt = fmt.get(end + 1..).unwrap_or("");
            continue;
        }
        for token in TOKENS {
            if let Some(f) = fmt.strip_prefix(token) {
                match token {
                    "YYYY" => year = Some(number(&mut rest, 4)? as i32),
                    "YY" => year = Some(2000 + number(&mut rest, 2)? as i32),
                    "MM" | "M" => month = Some(number(&mut rest, 2)?),
                    "DD" | "D" => day = Some(number(&mut rest, 2)?),
                    "Do" => {
                        day = Some(number(&mut rest, 2)?);
                        rest = rest.get(2..)?;
                    }
                    "MMMM" | "MMM" => {
                        let (m, len) = (1..=12).find_map(|m| {
                            let date = NaiveDate::from_ymd_opt(2000, m, 1)?;
                            let name = date
                                .format(if token == "MMMM" { "%B" } else { "%b" })
                                .to_string();
                            rest.to_lowercase()
                                .starts_with(&name.to_lowercase())
                                .then_some((m, name.len()))
                        })?;
                        month = Some(m);
                        rest = &rest[len..];
                    }
                    "HH" | "H" | "hh" | "h" => hour = number(&mut rest, 2)?,
                    "mm" | "m" => minute = number(&mut rest, 2)?,
                    "ss" | "s" => second = number(&mut rest, 2)?,
                    "A" | "a" => {
                        let lower = rest.get(..2)?.to_lowercase();
                        pm = Some(lower == "pm");
                        rest = &rest[2..];
                    }
                    "dddd" | "ddd" => {
                        // A weekday name says nothing the date doesn't.
                        let len = rest.chars().take_while(|c| c.is_alphabetic()).count();
                        rest = &rest[len..];
                    }
                    _ => return None,
                }
                fmt = f;
                continue 'outer;
            }
        }
        rest = rest.strip_prefix(c)?;
        fmt = &fmt[c.len_utf8()..];
    }
    if pm == Some(true) && hour < 12 {
        hour += 12;
    } else if pm == Some(false) && hour == 12 {
        hour = 0;
    }
    let date = NaiveDate::from_ymd_opt(year?, month.unwrap_or(1), day.unwrap_or(1))?;
    date.and_hms_opt(hour, minute, second)
}

/// `date` moved by `offset`: a number of days, or an ISO 8601 duration
/// (`P1Y2M3W4D`, `PT12H`, `-P1M`, `P-1W`). `None` if it's not one.
pub fn shift(date: NaiveDateTime, offset: &str) -> Option<NaiveDateTime> {
    let offset = offset.trim();
    if let Ok(days) = offset.parse::<i64>() {
        return date.checked_add_signed(Duration::days(days));
    }
    let (sign, rest) = match offset.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, offset.strip_prefix('+').unwrap_or(offset)),
    };
    let mut rest = rest.strip_prefix(['P', 'p'])?;
    let mut in_time = false;
    let mut out = date;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix(['T', 't']) {
            in_time = true;
            rest = r;
            continue;
        }
        let len = rest
            .char_indices()
            .find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && c == '-')))
            .map_or(rest.len(), |(i, _)| i);
        let n: i64 = rest[..len].parse().ok()?;
        let n = n * sign;
        let unit = rest[len..].chars().next()?.to_ascii_uppercase();
        rest = &rest[len + 1..];
        out = match (unit, in_time) {
            ('Y', false) => add_months(out, n * 12)?,
            ('M', false) => add_months(out, n)?,
            ('W', false) => out.checked_add_signed(Duration::weeks(n))?,
            ('D', false) => out.checked_add_signed(Duration::days(n))?,
            ('H', true) => out.checked_add_signed(Duration::hours(n))?,
            ('M', true) => out.checked_add_signed(Duration::minutes(n))?,
            ('S', true) => out.checked_add_signed(Duration::seconds(n))?,
            _ => return None,
        };
    }
    Some(out)
}

fn add_months(date: NaiveDateTime, n: i64) -> Option<NaiveDateTime> {
    let months = Months::new(u32::try_from(n.unsigned_abs()).ok()?);
    if n >= 0 {
        date.checked_add_months(months)
    } else {
        date.checked_sub_months(months)
    }
}

/// Day `weekday` of the week `date` is in (weeks start on Monday, as in
/// Templater's examples): 0 is that Monday, 6 Sunday, 7 next Monday, -7
/// last Monday.
pub fn weekday(date: NaiveDateTime, weekday: i64) -> NaiveDateTime {
    let monday = date.date() - Duration::days(i64::from(date.weekday().num_days_from_monday()));
    (monday + Duration::days(weekday)).and_time(NaiveTime::MIN)
}

#[cfg(test)]
mod tests {
    #[test]
    fn locale_weeks() {
        let d = |y, m, dd| {
            NaiveDate::from_ymd_opt(y, m, dd)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        };
        let us = Week {
            start: chrono::Weekday::Sun,
            min_days: 1,
        };
        // 1 January 2026 is a Thursday.
        assert_eq!(format_in(&d(2026, 1, 4), "gggg-[W]ww e", us), "2026-W02 0");
        assert_eq!(format_in(&d(2025, 12, 28), "gggg-[W]ww", us), "2026-W01");
        assert_eq!(
            format_in(&d(2026, 1, 4), "gggg-[W]ww", Week::ISO),
            "2026-W01"
        );
        assert_eq!(
            format(&d(2026, 1, 4), "gggg-[W]ww"),
            "2026-W01",
            "ISO by default"
        );
        assert_eq!(
            format_in(&d(2026, 1, 4), "GGGG-[W]WW", us),
            "2026-W01",
            "ISO tokens stay ISO"
        );
        assert_eq!(week_of(d(2026, 1, 4).date(), us), (2026, 2));
    }

    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 5)
            .unwrap()
    }

    #[test]
    fn moment_tokens() {
        let d = at(2026, 8, 9, 14, 7);
        assert_eq!(format(&d, "YYYY-MM-DD"), "2026-08-09");
        assert_eq!(
            format(&d, "dddd, MMMM Do, YYYY"),
            "Sunday, August 9th, 2026"
        );
        assert_eq!(format(&d, "ddd D MMM YY"), "Sun 9 Aug 26");
        assert_eq!(format(&d, "HH:mm:ss h A"), "14:07:05 2 PM");
        assert_eq!(
            format(&d, "[wk]GGWW.E"),
            "wk2632.7",
            "ISO week, as in the design's daily note"
        );
        assert_eq!(format(&d, "Q DDD [Day] d"), "3 221 Day 0");
        assert_eq!(format(&at(2026, 8, 22, 0, 0), "Do"), "22nd");
        assert_eq!(format(&at(2026, 8, 13, 0, 0), "Do"), "13th");
        assert_eq!(format(&at(2026, 8, 1, 0, 0), "h:mm a"), "12:00 am");
    }

    #[test]
    fn reading_dates_back() {
        assert_eq!(
            parse("2026-08-09", "YYYY-MM-DD"),
            Some(at(2026, 8, 9, 0, 0).with_second(0).unwrap())
        );
        assert_eq!(
            parse("August 9th, 2026", "MMMM Do, YYYY").map(|d| d.date()),
            NaiveDate::from_ymd_opt(2026, 8, 9)
        );
        assert_eq!(
            parse("09.08.26 2:30 pm", "DD.MM.YY h:mm a").map(|d| (d.day(), d.hour())),
            Some((9, 14))
        );
        assert_eq!(parse("Journal", "YYYY-MM-DD"), None);
        assert_eq!(parse("2026-13-01", "YYYY-MM-DD"), None, "no 13th month");
    }

    #[test]
    fn offsets_in_days_and_iso_durations() {
        let d = at(2026, 1, 31, 10, 0);
        assert_eq!(
            shift(d, "1").map(|d| d.date().to_string()),
            Some("2026-02-01".into())
        );
        assert_eq!(shift(d, "-7").map(|d| d.day()), Some(24));
        assert_eq!(
            shift(d, "P1M").map(|d| d.date().to_string()),
            Some("2026-02-28".into())
        );
        assert_eq!(shift(d, "-P1Y").map(|d| d.year()), Some(2025));
        assert_eq!(shift(d, "P-1W").map(|d| d.day()), Some(24));
        assert_eq!(
            shift(d, "P1DT2H").map(|d| (d.day(), d.hour())),
            Some((1, 12))
        );
        assert_eq!(shift(d, "soon"), None);
    }

    #[test]
    fn weekdays_count_from_monday() {
        let sunday = at(2026, 8, 9, 10, 0);
        assert_eq!(weekday(sunday, 0).date().to_string(), "2026-08-03");
        assert_eq!(weekday(sunday, 7).date().to_string(), "2026-08-10");
        assert_eq!(weekday(sunday, -7).date().to_string(), "2026-07-27");
    }
}
