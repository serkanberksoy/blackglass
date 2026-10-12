//! Expressions in templates (`{{sum()::%.1f}}`): numbers, datasets
//! (`dataset(1)`), the original's functions (`sum`, `average`,
//! `maxStreak`, `startDate` …), `+ - * / %` (on numbers and datasets),
//! and a format after `::` (printf for numbers, moment for dates).

use chrono::NaiveDate;

use super::collect::{Data, format_date};

/// An expression's value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Set(Vec<Option<f64>>),
    Date(NaiveDate),
}

/// `template` with each `{{expression::format}}` replaced by its value.
pub fn fill(template: &str, data: &Data) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let close = after.find("}}").ok_or("Unclosed {{ in the template")?;
        let inner = &after[..close];
        let (expr, format) = match inner.split_once("::") {
            Some((e, f)) => (e, Some(f.trim())),
            None => (inner, None),
        };
        let value = eval(expr, data)?;
        out.push_str(&show(&value, format, data)?);
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// One expression's number (a pie's data, a bullet's value).
pub fn number(text: &str, data: &Data) -> Result<f64, String> {
    let text = text.trim();
    let inner = text
        .strip_prefix("{{")
        .and_then(|t| t.strip_suffix("}}"))
        .unwrap_or(text);
    let expr = inner.split("::").next().unwrap_or(inner);
    match eval(expr, data)? {
        Value::Num(n) => Ok(n),
        _ => Err(format!("'{inner}' isn't a number")),
    }
}

/// A value as text: `format` (printf for a number, moment for a date),
/// else a number as short as it reads, a date in the block's format.
pub fn show(value: &Value, format: Option<&str>, data: &Data) -> Result<String, String> {
    match value {
        Value::Num(n) => Ok(match format {
            Some(f) if !f.is_empty() => printf(*n, f),
            _ => plain(*n),
        }),
        Value::Date(d) => Ok(match format {
            Some(f) if !f.is_empty() => format_date(*d, f),
            _ => data.format(*d),
        }),
        Value::Set(_) => {
            Err("A dataset can't be shown; use a function on it (sum, average …)".into())
        }
    }
}

/// A number with at most two decimals, without trailing zeros.
pub fn plain(n: f64) -> String {
    if !n.is_finite() {
        return "NaN".into();
    }
    let text = format!("{n:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

/// `n` by a printf format (`%d`, `%i`, `%.1f`, `%5.2f`, `%s`, `%%`; `i`,
/// `d`, `.1f` without `%`).
pub fn printf(n: f64, format: &str) -> String {
    let format = if format.contains('%') {
        format.to_string()
    } else {
        format!("%{format}")
    };
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == '0' {
                spec.push(c);
                chars.next();
            } else {
                break;
            }
        }
        let width: usize = spec
            .split('.')
            .next()
            .unwrap_or("")
            .trim_start_matches(['-', '+', '0'])
            .parse()
            .unwrap_or(0);
        let precision: Option<usize> = spec.split_once('.').and_then(|(_, p)| p.parse().ok());
        let text = match chars.next() {
            Some('d' | 'i') => format!("{}", n.round() as i64),
            Some('f' | 'F') => format!("{:.*}", precision.unwrap_or(6), n),
            Some('e') => format!("{:.*e}", precision.unwrap_or(6), n),
            Some('s') => plain(n),
            Some('%') => "%".into(),
            Some(other) => format!("%{spec}{other}"),
            None => "%".into(),
        };
        if spec.starts_with('-') {
            out.push_str(&format!("{text:<width$}"));
        } else {
            out.push_str(&format!("{text:>width$}"));
        }
    }
    out
}

/// Tokens of an expression.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Name(String),
    Op(char),
    Open,
    Close,
    Comma,
}

fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                out.push(Token::Num(
                    s.parse().map_err(|_| format!("Invalid number '{s}'"))?,
                ));
            }
            '+' | '-' | '*' | '/' | '%' => {
                out.push(Token::Op(c));
                i += 1;
            }
            '(' => {
                out.push(Token::Open);
                i += 1;
            }
            ')' => {
                out.push(Token::Close);
                i += 1;
            }
            ',' => {
                out.push(Token::Comma);
                i += 1;
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                out.push(Token::Name(chars[start..i].iter().collect()));
            }
            other => return Err(format!("Unknown character '{other}' in the expression")),
        }
    }
    Ok(out)
}

