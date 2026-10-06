//! Natural language dates (W-128), after the community plugin: typing
//! `@` and a date in words (`@today`, `@tomorrow`, `@next friday`,
//! `@in 3 days`, `@oct 20`) suggests it, and Enter puts in a link to that
//! day's note (`[[2026-10-07]]`; Shift+Enter keeps the words as its alias:
//! `[[2026-10-07|tomorrow]]`); `@time` suggests times. Commands turn the
//! selection into a date, insert the current date and time, and ask for a
//! date (the date picker). The settings are the vault's, in
//! `.blackglass/dates.toml`, on the settings window's Dates page.

use chrono::{Datelike, Duration, Months, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

use crate::plugins::Suggestions;
use crate::plugins::settings::{Kind, Setting, Values};
use crate::vault::Vault;

/// The settings file, in the vault.
pub const FILE: &str = ".blackglass/dates.toml";

/// The settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Suggest dates while typing.
    pub autosuggest: bool,
    /// What starts a date (`@`).
    pub trigger: String,
    /// Put in a link (`[[2026-10-07]]`), else the date as text.
    pub link: bool,
    pub format: String,
    pub time_format: String,
    /// Between the date and the time.
    pub separator: String,
    pub week_start: Weekday,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            autosuggest: true,
            trigger: "@".into(),
            link: true,
            format: "YYYY-MM-DD".into(),
            time_format: "HH:mm".into(),
            separator: " ".into(),
            week_start: Weekday::Mon,
        }
    }
}

/// The week's first days the settings offer.
const WEEK_STARTS: [&str; 3] = ["monday", "sunday", "saturday"];

impl Settings {
    /// The settings window's rows.
    pub fn schema() -> Vec<Setting> {
        let text = |key: &str, label: &str, help: &str, default: &str| {
            Setting::new("", key, label, help, Kind::Text, default)
        };
        let toggle = |key: &str, label: &str, help: &str| {
            Setting::new("", key, label, help, Kind::Toggle, "true")
        };
        vec![
            toggle(
                "autosuggest",
                "Suggest dates while typing",
                "Typing the trigger and a date in words (@tomorrow) suggests it",
            ),
            text("trigger", "Trigger", "What starts a date while typing", "@"),
            toggle(
                "link",
                "Insert dates as links",
                "A chosen date becomes a link to its note ([[2026-10-07]]), else plain text",
            ),
            text(
                "format",
                "Date format",
                "How dates are written (YYYY-MM-DD, DD.MM.YYYY, dddd D MMMM …)",
                "YYYY-MM-DD",
            ),
            text(
                "time_format",
                "Time format",
                "How times are written (HH:mm, h:mm A …)",
                "HH:mm",
            ),
            text(
                "separator",
                "Date and time separator",
                "Between the date and the time in \"Insert the current date and time\"",
                " ",
            ),
            Setting::new(
                "",
                "week_start",
                "Week starts on",
                "The first day of \"this week\", \"next week\" and \"next friday\"'s week",
                Kind::Choice(WEEK_STARTS.iter().map(|d| d.to_string()).collect()),
                "monday",
            ),
        ]
    }

    /// The vault's settings file's values.
    pub fn values(vault: &Vault) -> Values {
        Values::parse(&std::fs::read_to_string(vault.root.join(FILE)).unwrap_or_default())
    }

    /// The vault's settings.
    pub fn load(vault: &Vault) -> Settings {
        let values = Settings::values(vault);
        let schema = Settings::schema();
        let get = |key: &str| {
            values.value(
                schema
                    .iter()
                    .find(|s| s.key == key)
                    .expect("a declared setting"),
            )
        };
        let or = |value: String, default: &str| {
            if value.trim().is_empty() {
                default.to_string()
            } else {
                value
            }
        };
        Settings {
            autosuggest: get("autosuggest") != "false",
            trigger: or(get("trigger"), "@"),
            link: get("link") != "false",
            format: or(get("format"), "YYYY-MM-DD"),
            time_format: or(get("time_format"), "HH:mm"),
            separator: get("separator"),
            week_start: match get("week_start").as_str() {
                "sunday" => Weekday::Sun,
                "saturday" => Weekday::Sat,
                _ => Weekday::Mon,
            },
        }
    }
}

