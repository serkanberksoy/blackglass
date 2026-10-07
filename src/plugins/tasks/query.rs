//! ```` ```tasks ```` queries (TK-30 … TK-45): one instruction per line,
//! `#` comments. Filters (status, dates, priority, recurrence, file,
//! description, tags, dependencies) combine with AND / OR / NOT / XOR in
//! parentheses; then `sort by`, `group by`, `limit`, `hide` / `show`,
//! `short mode`, `explain`, `ignore global query`.

use std::collections::HashSet;

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

use super::task::{DateField, Priority, Statuses, Task, Type, weekday};

/// A parsed query.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Query {
    pub filters: Vec<Filter>,
    pub sorts: Vec<(Key, bool)>,
    pub groups: Vec<(Key, bool)>,
    pub limit: Option<usize>,
    /// Tasks nested under another list item left out.
    pub exclude_sub: bool,
    /// Columns side by side instead of groups below each other.
    pub columns: Option<Key>,
    pub group_limit: Option<usize>,
    /// What's hidden: `task count`, `backlink`, `priority`, `due date` ….
    pub hidden: HashSet<String>,
    pub short: bool,
    pub explain: bool,
    pub ignore_global: bool,
    /// The instructions as written, for `explain`.
    pub lines: Vec<String>,
    /// The JavaScript of its `… by function` instructions, in order
    /// (`Filter::Function` and `Key::Function` are indexes into it).
    pub functions: Vec<String>,
}

/// A filter.
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Done(bool),
    StatusName(Text),
    StatusType(Type, bool),
    StatusSymbol(char, bool),
    Date(DateKey, Compare, (NaiveDate, NaiveDate)),
    HasDate(DateKey, bool),
    DateInvalid(DateKey),
    Recurring(bool),
    RecurrenceIncludes(Text),
    Priority(Compare, Priority),
    Path(PathPart, Text),
    Description(Text),
    HasTags(bool),
    Tag(Text),
    HasId(bool),
    IdIncludes(Text),
    HasDepends(bool),
    Blocked(bool),
    Blocking(bool),
    Not(Box<Filter>),
    And(Box<Filter>, Box<Filter>),
    Or(Box<Filter>, Box<Filter>),
    Xor(Box<Filter>, Box<Filter>),
    /// `filter by function …`: the query's function `.0` is true.
    Function(usize, String),
}

/// `includes X` / `does not include X` / `is X` / `is not X` (without
/// case), or `regex matches /X/flags` / `regex does not match /X/flags`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub text: String,
    pub exact: bool,
    pub negated: bool,
    pub regex: Option<Pattern>,
}

/// A compiled regular expression (equal when written the same).
#[derive(Debug, Clone)]
pub struct Pattern(pub regex::Regex);

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for Pattern {}

impl Text {
    fn matches(&self, value: &str) -> bool {
        if let Some(Pattern(re)) = &self.regex {
            return re.is_match(value) != self.negated;
        }
        let (value, text) = (value.to_lowercase(), self.text.to_lowercase());
        let hit = if self.exact {
            value == text
        } else {
            value.contains(&text)
        };
        hit != self.negated
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPart {
    Path,
    Root,
    Folder,
    Filename,
    Heading,
}

/// A date filter's date: one of the six, or `happens`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateKey {
    Field(DateField),
    Happens,
}

impl DateKey {
    fn of(self, task: &Task) -> Option<NaiveDate> {
        match self {
            DateKey::Field(f) => task.date(f),
            DateKey::Happens => task.happens(),
        }
    }

