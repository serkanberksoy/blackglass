//! A block's datasets: the notes searched (a folder, files named), each
//! one's date (its name in `dateFormat`, or the X dataset's value), and
//! each target's value on each day from the first to the last.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use chrono::{Datelike, Duration, NaiveDate};
use regex::Regex;

use super::params::{Params, SearchType, TaskKind};
use crate::plugins::moment;
use crate::vault::{Note, Vault};

/// One target's values, a value (or none) a day.
#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    pub name: String,
    pub values: Vec<Option<f64>>,
    /// How many times the target was found.
    pub found: usize,
    /// Whether it gives the dates (`xDataset`): not drawn.
    pub is_x: bool,
}

/// What a block has to show.
#[derive(Debug, Clone, PartialEq)]
pub struct Data {
    /// The first day; `values[i]` is day `start + i`.
    pub start: NaiveDate,
    pub days: usize,
    pub sets: Vec<Dataset>,
    /// The notes by day (relative paths), for clicks.
    pub notes: BTreeMap<NaiveDate, String>,
    pub date_format: String,
}

impl Data {
    pub fn day(&self, i: usize) -> NaiveDate {
        self.start + Duration::days(i as i64)
    }

    pub fn end(&self) -> NaiveDate {
        self.day(self.days.saturating_sub(1))
    }

    /// A date as the block writes them.
    pub fn format(&self, day: NaiveDate) -> String {
        format_date(day, &self.date_format)
    }

    /// The datasets drawn (not the X one), with their indexes.
    pub fn shown(&self) -> impl Iterator<Item = (usize, &Dataset)> {
        self.sets.iter().enumerate().filter(|(_, s)| !s.is_x)
    }
}

pub fn format_date(day: NaiveDate, format: &str) -> String {
    let day = day.and_hms_opt(0, 0, 0).expect("midnight exists");
    if format.eq_ignore_ascii_case("iso-8601") {
        return moment::format(&day, "YYYY-MM-DD");
    }
    moment::format(&day, format)
}

/// `text` as a date in `format` (all of it; `[[…]]` taken off).
pub fn parse_date(text: &str, format: &str) -> Option<NaiveDate> {
    let text = text.trim();
    let text = text
        .strip_prefix("[[")
        .and_then(|t| t.strip_suffix("]]"))
        .unwrap_or(text);
    if format.eq_ignore_ascii_case("iso-8601") {
        return text
            .get(..10)
            .and_then(|t| NaiveDate::parse_from_str(t, "%Y-%m-%d").ok());
    }
    let date = moment::parse(text, format)?.date();
    // All of it: the date written back the same way (or as numbers).
    let back = format_date(date, format);
    (back.eq_ignore_ascii_case(text) || normalized(&back) == normalized(text)).then_some(date)
}

/// Digits without leading zeros, to compare `2026-1-5` with `2026-01-05`.
fn normalized(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .map(|p| p.trim_start_matches('0').to_lowercase())
        .collect()
}

/// A date given in the block: in its format, ISO, or relative to `today`
/// (`-7d`, `+1w`, `-1M`, `-1y2M`, `0d`).
pub fn resolve_date(text: &str, format: &str, today: NaiveDate) -> Option<NaiveDate> {
    if let Some(day) = parse_date(text, format).or_else(|| parse_date(text, "YYYY-MM-DD")) {
        return Some(day);
    }
    relative(text, today)
}

fn relative(text: &str, today: NaiveDate) -> Option<NaiveDate> {
    let text = text.trim();
    let (sign, mut rest) = match text.strip_prefix('-') {
        Some(r) => (-1i64, r),
        None => (1, text.strip_prefix('+').unwrap_or(text)),
    };
    static UNIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(\d+)\s*(years?|months?|weeks?|days?|[yYMwWdD])")
            .expect("the duration pattern is valid")
    });
    let unit = &*UNIT;
    let mut day = today;
    let mut any = false;
    while let Some(m) = unit.captures(rest) {
        let n: i64 = m[1].parse().ok()?;
        let n = n * sign;
        day = match &m[2] {
            u if u.starts_with("year") || u == "y" || u == "Y" => add_months(day, n * 12)?,
            u if u.starts_with("month") || u == "M" => add_months(day, n)?,
            u if u.starts_with("week") || u == "w" || u == "W" => day + Duration::weeks(n),
            _ => day + Duration::days(n),
        };
        rest = &rest[m[0].len()..];
        any = true;
    }
    (any && rest.trim().is_empty()).then_some(day)
}