/// A span of time a relative date counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

fn unit(word: &str) -> Option<Unit> {
    Some(match word {
        "m" | "min" | "mins" | "minute" | "minutes" => Unit::Minute,
        "h" | "hr" | "hrs" | "hour" | "hours" => Unit::Hour,
        "d" | "day" | "days" => Unit::Day,
        "w" | "wk" | "wks" | "week" | "weeks" => Unit::Week,
        "mo" | "month" | "months" => Unit::Month,
        "y" | "yr" | "yrs" | "year" | "years" => Unit::Year,
        _ => return None,
    })
}

/// A count: digits, `a` / `an`, or one to twelve in words.
fn number(word: &str) -> Option<i64> {
    const WORDS: [&str; 12] = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
        "twelve",
    ];
    match word {
        "a" | "an" => Some(1),
        _ => word
            .parse()
            .ok()
            .or_else(|| WORDS.iter().position(|w| *w == word).map(|i| i as i64 + 1)),
    }
}

const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// A day of the week by its name or its start (`fri`, `thurs`).
fn weekday(word: &str) -> Option<Weekday> {
    if word.len() < 3 {
        return None;
    }
    let i = WEEKDAYS.iter().position(|d| d.starts_with(word))?;
    Some(Weekday::try_from(i as u8).expect("0 to 6"))
}

/// A month (1 to 12) by its name or its start (`oct`, `sept`).
fn month(word: &str) -> Option<u32> {
    if word.len() < 3 {
        return None;
    }
    MONTHS
        .iter()
        .position(|m| m.starts_with(word))
        .map(|i| i as u32 + 1)
}

/// A day of the month: `20`, `20th`, `1st`, `2nd`, `3rd`.
fn day_number(word: &str) -> Option<u32> {
    let digits = word.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &word[digits.len()..];
    if !matches!(suffix, "" | "st" | "nd" | "rd" | "th") {
        return None;
    }
    digits.parse().ok().filter(|d| (1..=31).contains(d))
}

/// `when` moved by `n` units.
fn shift(when: NaiveDateTime, n: i64, unit: Unit) -> Option<NaiveDateTime> {
    let months = |m: i64| {
        let by = Months::new(u32::try_from(m.unsigned_abs()).ok()?);
        if m < 0 {
            when.checked_sub_months(by)
        } else {
            when.checked_add_months(by)
        }
    };
    match unit {
        Unit::Minute => when.checked_add_signed(Duration::minutes(n)),
        Unit::Hour => when.checked_add_signed(Duration::hours(n)),
        Unit::Day => when.checked_add_signed(Duration::days(n)),
        Unit::Week => when.checked_add_signed(Duration::weeks(n)),
        Unit::Month => months(n),
        Unit::Year => months(n.checked_mul(12)?),
    }
}

/// `in 3 days`, `3 days ago`, `2 weeks from now`, `+15 minutes`,
/// `5min ago`: `now` moved.
fn relative(t: &str, now: NaiveDateTime) -> Option<NaiveDateTime> {
    let mut words: Vec<&str> = t.split_whitespace().collect();
    let mut sign = 0;
    if words.first() == Some(&"in") {
        words.remove(0);
        sign = 1;
    }
    if words.last() == Some(&"ago") {
        words.pop();
        sign = -1;
    } else if words.ends_with(&["from", "now"]) {
        words.truncate(words.len() - 2);
        sign = 1;
    } else if words.last() == Some(&"later") {
        words.pop();
        sign = 1;
    }
    let mut first = (*words.first()?).to_string();
    if let Some(rest) = first.strip_prefix('+') {
        (sign, first) = (1, rest.to_string());
    } else if let Some(rest) = first.strip_prefix('-') {
        (sign, first) = (-1, rest.to_string());
    }
    if sign == 0 {
        return None;
    }
    // `15min`: the number and the unit in one word.
    let (n, u) = match words.len() {
        1 => {
            let digits = first.len() - first.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            (number(&first[..digits])?, unit(&first[digits..])?)
        }
        2 => (number(&first)?, unit(words[1])?),
        _ => return None,
    };
    shift(now, sign * n, u)
}