    fn name(self) -> &'static str {
        match self {
            DateKey::Field(f) => f.name(),
            DateKey::Happens => "happens",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compare {
    On,
    Before,
    After,
    OnOrBefore,
    OnOrAfter,
    Not,
    Above,
    Below,
}

/// What results are sorted or grouped by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// `sort by function` / `group by function`: the query's function.
    Function(usize),
    Status,
    StatusName,
    StatusType,
    Priority,
    Urgency,
    Date(DateKey),
    Description,
    Path,
    Root,
    Folder,
    Filename,
    Heading,
    Backlink,
    Tags,
    Recurring,
    Recurrence,
    Id,
}

/// Everything a filter needs besides the task.
pub struct Env<'a> {
    pub today: NaiveDate,
    pub statuses: &'a Statuses,
    /// Ids of tasks not done (for "blocked").
    pub open_ids: &'a HashSet<String>,
    /// Ids that open tasks depend on (for "blocking").
    pub depended_on: &'a HashSet<String>,
    /// The query's functions, worked out for every task.
    pub functions: Option<&'a super::function::Computed>,
}

/// Parses a query block (and the global query's lines before it, unless
/// it says `ignore global query`).
pub fn parse(text: &str, global: &str, today: NaiveDate) -> Result<Query, String> {
    let mut query = Query {
        hidden: HashSet::from(["urgency".to_string(), "tree".to_string()]),
        ..Query::default()
    };
    let own: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let ignore = own
        .iter()
        .any(|l| l.eq_ignore_ascii_case("ignore global query"));
    let global: Vec<&str> = if ignore {
        Vec::new()
    } else {
        global
            .split(';')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect()
    };
    for line in global.into_iter().chain(own) {
        instruction(&mut query, line, today).map_err(|e| format!("{line}: {e}"))?;
        query.lines.push(line.to_string());
    }
    Ok(query)
}

fn instruction(q: &mut Query, line: &str, today: NaiveDate) -> Result<(), String> {
    let lower = line.to_lowercase();
    // `… by function [reverse] <JavaScript>` (the code as written).
    for start in [
        "filter by function ",
        "sort by function ",
        "group by function ",
    ] {
        if lower.starts_with(start) {
            let code = line[start.len()..].trim();
            let (reverse, code) = match code.strip_prefix("reverse ") {
                Some(rest) if !start.starts_with("filter") => (true, rest.trim()),
                _ => (false, code),
            };
            if code.is_empty() {
                return Err("a function needs its JavaScript".into());
            }
            let i = q.functions.len();
            q.functions.push(code.to_string());
            match start {
                "filter by function " => q.filters.push(Filter::Function(i, code.to_string())),
                "sort by function " => q.sorts.push((Key::Function(i), reverse)),
                _ => q.groups.push((Key::Function(i), reverse)),
            }
            return Ok(());
        }
    }
    if let Some(rest) = lower.strip_prefix("sort by ") {
        q.sorts.push(key_with_reverse(rest)?);
        return Ok(());
    }
    if let Some(rest) = lower.strip_prefix("group by ") {
        q.groups.push(key_with_reverse(rest)?);
        return Ok(());
    }
    if let Some(rest) = lower.strip_prefix("columns by ") {
        q.columns = Some(key_with_reverse(rest)?.0);
        return Ok(());
    }
    if let Some(rest) = lower.strip_prefix("limit groups") {
        q.group_limit = Some(number(rest)?);
        return Ok(());
    }
    if let Some(rest) = lower.strip_prefix("limit") {
        q.limit = Some(number(rest)?);
        return Ok(());
    }
    for (prefix, hide) in [("hide ", true), ("show ", false)] {
        if let Some(what) = lower.strip_prefix(prefix) {
            let what = what.trim().trim_end_matches('s').to_string();
            if hide {
                q.hidden.insert(what);
            } else {
                q.hidden.remove(&what);
            }
            return Ok(());
        }
    }
    match lower.as_str() {
        "short mode" | "short" => q.short = true,
        "exclude sub-items" | "exclude sub-item" => q.exclude_sub = true,
        "full mode" | "full" => q.short = false,
        "explain" => q.explain = true,
        "ignore global query" => q.ignore_global = true,
        _ => q.filters.push(filter(line, today)?),
    }
    Ok(())
}

fn number(rest: &str) -> Result<usize, String> {
    rest.trim()
        .trim_start_matches("to")
        .trim()
        .trim_end_matches("tasks")
        .trim_end_matches("task")
        .trim()
        .parse()
        .map_err(|_| "a number of tasks".to_string())
}

fn key_with_reverse(rest: &str) -> Result<(Key, bool), String> {
    let rest = rest.trim();
    let (name, reverse) = match rest.strip_suffix("reverse") {
        Some(r) => (r.trim(), true),
        None => (rest, false),
    };
    let key = match name {
        "status" => Key::Status,
        "status.name" => Key::StatusName,
        "status.type" => Key::StatusType,
        "priority" => Key::Priority,
        "urgency" => Key::Urgency,
        "description" => Key::Description,
        "path" => Key::Path,
        "root" => Key::Root,
        "folder" => Key::Folder,
        "filename" => Key::Filename,
        "heading" => Key::Heading,
        "backlink" => Key::Backlink,
        "tag" | "tags" => Key::Tags,
        "recurring" => Key::Recurring,
        "recurrence" => Key::Recurrence,
        "id" => Key::Id,
        other => {
            Key::Date(date_key(other).ok_or_else(|| format!("can't sort or group by {other}"))?)
        }
    };
    Ok((key, reverse))
}

fn date_key(word: &str) -> Option<DateKey> {
    let word = word.trim();
    // `sort by due date` as well as `sort by due`.
    let word = word.strip_suffix(" date").unwrap_or(word);
    if word == "happens" {
        return Some(DateKey::Happens);
    }
    let word = if word == "starts" { "start" } else { word };
    DateField::ALL
        .iter()
        .find(|(_, _, n, _)| *n == word)
        .map(|(f, _, _, _)| DateKey::Field(*f))
}

/// One filter line, or a boolean combination of parenthesized ones.
fn filter(line: &str, today: NaiveDate) -> Result<Filter, String> {
    let line = line.trim();
    if line.starts_with('(') || line.to_uppercase().starts_with("NOT (") {
        return Boolean::new(line, today).parse();
    }
    // `a AND b` without brackets: read as `(a) AND (b)` (if that doesn't
    // make sense, as one filter after all).
    if let Some(bracketed) = bracket_operands(line)
        && let Ok(f) = Boolean::new(&bracketed, today).parse()
    {
        return Ok(f);
    }
    simple(line, today)
}

/// `a AND b OR NOT c` → `(a) AND (b) OR NOT (c)`; `None` without an
/// operator (`AND`, `OR`, `XOR`, `NOT` in capitals, outside quotes and
/// brackets).
fn bracket_operands(line: &str) -> Option<String> {
    let mut out = String::new();
    let mut operand: Vec<&str> = Vec::new();
    let (mut depth, mut quoted, mut found) = (0i32, false, false);
    for word in line.split(' ') {
        let top = depth == 0 && !quoted;
        if top && matches!(word, "AND" | "OR" | "XOR" | "NOT") {
            if !operand.is_empty() {
                out.push_str(&format!("({}) ", operand.join(" ")));
                operand.clear();
            }
            out.push_str(word);
            out.push(' ');
            found = true;
            continue;
        }
        for c in word.chars() {
            match c {
                '"' => quoted = !quoted,
                '(' if !quoted => depth += 1,
                ')' if !quoted => depth -= 1,
                _ => {}
            }
        }
        operand.push(word);
    }
    if !operand.is_empty() {
        out.push_str(&format!("({})", operand.join(" ")));
    }
    found.then(|| out.trim().to_string())
}

/// `(a) AND (b) OR NOT (c)`: left to right, NOT binding tightest,
/// brackets nesting.
struct Boolean<'a> {
    text: &'a str,
    at: usize,
    today: NaiveDate,
}