pub fn add_months(day: NaiveDate, n: i64) -> Option<NaiveDate> {
    let months = day.year() as i64 * 12 + day.month0() as i64 + n;
    let (year, month) = (
        months.div_euclid(12) as i32,
        months.rem_euclid(12) as u32 + 1,
    );
    (0..4).find_map(|back| NaiveDate::from_ymd_opt(year, month, day.day() - back))
}

/// The number at the start of `text` (`70.5kg`: 70.5).
pub fn number(text: &str) -> Option<f64> {
    let text = text.trim();
    let end = text
        .char_indices()
        .take_while(|&(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+')))
        .map(|(i, c)| i + c.len_utf8())
        .last()?;
    text[..end].parse().ok()
}

/// A time (`23:15`, `22:30:30`) or a duration (`7h30m`, `6h`, `1.5 h`)
/// in hours.
fn hours(text: &str) -> Option<f64> {
    static TIME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(\d{1,2}):(\d{2})(?::(\d{2}))?$").expect("a valid pattern"));
    static DURATION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(\d+(?:\.\d+)?)\s*h(?:ours?|rs?)?\s*(?:(\d+)\s*m(?:in(?:utes?)?)?)?$")
            .expect("a valid pattern")
    });
    if let Some(m) = TIME.captures(text) {
        let part = |i: usize| {
            m.get(i)
                .map_or(0.0, |v| v.as_str().parse::<f64>().unwrap_or(0.0))
        };
        return Some(part(1) + part(2) / 60.0 + part(3) / 3600.0);
    }
    let m = DURATION.captures(text)?;
    let h: f64 = m[1].parse().ok()?;
    let min: f64 = m.get(2).map_or(Ok(0.0), |v| v.as_str().parse()).ok()?;
    Some(h + min / 60.0)
}

/// A frontmatter property by name; `a.b` is key `b` under `a` (a list's
/// items joined with `, `).
fn property(note: &Note, key: &str) -> Option<String> {
    if let Some((_, v)) = note.properties.iter().find(|(k, _)| k == key) {
        return Some(v.clone());
    }
    if !key.contains('.') {
        return None;
    }
    let end = crate::vault::properties::frontmatter_end(&note.lines)?;
    let yaml = note.lines[1..end].join("\n");
    let doc = yaml_rust2::YamlLoader::load_from_str(&yaml)
        .ok()?
        .into_iter()
        .next()?;
    let mut at = &doc;
    for part in key.split('.') {
        at = &at[part];
    }
    let scalar = |y: &yaml_rust2::Yaml| match y {
        yaml_rust2::Yaml::String(s) | yaml_rust2::Yaml::Real(s) => Some(s.clone()),
        yaml_rust2::Yaml::Integer(n) => Some(n.to_string()),
        yaml_rust2::Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    };
    match at {
        yaml_rust2::Yaml::Array(items) => Some(
            items
                .iter()
                .filter_map(scalar)
                .collect::<Vec<_>>()
                .join(", "),
        ),
        other => scalar(other),
    }
}

/// A target's name and the index of a value in it (`bp[1]`).
fn indexed(target: &str) -> (&str, Option<usize>) {
    if let Some(open) = target.rfind('[')
        && let Some(inner) = target[open + 1..].strip_suffix(']')
        && let Ok(i) = inner.trim().parse()
    {
        return (&target[..open], Some(i));
    }
    (target, None)
}

/// The block's searching state for one target.
struct Search<'a> {
    params: &'a Params,
    i: usize,
    name: &'a str,
    index: Option<usize>,
    regex: Option<Regex>,
}