/// The first day of the week `day` is in.
fn week_of(day: NaiveDate, start: Weekday) -> NaiveDate {
    let back = (day.weekday().num_days_from_monday() + 7 - start.num_days_from_monday()) % 7;
    day - Duration::days(i64::from(back))
}

fn first_of(year: i32, month: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, 1)
}

/// The first day of the month `n` months from `day`'s.
fn month_from(day: NaiveDate, n: i32) -> Option<NaiveDate> {
    let index = day.year() * 12 + day.month0() as i32 + n;
    first_of(index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

fn last_of_month(first: NaiveDate) -> Option<NaiveDate> {
    month_from(first, 1)?.pred_opt()
}

/// The text the parsers read: lowercase, no commas, single spaces.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace(',', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The month a phrase names (`month`, `this month`, `next month`,
/// `february`): its first day.
fn month_named(t: &str, today: NaiveDate) -> Option<NaiveDate> {
    match t {
        "month" | "this month" | "the month" => month_from(today, 0),
        "next month" => month_from(today, 1),
        "last month" => month_from(today, -1),
        _ => first_of(today.year(), month(t)?),
    }
}

/// A date in words, from `now` (`week_start` begins a week): `today`,
/// `tomorrow`, `friday`, `next friday`, `next week`, `in 3 days`,
/// `oct 20`, `20th of december`, `end of month`, `mid november`,
/// `2026-12-01`, `12/24` … (a time after `at` is left out).
pub fn parse_date(text: &str, now: NaiveDateTime, week_start: Weekday) -> Option<NaiveDate> {
    let t = normalize(text);
    let t = t.split(" at ").next().unwrap_or_default();
    let t = t.strip_prefix("on ").unwrap_or(t);
    let t = t.strip_prefix("the ").unwrap_or(t);
    let today = now.date();
    let days = |n: i64| today.checked_add_signed(Duration::days(n));
    match t {
        "" => return None,
        "today" | "now" | "tonight" => return Some(today),
        "tomorrow" | "tmr" | "tmrw" => return days(1),
        "yesterday" => return days(-1),
        "day after tomorrow" => return days(2),
        "day before yesterday" => return days(-2),
        "christmas" | "christmas day" => return NaiveDate::from_ymd_opt(today.year(), 12, 25),
        _ => {}
    }
    for format in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, format) {
            return Some(d);
        }
    }
    // `12/24`, `12/24/2027`: month first.
    let parts: Vec<&str> = t.split('/').collect();
    if (2..=3).contains(&parts.len()) && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
    {
        let year = match parts.get(2) {
            Some(y) if y.len() == 2 => 2000 + y.parse::<i32>().ok()?,
            Some(y) => y.parse().ok()?,
            None => today.year(),
        };
        return NaiveDate::from_ymd_opt(year, parts[0].parse().ok()?, parts[1].parse().ok()?);
    }
    for prefix in ["end of ", "last day of ", "the end of "] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return last_of_month(month_named(rest, today)?);
        }
    }
    for prefix in ["mid ", "middle of ", "mid-"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return month_named(rest, today)?.with_day(15);
        }
    }
    if let Some(when) = relative(t, now) {
        return Some(when.date());
    }
    let words: Vec<&str> = t.split(' ').filter(|w| *w != "of").collect();
    match words.as_slice() {
        [w] if weekday(w).is_some() => {
            let target = weekday(w)?;
            let ahead =
                (target.num_days_from_monday() + 7 - today.weekday().num_days_from_monday()) % 7;
            days(i64::from(ahead))
        }
        [which @ ("this" | "next" | "last"), what] => {
            let step: i32 = match *which {
                "next" => 1,
                "last" => -1,
                _ => 0,
            };
            if let Some(target) = weekday(what) {
                let start = week_of(today, week_start);
                let into =
                    (target.num_days_from_monday() + 7 - week_start.num_days_from_monday()) % 7;
                return start
                    .checked_add_signed(Duration::days(i64::from(into) + 7 * i64::from(step)));
            }
            match *what {
                "week" => {
                    week_of(today, week_start).checked_add_signed(Duration::weeks(step.into()))
                }
                "month" => month_from(today, step),
                "year" => first_of(today.year() + step, 1),
                m => {
                    let m = month(m)?;
                    let year = match step {
                        1 if m <= today.month() => today.year() + 1,
                        -1 if m >= today.month() => today.year() - 1,
                        _ => today.year(),
                    };
                    first_of(year, m)
                }
            }
        }
        [d] if d.ends_with(|c: char| c.is_ascii_alphabetic()) && day_number(d).is_some() => {
            today.with_day(day_number(d)?)
        }
        [m] => first_of(today.year(), month(m)?),
        [a, b] | [a, b, _] => {
            let (m, d) = match (month(a), day_number(b)) {
                (Some(m), Some(d)) => (m, d),
                _ => (month(b)?, day_number(a)?),
            };
            let year = match words.get(2) {
                Some(y) if y.len() == 4 => y.parse().ok()?,
                Some(_) => return None,
                None => today.year(),
            };
            NaiveDate::from_ymd_opt(year, m, d)
        }
        _ => None,
    }
}

