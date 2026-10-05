//! Dataview values: what a field holds after typing its text, as Dataview
//! does: numbers, `true` / `false`, dates (`2026-08-09`,
//! `2026-08-11T10:36:33+02:00`), links (`[[Note]]`), lists and text; and
//! what queries make: objects (`{a: 1}`) and durations (`dur(1 day)`).

use std::cmp::Ordering;

use chrono::{DateTime, Months, NaiveDate, NaiveDateTime, NaiveTime};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    /// Local date and time (midnight for a date alone).
    Date(NaiveDateTime),
    /// A link to a note, by its name or path as written (no `.md`).
    Link(String),
    List(Vec<Value>),
    /// Keys and values, in order (`{a: 1}`, `object("a", 1)`, a group).
    Object(Vec<(String, Value)>),
    Duration(Dur),
}

/// A length of time: whole months (they differ in days) and milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dur {
    pub months: i64,
    pub millis: i64,
}

const SECOND: i64 = 1000;
const MINUTE: i64 = 60 * SECOND;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const WEEK: i64 = 7 * DAY;

impl Dur {
    /// `1 day`, `3 weeks`, `2h 30m`, `1 year, 2 months`; `None` if it isn't
    /// one.
    pub fn parse(text: &str) -> Option<Dur> {
        let mut dur = Dur::default();
        let mut words = text
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|w| !w.is_empty())
            .peekable();
        let mut any = false;
        while let Some(word) = words.next() {
            // `2h` or `2 hours`.
            let digits = word
                .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
                .unwrap_or(word.len());
            let (number, unit) = if digits < word.len() {
                (&word[..digits], word[digits..].to_string())
            } else {
                (word, words.next()?.to_string())
            };
            let n: f64 = number.parse().ok()?;
            let unit = unit.to_lowercase();
            // Plurals (`days`); `ms` is its own unit.
            let unit = if unit == "ms" {
                "millisecond"
            } else {
                unit.trim_end_matches('s')
            };
            match unit {
                "y" | "yr" | "year" => dur.months += (n * 12.0) as i64,
                "mo" | "month" => dur.months += n as i64,
                "w" | "wk" | "week" => dur.millis += (n * WEEK as f64) as i64,
                "d" | "day" => dur.millis += (n * DAY as f64) as i64,
                "h" | "hr" | "hour" => dur.millis += (n * HOUR as f64) as i64,
                "m" | "min" | "minute" => dur.millis += (n * MINUTE as f64) as i64,
                "" | "sec" | "second" => dur.millis += (n * SECOND as f64) as i64,
                "millisecond" => dur.millis += n as i64,
                _ => return None,
            }
            any = true;
        }
        any.then_some(dur)
    }

    /// Roughly, for comparing: a month is 30 days.
    pub fn approx_millis(self) -> i64 {
        self.months * 30 * DAY + self.millis
    }

    pub fn plus(self, other: Dur) -> Dur {
        Dur {
            months: self.months + other.months,
            millis: self.millis + other.millis,
        }
    }

    pub fn times(self, n: f64) -> Dur {
        Dur {
            months: (self.months as f64 * n) as i64,
            millis: (self.millis as f64 * n) as i64,
        }
    }

    /// `date` moved by this (`back`: the other way).
    pub fn shift(self, date: NaiveDateTime, back: bool) -> Option<NaiveDateTime> {
        let (months, millis) = if back {
            (-self.months, -self.millis)
        } else {
            (self.months, self.millis)
        };
        let moved = if months >= 0 {
            date.checked_add_months(Months::new(months as u32))?
        } else {
            date.checked_sub_months(Months::new((-months) as u32))?
        };
        moved.checked_add_signed(chrono::Duration::milliseconds(millis))
    }

    /// `3 days, 2 hours` (years, months, days, hours, minutes, seconds).
    pub fn display(self) -> String {
        let mut parts = Vec::new();
        let unit = |n: i64, one: &str, parts: &mut Vec<String>| {
            if n != 0 {
                parts.push(format!("{n} {one}{}", if n.abs() == 1 { "" } else { "s" }));
            }
        };
        unit(self.months / 12, "year", &mut parts);
        unit(self.months % 12, "month", &mut parts);
        let mut ms = self.millis;
        for (size, name) in [
            (DAY, "day"),
            (HOUR, "hour"),
            (MINUTE, "minute"),
            (SECOND, "second"),
        ] {
            unit(ms / size, name, &mut parts);
            ms %= size;
        }
        if parts.is_empty() {
            "0 seconds".into()
        } else {
            parts.join(", ")
        }
    }
}

impl Value {
    /// A field's text, typed: `[a, b]` is a list, and quotes make text.
    pub fn parse(raw: &str) -> Value {
        let raw = raw.trim();
        if raw.is_empty() {
            return Value::Null;
        }
        if let Some(inner) = raw.strip_prefix('[').and_then(|r| r.strip_suffix(']'))
            && !raw.starts_with("[[")
        {
            return Value::List(
                inner
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(Value::parse)
                    .collect(),
            );
        }
        Value::scalar(raw)
    }

