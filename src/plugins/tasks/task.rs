//! A task line and what the Tasks plugin reads from it (TK-01 … TK-08):
//! the status, the description, tags, the priority, six dates, the
//! recurrence rule, what happens on completion, and dependencies, in the
//! emoji format (`📅 2026-10-01`) or Dataview's (`[due:: 2026-10-01]`).
//! Also toggling (TK-11 … TK-13): done and cancelled dates, the next
//! occurrence of a recurring task, and 🏁 `delete`.

use std::path::PathBuf;

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

/// A status's type (TK-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    InProgress,
    Todo,
    OnHold,
    Done,
    Cancelled,
    NonTask,
}

impl Type {
    pub const ALL: [(Type, &'static str); 6] = [
        (Type::Todo, "TODO"),
        (Type::InProgress, "IN_PROGRESS"),
        (Type::OnHold, "ON_HOLD"),
        (Type::Done, "DONE"),
        (Type::Cancelled, "CANCELLED"),
        (Type::NonTask, "NON_TASK"),
    ];

    pub fn name(self) -> &'static str {
        Type::ALL
            .iter()
            .find(|(t, _)| *t == self)
            .map(|(_, n)| *n)
            .expect("every type has a name")
    }

    pub fn parse(name: &str) -> Option<Type> {
        let name = name.trim().to_uppercase().replace([' ', '-'], "_");
        Type::ALL.iter().find(|(_, n)| *n == name).map(|(t, _)| *t)
    }
}

/// A status: its symbol, name, type and the symbol a toggle goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub symbol: char,
    pub name: String,
    pub kind: Type,
    pub next: char,
}

impl Status {
    fn new(symbol: char, name: &str, kind: Type, next: char) -> Status {
        Status {
            symbol,
            name: name.into(),
            kind,
            next,
        }
    }
}

/// The statuses: the core ones, then the characters the editor shows
/// (TK-50: `[.]` is a log, a non-task), then the user's (which replace a
/// default with the same symbol).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statuses(Vec<Status>);

impl Default for Statuses {
    fn default() -> Self {
        Statuses(vec![
            Status::new(' ', "Todo", Type::Todo, 'x'),
            Status::new('x', "Done", Type::Done, ' '),
            Status::new('X', "Done", Type::Done, ' '),
            Status::new('/', "In Progress", Type::InProgress, 'x'),
            Status::new('-', "Cancelled", Type::Cancelled, ' '),
            Status::new('>', "Rescheduled", Type::Todo, 'x'),
            Status::new('<', "Scheduled", Type::Todo, 'x'),
            Status::new('!', "Important", Type::Todo, 'x'),
            Status::new('?', "Question", Type::Todo, 'x'),
            Status::new('*', "Star", Type::Todo, 'x'),
            Status::new('.', "Log", Type::NonTask, '.'),
        ])
    }
}

impl Statuses {
    /// The defaults with the user's statuses: `symbol|name|next|TYPE`,
    /// separated by `;`. Returns the problems.
    pub fn with_custom(text: &str) -> (Statuses, Vec<String>) {
        let mut all = Statuses::default();
        let mut problems = Vec::new();
        for entry in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
            let parts: Vec<&str> = entry.split('|').map(str::trim).collect();
            let one = |s: &str| {
                let mut c = s.chars();
                c.next().filter(|_| c.next().is_none())
            };
            let status = match parts[..] {
                [symbol, name, next, kind] => one(symbol)
                    .zip(one(next))
                    .zip(Type::parse(kind))
                    .map(|((s, n), k)| Status::new(s, name, k, n)),
                _ => None,
            };
            match status {
                Some(s) => match all.0.iter_mut().find(|x| x.symbol == s.symbol) {
                    Some(old) => *old = s,
                    None => all.0.push(s),
                },
                None => problems.push(format!("status {entry:?} isn't symbol|name|next|TYPE")),
            }
        }
        (all, problems)
    }

    /// The status for `symbol` (an unknown one is a to-do).
    pub fn get(&self, symbol: char) -> Status {
        self.0
            .iter()
            .find(|s| s.symbol == symbol)
            .cloned()
            .unwrap_or_else(|| Status::new(symbol, "Unknown", Type::Todo, 'x'))
    }
}

/// A task's priority, highest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Priority {
    Highest,
    High,
    Medium,
    #[default]
    None,
    Low,
    Lowest,
}