impl<'a> Boolean<'a> {
    fn new(text: &'a str, today: NaiveDate) -> Self {
        Boolean { text, at: 0, today }
    }

    fn rest(&self) -> &'a str {
        self.text[self.at..].trim_start()
    }

    fn skip(&mut self) {
        let rest = &self.text[self.at..];
        self.at += rest.len() - rest.trim_start().len();
    }

    fn word(&mut self, word: &str) -> bool {
        self.skip();
        let rest = &self.text[self.at..];
        let hit = rest.len() >= word.len()
            && rest[..word.len()].eq_ignore_ascii_case(word)
            && rest[word.len()..].starts_with([' ', '(']);
        if hit {
            self.at += word.len();
        }
        hit
    }

    fn parse(mut self) -> Result<Filter, String> {
        let filter = self.expr()?;
        if !self.rest().is_empty() {
            return Err(format!("unexpected {:?}", self.rest()));
        }
        Ok(filter)
    }

    fn expr(&mut self) -> Result<Filter, String> {
        let mut left = self.unary()?;
        loop {
            left = if self.word("AND") {
                let not = self.word("NOT");
                let right = self.unary()?;
                let right = if not {
                    Filter::Not(Box::new(right))
                } else {
                    right
                };
                Filter::And(Box::new(left), Box::new(right))
            } else if self.word("OR") {
                let not = self.word("NOT");
                let right = self.unary()?;
                let right = if not {
                    Filter::Not(Box::new(right))
                } else {
                    right
                };
                Filter::Or(Box::new(left), Box::new(right))
            } else if self.word("XOR") {
                Filter::Xor(Box::new(left), Box::new(self.unary()?))
            } else {
                return Ok(left);
            };
        }
    }

    fn unary(&mut self) -> Result<Filter, String> {
        if self.word("NOT") {
            return Ok(Filter::Not(Box::new(self.unary()?)));
        }
        self.skip();
        if !self.text[self.at..].starts_with('(') {
            return Err(format!("a ( expected at {:?}", self.rest()));
        }
        // The matching bracket.
        let mut depth = 0;
        let start = self.at;
        for (i, c) in self.text[start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        let inner = &self.text[start + 1..start + i];
                        self.at = start + i + 1;
                        let inner = inner.trim();
                        return if inner.starts_with('(') || inner.to_uppercase().starts_with("NOT")
                        {
                            Boolean::new(inner, self.today).parse()
                        } else {
                            simple(inner, self.today)
                        };
                    }
                }
                _ => {}
            }
        }
        Err("a ( without its )".into())
    }
}