/// Evaluates `text` on `data`.
pub fn eval(text: &str, data: &Data) -> Result<Value, String> {
    let tokens = tokens(text)?;
    if tokens.is_empty() {
        return Err("An empty expression".into());
    }
    let mut parser = Parser {
        tokens,
        at: 0,
        data,
    };
    let value = parser.sum()?;
    if parser.at < parser.tokens.len() {
        return Err(format!("Unexpected text in '{}'", text.trim()));
    }
    Ok(value)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    at: usize,
    data: &'a Data,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.at).cloned();
        self.at += 1;
        t
    }

    fn sum(&mut self) -> Result<Value, String> {
        let mut left = self.product()?;
        while let Some(&Token::Op(op @ ('+' | '-'))) = self.peek() {
            self.at += 1;
            let right = self.product()?;
            left = binary(op, left, right)?;
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Value, String> {
        let mut left = self.unary()?;
        while let Some(&Token::Op(op @ ('*' | '/' | '%'))) = self.peek() {
            self.at += 1;
            let right = self.unary()?;
            left = binary(op, left, right)?;
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some(Token::Op('-')) => {
                self.at += 1;
                binary('*', Value::Num(-1.0), self.unary()?)
            }
            Some(Token::Op('+')) => {
                self.at += 1;
                self.unary()
            }
            _ => self.atom(),
        }
    }

    fn atom(&mut self) -> Result<Value, String> {
        match self.next() {
            Some(Token::Num(n)) => Ok(Value::Num(n)),
            Some(Token::Open) => {
                let v = self.sum()?;
                match self.next() {
                    Some(Token::Close) => Ok(v),
                    _ => Err("A missing )".into()),
                }
            }
            Some(Token::Name(name)) => {
                let mut args = Vec::new();
                if self.peek() == Some(&Token::Open) {
                    self.at += 1;
                    if self.peek() == Some(&Token::Close) {
                        self.at += 1;
                    } else {
                        loop {
                            args.push(self.sum()?);
                            match self.next() {
                                Some(Token::Comma) => {}
                                Some(Token::Close) => break,
                                _ => return Err(format!("A missing ) after {name}(")),
                            }
                        }
                    }
                }
                call(&name, args, self.data)
            }
            _ => Err("An expression is missing a value".into()),
        }
    }
}

fn binary(op: char, a: Value, b: Value) -> Result<Value, String> {
    let f = |x: f64, y: f64| match op {
        '+' => x + y,
        '-' => x - y,
        '*' => x * y,
        '/' => x / y,
        _ => x % y,
    };
    Ok(match (a, b) {
        (Value::Num(x), Value::Num(y)) => Value::Num(f(x, y)),
        (Value::Set(s), Value::Num(y)) => {
            Value::Set(s.iter().map(|x| x.map(|x| f(x, y))).collect())
        }
        (Value::Num(x), Value::Set(s)) => {
            Value::Set(s.iter().map(|y| y.map(|y| f(x, y))).collect())
        }
        (Value::Set(s), Value::Set(t)) => Value::Set(
            s.iter()
                .zip(&t)
                .map(|(x, y)| Some(f((*x)?, (*y)?)))
                .collect(),
        ),
        _ => return Err(format!("'{op}' doesn't take dates")),
    })
}

/// Runs of days with data (or, `missing`, without): (length, first day
/// index) of each.
fn runs(values: &[Option<f64>], missing: bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, v) in values.iter().enumerate() {
        if v.is_none() == missing {
            start.get_or_insert(i);
        } else if let Some(s) = start.take() {
            out.push((i - s, s));
        }
    }
    if let Some(s) = start {
        out.push((values.len() - s, s));
    }
    out
}