impl Priority {
    pub const ALL: [(Priority, &'static str, &'static str); 6] = [
        (Priority::Highest, "highest", "🔺"),
        (Priority::High, "high", "⏫"),
        (Priority::Medium, "medium", "🔼"),
        (Priority::None, "none", ""),
        (Priority::Low, "low", "🔽"),
        (Priority::Lowest, "lowest", "⏬"),
    ];

    pub fn name(self) -> &'static str {
        Priority::ALL
            .iter()
            .find(|(p, _, _)| *p == self)
            .map(|(_, n, _)| *n)
            .expect("every priority has a name")
    }

    pub fn emoji(self) -> &'static str {
        Priority::ALL
            .iter()
            .find(|(p, _, _)| *p == self)
            .map(|(_, _, e)| *e)
            .expect("every priority has an emoji")
    }

    pub fn parse(name: &str) -> Option<Priority> {
        let name = name.trim().to_lowercase();
        Priority::ALL
            .iter()
            .find(|(_, n, _)| *n == name)
            .map(|(p, _, _)| *p)
    }
}

/// The six dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DateField {
    Created,
    Start,
    Scheduled,
    Due,
    Done,
    Cancelled,
}

impl DateField {
    /// (field, emoji, query name, Dataview key).
    pub const ALL: [(DateField, &'static str, &'static str, &'static str); 6] = [
        (DateField::Created, "➕", "created", "created"),
        (DateField::Start, "🛫", "start", "start"),
        (DateField::Scheduled, "⏳", "scheduled", "scheduled"),
        (DateField::Due, "📅", "due", "due"),
        (DateField::Done, "✅", "done", "completion"),
        (DateField::Cancelled, "❌", "cancelled", "cancelled"),
    ];

    pub fn emoji(self) -> &'static str {
        DateField::ALL[self as usize].1
    }

    pub fn name(self) -> &'static str {
        DateField::ALL[self as usize].2
    }

    fn dataview(self) -> &'static str {
        DateField::ALL[self as usize].3
    }
}

/// Emoji that also mean a field (other spellings upstream accepts).
const ALIASES: [(&str, &str); 3] = [("📆", "📅"), ("🗓", "📅"), ("⌛", "⏳")];

/// A task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// The note, relative to the vault; the line in it.
    pub path: PathBuf,
    pub line: usize,
    /// The heading above it, if any.
    pub heading: Option<String>,
    /// The line as written.
    pub raw: String,
    /// What's before the checkbox (`  - `).
    pub prefix: String,
    pub status: char,
    /// The text before the fields.
    pub description: String,
    /// Tags (with `#`), anywhere in the line.
    pub tags: Vec<String>,
    pub priority: Priority,
    pub dates: [Option<NaiveDate>; 6],
    /// Dates written but not valid, by field (TK-08).
    pub invalid: Vec<DateField>,
    pub recurrence: Option<String>,
    /// 🏁 `keep` or `delete`.
    pub on_completion: Option<String>,
    pub id: Option<String>,
    pub depends: Vec<String>,
    /// The list item this task is nested under (its line), if any.
    pub parent: Option<usize>,
}

/// A field marker found in a line: where it starts and ends (bytes), and
/// what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Date(DateField),
    Priority(Priority),
    Recurrence,
    OnCompletion,
    Id,
    Depends,
}

const MARKERS: [(&str, Field); 16] = [
    ("➕", Field::Date(DateField::Created)),
    ("🛫", Field::Date(DateField::Start)),
    ("⏳", Field::Date(DateField::Scheduled)),
    ("📅", Field::Date(DateField::Due)),
    ("✅", Field::Date(DateField::Done)),
    ("❌", Field::Date(DateField::Cancelled)),
    ("🔺", Field::Priority(Priority::Highest)),
    ("⏫", Field::Priority(Priority::High)),
    ("🔼", Field::Priority(Priority::Medium)),
    ("🔽", Field::Priority(Priority::Low)),
    ("⏬", Field::Priority(Priority::Lowest)),
    ("🔁", Field::Recurrence),
    ("🏁", Field::OnCompletion),
    ("🆔", Field::Id),
    ("⛔", Field::Depends),
    ("📆", Field::Date(DateField::Due)),
];