/// A filter that isn't a combination.
fn simple(line: &str, today: NaiveDate) -> Result<Filter, String> {
    let lower = line.trim().to_lowercase();
    let original = line.trim();
    let tail = |prefix: &str| original[prefix.len()..].trim().to_string();
    let fixed = match lower.as_str() {
        "done" => Some(Filter::Done(true)),
        "not done" => Some(Filter::Done(false)),
        "is recurring" => Some(Filter::Recurring(true)),
        "is not recurring" => Some(Filter::Recurring(false)),
        "has tags" | "has tag" => Some(Filter::HasTags(true)),
        "no tags" | "no tag" => Some(Filter::HasTags(false)),
        "has id" => Some(Filter::HasId(true)),
        "no id" => Some(Filter::HasId(false)),
        "has depends on" => Some(Filter::HasDepends(true)),
        "no depends on" => Some(Filter::HasDepends(false)),
        "is blocked" => Some(Filter::Blocked(true)),
        "is not blocked" => Some(Filter::Blocked(false)),
        "is blocking" => Some(Filter::Blocking(true)),
        "is not blocking" => Some(Filter::Blocking(false)),
        _ => None,
    };
    if let Some(f) = fixed {
        return Ok(f);
    }
    if let Some(rest) = lower.strip_prefix("status.type ") {
        let (negated, name) = is_not(rest.trim());
        let kind = Type::parse(name).ok_or_else(|| format!("no status type {name}"))?;
        return Ok(Filter::StatusType(kind, negated));
    }
    if lower.starts_with("status.symbol ") {
        let rest = tail("status.symbol ");
        let (negated, symbol) = is_not(&rest);
        let mut chars = symbol.chars();
        let c = match (chars.next(), chars.next()) {
            (None, _) => ' ',
            (Some(c), None) => c,
            _ => return Err("status.symbol is one character".into()),
        };
        return Ok(Filter::StatusSymbol(c, negated));
    }
    if lower.starts_with("status.name ") {
        return Ok(Filter::StatusName(text_match(&tail("status.name "))?));
    }
    for (prefix, part) in [
        ("path ", PathPart::Path),
        ("root ", PathPart::Root),
        ("folder ", PathPart::Folder),
        ("filename ", PathPart::Filename),
        ("heading ", PathPart::Heading),
    ] {
        if lower.starts_with(prefix) {
            return Ok(Filter::Path(part, text_match(&tail(prefix))?));
        }
    }
    if lower.starts_with("description ") {
        return Ok(Filter::Description(text_match(&tail("description "))?));
    }
    for prefix in ["tags ", "tag "] {
        if lower.starts_with(prefix) {
            let mut rest = tail(prefix);
            for (from, to) in [
                ("include ", "includes "),
                ("do not include ", "does not include "),
            ] {
                if rest.to_lowercase().starts_with(from) {
                    rest = format!("{to}{}", &rest[from.len()..]);
                }
            }
            return Ok(Filter::Tag(text_match(&rest)?));
        }
    }
    if lower.starts_with("recurrence ") {
        return Ok(Filter::RecurrenceIncludes(text_match(&tail(
            "recurrence ",
        ))?));
    }
    if lower.starts_with("id ") {
        return Ok(Filter::IdIncludes(text_match(&tail("id "))?));
    }
    if let Some(rest) = lower.strip_prefix("priority is ") {
        let rest = rest.trim();
        let (compare, name) = [
            ("above ", Compare::Above),
            ("below ", Compare::Below),
            ("not ", Compare::Not),
        ]
        .into_iter()
        .find_map(|(p, c)| rest.strip_prefix(p).map(|n| (c, n)))
        .unwrap_or((Compare::On, rest));
        let p = Priority::parse(name).ok_or_else(|| format!("no priority {name}"))?;
        return Ok(Filter::Priority(compare, p));
    }
    for (prefix, has) in [("has ", true), ("no ", false)] {
        if let Some(rest) = lower.strip_prefix(prefix)
            && let Some(key) = rest.strip_suffix(" date").and_then(date_key)
        {
            return Ok(Filter::HasDate(key, has));
        }
    }
    if let Some(key) = lower.strip_suffix(" date is invalid").and_then(date_key) {
        return Ok(Filter::DateInvalid(key));
    }
    // `due before tomorrow`, `happens in this week`, `done 2026-10-01`.
    let (word, rest) = lower.split_once(' ').unwrap_or((&lower, ""));
    if let Some(key) = date_key(word) {
        let rest = rest.trim();
        let rest = rest.strip_prefix("date ").unwrap_or(rest);
        let (compare, phrase) = [
            ("on or before ", Compare::OnOrBefore),
            ("on or after ", Compare::OnOrAfter),
            ("before ", Compare::Before),
            ("after ", Compare::After),
            ("on ", Compare::On),
            ("in ", Compare::On),
            ("is ", Compare::On),
        ]
        .into_iter()
        .find_map(|(p, c)| rest.strip_prefix(p).map(|r| (c, r)))
        .unwrap_or((Compare::On, rest));
        let range =
            date_range(phrase, today).ok_or_else(|| format!("can't read the date {phrase:?}"))?;
        return Ok(Filter::Date(key, compare, range));
    }
    Err("unknown instruction".into())
}

/// `is X`, `is not X` → (negated, X).
fn is_not(rest: &str) -> (bool, &str) {
    if let Some(x) = rest.strip_prefix("is not ") {
        (true, x.trim())
    } else {
        (false, rest.strip_prefix("is ").unwrap_or(rest).trim())
    }
}

fn text_match(rest: &str) -> Result<Text, String> {
    let lower = rest.to_lowercase();
    for (prefix, exact, negated) in [
        ("does not include ", false, true),
        ("includes ", false, false),
        ("include ", false, false),
        ("is not ", true, true),
        ("is ", true, false),
    ] {
        if lower.starts_with(prefix) {
            return Ok(Text {
                text: rest[prefix.len()..].trim().to_string(),
                exact,
                negated,
                regex: None,
            });
        }
    }
    for (prefix, negated) in [("regex matches ", false), ("regex does not match ", true)] {
        if lower.starts_with(prefix) {
            let text = rest[prefix.len()..].trim().to_string();
            return Ok(Text {
                regex: Some(Pattern(crate::plugins::slash_regex(&text)?)),
                text,
                exact: false,
                negated,
            });
        }
    }
    Err("includes, does not include, is or is not".into())
}