/// A time of day: `5pm`, `5:30 pm`, `17:45`, `9am`, `noon`, `midnight`.
fn clock(t: &str) -> Option<NaiveTime> {
    let t = t.strip_prefix("at ").unwrap_or(t).replace(' ', "");
    match t.as_str() {
        "noon" | "midday" => return NaiveTime::from_hms_opt(12, 0, 0),
        "midnight" => return NaiveTime::from_hms_opt(0, 0, 0),
        _ => {}
    }
    let (t, half) = match (t.strip_suffix("am"), t.strip_suffix("pm")) {
        (Some(t), _) => (t, Some(0)),
        (_, Some(t)) => (t, Some(12)),
        _ => (t.as_str(), None),
    };
    let (h, m) = match t.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None if half.is_some() => (t.parse::<u32>().ok()?, 0),
        None => return None,
    };
    let h = match half {
        Some(add) if (1..=12).contains(&h) => h % 12 + add,
        Some(_) => return None,
        None => h,
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

/// A time in words, from `now`: `now`, `+15 minutes`, `in 1 hour`,
/// `5min ago`, `5pm`, `17:45`, `noon`, `tomorrow at 9am`.
pub fn parse_time(text: &str, now: NaiveDateTime) -> Option<NaiveDateTime> {
    let t = normalize(text);
    if t == "now" {
        return Some(now);
    }
    if let Some(when) = relative(&t, now) {
        return Some(when);
    }
    if let Some((day, at)) = t.split_once(" at ") {
        let day = parse_date(day, now, Weekday::Mon)?;
        return Some(day.and_time(clock(at)?));
    }
    Some(now.date().and_time(clock(&t)?))
}

/// What the original suggests for what's typed after the trigger.
fn labels(query: &str) -> Vec<String> {
    let q = query.to_lowercase();
    let starting = |all: Vec<String>| -> Vec<String> {
        all.into_iter()
            .filter(|l| l.to_lowercase().starts_with(&q))
            .collect()
    };
    if q.starts_with("time") {
        return starting(
            ["now", "+15 minutes", "+1 hour", "-15 minutes", "-1 hour"]
                .iter()
                .map(|v| format!("time:{v}"))
                .collect(),
        );
    }
    if let Some(which) = q
        .split(|c: char| !c.is_alphanumeric())
        .find(|w| matches!(*w, "next" | "last" | "this"))
    {
        let mut whats = vec!["week".to_string(), "month".into(), "year".into()];
        whats.extend(
            [
                "Sunday",
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
            ]
            .map(String::from),
        );
        return starting(whats.into_iter().map(|w| format!("{which} {w}")).collect());
    }
    let digits = |s: &str| {
        let sign = usize::from(s.starts_with(['+', '-']));
        let n = s[sign..].len()
            - s[sign..]
                .trim_start_matches(|c: char| c.is_ascii_digit())
                .len();
        (n > 0).then(|| s[..sign + n].to_string())
    };
    if let Some(n) = q
        .strip_prefix("in ")
        .and_then(digits)
        .or_else(|| digits(&q))
    {
        return starting(vec![
            format!("in {n} minutes"),
            format!("in {n} hours"),
            format!("in {n} days"),
            format!("in {n} weeks"),
            format!("in {n} months"),
            format!("{n} days ago"),
            format!("{n} weeks ago"),
            format!("{n} months ago"),
        ]);
    }
    starting(vec!["Today".into(), "Yesterday".into(), "Tomorrow".into()])
}

/// A date as the settings write it.
pub fn format_date(day: NaiveDate, s: &Settings) -> String {
    crate::plugins::moment::format(&day.and_time(NaiveTime::MIN), &s.format)
}

/// A link to a day's note (with `alias`), or the date, as the settings
/// say.
pub fn date_text(day: NaiveDate, alias: Option<&str>, s: &Settings) -> String {
    let date = format_date(day, s);
    match (s.link, alias) {
        (false, _) => date,
        (true, Some(alias)) => format!("[[{date}|{alias}]]"),
        (true, None) => format!("[[{date}]]"),
    }
}

/// The dates suggested for the cursor at char `col` of `line`: after the
/// trigger (not inside a word or code).
pub fn suggestions(
    line: &str,
    col: usize,
    s: &Settings,
    now: NaiveDateTime,
) -> Option<Suggestions> {
    if !s.autosuggest || s.trigger.is_empty() {
        return None;
    }
    let before: String = line.chars().take(col).collect();
    let at = before.rfind(&s.trigger)?;
    let ahead = &before[..at];
    if ahead
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '`')
        || ahead.matches('`').count() % 2 == 1
    {
        return None;
    }
    let query = &before[at + s.trigger.len()..];
    if query.starts_with(' ') || query.chars().count() > 40 {
        return None;
    }
    let mut labels = labels(query);
    if labels.is_empty() && !query.trim().is_empty() {
        labels.push(query.to_string());
    }
    let mut out = Suggestions {
        start: ahead.chars().count(),
        ..Suggestions::default()
    };
    for label in labels {
        if let Some(rest) = label.strip_prefix("time:") {
            let Some(time) = parse_time(rest, now) else {
                continue;
            };
            let text = crate::plugins::moment::format(&time, &s.time_format);
            out.items.push((format!("{label}  {text}"), text));
            continue;
        }
        let Some(day) = parse_date(&label, now, s.week_start) else {
            continue;
        };
        if s.link {
            out.alts
                .push((out.items.len(), date_text(day, Some(&label), s)));
        }
        out.items.push((
            format!("{label}  {}", format_date(day, s)),
            date_text(day, None, s),
        ));
    }
    (!out.items.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tuesday, 6 October 2026, 10:00.
    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap()
    }

    fn date(text: &str) -> String {
        parse_date(text, now(), Weekday::Mon).map_or_else(|| "-".into(), |d| d.to_string())
    }

    fn time(text: &str) -> String {
        parse_time(text, now()).map_or_else(|| "-".into(), |t| t.format("%m-%d %H:%M").to_string())
    }

    #[test]
    fn days_in_words() {
        for (text, day) in [
            ("today", "2026-10-06"),
            ("Tomorrow", "2026-10-07"),
            ("yesterday", "2026-10-05"),
            ("the day after tomorrow", "2026-10-08"),
            ("day before yesterday", "2026-10-04"),
            ("friday", "2026-10-09"),
            ("on Tuesday", "2026-10-06"),
            ("monday", "2026-10-12"),
            ("this friday", "2026-10-09"),
            ("this monday", "2026-10-05"),
            ("next friday", "2026-10-16"),
            ("last friday", "2026-10-02"),
            ("next week", "2026-10-12"),
            ("this week", "2026-10-05"),
            ("last week", "2026-09-28"),
            ("next month", "2026-11-01"),
            ("last month", "2026-09-01"),
            ("next year", "2027-01-01"),
            ("in 3 days", "2026-10-09"),
            ("in two weeks", "2026-10-20"),
            ("in a month", "2026-11-06"),
            ("3 days ago", "2026-10-03"),
            ("a week ago", "2026-09-29"),
            ("2 weeks from now", "2026-10-20"),
            ("+5 days", "2026-10-11"),
            ("-1 year", "2025-10-06"),
            ("october 20", "2026-10-20"),
            ("Oct 20th, 2027", "2027-10-20"),
            ("20 oct 2027", "2027-10-20"),
            ("20th of december", "2026-12-20"),
            ("the 20th", "2026-10-20"),
            ("next october", "2027-10-01"),
            ("end of month", "2026-10-31"),
            ("end of february", "2026-02-28"),
            ("last day of next month", "2026-11-30"),
            ("mid november", "2026-11-15"),
            ("christmas", "2026-12-25"),
            ("2026-12-01", "2026-12-01"),
            ("12/24", "2026-12-24"),
            ("12/24/2027", "2027-12-24"),
            ("tomorrow at 5pm", "2026-10-07"),
            ("next friday at 9:30", "2026-10-16"),
            ("gibberish", "-"),
            ("tom and jerry", "-"),
            ("", "-"),
        ] {
            assert_eq!(date(text), day, "{text}");
        }
        let sunday = parse_date("next week", now(), Weekday::Sun).unwrap();
        assert_eq!(sunday.to_string(), "2026-10-11", "a week from Sunday");
    }

    #[test]
    fn times_in_words() {
        for (text, at) in [
            ("now", "10-06 10:00"),
            ("+15 minutes", "10-06 10:15"),
            ("-1 hour", "10-06 09:00"),
            ("in 1 hour", "10-06 11:00"),
            ("in 15min", "10-06 10:15"),
            ("5min ago", "10-06 09:55"),
            ("5pm", "10-06 17:00"),
            ("at 5:30 pm", "10-06 17:30"),
            ("17:45", "10-06 17:45"),
            ("noon", "10-06 12:00"),
            ("midnight", "10-06 00:00"),
            ("tomorrow at 9am", "10-07 09:00"),
            ("soon", "-"),
        ] {
            assert_eq!(time(text), at, "{text}");
        }
    }

    fn texts(s: Option<Suggestions>) -> Vec<(String, String)> {
        s.map(|s| s.items).unwrap_or_default()
    }

    #[test]
    fn suggested_while_typing() {
        let s = Settings::default();
        let found = suggestions("Meet @to", 8, &s, now()).expect("suggestions");
        assert_eq!(found.start, 5, "from the @");
        assert_eq!(
            found.items,
            [
                (
                    "Today  2026-10-06".to_string(),
                    "[[2026-10-06]]".to_string()
                ),
                (
                    "Tomorrow  2026-10-07".to_string(),
                    "[[2026-10-07]]".to_string()
                ),
            ]
        );
        assert_eq!(
            found.alts,
            [
                (0, "[[2026-10-06|Today]]".to_string()),
                (1, "[[2026-10-07|Tomorrow]]".to_string())
            ],
            "Shift+Enter keeps the words"
        );
        let labels = |line: &str| -> Vec<String> {
            texts(suggestions(line, line.chars().count(), &s, now()))
                .into_iter()
                .map(|(l, _)| l.split("  ").next().unwrap().to_string())
                .collect()
        };
        assert_eq!(labels("@"), ["Today", "Yesterday", "Tomorrow"]);
        assert_eq!(labels("@next f"), ["next Friday"]);
        assert_eq!(labels("@in 3 d"), ["in 3 days"]);
        assert_eq!(labels("@friday"), ["friday"], "anything it reads");
        assert_eq!(labels("@time:+1"), ["time:+15 minutes", "time:+1 hour"]);
        for no in ["a@to", "`@to", "@gibberish", "no trigger"] {
            assert!(labels(no).is_empty(), "{no}");
        }
        assert_eq!(
            texts(suggestions("@time:now", 9, &s, now()))[0].1,
            "10:00",
            "a time is text"
        );
        let plain = Settings {
            link: false,
            format: "DD.MM.YYYY".into(),
            ..Settings::default()
        };
        assert_eq!(
            texts(suggestions("@tod", 4, &plain, now()))[0].1,
            "06.10.2026"
        );
        let off = Settings {
            autosuggest: false,
            ..Settings::default()
        };
        assert!(suggestions("@tod", 4, &off, now()).is_none());
    }
}