/// Splits `- [x] text` into (prefix, status, text).
pub fn checkbox(line: &str) -> Option<(&str, char, &str)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let marker = if let Some(rest) = trimmed.strip_prefix(['-', '*', '+']) {
        trimmed.len() - rest.len()
    } else {
        let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
        let rest = trimmed[digits..]
            .strip_prefix(['.', ')'])
            .filter(|_| digits > 0)?;
        trimmed.len() - rest.len()
    };
    let after = &trimmed[marker..];
    let spaces = after.len() - after.trim_start().len();
    if spaces == 0 {
        return None;
    }
    let rest = &after[spaces..];
    let mut chars = rest.chars();
    if chars.next()? != '[' {
        return None;
    }
    let status = chars.next()?;
    if chars.next()? != ']' {
        return None;
    }
    let text = &rest[2 + status.len_utf8()..];
    if !(text.is_empty() || text.starts_with(' ')) {
        return None;
    }
    let prefix_len = indent + marker + spaces;
    Some((&line[..prefix_len], status, text.trim_start()))
}

impl Task {
    /// Reads a task from `line`, or `None` if it isn't one.
    pub fn parse(line: &str) -> Option<Task> {
        let (prefix, status, text) = checkbox(line)?;
        let text = normalize(text);
        let mut task = Task {
            path: PathBuf::new(),
            line: 0,
            heading: None,
            raw: line.to_string(),
            prefix: prefix.to_string(),
            status,
            description: String::new(),
            tags: tags(&text),
            priority: Priority::None,
            dates: [None; 6],
            invalid: Vec::new(),
            recurrence: None,
            on_completion: None,
            id: None,
            depends: Vec::new(),
            parent: None,
        };
        let text = task.dataview_fields(&text);
        let found = markers(&text);
        let first = found.first().map_or(text.len(), |(at, _, _)| *at);
        task.description = text[..first].trim().to_string();
        for (i, &(_, end, field)) in found.iter().enumerate() {
            let until = found.get(i + 1).map_or(text.len(), |(at, _, _)| *at);
            let value = text[end..until].trim();
            // A tag after a field belongs to the line, not the field.
            let value = value
                .split_whitespace()
                .filter(|w| !w.starts_with('#'))
                .collect::<Vec<_>>()
                .join(" ");
            task.set(field, &value);
        }
        Some(task)
    }

    fn set(&mut self, field: Field, value: &str) {
        match field {
            Field::Date(d) => match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
                Ok(date) => self.dates[d as usize] = Some(date),
                Err(_) => self.invalid.push(d),
            },
            Field::Priority(p) => self.priority = p,
            Field::Recurrence => {
                self.recurrence = Some(value.to_string()).filter(|v| !v.is_empty())
            }
            Field::OnCompletion => {
                self.on_completion = Some(value.to_lowercase()).filter(|v| !v.is_empty())
            }
            Field::Id => self.id = Some(value.to_string()).filter(|v| !v.is_empty()),
            Field::Depends => {
                self.depends = value
                    .split(',')
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .collect()
            }
        }
    }

    /// Reads `[key:: value]` fields (TK-06) and returns the text without
    /// them.
    fn dataview_fields(&mut self, text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(open) = rest.find('[') {
            let Some(close) = rest[open..].find(']').map(|c| open + c) else {
                break;
            };
            let inner = &rest[open + 1..close];
            let field = inner.split_once("::").and_then(|(key, value)| {
                let key = key.trim().to_lowercase();
                let field = match key.as_str() {
                    "priority" => Some(Field::Priority(Priority::parse(value)?)),
                    "repeat" => Some(Field::Recurrence),
                    "oncompletion" => Some(Field::OnCompletion),
                    "id" => Some(Field::Id),
                    "dependson" => Some(Field::Depends),
                    _ => DateField::ALL
                        .iter()
                        .find(|(_, _, _, k)| *k == key)
                        .map(|(d, _, _, _)| Field::Date(*d)),
                }?;
                Some((field, value.trim().to_string()))
            });
            match field {
                Some((field, value)) => {
                    out.push_str(&rest[..open]);
                    self.set(field, &value);
                }
                None => out.push_str(&rest[..=close]),
            }
            rest = &rest[close + 1..];
        }
        out.push_str(rest);
        out
    }

    pub fn date(&self, field: DateField) -> Option<NaiveDate> {
        self.dates[field as usize]
    }

    /// The earliest of start, scheduled and due (Tasks' "happens").
    pub fn happens(&self) -> Option<NaiveDate> {
        [DateField::Start, DateField::Scheduled, DateField::Due]
            .into_iter()
            .filter_map(|d| self.date(d))
            .min()
    }

    /// The note's name.
    pub fn filename(&self) -> String {
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Tasks' urgency score (TK-39).
    pub fn urgency(&self, today: NaiveDate) -> f64 {
        let due = self.date(DateField::Due).map_or(0.0, |due| {
            let overdue = (today - due).num_days() as f64;
            let factor = if overdue >= 7.0 {
                1.0
            } else if overdue >= -14.0 {
                (overdue + 14.0) * 0.8 / 21.0 + 0.2
            } else {
                0.2
            };
            factor * 12.0
        });
        let scheduled = match self.date(DateField::Scheduled) {
            Some(d) if d <= today => 5.0,
            _ => 0.0,
        };
        let start = match self.date(DateField::Start) {
            Some(d) if d > today => -3.0,
            _ => 0.0,
        };
        let priority = match self.priority {
            Priority::Highest => 9.0,
            Priority::High => 6.0,
            Priority::Medium => 3.9,
            Priority::None => 1.95,
            Priority::Low => 0.0,
            Priority::Lowest => -1.8,
        };
        ((due + scheduled + start + priority) * 100.0).round() / 100.0
    }

    /// The line with `field` set to `date` (or removed), in the format the
    /// line already uses for dates (emoji unless it has Dataview fields).
    pub fn with_date(line: &str, field: DateField, date: Option<NaiveDate>) -> String {
        let dataview = format!("[{}::", field.dataview());
        let mut out = remove_field(line, field);
        if let Some(date) = date {
            let date = date.format("%Y-%m-%d");
            if line.contains("::") && !line.contains(field.emoji()) || line.contains(&dataview) {
                out.push_str(&format!(" [{}:: {date}]", field.dataview()));
            } else {
                out.push_str(&format!(" {} {date}", field.emoji()));
            }
        }
        out
    }
}