/// A date or a range of dates, as a (first, last) pair: `today`,
/// `tomorrow`, `yesterday`, `2026-10-01`, `in 3 days`, `2 weeks ago`,
/// `next monday`, `friday`, `2026-10-01 2026-10-31`, `last / this / next
/// week / month / quarter / year`, `2026-W40`, `2026-10`, `2026-Q4`,
/// `2026`.
pub fn date_range(phrase: &str, today: NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
    let phrase = phrase.trim().to_lowercase();
    let words: Vec<&str> = phrase.split_whitespace().collect();
    if let [a, b] = words[..]
        && let (Some(a), Some(b)) = (single(a, today), single(b, today))
    {
        return Some((a.min(b), a.max(b)));
    }
    if let [which @ ("last" | "this" | "next"), unit] = words[..]
        && let Some(range) = period(which, unit, today)
    {
        return Some(range);
    }
    if let Some(d) = date(&phrase, today) {
        return Some((d, d));
    }
    // 2026-W40, 2026-10, 2026-Q4, 2026.
    if let Some((y, w)) = phrase.split_once("-w") {
        let first = NaiveDate::from_isoywd_opt(y.parse().ok()?, w.parse().ok()?, Weekday::Mon)?;
        return Some((first, first + Days::new(6)));
    }
    if let Some((y, q)) = phrase.split_once("-q") {
        let q: u32 = q.parse().ok()?;
        let first = NaiveDate::from_ymd_opt(y.parse().ok()?, (q.checked_sub(1)?) * 3 + 1, 1)?;
        return Some((first, first.checked_add_months(Months::new(3))?.pred_opt()?));
    }
    if let Some((y, m)) = phrase.split_once('-') {
        let first = NaiveDate::from_ymd_opt(y.parse().ok()?, m.parse().ok()?, 1)?;
        return Some((first, first.checked_add_months(Months::new(1))?.pred_opt()?));
    }
    let year: i32 = phrase.parse().ok().filter(|y| (1000..=9999).contains(y))?;
    Some((
        NaiveDate::from_ymd_opt(year, 1, 1)?,
        NaiveDate::from_ymd_opt(year, 12, 31)?,
    ))
}

fn single(word: &str, today: NaiveDate) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(word, "%Y-%m-%d")
        .ok()
        .or_else(|| date(word, today))
}

/// One date in words (the create-or-edit window takes these too).
pub fn date(phrase: &str, today: NaiveDate) -> Option<NaiveDate> {
    let phrase = phrase.trim().to_lowercase();
    if let Ok(d) = NaiveDate::parse_from_str(&phrase, "%Y-%m-%d") {
        return Some(d);
    }
    match phrase.as_str() {
        "today" => return Some(today),
        "tomorrow" => return today.succ_opt(),
        "yesterday" => return today.pred_opt(),
        _ => {}
    }
    let words: Vec<&str> = phrase.split_whitespace().collect();
    let shift = |n: &str, unit: &str, forward: bool| -> Option<NaiveDate> {
        let n: u32 = count(n)?;
        let unit = unit.trim_end_matches('s');
        match (unit, forward) {
            ("day", true) => today.checked_add_days(Days::new(n.into())),
            ("day", false) => today.checked_sub_days(Days::new(n.into())),
            ("week", true) => today.checked_add_days(Days::new(7 * u64::from(n))),
            ("week", false) => today.checked_sub_days(Days::new(7 * u64::from(n))),
            ("month", true) => today.checked_add_months(Months::new(n)),
            ("month", false) => today.checked_sub_months(Months::new(n)),
            ("year", true) => today.checked_add_months(Months::new(12 * n)),
            ("year", false) => today.checked_sub_months(Months::new(12 * n)),
            _ => None,
        }
    };
    match words[..] {
        ["in", n, unit] => shift(n, unit, true),
        [n, unit, "ago"] => shift(n, unit, false),
        [n, unit] if count(n).is_some() => shift(n, unit, true),
        ["next", day] => next_weekday(today, weekday(day)?, true),
        ["last", day] => next_weekday(today, weekday(day)?, false),
        [day] => next_weekday(today, weekday(day)?, true),
        _ => None,
    }
}

/// A count in a date: `3`, or in words (`a`, `an`, `one` … `twelve`).
fn count(word: &str) -> Option<u32> {
    const WORDS: [&str; 12] = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
        "twelve",
    ];
    match word {
        "a" | "an" => Some(1),
        w => w
            .parse()
            .ok()
            .or_else(|| WORDS.iter().position(|n| *n == w).map(|i| i as u32 + 1)),
    }
}

/// The next (or last) `day` after (before) `today`.
fn next_weekday(today: NaiveDate, day: Weekday, forward: bool) -> Option<NaiveDate> {
    let mut d = today;
    for _ in 0..7 {
        d = if forward {
            d.succ_opt()?
        } else {
            d.pred_opt()?
        };
        if d.weekday() == day {
            return Some(d);
        }
    }
    None
}