fn call(name: &str, mut args: Vec<Value>, data: &Data) -> Result<Value, String> {
    if name == "dataset" {
        let Some(Value::Num(i)) = args.first() else {
            return Err("dataset() takes an index".into());
        };
        let set = data
            .sets
            .get(*i as usize)
            .ok_or(format!("No dataset {i}"))?;
        return Ok(Value::Set(set.values.clone()));
    }
    // The first dataset drawn when none is given.
    let default = || {
        data.shown()
            .next()
            .map(|(_, s)| s.values.clone())
            .unwrap_or_default()
    };
    // A dataset by number is the old way (`sum(1)`, before 1.9).
    let set = match args.first() {
        Some(Value::Set(s)) => s.clone(),
        Some(Value::Num(i)) => data
            .sets
            .get(*i as usize)
            .ok_or(format!("No dataset {i}"))?
            .values
            .clone(),
        _ => default(),
    };
    let present: Vec<f64> = set.iter().flatten().copied().collect();
    let num = |n: Option<f64>| Value::Num(n.unwrap_or(f64::NAN));
    let day = |i: usize| Value::Date(data.day(i));
    let best = |cmp: fn(f64, f64) -> bool| {
        let mut at: Option<(usize, f64)> = None;
        for (i, v) in set.iter().enumerate() {
            if let Some(v) = v
                && at.is_none_or(|(_, b)| cmp(*v, b))
            {
                at = Some((i, *v));
            }
        }
        at
    };
    let longest = |missing: bool| {
        runs(&set, missing)
            .into_iter()
            .fold(None::<(usize, usize)>, |m, r| match m {
                Some(m) if m.0 >= r.0 => Some(m),
                _ => Some(r),
            })
    };
    // The run that reaches the last day (none if the last day differs).
    let current = |missing: bool| {
        runs(&set, missing)
            .into_iter()
            .find(|(len, start)| start + len == set.len())
    };
    let sum: f64 = present.iter().sum();
    let n = present.len();
    Ok(match name {
        "sum" => Value::Num(sum),
        "first" => num(present.first().copied()),
        "last" => num(present.last().copied()),
        "min" => num(present.iter().copied().reduce(f64::min)),
        "max" => num(present.iter().copied().reduce(f64::max)),
        "average" => num((n > 0).then(|| sum / n as f64)),
        "median" => {
            let mut sorted = present.clone();
            sorted.sort_by(f64::total_cmp);
            num((n > 0).then(|| {
                if n % 2 == 1 {
                    sorted[n / 2]
                } else {
                    (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
                }
            }))
        }
        "variance" => num((n > 0).then(|| {
            let mean = sum / n as f64;
            present.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64
        })),
        "numTargets" => Value::Num(match args.first() {
            Some(Value::Set(_)) => n as f64,
            Some(Value::Num(i)) => data.sets.get(*i as usize).map_or(0, |s| s.found) as f64,
            _ => data.shown().next().map_or(0, |(_, s)| s.found) as f64,
        }),
        "numDays" => Value::Num(set.len() as f64),
        "numDaysHavingData" => Value::Num(n as f64),
        "startDate" => Value::Date(data.start),
        "endDate" => Value::Date(data.end()),
        "minDate" => best(|v, b| v < b).map_or(Value::Num(f64::NAN), |(i, _)| day(i)),
        "maxDate" => best(|v, b| v > b).map_or(Value::Num(f64::NAN), |(i, _)| day(i)),
        "maxStreak" => Value::Num(longest(false).map_or(0, |r| r.0) as f64),
        "maxStreakStart" => longest(false).map_or(Value::Num(f64::NAN), |r| day(r.1)),
        "maxStreakEnd" => longest(false).map_or(Value::Num(f64::NAN), |r| day(r.1 + r.0 - 1)),
        "currentStreak" => Value::Num(current(false).map_or(0, |r| r.0) as f64),
        "currentStreakStart" => current(false).map_or(Value::Num(f64::NAN), |r| day(r.1)),
        "currentStreakEnd" => current(false).map_or(Value::Num(f64::NAN), |r| day(r.1 + r.0 - 1)),
        "maxBreaks" => Value::Num(longest(true).map_or(0, |r| r.0) as f64),
        "maxBreaksStart" => longest(true).map_or(Value::Num(f64::NAN), |r| day(r.1)),
        "maxBreaksEnd" => longest(true).map_or(Value::Num(f64::NAN), |r| day(r.1 + r.0 - 1)),
        "currentBreaks" => Value::Num(current(true).map_or(0, |r| r.0) as f64),
        "currentBreaksStart" => current(true).map_or(Value::Num(f64::NAN), |r| day(r.1)),
        "currentBreaksEnd" => current(true).map_or(Value::Num(f64::NAN), |r| day(r.1 + r.0 - 1)),
        "normalize" => {
            let (lo, hi) = (
                present.iter().copied().fold(f64::INFINITY, f64::min),
                present.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            );
            let span = if hi > lo { hi - lo } else { 1.0 };
            Value::Set(set.iter().map(|v| v.map(|v| (v - lo) / span)).collect())
        }
        "setMissingValues" => {
            let fill = match args.get_mut(1) {
                Some(Value::Num(n)) => *n,
                _ => return Err("setMissingValues(dataset, value) takes a number".into()),
            };
            Value::Set(set.iter().map(|v| Some(v.unwrap_or(fill))).collect())
        }
        other => return Err(format!("Unknown function '{other}'")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::tracker::collect::Dataset;

    fn data() -> Data {
        let set = |name: &str, values: Vec<Option<f64>>| Dataset {
            name: name.into(),
            found: values.iter().flatten().count(),
            values,
            is_x: false,
        };
        Data {
            start: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            days: 6,
            sets: vec![
                set(
                    "a",
                    vec![Some(1.0), Some(2.0), None, Some(4.0), Some(5.0), Some(6.0)],
                ),
                set("b", vec![Some(10.0), None, None, None, Some(1.0), None]),
            ],
            notes: Default::default(),
            date_format: "YYYY-MM-DD".into(),
        }
    }

    fn t(template: &str) -> String {
        fill(template, &data()).unwrap_or_else(|e| format!("ERR {e}"))
    }

    #[test]
    fn functions_over_the_first_dataset_or_one_given() {
        assert_eq!(
            t("{{sum()}} {{min()}} {{max()}} {{first()}} {{last()}}"),
            "18 1 6 1 6"
        );
        assert_eq!(
            t("{{average()}} {{median()}} {{variance()::%.2f}}"),
            "3.6 4 3.44"
        );
        assert_eq!(
            t("{{numDays()}} {{numDaysHavingData()}} {{numTargets()}}"),
            "6 5 5"
        );
        assert_eq!(t("{{sum(dataset(1))}} {{maxBreaks(dataset(1))}}"), "11 3");
        assert_eq!(
            t("{{maxStreak()}} from {{maxStreakStart()}} to {{maxStreakEnd()::D MMM}}"),
            "3 from 2026-10-04 to 6 Oct"
        );
        assert_eq!(
            t("{{currentStreak()}} {{currentBreaks(dataset(1))}}"),
            "3 1"
        );
        assert_eq!(
            t("{{startDate()}}…{{endDate()::DD.MM}} {{maxDate()}}"),
            "2026-10-01…06.10 2026-10-06"
        );
    }

    #[test]
    fn operators_formats_and_errors() {
        assert_eq!(t("{{sum() / numDays() * 2::%.1f}}"), "6.0");
        assert_eq!(t("{{-(1 + 2) % 2}} {{10 / 4}}"), "-1 2.5");
        assert_eq!(
            t("{{sum(dataset(0) + dataset(1))}}"),
            "17",
            "missing in either: missing"
        );
        assert_eq!(t("{{sum(setMissingValues(dataset(1), 0) * 2)}}"), "22");
        assert_eq!(t("{{max(normalize(dataset(0)))}}"), "1");
        assert_eq!(t("{{sum()::i}} {{average()::%5.1f}}|"), "18   3.6|");
        assert_eq!(t("{{nope()}}"), "ERR Unknown function 'nope'");
        // The old way (before 1.9): no parentheses, a dataset by number.
        assert_eq!(t("{{sum}} {{sum(1)}} {{maxStreak(1)}}"), "18 11 1");
        assert_eq!(
            t("{{dataset(0)}}"),
            "ERR A dataset can't be shown; use a function on it (sum, average …)"
        );
        assert_eq!(t("{{sum("), "ERR Unclosed {{ in the template");
        assert_eq!(number("{{sum() + 2}}", &data()), Ok(20.0));
        assert_eq!(number("7", &data()), Ok(7.0));
    }
}