/// The line without `field`'s value (emoji or Dataview), trailing spaces
/// trimmed.
fn remove_field(line: &str, field: DateField) -> String {
    let normalized = normalize(line);
    let mut out = normalized.clone();
    if let Some(at) = normalized.find(field.emoji()) {
        let after = at + field.emoji().len();
        let rest = &normalized[after..];
        let value = rest.trim_start();
        let end = after
            + (rest.len() - value.len())
            + value.find(char::is_whitespace).unwrap_or(value.len());
        out = format!("{}{}", normalized[..at].trim_end(), &normalized[end..]);
    }
    let key = format!("[{}::", field.dataview());
    if let Some(at) = out.find(&key)
        && let Some(close) = out[at..].find(']')
    {
        out = format!("{}{}", out[..at].trim_end(), &out[at + close + 1..]);
    }
    out.trim_end().to_string()
}

/// The text with other spellings of the markers made the usual ones, and
/// emoji variation selectors dropped.
fn normalize(text: &str) -> String {
    let mut out = text.replace('\u{fe0f}', "");
    for (from, to) in ALIASES {
        out = out.replace(from, to);
    }
    out
}

/// The field markers in `text`, in order: (start, end, field).
fn markers(text: &str) -> Vec<(usize, usize, Field)> {
    let mut found: Vec<(usize, usize, Field)> = MARKERS
        .iter()
        .flat_map(|&(emoji, field)| {
            text.match_indices(emoji)
                .map(move |(at, m)| (at, at + m.len(), field))
        })
        .collect();
    found.sort_by_key(|f| f.0);
    found
}