fn period(which: &str, unit: &str, today: NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
    let step: i32 = match which {
        "last" => -1,
        "next" => 1,
        _ => 0,
    };
    let add_months = |d: NaiveDate, n: i32| {
        if n >= 0 {
            d.checked_add_months(Months::new(n as u32))
        } else {
            d.checked_sub_months(Months::new(n.unsigned_abs()))
        }
    };
    match unit {
        "week" => {
            let monday = today - Days::new(u64::from(today.weekday().num_days_from_monday()));
            let first = if step >= 0 {
                monday + Days::new(7 * step as u64)
            } else {
                monday - Days::new(7)
            };
            Some((first, first + Days::new(6)))
        }
        "month" | "quarter" | "year" => {
            let len = match unit {
                "month" => 1,
                "quarter" => 3,
                _ => 12,
            };
            let start_month = (today.month0() / len) * len;
            let first = NaiveDate::from_ymd_opt(today.year(), start_month + 1, 1)?;
            let first = add_months(first, step * len as i32)?;
            let last = first.checked_add_months(Months::new(len))?.pred_opt()?;
            Some((first, last))
        }
        _ => None,
    }
}

impl Filter {
    pub fn matches(&self, t: &Task, env: &Env) -> bool {
        let status = env.statuses.get(t.status);
        match self {
            Filter::Function(i, _) => env
                .functions
                .is_none_or(|c| super::function::truthy(c.get(t, *i))),
            Filter::Done(done) => {
                matches!(status.kind, Type::Done | Type::Cancelled | Type::NonTask) == *done
            }
            Filter::StatusName(text) => text.matches(&status.name),
            Filter::StatusType(kind, negated) => (status.kind == *kind) != *negated,
            Filter::StatusSymbol(c, negated) => (t.status == *c) != *negated,
            Filter::Date(key, compare, (first, last)) => key.of(t).is_some_and(|d| match compare {
                Compare::Before => d < *first,
                Compare::After => d > *last,
                Compare::OnOrBefore => d <= *last,
                Compare::OnOrAfter => d >= *first,
                _ => d >= *first && d <= *last,
            }),
            Filter::HasDate(key, has) => key.of(t).is_some() == *has,
            Filter::DateInvalid(key) => match key {
                DateKey::Field(f) => t.invalid.contains(f),
                DateKey::Happens => !t.invalid.is_empty(),
            },
            Filter::Recurring(yes) => t.recurrence.is_some() == *yes,
            Filter::RecurrenceIncludes(text) => {
                t.recurrence.as_deref().is_some_and(|r| text.matches(r))
            }
            // Higher priorities are smaller.
            Filter::Priority(compare, p) => match compare {
                Compare::Above => t.priority < *p,
                Compare::Below => t.priority > *p,
                Compare::Not => t.priority != *p,
                _ => t.priority == *p,
            },
            Filter::Path(part, text) => text.matches(&path_part(t, *part)),
            Filter::Description(text) => text.matches(&t.description),
            Filter::HasTags(has) => t.tags.is_empty() != *has,
            Filter::Tag(text) => {
                let hit = t.tags.iter().any(|tag| {
                    let needle = Text {
                        negated: false,
                        ..text.clone()
                    };
                    needle.matches(tag) || needle.matches(tag.trim_start_matches('#'))
                });
                hit != text.negated
            }
            Filter::HasId(has) => t.id.is_some() == *has,
            Filter::IdIncludes(text) => t.id.as_deref().is_some_and(|id| text.matches(id)),
            Filter::HasDepends(has) => t.depends.is_empty() != *has,
            Filter::Blocked(yes) => {
                let open = !matches!(status.kind, Type::Done | Type::Cancelled);
                let blocked = open && t.depends.iter().any(|d| env.open_ids.contains(d));
                blocked == *yes
            }
            Filter::Blocking(yes) => {
                let open = !matches!(status.kind, Type::Done | Type::Cancelled);
                let blocking = open
                    && t.id
                        .as_deref()
                        .is_some_and(|id| env.depended_on.contains(id));
                blocking == *yes
            }
            Filter::Not(f) => !f.matches(t, env),
            Filter::And(a, b) => a.matches(t, env) && b.matches(t, env),
            Filter::Or(a, b) => a.matches(t, env) || b.matches(t, env),
            Filter::Xor(a, b) => a.matches(t, env) != b.matches(t, env),
        }
    }