impl Search<'_> {
    fn constant(&self) -> f64 {
        self.params.num("constValue", self.i).unwrap_or(1.0)
    }

    /// A value as written: a number, `true` / `false`, or a mapped text
    /// (`textValueMap`); the `index`th of several (`separator`).
    fn value(&self, text: &str, list_sep: Option<&str>) -> Option<f64> {
        let text = text.trim();
        let text = match self.index {
            Some(i) => {
                let sep = self.params.separator(self.i);
                let mut parts: Vec<&str> = text.split(sep.as_str()).collect();
                if parts.len() == 1
                    && let Some(list) = list_sep
                {
                    parts = text.split(list).collect();
                }
                parts.get(i)?.trim()
            }
            None => text,
        };
        if let Some(hours) = hours(text) {
            return Some(hours);
        }
        if let Some(n) = number(text) {
            return Some(n);
        }
        match text.to_lowercase().as_str() {
            "true" => return Some(1.0),
            "false" => return Some(0.0),
            _ => {}
        }
        self.params.text_values.iter().find_map(|(key, value)| {
            let hit =
                key == text || Regex::new(&format!("^(?:{key})$")).is_ok_and(|r| r.is_match(text));
            hit.then_some(*value)
        })
    }

    /// The target's values in `note` (several when it's there more than
    /// once), or why the note is skipped.
    fn values(&self, note: &Note, file: Option<&std::fs::Metadata>) -> Vec<f64> {
        let lines = &note.lines;
        let fm_end = crate::vault::properties::frontmatter_end(lines);
        let body = &lines[fm_end.map_or(0, |e| e + 1)..];
        let attached = !self.params.flag("ignoreAttachedValue", self.i);
        match self.params.types[self.i] {
            SearchType::Tag => {
                let tag = self.name.trim_start_matches('#');
                let pattern = format!(
                    r"(?:^|\s)#{}(?:/[^\s:#]+)*(?::([^\s#]+))?",
                    regex::escape(tag)
                );
                let re = Regex::new(&pattern).expect("an escaped tag is a valid pattern");
                let mut out = Vec::new();
                let mut code = false;
                for line in body {
                    if line.trim_start().starts_with("```") {
                        code = !code;
                    }
                    if code {
                        continue;
                    }
                    for m in re.captures_iter(line) {
                        let v = match m.get(1) {
                            Some(v) if attached => self.value(v.as_str(), None),
                            _ => Some(self.constant()),
                        };
                        out.extend(v);
                    }
                }
                // Frontmatter tags (no values).
                if out.is_empty()
                    && note.properties.iter().any(|(k, v)| {
                        matches!(k.as_str(), "tags" | "tag")
                            && v.split([',', ' '])
                                .map(|t| t.trim().trim_start_matches('#'))
                                .any(|t| t == tag || t.starts_with(&format!("{tag}/")))
                    })
                {
                    out.push(self.constant());
                }
                out
            }
            SearchType::Frontmatter => property(note, self.name)
                .and_then(|v| self.value(&v, Some(", ")))
                .into_iter()
                .collect(),
            SearchType::FrontmatterExists => property(note, self.name)
                .filter(|v| !v.trim().is_empty())
                .map(|_| self.constant())
                .into_iter()
                .collect(),
            SearchType::Wiki => {
                let target = self.name.trim_start_matches("[[").trim_end_matches("]]");
                let mut n = 0;
                for line in body {
                    let mut rest = line.as_str();
                    while let Some(at) = rest.find("[[") {
                        rest = &rest[at + 2..];
                        let end = rest.find("]]").unwrap_or(rest.len());
                        let link = rest[..end].split(['|', '#']).next().unwrap_or_default();
                        let link = link.rsplit('/').next().unwrap_or(link);
                        if link.trim().eq_ignore_ascii_case(target) {
                            n += 1;
                        }
                    }
                }
                std::iter::repeat_n(self.constant(), n).collect()
            }
            SearchType::Text => {
                let Some(re) = &self.regex else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                for line in lines {
                    for m in re.captures_iter(line) {
                        let v = match m.name("value") {
                            Some(v) if attached => self.value(v.as_str(), None),
                            _ => Some(self.constant()),
                        };
                        out.extend(v);
                    }
                }
                out
            }
            SearchType::DvField => {
                let key = format!("{}::", self.name);
                let mut out = Vec::new();
                for line in body {
                    let mut rest = line.as_str();
                    while let Some(at) = rest.find(&key) {
                        // A field's name starts the line or follows a space
                        // or a bracket.
                        let before = rest[..at].chars().next_back();
                        let after = &rest[at + key.len()..];
                        rest = after;
                        if before.is_some_and(|c| !(c.is_whitespace() || c == '[' || c == '(')) {
                            continue;
                        }
                        let closing = match before {
                            Some('[') => after.find(']'),
                            Some('(') => after.find(')'),
                            _ => None,
                        };
                        let raw = &after[..closing.unwrap_or(after.len())];
                        // An unbracketed field's value runs to the next
                        // field.
                        let raw = match closing {
                            None => raw
                                .find(['[', '('])
                                .filter(|&i| raw[i..].contains("::"))
                                .map_or(raw, |i| &raw[..i]),
                            Some(_) => raw,
                        };
                        out.extend(self.value(raw, Some(", ")));
                    }
                }
                out
            }
            SearchType::Task(kind) => {
                let filter = self.name.trim().to_lowercase();
                let n = body
                    .iter()
                    .filter_map(|l| {
                        let t = l.trim_start();
                        let t = t.strip_prefix(['-', '*', '+']).or_else(|| {
                            let digits = t.find(|c: char| !c.is_ascii_digit())?;
                            t[digits..].strip_prefix(['.', ')'])
                        })?;
                        let t = t.strip_prefix(' ')?.strip_prefix('[')?;
                        let mark = t.chars().next()?;
                        let text = t[mark.len_utf8()..].strip_prefix(']')?;
                        Some((mark != ' ', text.to_lowercase()))
                    })
                    .filter(|(done, text)| {
                        let kind_ok = match kind {
                            TaskKind::All => true,
                            TaskKind::Done => *done,
                            TaskKind::NotDone => !*done,
                        };
                        kind_ok && (filter.is_empty() || text.contains(&filter))
                    })
                    .count();
                if n == 0 {
                    Vec::new()
                } else {
                    vec![n as f64 * self.constant()]
                }
            }
            SearchType::FileMeta => {
                let text = body.join("\n");
                let v = match self.name {
                    "size" => file.map(|m| m.len() as f64),
                    "numWords" => Some(text.split_whitespace().count() as f64),
                    "numChars" => Some(text.chars().count() as f64),
                    "numSentences" => Some(
                        text.split_inclusive(['.', '!', '?'])
                            .filter(|s| s.trim().len() > 1)
                            .count() as f64,
                    ),
                    _ => None,
                };
                v.into_iter().collect()
            }
            SearchType::Table => Vec::new(),
        }
    }

    /// The note's date by this target (for `xDataset`).
    fn date(&self, note: &Note, file: Option<&std::fs::Metadata>) -> Option<NaiveDate> {
        let format = &self.params.date_format;
        let local =
            |t: std::time::SystemTime| chrono::DateTime::<chrono::Local>::from(t).date_naive();
        match (self.params.types[self.i], self.name) {
            (SearchType::FileMeta, "cDate") => file?.created().ok().map(local),
            (SearchType::FileMeta, "mDate") => file?.modified().ok().map(local),
            (SearchType::Frontmatter, _) => {
                property(note, self.name).and_then(|v| parse_date(&v, format))
            }
            (SearchType::DvField, _) => {
                let key = format!("{}::", self.name);
                note.lines.iter().find_map(|l| {
                    let at = l.find(&key)?;
                    let v = l[at + key.len()..].trim().trim_end_matches([']', ')']);
                    parse_date(v, format)
                })
            }
            _ => None,
        }
    }
}