/// The tags in `text` (`#tag`, not `#` alone or a heading).
fn tags(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices('#') {
        let before_ok = i == 0 || text[..i].ends_with(char::is_whitespace);
        let tag: String = text[i + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
            .collect();
        if before_ok && tag.chars().any(|c| !c.is_ascii_digit()) {
            out.push(format!("#{tag}"));
        }
    }
    out
}

/// What toggling a task writes in place of its line (TK-11 … TK-13):
/// the next status, done / cancelled dates, a recurring task's next
/// occurrence above (or below) it, nothing for 🏁 `delete`.
pub fn toggle(task: &Task, statuses: &Statuses, today: NaiveDate, options: &Toggle) -> Vec<String> {
    let now = statuses.get(task.status);
    let next = statuses.get(now.next);
    let mut line = set_status(&task.raw, next.symbol);
    for (field, kind) in [
        (DateField::Done, Type::Done),
        (DateField::Cancelled, Type::Cancelled),
    ] {
        let set = next.kind == kind && now.kind != kind && options.dates;
        if next.kind != kind || set {
            let date = set.then_some(today);
            if task.date(field).is_some() || date.is_some() {
                line = Task::with_date(&line, field, date);
            }
        }
    }
    let completed = next.kind == Type::Done && now.kind != Type::Done;
    let mut out = Vec::new();
    if completed
        && let Some(rule) = &task.recurrence
        && let Some(next_task) = next_occurrence(task, rule, today, options)
    {
        out.push(next_task);
    }
    let delete = completed && task.on_completion.as_deref() == Some("delete");
    if !delete {
        if options.below {
            out.insert(0, line);
        } else {
            out.push(line);
        }
    }
    out
}

/// How toggling behaves (the settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toggle {
    /// Write ✅ / ❌ dates.
    pub dates: bool,
    /// The next occurrence goes below the done task (else above).
    pub below: bool,
    /// The next occurrence gets ➕ today.
    pub created: bool,
}

impl Default for Toggle {
    fn default() -> Self {
        Toggle {
            dates: true,
            below: false,
            created: false,
        }
    }
}

/// The line with its checkbox holding `symbol`.
pub fn set_status(line: &str, symbol: char) -> String {
    match checkbox(line) {
        Some((prefix, status, _)) => {
            let at = prefix.len() + 1;
            format!("{}{symbol}{}", &line[..at], &line[at + status.len_utf8()..])
        }
        None => line.to_string(),
    }
}

/// The recurring task's next line: its dates moved by the rule, not done.
fn next_occurrence(task: &Task, rule: &str, today: NaiveDate, options: &Toggle) -> Option<String> {
    let recurrence = Recurrence::parse(rule).ok()?;
    let reference = [DateField::Due, DateField::Scheduled, DateField::Start]
        .into_iter()
        .find_map(|d| task.date(d));
    let base = if recurrence.when_done {
        today
    } else {
        reference.unwrap_or(today)
    };
    let next = recurrence.next(base)?;
    let shift = next - reference.unwrap_or(base);
    let mut line = set_status(&task.raw, ' ');
    for field in [DateField::Done, DateField::Cancelled] {
        if task.date(field).is_some() {
            line = Task::with_date(&line, field, None);
        }
    }
    for field in [DateField::Start, DateField::Scheduled, DateField::Due] {
        if let Some(date) = task.date(field) {
            line = Task::with_date(&line, field, Some(date + shift));
        }
    }
    if options.created || task.date(DateField::Created).is_some() {
        line = Task::with_date(&line, DateField::Created, Some(today));
    }
    Some(line)
}

/// A 🔁 rule (TK-03): `every day`, `every 3 weeks`, `every weekday`,
/// `every week on Monday, Friday`, `every month on the 15th`, `every month
/// on the last`, `every year`, each with `when done`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recurrence {
    every: u32,
    unit: Unit,
    /// `every week on …`
    weekdays: Vec<Weekday>,
    /// `every month on the 15th` (0: the last day).
    day: Option<u32>,
    /// From the day it's done, not its dates.
    pub when_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Day,
    Weekday,
    Week,
    Month,
    Year,
}

impl Recurrence {
    pub fn parse(rule: &str) -> Result<Recurrence, String> {
        let lower = rule.trim().to_lowercase();
        let (text, when_done) = match lower.strip_suffix("when done") {
            Some(t) => (t.trim().to_string(), true),
            None => (lower.clone(), false),
        };
        let bad = || format!("can't read the rule {rule:?}");
        let rest = text.strip_prefix("every").ok_or_else(bad)?.trim();
        let (every, rest) = match rest.split_once(' ') {
            Some((n, r)) if n.parse::<u32>().is_ok() => (n.parse().expect("checked"), r.trim()),
            _ => (1, rest),
        };
        let (unit_word, on) = match rest.split_once(" on ") {
            Some((u, o)) => (u.trim(), Some(o.trim())),
            None => (rest, None),
        };
        let unit = match unit_word.trim_end_matches('s') {
            "day" => Unit::Day,
            "weekday" => Unit::Weekday,
            "week" => Unit::Week,
            "month" => Unit::Month,
            "year" => Unit::Year,
            _ => return Err(bad()),
        };
        let mut recurrence = Recurrence {
            every: every.max(1),
            unit,
            weekdays: Vec::new(),
            day: None,
            when_done,
        };
        if let Some(on) = on {
            match unit {
                Unit::Week => {
                    for day in on
                        .split([',', ' '])
                        .filter(|d| !d.is_empty() && *d != "and")
                    {
                        recurrence.weekdays.push(weekday(day).ok_or_else(bad)?);
                    }
                }
                Unit::Month => {
                    let day = on.trim_start_matches("the").trim();
                    recurrence.day = Some(if day == "last" {
                        0
                    } else {
                        day.trim_end_matches(char::is_alphabetic)
                            .parse()
                            .map_err(|_| bad())?
                    });
                }
                _ => return Err(bad()),
            }
        }
        Ok(recurrence)
    }