    /// The filter in words, for `explain`.
    pub fn explain(&self) -> String {
        let text = |t: &Text| {
            if t.regex.is_some() {
                let verb = if t.negated {
                    "regex does not match"
                } else {
                    "regex matches"
                };
                return format!("{verb} {}", t.text);
            }
            let verb = match (t.exact, t.negated) {
                (true, false) => "is",
                (true, true) => "is not",
                (false, false) => "includes",
                (false, true) => "does not include",
            };
            format!("{verb} {}", t.text)
        };
        match self {
            Filter::Function(_, code) => format!("filter by function {code}"),
            Filter::Done(true) => "done".into(),
            Filter::Done(false) => "not done".into(),
            Filter::StatusName(t) => format!("status.name {}", text(t)),
            Filter::StatusType(k, n) => format!(
                "status.type is{} {}",
                if *n { " not" } else { "" },
                k.name()
            ),
            Filter::StatusSymbol(c, n) => {
                format!("status.symbol is{} {c}", if *n { " not" } else { "" })
            }
            Filter::Date(key, compare, (a, b)) => {
                let how = match compare {
                    Compare::Before => "before",
                    Compare::After => "after",
                    Compare::OnOrBefore => "on or before",
                    Compare::OnOrAfter => "on or after",
                    _ => {
                        if a == b {
                            "on"
                        } else {
                            "in"
                        }
                    }
                };
                let when = |d: &NaiveDate| d.format("%Y-%m-%d (%A %-d %B %Y)").to_string();
                let dates = match compare {
                    Compare::Before | Compare::OnOrAfter => when(a),
                    Compare::After | Compare::OnOrBefore => when(b),
                    _ if a == b => when(a),
                    _ => format!("{} to {}", when(a), when(b)),
                };
                format!("{} date is {how} {dates}", key.name())
            }
            Filter::HasDate(key, true) => format!("has a {} date", key.name()),
            Filter::HasDate(key, false) => format!("has no {} date", key.name()),
            Filter::DateInvalid(key) => format!("{} date is invalid", key.name()),
            Filter::Recurring(true) => "is recurring".into(),
            Filter::Recurring(false) => "is not recurring".into(),
            Filter::RecurrenceIncludes(t) => format!("recurrence {}", text(t)),
            Filter::Priority(c, p) => {
                let how = match c {
                    Compare::Above => "above ",
                    Compare::Below => "below ",
                    Compare::Not => "not ",
                    _ => "",
                };
                format!("priority is {how}{}", p.name())
            }
            Filter::Path(part, t) => {
                let name = match part {
                    PathPart::Path => "path",
                    PathPart::Root => "root",
                    PathPart::Folder => "folder",
                    PathPart::Filename => "filename",
                    PathPart::Heading => "heading",
                };
                format!("{name} {}", text(t))
            }
            Filter::Description(t) => format!("description {}", text(t)),
            Filter::HasTags(true) => "has tags".into(),
            Filter::HasTags(false) => "has no tags".into(),
            Filter::Tag(t) => format!("tag {}", text(t)),
            Filter::HasId(true) => "has an id".into(),
            Filter::HasId(false) => "has no id".into(),
            Filter::IdIncludes(t) => format!("id {}", text(t)),
            Filter::HasDepends(true) => "depends on another task".into(),
            Filter::HasDepends(false) => "depends on no task".into(),
            Filter::Blocked(y) => format!("is{} blocked", if *y { "" } else { " not" }),
            Filter::Blocking(y) => format!("is{} blocking", if *y { "" } else { " not" }),
            Filter::Not(f) => format!("NOT ({})", f.explain()),
            Filter::And(a, b) => format!("({}) AND ({})", a.explain(), b.explain()),
            Filter::Or(a, b) => format!("({}) OR ({})", a.explain(), b.explain()),
            Filter::Xor(a, b) => format!("({}) XOR ({})", a.explain(), b.explain()),
        }
    }
}