/// A note's date from its name (`dateFormatPrefix` / `Suffix` taken off).
fn name_date(name: &str, params: &Params) -> Option<NaiveDate> {
    let mut text = name;
    if !params.date_prefix.is_empty() {
        let re = Regex::new(&format!("^(?:{})", params.date_prefix)).ok()?;
        text = re.find(text).map_or(text, |m| &text[m.end()..]);
    }
    if !params.date_suffix.is_empty() {
        let re = Regex::new(&format!("(?:{})$", params.date_suffix)).ok()?;
        text = re.find(text).map_or(text, |m| &text[..m.start()]);
    }
    parse_date(text, &params.date_format)
}

/// Whether `note` is the one `name` names (its path or its name, with or
/// without `.md`).
fn is_named(note: &Note, name: &str) -> bool {
    let f = name.trim().trim_start_matches('/');
    let f = f.strip_suffix(".md").unwrap_or(f);
    note.rel_name().eq_ignore_ascii_case(f) || note.name().eq_ignore_ascii_case(f)
}

/// The notes a block reads, each with the number its values are
/// multiplied by (`fileMultiplierAfterLink`; else 1).
fn notes<'a>(vault: &'a Vault, params: &Params) -> Vec<(&'a Note, f64)> {
    let named = |n: &Note| params.files.iter().any(|f| is_named(n, f));
    let in_folder = |n: &Note| {
        let rel = n.rel_name();
        params.folder.is_empty() || rel.starts_with(&format!("{}/", params.folder))
    };
    let mut out: Vec<(&Note, f64)> = vault
        .notes
        .iter()
        .filter(|n| named(n) || (!params.specified_only && in_folder(n)))
        .map(|n| (n, 1.0))
        .collect();
    // The notes the files in `fileContainsLinkedFiles` link to.
    let linking = params.root.list("fileContainsLinkedFiles");
    let multiplier = params
        .root
        .str("fileMultiplierAfterLink")
        .map(|r| r.trim().trim_matches(['\'', '"']).to_string())
        .filter(|r| !r.is_empty())
        .and_then(|r| Regex::new(&r).ok());
    for file in vault
        .notes
        .iter()
        .filter(|n| linking.iter().any(|f| is_named(n, f)))
    {
        for line in &file.lines {
            let mut rest = line.as_str();
            while let Some(at) = rest.find("[[") {
                let after = &rest[at + 2..];
                let Some(end) = after.find("]]") else { break };
                let target = after[..end].split(['|', '#']).next().unwrap_or_default();
                rest = &after[end + 2..];
                let Some(note) = vault.notes.iter().find(|n| is_named(n, target)) else {
                    continue;
                };
                let times = multiplier
                    .as_ref()
                    .and_then(|re| re.captures(rest))
                    .and_then(|m| m.name("value").and_then(|v| number(v.as_str())))
                    .unwrap_or(1.0);
                match out.iter_mut().find(|(n, _)| n.path == note.path) {
                    Some((_, m)) => *m = times,
                    None => out.push((note, times)),
                }
            }
        }
    }
    out
}