    /// The first date after `from` the rule gives.
    pub fn next(&self, from: NaiveDate) -> Option<NaiveDate> {
        match self.unit {
            Unit::Day => from.checked_add_days(Days::new(u64::from(self.every))),
            Unit::Weekday => {
                let mut d = from.succ_opt()?;
                while matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
                    d = d.succ_opt()?;
                }
                Some(d)
            }
            Unit::Week if self.weekdays.is_empty() => {
                from.checked_add_days(Days::new(7 * u64::from(self.every)))
            }
            Unit::Week => {
                let mut d = from.succ_opt()?;
                // The next listed day in this week, else in the week
                // `every` weeks on.
                for _ in 0..7 {
                    if self.weekdays.contains(&d.weekday()) {
                        let crossed = d.weekday().num_days_from_monday()
                            <= from.weekday().num_days_from_monday();
                        if crossed && self.every > 1 {
                            d = d.checked_add_days(Days::new(7 * u64::from(self.every - 1)))?;
                        }
                        return Some(d);
                    }
                    d = d.succ_opt()?;
                }
                None
            }
            Unit::Month => {
                let month = from.checked_add_months(Months::new(self.every))?;
                match self.day {
                    None => Some(month),
                    Some(day) => {
                        // The day in this month if it's still ahead.
                        let this = on_day(from, day).filter(|d| *d > from);
                        this.filter(|_| self.every == 1)
                            .or_else(|| on_day(month, day))
                    }
                }
            }
            Unit::Year => from.checked_add_months(Months::new(12 * self.every)),
        }
    }
}

/// `date`'s month on day `day` (0: its last day; a day past the month's
/// end is its last).
fn on_day(date: NaiveDate, day: u32) -> Option<NaiveDate> {
    let first = date.with_day(1)?;
    let last = first.checked_add_months(Months::new(1))?.pred_opt()?.day();
    let day = if day == 0 { last } else { day.min(last) };
    first.with_day(day)
}