/// A part of the task's path: the whole path, the first folder, the
/// folder, the note's name, the heading.
pub fn path_part(t: &Task, part: PathPart) -> String {
    let path = t.path.to_string_lossy().replace('\\', "/");
    match part {
        PathPart::Path => path,
        PathPart::Root => match path.split_once('/') {
            Some((root, _)) => format!("{root}/"),
            None => "/".into(),
        },
        PathPart::Folder => match path.rsplit_once('/') {
            Some((folder, _)) => format!("{folder}/"),
            None => "/".into(),
        },
        PathPart::Filename => t.filename(),
        PathPart::Heading => t.heading.clone().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    const TODAY: &str = "2026-09-30"; // a Wednesday

    #[test]
    fn dates_in_words() {
        let today = d(TODAY);
        let one = |p: &str| date_range(p, today).map(|(a, b)| (a.to_string(), b.to_string()));
        let day = |s: &str| Some((s.to_string(), s.to_string()));
        assert_eq!(one("tomorrow"), day("2026-10-01"));
        assert_eq!(one("in 2 weeks"), day("2026-10-14"));
        assert_eq!(one("in two weeks"), day("2026-10-14"), "numbers in words");
        assert_eq!(one("two weeks"), day("2026-10-14"), "from today");
        assert_eq!(one("in a month"), day("2026-10-30"));
        assert_eq!(one("three days ago"), day("2026-09-27"));
        assert_eq!(one("3 days ago"), day("2026-09-27"));
        assert_eq!(one("friday"), day("2026-10-02"));
        assert_eq!(one("last monday"), day("2026-09-28"));
        assert_eq!(
            one("this week"),
            Some(("2026-09-28".into(), "2026-10-04".into()))
        );
        assert_eq!(
            one("next month"),
            Some(("2026-10-01".into(), "2026-10-31".into()))
        );
        assert_eq!(
            one("last quarter"),
            Some(("2026-04-01".into(), "2026-06-30".into()))
        );
        assert_eq!(
            one("2026-W40"),
            Some(("2026-09-28".into(), "2026-10-04".into()))
        );
        assert_eq!(
            one("2026-Q4"),
            Some(("2026-10-01".into(), "2026-12-31".into()))
        );
        assert_eq!(
            one("2026-02"),
            Some(("2026-02-01".into(), "2026-02-28".into()))
        );
        assert_eq!(
            one("2026-10-05 2026-10-01"),
            Some(("2026-10-01".into(), "2026-10-05".into()))
        );
        assert_eq!(one("soon"), None);
    }

    #[test]
    fn regular_expressions_filter() {
        let today = d(TODAY);
        let statuses = Statuses::default();
        let none = HashSet::new();
        let env = Env {
            today,
            statuses: &statuses,
            open_ids: &none,
            depended_on: &none,
            functions: None,
        };
        let task = |line: &str, path: &str| {
            let mut t = Task::parse(line).unwrap();
            t.path = path.into();
            t
        };
        let keep = |q: &str, t: &Task| {
            parse(q, "", today)
                .unwrap()
                .filters
                .iter()
                .all(|f| f.matches(t, &env))
        };
        let call = task("- [ ] Call Bob at 5", "Work/Calls.md");
        assert!(keep("description regex matches /^Call \\w+/", &call));
        assert!(
            !keep("description regex matches /^call/", &call),
            "case matters"
        );
        assert!(
            keep("description regex matches /^call/i", &call),
            "unless i"
        );
        assert!(keep("path regex does not match /Home/", &call));
        assert!(keep("status.name regex matches /^To/", &call));
        assert!(parse("description regex matches /(/", "", today).is_err());
    }

    #[test]
    fn combinations_without_brackets() {
        let today = d(TODAY);
        assert_eq!(
            bracket_operands("due before 2026-08-04 AND priority is above none").as_deref(),
            Some("(due before 2026-08-04) AND (priority is above none)")
        );
        assert_eq!(
            bracket_operands("NOT done OR description includes \"A AND B\"").as_deref(),
            Some("NOT (done) OR (description includes \"A AND B\")")
        );
        assert_eq!(bracket_operands("not done"), None);
        let q = parse(
            "due before 2026-08-04 AND priority is above none\nsort by due date",
            "",
            today,
        )
        .unwrap();
        assert!(matches!(q.filters[0], Filter::And(..)));
        assert_eq!(
            q.sorts,
            [(Key::Date(DateKey::Field(DateField::Due)), false)]
        );
        // Not a combination after all: one filter.
        let q = parse("description includes ROCK AND", "", today).unwrap();
        assert!(matches!(&q.filters[0], Filter::Description(t) if t.text == "ROCK AND"));
    }

    #[test]
    fn filters_combine() {
        let today = d(TODAY);
        let q = parse(
            "# my tasks\nnot done\n(due before tomorrow) OR (priority is above medium)\nNOT (path includes Archive)\nsort by due reverse\ngroup by filename\nlimit to 5 tasks\nhide backlinks",
            "",
            today,
        )
        .unwrap();
        assert_eq!(q.filters.len(), 3);
        assert_eq!(q.sorts, [(Key::Date(DateKey::Field(DateField::Due)), true)]);
        assert_eq!(q.groups, [(Key::Filename, false)]);
        assert_eq!(q.limit, Some(5));
        assert!(q.hidden.contains("backlink"));
        let statuses = Statuses::default();
        let none = HashSet::new();
        let env = Env {
            today,
            statuses: &statuses,
            open_ids: &none,
            depended_on: &none,
            functions: None,
        };
        let task = |line: &str, path: &str| {
            let mut t = Task::parse(line).unwrap();
            t.path = path.into();
            t
        };
        let keep = |t: &Task| q.filters.iter().all(|f| f.matches(t, &env));
        assert!(keep(&task("- [ ] a 📅 2026-09-30", "A.md")));
        assert!(keep(&task("- [ ] b ⏫", "A.md")));
        assert!(!keep(&task("- [ ] c 🔼", "A.md")));
        assert!(!keep(&task("- [x] d ⏫", "A.md")));
        assert!(!keep(&task("- [ ] e ⏫", "Archive/B.md")));
        assert!(parse("wibble", "", today).unwrap_err().contains("unknown"));
        assert!(parse("(done) AND", "", today).is_err());
        let g = parse("done", "path includes Work; not done", today).unwrap();
        assert_eq!(g.filters.len(), 3, "the global query first");
        let g = parse("ignore global query", "not done", today).unwrap();
        assert!(g.filters.is_empty());
        let s = parse(
            "status.symbol is .\nstatus.type is NON_TASK\ntags include #home",
            "",
            today,
        )
        .unwrap();
        assert!(
            s.filters
                .iter()
                .all(|f| f.matches(&task("- [.] Log #home", "A.md"), &env))
        );
    }
}