/// A row of values on a day (by target).
type Row = (NaiveDate, Vec<Option<f64>>);

fn tables(vault: &Vault, params: &Params) -> Result<Vec<Row>, String> {
    let spec = Regex::new(r"^(.+?)\[(\d+)\]\[(\d+)\](?:\[(\d+)\])?$").expect("a valid pattern");
    let x = params.x_dataset().unwrap_or(0);
    let mut parsed = Vec::new();
    for t in &params.targets {
        let m = spec
            .captures(t.trim())
            .ok_or(format!("Invalid table target '{t}'"))?;
        let path = m[1].trim().trim_start_matches('/');
        let path = path.strip_suffix(".md").unwrap_or(path);
        let note = vault
            .notes
            .iter()
            .find(|n| n.rel_name().eq_ignore_ascii_case(path))
            .ok_or(format!("File '{path}' containing tables not found"))?;
        let table: usize = m[2].parse().map_err(|_| "Invalid table index")?;
        let column: usize = m[3].parse().map_err(|_| "Invalid column index")?;
        let value = m.get(4).and_then(|v| v.as_str().parse::<usize>().ok());
        parsed.push((note, table, column, value));
    }
    let rows_of = |note: &Note, table: usize| -> Vec<Vec<String>> {
        let mut tables: Vec<Vec<Vec<String>>> = Vec::new();
        let mut in_table = false;
        for line in &note.lines {
            let t = line.trim();
            if t.starts_with('|') {
                let cells: Vec<String> = t
                    .trim_matches('|')
                    .split('|')
                    .map(|c| c.trim().to_string())
                    .collect();
                if !in_table {
                    tables.push(Vec::new());
                    in_table = true;
                }
                tables.last_mut().expect("a table was started").push(cells);
            } else {
                in_table = false;
            }
        }
        tables
            .get(table)
            .map(|rows| {
                rows.iter()
                    .skip(1)
                    .filter(|r| {
                        !r.iter()
                            .all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    };
    let (xnote, xtable, xcol, _) = parsed[x];
    let dates: Vec<Option<NaiveDate>> = rows_of(xnote, xtable)
        .iter()
        .map(|r| r.get(xcol).and_then(|c| parse_date(c, &params.date_format)))
        .collect();
    let mut out: Vec<Row> = Vec::new();
    for (row, date) in dates.iter().enumerate() {
        let Some(date) = date else { continue };
        let mut values = Vec::new();
        for (i, (note, table, column, value)) in parsed.iter().enumerate() {
            if i == x {
                values.push(None);
                continue;
            }
            let rows = rows_of(note, *table);
            let cell = rows
                .get(row)
                .and_then(|r| r.get(*column))
                .cloned()
                .unwrap_or_default();
            let cell = match value {
                Some(v) => cell
                    .split(params.separator(i).as_str())
                    .nth(*v)
                    .unwrap_or_default()
                    .to_string(),
                None => cell,
            };
            values.push(number(&cell));
        }
        out.push((*date, values));
    }
    Ok(out)
}

/// The block's data from the vault, `today` for relative dates.
pub fn collect(vault: &Vault, params: &Params, today: NaiveDate) -> Result<Data, String> {
    let count = params.targets.len();
    let x = params.x_dataset();
    // Values by day, by target.
    let mut by_day: BTreeMap<NaiveDate, Vec<Option<f64>>> = BTreeMap::new();
    let mut found = vec![0usize; count];
    let mut notes_by_day = BTreeMap::new();
    if params.types.contains(&SearchType::Table) {
        for (day, values) in tables(vault, params)? {
            let slot = by_day.entry(day).or_insert_with(|| vec![None; count]);
            for (i, v) in values.into_iter().enumerate() {
                if let Some(v) = v {
                    *slot[i].get_or_insert(0.0) += v;
                    found[i] += 1;
                }
            }
        }
    } else {
        let searches: Vec<Search> = (0..count)
            .map(|i| {
                let (name, index) = indexed(&params.targets[i]);
                let regex = (params.types[i] == SearchType::Text).then(|| {
                    Regex::new(name).unwrap_or_else(|_| {
                        Regex::new(&regex::escape(name)).expect("an escaped text is valid")
                    })
                });
                Search {
                    params,
                    i,
                    name,
                    index,
                    regex,
                }
            })
            .collect();
        let needs_file = params.types.contains(&SearchType::FileMeta);
        for (note, times) in notes(vault, params) {
            let meta = needs_file
                .then(|| std::fs::metadata(&note.path).ok())
                .flatten();
            let day = match x {
                Some(x) => searches[x].date(note, meta.as_ref()),
                None => name_date(&note.name(), params),
            };
            let Some(day) = day else { continue };
            notes_by_day
                .entry(day)
                .or_insert_with(|| crate::vault::slash(&note.rel));
            let slot = by_day.entry(day).or_insert_with(|| vec![None; count]);
            for (i, search) in searches.iter().enumerate() {
                if Some(i) == x {
                    continue;
                }
                let values = search.values(note, meta.as_ref());
                found[i] += values.len();
                if !values.is_empty() {
                    *slot[i].get_or_insert(0.0) += values.iter().sum::<f64>() * times;
                }
            }
        }
    }
    let first = by_day.keys().next().copied();
    let last = by_day.keys().next_back().copied();
    let resolve = |d: &Option<String>, name: &str| -> Result<Option<NaiveDate>, String> {
        match d {
            Some(text) => resolve_date(text, &params.date_format, today)
                .map(Some)
                .ok_or(format!("Invalid {name} '{text}'")),
            None => Ok(None),
        }
    };
    let start = resolve(&params.start, "startDate")?.or(first);
    let end = resolve(&params.end, "endDate")?.or(last);
    let (Some(start), Some(end)) = (start, end) else {
        return Err("No valid date found".into());
    };
    if end < start {
        return Err("Invalid date range (startDate later than endDate)".into());
    }
    let days = (end - start).num_days() as usize + 1;
    let has_data = by_day.range(start..=end).any(|(_, v)| {
        v.iter()
            .enumerate()
            .any(|(i, v)| Some(i) != x && v.is_some())
    });
    if !has_data {
        return Err("No valid data found".into());
    }
    let mut sets = Vec::new();
    for i in 0..count {
        let mut values: Vec<Option<f64>> = (0..days)
            .map(|d| {
                let day = start + Duration::days(d as i64);
                by_day.get(&day).and_then(|v| v[i])
            })
            .collect();
        let penalty = params.num("penalty", i);
        let shift = params.num("valueShift", i).unwrap_or(0.0);
        let above = params.num("shiftOnlyValueLargerThan", i);
        let ignore_zero = params.flag("ignoreZeroValue", i);
        let mut total = 0.0;
        for v in &mut values {
            if ignore_zero && *v == Some(0.0) {
                *v = None;
            }
            if v.is_none() {
                *v = penalty;
            }
            if let Some(n) = v
                && above.is_none_or(|a| *n > a)
            {
                *n += shift;
            }
            if params.flag("accum", i)
                && let Some(n) = v
            {
                total += *n;
                *n = total;
            }
        }
        sets.push(Dataset {
            name: params.name(i),
            values,
            found: found[i],
            is_x: Some(i) == x,
        });
    }
    Ok(Data {
        start,
        days,
        sets,
        notes: notes_by_day,
        date_format: params.date_format.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    fn day(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    fn data(name: &str, files: &[(&str, &str)], block: &str) -> Result<Data, String> {
        let dir = scratch(name);
        write(&dir, files);
        let vault = Vault::open(&dir).unwrap();
        collect(&vault, &Params::parse(block).unwrap(), day(10, 13))
    }

    #[test]
    fn dates_come_from_names_and_relative_dates_from_today() {
        assert_eq!(parse_date("2026-10-05", "YYYY-MM-DD"), Some(day(10, 5)));
        assert_eq!(parse_date("[[2026-10-05]]", "YYYY-MM-DD"), Some(day(10, 5)));
        assert_eq!(
            parse_date("2026-10-05 extra", "YYYY-MM-DD"),
            None,
            "all of it"
        );
        assert_eq!(parse_date("05.10.2026", "DD.MM.YYYY"), Some(day(10, 5)));
        assert_eq!(parse_date("2026-10-05T10:00", "iso-8601"), Some(day(10, 5)));
        let today = day(10, 13);
        assert_eq!(resolve_date("-7d", "YYYY-MM-DD", today), Some(day(10, 6)));
        assert_eq!(resolve_date("-1w", "YYYY-MM-DD", today), Some(day(10, 6)));
        assert_eq!(resolve_date("-1M", "YYYY-MM-DD", today), Some(day(9, 13)));
        assert_eq!(
            resolve_date("+1y", "YYYY-MM-DD", today),
            NaiveDate::from_ymd_opt(2027, 10, 13)
        );
        assert_eq!(resolve_date("0d", "YYYY-MM-DD", today), Some(today));
        assert_eq!(
            resolve_date("2026-01-02", "DD.MM.YYYY", today),
            Some(day(1, 2)),
            "ISO too"
        );
        assert_eq!(resolve_date("soon", "YYYY-MM-DD", today), None);
        assert_eq!(number("70.5kg"), Some(70.5));
        assert_eq!(number("-3"), Some(-3.0));
        assert_eq!(number("kg"), None);
    }

    #[test]
    fn tags_fields_properties_and_tasks_by_day() {
        let files = [
            (
                "D/2026-10-01.md",
                "---\nmood: 7\nbp: 120/80\ntags: [walk]\n---\n#run:3 #run:2km weight:: 70 [sleep:: 7.5]\n- [x] Read a book\n- [ ] Read more\n[[Mom]] [[Mom|her]]\n",
            ),
            (
                "D/2026-10-03.md",
                "---\nmood: good\n---\n#run #run/long weight:: 69\n```\n#run:100\n```\n",
            ),
            ("D/pre-2026-10-04-post.md", "#run:9"),
            ("Other/2026-10-02.md", "#run:50"),
        ];
        let d = data(
            "tracker-collect",
            &files,
            "searchType: tag, dvField, frontmatter, frontmatter, task.done, task, wiki, dvField, tag\n\
             searchTarget: run, weight, mood, bp[1], read, read, Mom, sleep, walk\n\
             folder: D\ntextValueMap:\n  good: 8\nline:\n",
        )
        .unwrap();
        assert_eq!(
            (d.start, d.days),
            (day(10, 1), 3),
            "named dates only, from the first to the last"
        );
        let v = |i: usize| d.sets[i].values.clone();
        assert_eq!(
            v(0),
            [Some(5.0), None, Some(2.0)],
            "values summed; a code block skipped; subtags"
        );
        assert_eq!(d.sets[0].found, 4);
        assert_eq!(v(1), [Some(70.0), None, Some(69.0)]);
        assert_eq!(v(2), [Some(7.0), None, Some(8.0)], "text as a value");
        assert_eq!(v(3), [Some(80.0), None, None], "a value's part");
        assert_eq!(v(4), [Some(1.0), None, None]);
        assert_eq!(v(5), [Some(2.0), None, None]);
        assert_eq!(v(6), [Some(2.0), None, None]);
        assert_eq!(v(7), [Some(7.5), None, None], "a bracketed field");
        assert_eq!(v(8), [Some(1.0), None, None], "a frontmatter tag");
        assert_eq!(
            d.notes.get(&day(10, 1)).map(String::as_str),
            Some("D/2026-10-01.md")
        );
        // Text around the date.
        let d = data(
            "tracker-prefix",
            &files,
            "searchType: tag\nsearchTarget: run\nfolder: D\ndateFormatPrefix: 'pre-'\ndateFormatSuffix: '-post'\nbar:\n",
        )
        .unwrap();
        assert_eq!(d.sets[0].values.last(), Some(&Some(9.0)));
    }

    #[test]
    fn ranges_penalties_shifts_and_sums_over_time() {
        let files = [
            ("2026-10-01.md", "#a:1 #b:0"),
            ("2026-10-02.md", "#b:4"),
            ("2026-10-04.md", "#a:3 #b:2"),
        ];
        let d = data(
            "tracker-values",
            &files,
            "searchType: tag\nsearchTarget: a, b\npenalty: -1, 0\naccum: false, true\nignoreZeroValue: false, true\n\
             valueShift: 10, 0\nshiftOnlyValueLargerThan: 2, 0\nstartDate: 2026-09-30\nendDate: -9d\nline:\n",
        )
        .unwrap();
        assert_eq!((d.start, d.days), (day(9, 30), 5));
        assert_eq!(
            d.sets[0].values,
            [Some(-1.0), Some(1.0), Some(-1.0), Some(-1.0), Some(13.0)],
            "the penalty where it's missing, shifted above 2"
        );
        assert_eq!(
            d.sets[1].values,
            [Some(0.0), Some(0.0), Some(4.0), Some(4.0), Some(6.0)],
            "zero ignored (then the penalty), summed over time"
        );
        let err = data(
            "tracker-none",
            &files,
            "searchType: tag\nsearchTarget: zzz\nline:\n",
        )
        .unwrap_err();
        assert_eq!(err, "No valid data found");
        let err = data(
            "tracker-nodate",
            &[("a.md", "#a")],
            "searchType: tag\nsearchTarget: a\nline:\n",
        )
        .unwrap_err();
        assert_eq!(err, "No valid date found");
    }

    #[test]
    fn file_data_and_files_named() {
        let files = [
            ("2026-10-01.md", "One two three. Four!"),
            ("2026-10-02.md", "Five"),
            ("Elsewhere/2026-10-03.md", "Six seven"),
        ];
        let d = data(
            "tracker-filemeta",
            &files,
            "searchType: fileMeta, fileMeta, fileMeta, fileMeta
searchTarget: numWords, numChars, numSentences, size
line:
",
        )
        .unwrap();
        assert_eq!(d.sets[0].values, [Some(4.0), Some(1.0), Some(2.0)]);
        assert_eq!(d.sets[1].values[0], Some(20.0));
        assert_eq!(d.sets[2].values[0], Some(2.0));
        assert_eq!(d.sets[3].values[1], Some(4.0), "bytes");
        // Only the files named.
        let d = data(
            "tracker-files",
            &files,
            "searchType: fileMeta
searchTarget: numWords
folder: Nowhere
file: 2026-10-02, Elsewhere/2026-10-03.md
specifiedFilesOnly: true
line:
",
        )
        .unwrap();
        assert_eq!(
            (d.start, d.sets[0].values.clone()),
            (day(10, 2), vec![Some(1.0), Some(2.0)])
        );
    }

    #[test]
    fn nested_keys_times_and_linked_files() {
        let files = [
            (
                "Log/2026-10-01.md",
                "---\nbody:\n  weight: 70\n  bp: [120, 80]\n---\nslept:: 7h30m\nbed:: 23:15\n",
            ),
            (
                "Log/2026-10-02.md",
                "---\nbody:\n  weight: 69\n---\nslept:: 6h\nbed:: 22:30:30\n",
            ),
            (
                "Week.md",
                "- [[2026-10-01]] x2\n- [[Log/2026-10-02|Tuesday]]\n",
            ),
        ];
        let d = data(
            "tracker-nested",
            &files,
            "searchType: frontmatter, frontmatter, dvField, dvField\nsearchTarget: body.weight, body.bp[1], slept, bed\nfolder: Log\nline:\n",
        )
        .unwrap();
        assert_eq!(d.sets[0].values, [Some(70.0), Some(69.0)], "a key in a key");
        assert_eq!(d.sets[1].values, [Some(80.0), None], "a list's item");
        assert_eq!(
            d.sets[2].values,
            [Some(7.5), Some(6.0)],
            "a duration in hours"
        );
        let bed = d.sets[3].values.clone();
        assert_eq!(bed[0], Some(23.25), "a time in hours");
        assert!((bed[1].unwrap() - 22.508).abs() < 0.001, "{bed:?}");
        // The notes a note links to, each by the number after its link.
        let d = data(
            "tracker-linked",
            &files,
            "searchType: frontmatter\nsearchTarget: body.weight\nfolder: Nowhere\nfileContainsLinkedFiles: Week\n\
             fileMultiplierAfterLink: '^\\s*x(?<value>\\d+)'\nspecifiedFilesOnly: true\nline:\n",
        )
        .unwrap();
        assert_eq!(
            d.sets[0].values,
            [Some(140.0), Some(69.0)],
            "times 2 by its link"
        );
    }

    #[test]
    fn text_tables_and_dates_from_a_dataset() {
        let files = [
            (
                "Log.md",
                "| Day | Weight | BP |\n|---|---|---|\n| 2026-10-01 | 70 | 120/80 |\n| [[2026-10-02]] | 69.5 | 118/79 |\n",
            ),
            ("Runs/a.md", "date:: 2026-10-01\nRan 5 km, then 3 km.\n"),
            ("Runs/b.md", "date:: 2026-10-02\nRan 10 km\n"),
        ];
        let d = data(
            "tracker-table",
            &files,
            "searchType: table\nsearchTarget: Log[0][0], Log[0][1], Log[0][2][1]\nxDataset: 0\nline:\n",
        )
        .unwrap();
        assert!(d.sets[0].is_x);
        assert_eq!(d.sets[1].values, [Some(70.0), Some(69.5)]);
        assert_eq!(d.sets[2].values, [Some(80.0), Some(79.0)]);
        let d = data(
            "tracker-text",
            &files,
            "searchType: dvField, text, text\nsearchTarget: date, '(?<value>\\d+) km', Ran\nxDataset: 0\nfolder: Runs\nsummary:\n  template: x\n",
        )
        .unwrap();
        assert_eq!(d.sets[1].values, [Some(8.0), Some(10.0)], "a value group");
        assert_eq!(d.sets[2].values, [Some(1.0), Some(1.0)], "matches counted");
    }
}