/// A weekday's English name (or its first three letters).
pub fn weekday(name: &str) -> Option<Weekday> {
    let name = name.trim().to_lowercase();
    let days = [
        ("monday", Weekday::Mon),
        ("tuesday", Weekday::Tue),
        ("wednesday", Weekday::Wed),
        ("thursday", Weekday::Thu),
        ("friday", Weekday::Fri),
        ("saturday", Weekday::Sat),
        ("sunday", Weekday::Sun),
    ];
    days.iter()
        .find(|(full, _)| name.len() >= 3 && full.starts_with(&name))
        .map(|(_, d)| *d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn fields_are_read_from_emoji_and_dataview() {
        let t = Task::parse(
            "  - [ ] Pay rent #home ⏫ 🔁 every month on the 1st 📅 2026-10-01 ⏳ 2026-09-28 🆔 rent ⛔ a,b",
        )
        .unwrap();
        assert_eq!(t.prefix, "  - ");
        assert_eq!(t.description, "Pay rent #home");
        assert_eq!(t.tags, ["#home"]);
        assert_eq!(t.priority, Priority::High);
        assert_eq!(t.recurrence.as_deref(), Some("every month on the 1st"));
        assert_eq!(t.date(DateField::Due), Some(d("2026-10-01")));
        assert_eq!(t.happens(), Some(d("2026-09-28")));
        assert_eq!(t.id.as_deref(), Some("rent"));
        assert_eq!(t.depends, ["a", "b"]);
        let t = Task::parse(
            "1. [x] Call [due:: 2026-10-02] [priority:: low] [completion:: 2026-10-01]",
        )
        .unwrap();
        assert_eq!(t.description, "Call");
        assert_eq!(t.date(DateField::Due), Some(d("2026-10-02")));
        assert_eq!(t.date(DateField::Done), Some(d("2026-10-01")));
        assert_eq!(t.priority, Priority::Low);
        let t = Task::parse("- [ ] Bad 📅 2026-13-45").unwrap();
        assert_eq!(t.invalid, [DateField::Due]);
        assert!(Task::parse("- [] no").is_none());
        assert!(Task::parse("- text").is_none());
        assert_eq!(Task::parse("* [/] Half").unwrap().status, '/');
    }

    #[test]
    fn toggling_writes_dates_and_the_next_occurrence() {
        let statuses = Statuses::default();
        let today = d("2026-09-30");
        let opts = Toggle::default();
        let t = Task::parse("- [ ] Water 🔁 every week 📅 2026-10-01").unwrap();
        assert_eq!(
            toggle(&t, &statuses, today, &opts),
            [
                "- [ ] Water 🔁 every week 📅 2026-10-08",
                "- [x] Water 🔁 every week 📅 2026-10-01 ✅ 2026-09-30",
            ]
        );
        let done = Task::parse("- [x] Old ✅ 2026-09-01").unwrap();
        assert_eq!(toggle(&done, &statuses, today, &opts), ["- [ ] Old"]);
        let delete = Task::parse("- [ ] Once 🏁 delete").unwrap();
        assert!(toggle(&delete, &statuses, today, &opts).is_empty());
        let (custom, problems) = Statuses::with_custom("-|Cancelled|x|CANCELLED; ~|Odd");
        assert_eq!(problems.len(), 1);
        let cancel = Task::parse("- [/] Busy").unwrap();
        let mut s = custom.clone();
        s.0[3].next = '-';
        assert_eq!(
            toggle(&cancel, &s, today, &opts),
            ["- [-] Busy ❌ 2026-09-30"]
        );
        let dv = Task::parse("- [ ] Call [due:: 2026-10-02]").unwrap();
        assert_eq!(
            toggle(&dv, &statuses, today, &opts),
            ["- [x] Call [due:: 2026-10-02] [completion:: 2026-09-30]"]
        );
        assert_eq!(statuses.get('.').kind, Type::NonTask, "logs");
    }

    #[test]
    fn recurrence_rules() {
        let next = |rule: &str, from: &str| Recurrence::parse(rule).unwrap().next(d(from));
        assert_eq!(next("every day", "2026-09-30"), Some(d("2026-10-01")));
        assert_eq!(next("every 2 weeks", "2026-09-30"), Some(d("2026-10-14")));
        assert_eq!(next("every weekday", "2026-10-02"), Some(d("2026-10-05")));
        assert_eq!(
            next("every week on Monday, Friday", "2026-09-30"),
            Some(d("2026-10-02"))
        );
        assert_eq!(
            next("every week on monday", "2026-10-02"),
            Some(d("2026-10-05"))
        );
        assert_eq!(
            next("every month on the 15th", "2026-09-30"),
            Some(d("2026-10-15"))
        );
        assert_eq!(
            next("every month on the 15th", "2026-09-10"),
            Some(d("2026-09-15"))
        );
        assert_eq!(
            next("every month on the last", "2026-01-31"),
            Some(d("2026-02-28"))
        );
        assert_eq!(next("every month", "2026-01-31"), Some(d("2026-02-28")));
        assert_eq!(next("every year", "2024-02-29"), Some(d("2025-02-28")));
        assert!(Recurrence::parse("every day when done").unwrap().when_done);
        assert!(Recurrence::parse("sometimes").is_err());
    }

    #[test]
    fn urgency_follows_the_dates_and_priority() {
        let today = d("2026-09-30");
        let u = |line: &str| Task::parse(line).unwrap().urgency(today);
        assert_eq!(u("- [ ] a"), 1.95);
        assert_eq!(u("- [ ] a 📅 2026-09-20"), 13.95, "overdue a week or more");
        assert_eq!(u("- [ ] a 📅 2026-12-30 ⏫"), 8.4);
        assert_eq!(u("- [ ] a ⏳ 2026-09-30 🛫 2026-10-05"), 3.95);
    }
}