    /// One value (no lists).
    pub fn scalar(raw: &str) -> Value {
        let raw = raw.trim();
        for quote in ['"', '\''] {
            if raw.len() >= 2 && raw.starts_with(quote) && raw.ends_with(quote) {
                let inner = &raw[1..raw.len() - 1];
                // `"[[Note]]"`: YAML needs the quotes; it's a link.
                if let Some(link) = inner
                    .strip_prefix("[[")
                    .and_then(|r| r.strip_suffix("]]"))
                    .filter(|l| !l.contains("]]"))
                {
                    return Value::Link(link_name(link));
                }
                return Value::Text(inner.to_string());
            }
        }
        if let Some(link) = raw.strip_prefix("[[").and_then(|r| r.strip_suffix("]]")) {
            return Value::Link(link_name(link));
        }
        match raw {
            "" => return Value::Null,
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            _ => {}
        }
        if let Ok(n) = raw.parse::<f64>()
            && n.is_finite()
        {
            return Value::Number(n);
        }
        if let Some(date) = parse_date(raw) {
            return Value::Date(date);
        }
        Value::Text(raw.to_string())
    }

    /// Whether the value counts as true in `WHERE`.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::Text(t) | Value::Link(t) => !t.is_empty(),
            Value::Date(_) => true,
            Value::List(l) => !l.is_empty(),
            Value::Object(o) => !o.is_empty(),
            Value::Duration(d) => d.approx_millis() != 0,
        }
    }

    /// Whether two values are of the same kind (a number and a text that
    /// read alike aren't one group).
    pub fn kind_eq(&self, other: &Value) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// A key of an object (`None` if it isn't one, or hasn't it).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Equality as `=` sees it: links match by note name (ignoring case
    /// and folders), also against text; numbers and dates by value.
    pub fn equals(&self, other: &Value) -> bool {
        let note = |l: &str| l.rsplit('/').next().unwrap_or(l).to_lowercase();
        match (self, other) {
            (Value::Link(a), Value::Link(b)) => note(a) == note(b),
            (Value::Link(a), Value::Text(b)) | (Value::Text(b), Value::Link(a)) => {
                note(a) == note(b) || a.eq_ignore_ascii_case(b)
            }
            _ => self.compare(other) == Some(Ordering::Equal),
        }
    }

    /// The order of two values of the same kind; `None` if they can't be
    /// compared (then `<` and `>` are false).
    pub fn compare(&self, other: &Value) -> Option<Ordering> {
        match (self, other) {
            (Value::Null, Value::Null) => Some(Ordering::Equal),
            (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b),
            (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
            (Value::Link(a), Value::Link(b)) => Some(a.to_lowercase().cmp(&b.to_lowercase())),
            (Value::Date(a), Value::Date(b)) => Some(a.cmp(b)),
            (Value::Duration(a), Value::Duration(b)) => {
                Some(a.approx_millis().cmp(&b.approx_millis()))
            }
            (Value::Object(a), Value::Object(b)) => (a == b).then_some(Ordering::Equal),
            // A date compares with a date written as text.
            (Value::Date(a), Value::Text(b)) => parse_date(b).map(|b| a.cmp(&b)),
            (Value::Text(a), Value::Date(b)) => parse_date(a).map(|a| a.cmp(b)),
            (Value::List(a), Value::List(b)) => {
                for (x, y) in a.iter().zip(b) {
                    match x.compare(y)? {
                        Ordering::Equal => {}
                        other => return Some(other),
                    }
                }
                Some(a.len().cmp(&b.len()))
            }
            _ => None,
        }
    }

    /// A total order for `SORT`: nulls first, then by kind, then by value.
    pub fn sort_order(&self, other: &Value) -> Ordering {
        self.compare(other)
            .unwrap_or_else(|| self.kind_rank().cmp(&other.kind_rank()))
    }

    fn kind_rank(&self) -> u8 {
        match self {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::Number(_) => 2,
            Value::Date(_) => 3,
            Value::Text(_) => 4,
            Value::Link(_) => 5,
            Value::List(_) => 6,
            Value::Duration(_) => 7,
            Value::Object(_) => 8,
        }
    }

    /// How the value is shown in a result.
    pub fn display(&self) -> String {
        match self {
            Value::Null => "-".into(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
            Value::Number(n) => format!("{n}"),
            Value::Text(t) => t.clone(),
            Value::Link(l) => l.rsplit('/').next().unwrap_or(l).to_string(),
            Value::Date(d) if d.time() == NaiveTime::MIN => d.format("%Y-%m-%d").to_string(),
            Value::Date(d) => d.format("%Y-%m-%d %H:%M").to_string(),
            Value::List(l) => l.iter().map(Value::display).collect::<Vec<_>>().join(", "),
            Value::Object(o) => o
                .iter()
                .map(|(k, v)| format!("{k}: {}", v.display()))
                .collect::<Vec<_>>()
                .join(", "),
            Value::Duration(d) => d.display(),
        }
    }
}

/// The note a link's text points to: before `|` (alias) and `#` (heading).
pub fn link_name(link: &str) -> String {
    let target = link.split('|').next().unwrap_or_default();
    let target = target.split('#').next().unwrap_or_default();
    target.trim().trim_end_matches(".md").to_string()
}

/// A date or date and time, in the forms Dataview recognizes (a time zone
/// is dropped: times are local).
pub fn parse_date(text: &str) -> Option<NaiveDateTime> {
    let text = text.trim();
    // Quick reject: dates start with four digits and a dash.
    let bytes = text.as_bytes();
    if bytes.len() < 10 || !bytes[..4].iter().all(u8::is_ascii_digit) || bytes[4] != b'-' {
        return None;
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(text) {
        return Some(d.naive_local());
    }
    for format in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(d) = NaiveDateTime::parse_from_str(text, format) {
            return Some(d);
        }
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .map(|d| d.and_time(NaiveTime::MIN))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_time(NaiveTime::MIN)
    }

    #[test]
    fn fields_are_typed_like_dataview_does() {
        assert_eq!(Value::parse("138"), Value::Number(138.0));
        assert_eq!(Value::parse("-2.5"), Value::Number(-2.5));
        assert_eq!(Value::parse("true"), Value::Bool(true));
        assert_eq!(Value::parse("2026-08-09"), Value::Date(date(2026, 8, 9)));
        assert_eq!(
            Value::parse("2026-08-11T10:36:33+02:00").display(),
            "2026-08-11 10:36"
        );
        assert_eq!(
            Value::parse("[[Books/Dune|the book]]"),
            Value::Link("Books/Dune".into())
        );
        assert_eq!(
            Value::parse("[a, 2]"),
            Value::List(vec![Value::Text("a".into()), Value::Number(2.0)])
        );
        assert_eq!(Value::parse("\"42\""), Value::Text("42".into()), "quoted");
        assert_eq!(
            Value::parse("Frank Herbert"),
            Value::Text("Frank Herbert".into())
        );
        assert_eq!(Value::parse(""), Value::Null);
        assert_eq!(
            Value::parse("日本語日本語"),
            Value::Text("日本語日本語".into())
        );
    }

    #[test]
    fn quoted_links_in_properties_are_links() {
        // YAML needs the quotes; Dataview reads them as links.
        assert_eq!(
            Value::parse("\"[[Roadmap]]\""),
            Value::Link("Roadmap".into())
        );
        assert_eq!(
            Value::scalar("'[[Sub/Note|alias]]'"),
            Value::Link(link_name("Sub/Note|alias"))
        );
        assert_eq!(
            Value::parse("[\"[[A]]\", \"[[B]]\"]"),
            Value::List(vec![Value::Link("A".into()), Value::Link("B".into())])
        );
        assert_eq!(
            Value::parse("\"see [[A]]\""),
            Value::Text("see [[A]]".into()),
            "text with a link"
        );
    }

    #[test]
    fn comparing_values() {
        let n = |x| Value::Number(x);
        assert_eq!(n(1.0).compare(&n(2.0)), Some(Ordering::Less));
        assert_eq!(n(1.0).compare(&Value::Text("1".into())), None);
        assert!(Value::Link("Dune".into()).equals(&Value::Text("dune".into())));
        assert!(Value::Link("Books/Dune".into()).equals(&Value::Link("dune".into())));
        let d = Value::Date(date(2026, 8, 9));
        assert_eq!(
            d.compare(&Value::Text("2026-08-10".into())),
            Some(Ordering::Less)
        );
        assert_eq!(
            Value::Null.sort_order(&n(0.0)),
            Ordering::Less,
            "nulls first"
        );
    }

    #[test]
    fn durations() {
        let d = |t: &str| Dur::parse(t).unwrap_or_else(|| panic!("{t}"));
        assert_eq!(d("1 day").millis, DAY);
        assert_eq!(d("2h 30m").millis, 2 * HOUR + 30 * MINUTE);
        assert_eq!(
            d("1 year, 2 months"),
            Dur {
                months: 14,
                millis: 0
            }
        );
        assert_eq!(d("3 weeks").display(), "21 days");
        assert_eq!(d("1 day 2 hours").display(), "1 day, 2 hours");
        assert!(Dur::parse("soon").is_none());
        let jan31 = NaiveDate::from_ymd_opt(2026, 1, 31)
            .unwrap()
            .and_time(NaiveTime::MIN);
        assert_eq!(
            d("1 month")
                .shift(jan31, false)
                .map(|x| x.date().to_string())
                .as_deref(),
            Some("2026-02-28"),
            "the month's last day"
        );
        assert_eq!(
            Value::Duration(d("1 day")).compare(&Value::Duration(d("23 hours"))),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn display() {
        assert_eq!(Value::Number(3.0).display(), "3");
        assert_eq!(Value::Number(2.5).display(), "2.5");
        assert_eq!(Value::Link("Books/Dune".into()).display(), "Dune");
        assert_eq!(Value::Null.display(), "-");
        assert_eq!(
            Value::List(vec![Value::Text("a".into()), Value::Bool(false)]).display(),
            "a, false"
        );
    }
}
