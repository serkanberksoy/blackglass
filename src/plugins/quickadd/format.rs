//! QuickAdd's format syntax (QA-10 … QA-36): a format becomes its
//! questions ([`questions`]), then with the answers its text ([`fill`]).
//!
//! - Values: `{{VALUE}}` / `{{NAME}}`, `{{VALUE:name}}` (asked once),
//!   `{{VALUE:a,b,c}}` (a choice; `|custom` takes other text too, `|multi`
//!   several), with `|default` (or `|default:x`), `|label:`,
//!   `|type:multiline`, `|case:`, `|trim:false`; `{{MVALUE}}` (LaTeX).
//! - Dates: `{{DATE}}`, `{{DATE:format}}`, `{{DATE+3}}` (days from now),
//!   `{{TIME}}`, `{{TIME:format}}`, `{{VDATE:name, format}}` (asked, in
//!   words too), with `|case:`, `|startof:` / `|endof:` (`day`, `week`,
//!   `isoweek`, `month`, `quarter`, `year`).
//! - The vault: `{{FIELD:property}}` (its values in the vault suggested;
//!   `|folder:`, `|tag:`, `|exclude-folder:`, `|exclude-tag:`,
//!   `|exclude-file:`, `|inline:true` for `key:: value` fields too),
//!   `{{FILE:folder}}` (a note to choose: its name, `|link`, `|path`,
//!   `|multi`, `|optional`; blackglass's older `{{NOTE:folder}}` too).
//! - The active note: `{{LINKCURRENT}}`, `{{LINKSECTION}}`,
//!   `{{FILENAMECURRENT}}`, `{{FOLDERCURRENT}}` (`|name`), `{{SELECTED}}`;
//!   the note written to: `{{TITLE}}`, `{{FOLDER}}` (`|name`).
//! - Others: `{{RANDOM:n}}`, `{{CURSOR}}`; `{{GLOBAL_VAR:name}}` and
//!   `{{TEMPLATE:path}}` are put in before anything else ([`include`]).
//!
//! Unknown `{{…}}` (`{{MACRO:…}}`) stay as written.

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::plugins::Question;
use crate::vault::Vault;

/// Where `{{CURSOR}}` was, in a filled format.
pub const CURSOR: char = '\u{1}';

/// The item of an optional `{{FILE}}` choice that means none.
const NONE: &str = "(none)";

/// A token's options (`|key:value`, `|flag`, `|default`), in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Opts(Vec<(String, String)>);

impl Opts {
    fn parse(pieces: &[&str]) -> Opts {
        const KEYS: [&str; 22] = [
            "default",
            "label",
            "type",
            "case",
            "format",
            "min",
            "max",
            "step",
            "name",
            "text",
            "folder",
            "tag",
            "exclude-folder",
            "exclude-tag",
            "exclude-file",
            "inline",
            "startof",
            "endof",
            "default-from",
            "default-empty",
            "default-always",
            "trim",
        ];
        const FLAGS: [&str; 9] = [
            "custom", "multi", "optional", "link", "path", "trim", "time", "datetime", "name",
        ];
        let mut out = Vec::new();
        for piece in pieces.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
            match piece.split_once(':') {
                Some((k, v)) if KEYS.contains(&k.trim().to_lowercase().as_str()) => {
                    out.push((k.trim().to_lowercase(), v.trim().to_string()))
                }
                _ if FLAGS.contains(&piece.to_lowercase().as_str()) => {
                    out.push((piece.to_lowercase(), String::new()))
                }
                _ => out.push(("default".into(), piece.to_string())),
            }
        }
        Opts(out)
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn all(&self, key: &str) -> Vec<&str> {
        self.0
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    fn has(&self, flag: &str) -> bool {
        self.get(flag).is_some_and(|v| v != "false")
    }
}

/// A piece of a format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Text(String),
    /// A value to type, by name (`""`: the unnamed `{{VALUE}}`).
    Value {
        name: String,
        opts: Opts,
    },
    /// One (or, `|multi`, several) of these to choose.
    Options {
        items: Vec<String>,
        opts: Opts,
    },
    /// Now (or `offset` days from now), in this moment.js format.
    Date {
        format: String,
        offset: i64,
        opts: Opts,
    },
    /// A date to type (in words too), by name, in this format.
    VDate {
        name: String,
        format: String,
        opts: Opts,
    },
    /// A property's values in the vault to choose from, or new text.
    Field {
        name: String,
        opts: Opts,
    },
    /// A note in this folder to choose.
    File {
        folder: String,
        opts: Opts,
    },
    /// LaTeX to type.
    Math,
    LinkCurrent,
    LinkSection,
    FileNameCurrent,
    FolderCurrent {
        name_only: bool,
    },
    Selected,
    Title,
    Folder {
        name_only: bool,
    },
    Random(usize),
    Cursor,
}

/// The pieces of `format`.
pub fn parse(format: &str) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut rest = format;
    while let Some(open) = rest.find("{{") {
        let Some(len) = rest[open + 2..].find("}}") else {
            break;
        };
        text.push_str(&rest[..open]);
        let inner = &rest[open + 2..open + 2 + len];
        match token(inner) {
            Some(part) => {
                if !text.is_empty() {
                    parts.push(Part::Text(std::mem::take(&mut text)));
                }
                parts.push(part);
            }
            None => text.push_str(&rest[open..open + 2 + len + 2]),
        }
        rest = &rest[open + 2 + len + 2..];
    }
    text.push_str(rest);
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    parts
}

/// What's between `{{` and `}}`, if QuickAdd knows it.
fn token(inner: &str) -> Option<Part> {
    let mut pieces = inner.split('|');
    let head = pieces.next().unwrap_or_default();
    let opts = Opts::parse(&pieces.collect::<Vec<_>>());
    let (tag, arg) = match head.split_once(':') {
        Some((tag, arg)) => (tag.trim().to_uppercase(), Some(arg.trim())),
        None => (head.trim().to_uppercase(), None),
    };
    // `DATE+3`, `DATE:YYYY-MM-DD+-3`: days from now (not for `TIME`).
    for (word, default) in [("DATE", "YYYY-MM-DD"), ("TIME", "HH:mm")] {
        let Some(after) = tag.strip_prefix(word) else {
            continue;
        };
        let mut offset = match after {
            "" => 0,
            _ if word == "DATE" => after.strip_prefix('+')?.parse().ok()?,
            _ => return None,
        };
        let mut format = arg.unwrap_or(default).to_string();
        if word == "DATE"
            && let Some((f, days)) = format.rsplit_once('+')
            && let Ok(days) = days.parse::<i64>()
        {
            offset += days;
            format = f.to_string();
        }
        return Some(Part::Date {
            format,
            offset,
            opts,
        });
    }
    Some(match (tag.as_str(), arg) {
        ("VALUE" | "NAME", None) => Part::Value {
            name: String::new(),
            opts,
        },
        ("VALUE", Some(arg)) if arg.contains(',') => Part::Options {
            items: options(arg),
            opts,
        },
        ("VALUE", Some(arg)) => Part::Value {
            name: arg.to_string(),
            opts,
        },
        ("MVALUE", None) => Part::Math,
        ("VDATE", Some(arg)) => {
            let (name, format) = arg.split_once(',').unwrap_or((arg, "YYYY-MM-DD"));
            Part::VDate {
                name: name.trim().to_string(),
                format: format.trim().to_string(),
                opts,
            }
        }
        ("FIELD", Some(arg)) => Part::Field {
            name: arg.to_string(),
            opts,
        },
        ("FILE" | "NOTE", Some(arg)) => Part::File {
            folder: arg.trim_matches('/').to_string(),
            opts,
        },
        ("LINKCURRENT", None) => Part::LinkCurrent,
        ("LINKSECTION", None) => Part::LinkSection,
        ("FILENAMECURRENT", None) => Part::FileNameCurrent,
        ("FOLDERCURRENT", None) => Part::FolderCurrent {
            name_only: opts.has("name"),
        },
        ("SELECTED", None) => Part::Selected,
        ("TITLE", None) => Part::Title,
        ("FOLDER", None) => Part::Folder {
            name_only: opts.has("name"),
        },
        ("RANDOM", Some(n)) => Part::Random(n.parse::<usize>().ok()?.clamp(1, 100)),
        ("CURSOR", None) => Part::Cursor,
        _ => return None,
    })
}

/// `a, "b, c", d` as its items (quotes keep commas).
fn options(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut item = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => items.push(std::mem::take(&mut item)),
            c => item.push(c),
        }
    }
    items.push(item);
    items
        .into_iter()
        .map(|i| i.trim().to_string())
        .filter(|i| !i.is_empty())
        .collect()
}

/// `format` with its `{{GLOBAL_VAR:name}}` and `{{TEMPLATE:path}}` put in
/// (`read` gives a note's text by its path in the vault), again in what
/// they put in (a few levels deep).
pub fn include(
    format: &str,
    globals: &HashMap<String, String>,
    read: &dyn Fn(&str) -> Option<String>,
) -> String {
    let mut text = format.to_string();
    for _ in 0..5 {
        let mut out = String::new();
        let mut rest = text.as_str();
        let mut changed = false;
        while let Some(open) = rest.find("{{") {
            let Some(len) = rest[open + 2..].find("}}") else {
                break;
            };
            out.push_str(&rest[..open]);
            let inner = &rest[open + 2..open + 2 + len];
            let put = match inner.split_once(':') {
                Some((tag, name)) if tag.trim().eq_ignore_ascii_case("GLOBAL_VAR") => {
                    globals.get(name.trim()).cloned()
                }
                Some((tag, path)) if tag.trim().eq_ignore_ascii_case("TEMPLATE") => {
                    let path = path.trim();
                    read(path).or_else(|| read(&format!("{path}.md")))
                }
                _ => None,
            };
            match put {
                Some(p) => {
                    out.push_str(&p);
                    changed = true;
                }
                None => out.push_str(&rest[open..open + 2 + len + 2]),
            }
            rest = &rest[open + 2 + len + 2..];
        }
        out.push_str(rest);
        text = out;
        if !changed {
            break;
        }
    }
    text
}

/// What a part asks, keyed so that it's asked once: (key, question).
fn question(part: &Part, vault: &Vault) -> Option<(String, Question)> {
    let label = |opts: &Opts, fallback: &str| {
        opts.get("label")
            .map(String::from)
            .unwrap_or_else(|| fallback.to_string())
    };
    let default = |opts: &Opts| opts.get("default").unwrap_or_default().to_string();
    Some(match part {
        Part::Value { name, opts } => {
            let prompt = label(opts, if name.is_empty() { "Value" } else { name });
            let q = if opts.get("type") == Some("multiline") {
                Question::Lines {
                    prompt,
                    default: default(opts),
                }
            } else {
                Question::Text {
                    prompt,
                    default: default(opts),
                }
            };
            (format!("value:{name}"), q)
        }
        Part::Options { items, opts } => {
            let prompt = label(opts, "Choose");
            let items = items.clone();
            let q = if opts.has("multi") {
                Question::Many { prompt, items }
            } else if opts.has("custom") {
                Question::Suggest {
                    prompt,
                    items,
                    default: default(opts),
                }
            } else {
                Question::Choose { prompt, items }
            };
            (format!("options:{}", options_key(part)), q)
        }
        Part::VDate { name, opts, .. } => (
            format!("vdate:{name}"),
            Question::Text {
                prompt: label(
                    opts,
                    &format!("{name} (a date: 2026-10-20, tomorrow, next friday)"),
                ),
                default: match opts.get("default") {
                    Some(d) => d.to_string(),
                    None if opts.has("optional") => String::new(),
                    None => "today".into(),
                },
            },
        ),
        Part::Field { name, opts } => (
            format!("field:{name}"),
            Question::Suggest {
                prompt: label(opts, name),
                items: field_values(vault, name, opts),
                default: default(opts),
            },
        ),
        Part::File { folder, opts } => {
            let mut items = notes_in(vault, folder);
            if opts.has("optional") {
                items.insert(0, NONE.into());
            }
            let prompt = label(opts, &format!("A note in {folder}/"));
            let q = if opts.has("multi") {
                Question::Many { prompt, items }
            } else {
                Question::Choose { prompt, items }
            };
            (format!("file:{folder}"), q)
        }
        Part::Math => (
            "math".into(),
            Question::Text {
                prompt: "Math (LaTeX)".into(),
                default: String::new(),
            },
        ),
        _ => return None,
    })
}

fn options_key(part: &Part) -> String {
    match part {
        Part::Options { items, .. } => items.join(","),
        _ => String::new(),
    }
}

/// The notes in `folder` (and the folders in it), by their path from it
/// without `.md`, sorted.
pub fn notes_in(vault: &Vault, folder: &str) -> Vec<String> {
    let prefix = if folder.is_empty() {
        String::new()
    } else {
        format!("{}/", folder.to_lowercase())
    };
    let mut names: Vec<String> = vault
        .notes
        .iter()
        .filter_map(|n| {
            let rel = n.rel_name().replace('\\', "/");
            rel.to_lowercase()
                .starts_with(&prefix)
                .then(|| rel[prefix.len()..].to_string())
        })
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// The values a property has in the vault's notes (its list items one by
/// one), with the token's filters; `|inline:true`: `key:: value` fields
/// in the text too. Sorted, each once.
pub fn field_values(vault: &Vault, name: &str, opts: &Opts) -> Vec<String> {
    let lower = |s: &str| s.trim().trim_start_matches('#').to_lowercase();
    let in_folder = |rel: &str, folder: &str| {
        let folder = folder.trim_matches('/').to_lowercase();
        rel.to_lowercase().starts_with(&format!("{folder}/"))
    };
    let folders = opts.all("folder");
    let tags: Vec<String> = opts.all("tag").into_iter().map(lower).collect();
    let no_folders = opts.all("exclude-folder");
    let no_tags: Vec<String> = opts.all("exclude-tag").into_iter().map(lower).collect();
    let no_files = opts.all("exclude-file");
    let inline = opts.get("inline") == Some("true");
    let mut values: Vec<String> = Vec::new();
    let mut add = |v: &str| {
        let v = v.trim().trim_matches('"');
        if !v.is_empty() && !values.iter().any(|x| x == v) {
            values.push(v.to_string());
        }
    };
    for note in &vault.notes {
        let rel = note.rel_name().replace('\\', "/");
        let note_tags: Vec<String> = note.tags.iter().map(|t| lower(t)).collect();
        let keep = (folders.is_empty() || folders.iter().any(|f| in_folder(&rel, f)))
            && tags.iter().all(|t| note_tags.contains(t))
            && !no_folders.iter().any(|f| in_folder(&rel, f))
            && !no_tags.iter().any(|t| note_tags.contains(t))
            && !no_files
                .iter()
                .any(|f| f.eq_ignore_ascii_case(&note.name()) || f.eq_ignore_ascii_case(&rel));
        if !keep {
            continue;
        }
        for (key, value) in &note.properties {
            if key.eq_ignore_ascii_case(name) {
                value.split(", ").for_each(&mut add);
            }
        }
        if inline {
            let field = format!("{}::", name.to_lowercase());
            for line in &note.lines {
                let low = line.to_lowercase();
                let mut from = 0;
                while let Some(at) = low[from..].find(&field) {
                    let start = from + at;
                    let before = low[..start].chars().next_back();
                    let after = start + field.len();
                    from = after;
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let bracketed = before == Some('[') || before == Some('(');
                    let rest = &line[after..];
                    let value = if bracketed {
                        rest.split([']', ')']).next().unwrap_or_default()
                    } else {
                        rest
                    };
                    add(value);
                }
            }
        }
    }
    values.sort_by_key(|v| v.to_lowercase());
    values
}

/// The questions of these formats, each asked once, in order.
pub fn questions(formats: &[&[Part]], vault: &Vault) -> Vec<(String, Question)> {
    let mut out: Vec<(String, Question)> = Vec::new();
    for part in formats.iter().flat_map(|f| f.iter()) {
        if let Some((key, q)) = question(part, vault)
            && !out.iter().any(|(k, _)| *k == key)
        {
            out.push((key, q));
        }
    }
    out
}

/// What the format's text needs besides the answers.
#[derive(Debug, Clone, Default)]
pub struct Env {
    /// The moment the capture is about (now, or the day chosen).
    pub now: NaiveDateTime,
    /// The active note: its path in the vault without `.md`, and the
    /// heading the cursor is under.
    pub current: Option<String>,
    pub section: Option<String>,
    pub selected: Option<String>,
    /// The note written to: its name and folder.
    pub title: String,
    pub folder: String,
}

/// The format's text, with `answers` (by question key; a choice as its
/// item's text, several on lines of their own).
pub fn fill(parts: &[Part], answers: &HashMap<String, String>, env: &Env) -> String {
    let answer = |key: String| answers.get(&key).cloned().unwrap_or_default();
    let last = |path: &str| path.rsplit('/').next().unwrap_or(path).to_string();
    let mut out = String::new();
    for part in parts {
        match part {
            Part::Text(t) => out.push_str(t),
            Part::Value { name, opts } => {
                let mut v = answer(format!("value:{name}"));
                if opts.get("trim") != Some("false") {
                    v = v.trim().to_string();
                }
                out.push_str(&case(&v, opts));
            }
            Part::Options { opts, .. } => {
                let v = answer(format!("options:{}", options_key(part)));
                let v = v.lines().collect::<Vec<_>>().join(", ");
                out.push_str(&case(&v, opts));
            }
            Part::Date {
                format,
                offset,
                opts,
            } => {
                let day = shift(env.now + Duration::days(*offset), opts);
                out.push_str(&case(&crate::plugins::moment::format(&day, format), opts));
            }
            Part::VDate { name, format, opts } => {
                let typed = answer(format!("vdate:{name}"));
                if typed.trim().is_empty() {
                    continue;
                }
                match crate::nldates::parse_date(&typed, env.now, chrono::Weekday::Mon) {
                    Some(day) => {
                        let day = shift(day.and_time(chrono::NaiveTime::MIN), opts);
                        out.push_str(&case(&crate::plugins::moment::format(&day, format), opts));
                    }
                    None => out.push_str(&typed),
                }
            }
            Part::Field { name, opts } => {
                out.push_str(&case(answer(format!("field:{name}")).trim(), opts))
            }
            Part::File { folder, opts } => {
                let shown: Vec<String> = answer(format!("file:{folder}"))
                    .lines()
                    .filter(|n| !n.is_empty() && *n != NONE)
                    .map(|n| {
                        if opts.has("path") {
                            let full = if folder.is_empty() {
                                n.to_string()
                            } else {
                                format!("{folder}/{n}")
                            };
                            format!("{full}.md")
                        } else if opts.has("link") {
                            format!("[[{}]]", last(n))
                        } else {
                            last(n)
                        }
                    })
                    .collect();
                out.push_str(&shown.join(", "));
            }
            Part::Math => out.push_str(&answer("math".into())),
            Part::LinkCurrent => {
                if let Some(c) = &env.current {
                    out.push_str(&format!("[[{}]]", last(c)));
                }
            }
            Part::LinkSection => match (&env.current, &env.section) {
                (Some(c), Some(s)) => out.push_str(&format!("[[{}#{s}]]", last(c))),
                (Some(c), None) => out.push_str(&format!("[[{}]]", last(c))),
                _ => {}
            },
            Part::FileNameCurrent => {
                if let Some(c) = &env.current {
                    out.push_str(&last(c));
                }
            }
            Part::FolderCurrent { name_only } => {
                if let Some(c) = &env.current {
                    let folder = c.rsplit_once('/').map(|(f, _)| f).unwrap_or_default();
                    out.push_str(&if *name_only {
                        last(folder)
                    } else {
                        folder.to_string()
                    });
                }
            }
            Part::Selected => out.push_str(env.selected.as_deref().unwrap_or_default()),
            Part::Title => out.push_str(&env.title),
            Part::Folder { name_only } => out.push_str(&if *name_only {
                last(&env.folder)
            } else {
                env.folder.clone()
            }),
            Part::Random(n) => out.push_str(&random(*n)),
            Part::Cursor => out.push(CURSOR),
        }
    }
    out
}

/// `day` moved to the start or end of its `|startof:` / `|endof:` unit.
fn shift(day: NaiveDateTime, opts: &Opts) -> NaiveDateTime {
    let (unit, end) = match (opts.get("startof"), opts.get("endof")) {
        (Some(u), _) => (u, false),
        (None, Some(u)) => (u, true),
        _ => return day,
    };
    let d = day.date();
    let (first, last): (NaiveDate, NaiveDate) = match unit.to_lowercase().as_str() {
        "year" => (
            NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap_or(d),
            NaiveDate::from_ymd_opt(d.year(), 12, 31).unwrap_or(d),
        ),
        "quarter" => {
            let q = (d.month() - 1) / 3 * 3 + 1;
            let first = NaiveDate::from_ymd_opt(d.year(), q, 1).unwrap_or(d);
            let next = first
                .checked_add_months(chrono::Months::new(3))
                .unwrap_or(first);
            (first, next.pred_opt().unwrap_or(next))
        }
        "month" => {
            let first = d.with_day(1).unwrap_or(d);
            let next = first
                .checked_add_months(chrono::Months::new(1))
                .unwrap_or(first);
            (first, next.pred_opt().unwrap_or(next))
        }
        "week" | "isoweek" => {
            let back = if unit.eq_ignore_ascii_case("isoweek") {
                d.weekday().num_days_from_monday()
            } else {
                d.weekday().num_days_from_sunday()
            };
            let first = d - Duration::days(back.into());
            (first, first + Duration::days(6))
        }
        _ => (d, d),
    };
    let chosen = if end { last } else { first };
    if end {
        chosen.and_hms_opt(23, 59, 59).unwrap_or(day)
    } else {
        chosen.and_time(chrono::NaiveTime::MIN)
    }
}

/// `text` in the `|case:` style: `kebab`, `snake`, `camel`, `pascal`,
/// `title`, `lower`, `upper`, `slug`.
fn case(text: &str, opts: &Opts) -> String {
    let Some(style) = opts.get("case") else {
        return text.to_string();
    };
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let cap = |w: &str| {
        let mut c = w.chars();
        c.next()
            .map(|f| f.to_uppercase().chain(c).collect::<String>())
            .unwrap_or_default()
    };
    match style.to_lowercase().as_str() {
        "lower" => text.to_lowercase(),
        "upper" => text.to_uppercase(),
        "kebab" | "slug" => words.join("-"),
        "snake" => words.join("_"),
        "title" => words.iter().map(|w| cap(w)).collect::<Vec<_>>().join(" "),
        "pascal" => words.iter().map(|w| cap(w)).collect(),
        "camel" => words
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { w.clone() } else { cap(w) })
            .collect(),
        _ => text.to_string(),
    }
}

/// `n` random letters and digits.
fn random(n: usize) -> String {
    use std::hash::{BuildHasher, Hasher};
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut out = String::new();
    while out.len() < n {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_usize(out.len());
        let mut x = h.finish();
        for _ in 0..8 {
            if out.len() < n {
                out.push(CHARS[(x % CHARS.len() as u64) as usize] as char);
                x /= CHARS.len() as u64;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    fn env() -> Env {
        Env {
            now: NaiveDate::from_ymd_opt(2026, 10, 8)
                .unwrap()
                .and_hms_opt(9, 30, 0)
                .unwrap(),
            current: Some("Work/Projects/Plan".into()),
            section: Some("Goals".into()),
            selected: Some("this bit".into()),
            title: "Log".into(),
            folder: "Logs/2026".into(),
        }
    }

    fn answers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn filled(format: &str, given: &[(&str, &str)]) -> String {
        fill(&parse(format), &answers(given), &env())
    }

    #[test]
    fn values_dates_and_choices() {
        let dir = scratch("quickadd-format");
        write(
            &dir,
            &[
                ("Budget/Envelopes/Rent.md", ""),
                ("Budget/Envelopes/Fun.md", ""),
                ("Other.md", ""),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        let parts = parse(
            "- [[{{NOTE:Budget/Envelopes/}}]] {{value:amount}} {{VALUE}} {{VALUE:a, b}} {{VALUE:amount}} \
             {{DATE}} {{DATE:HH:mm}} {{VDATE:due, D MMM}} {{LINKCURRENT}} {{selected}} {{MACRO:x}}",
        );
        let asked = questions(&[&parts], &vault);
        let keys: Vec<&str> = asked.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            [
                "file:Budget/Envelopes",
                "value:amount",
                "value:",
                "options:a,b",
                "vdate:due"
            ],
            "each asked once, in order"
        );
        assert_eq!(
            asked[0].1,
            Question::Choose {
                prompt: "A note in Budget/Envelopes/".into(),
                items: vec!["Fun".into(), "Rent".into()],
            }
        );
        let given = answers(&[
            ("file:Budget/Envelopes", "Fun"),
            ("value:amount", "28"),
            ("value:", "cinema"),
            ("options:a,b", "b"),
            ("vdate:due", "tomorrow"),
        ]);
        assert_eq!(
            fill(&parts, &given, &env()),
            "- [[Fun]] 28 cinema b 28 2026-10-08 09:30 9 Oct [[Plan]] this bit {{MACRO:x}}"
        );
    }

    #[test]
    fn a_path_is_formatted_too() {
        assert_eq!(
            filled("Logs/{{DATE:YYYY}}/{{DATE:MM}}.md", &[]),
            "Logs/2026/10.md"
        );
        assert_eq!(parse("no {{ tokens"), [Part::Text("no {{ tokens".into())]);
    }

    #[test]
    fn value_options_defaults_labels_and_kinds() {
        let dir = scratch("quickadd-options");
        let vault = Vault::open(&dir).unwrap();
        let parts = parse(
            "{{VALUE:title|Untitled}} {{VALUE:size|label:How big?}} {{VALUE:a,b,\"c, d\"|custom}} \
             {{VALUE:x,y|multi}} {{VALUE:notes|type:multiline}} {{MVALUE}}",
        );
        let asked = questions(&[&parts], &vault);
        let qs: Vec<&Question> = asked.iter().map(|(_, q)| q).collect();
        assert_eq!(
            qs[0],
            &Question::Text {
                prompt: "title".into(),
                default: "Untitled".into()
            }
        );
        assert_eq!(
            qs[1],
            &Question::Text {
                prompt: "How big?".into(),
                default: String::new()
            }
        );
        assert_eq!(
            qs[2],
            &Question::Suggest {
                prompt: "Choose".into(),
                items: vec!["a".into(), "b".into(), "c, d".into()],
                default: String::new()
            },
            "quotes keep a comma"
        );
        assert!(matches!(qs[3], Question::Many { items, .. } if items.len() == 2));
        assert!(matches!(qs[4], Question::Lines { .. }));
        assert!(matches!(qs[5], Question::Text { prompt, .. } if prompt == "Math (LaTeX)"));
        assert_eq!(
            filled(
                "{{VALUE:x,y|multi}} / {{VALUE:t|case:kebab}} / {{VALUE:t|case:pascal}} / {{VALUE:t|case:title}} / {{MVALUE}}",
                &[
                    ("options:x,y", "x\ny"),
                    ("value:t", "  my New idea "),
                    ("math", "e = mc^2")
                ]
            ),
            "x, y / my-new-idea / MyNewIdea / My New Idea / e = mc^2"
        );
    }

    #[test]
    fn dates_offsets_times_and_modifiers() {
        // 2026-10-08 is a Thursday.
        assert_eq!(
            filled(
                "{{DATE+1}} {{DATE:YYYY-MM-DD+-3}} {{TIME}} {{TIME:HH}} {{DATE:MMMM|case:upper}}",
                &[]
            ),
            "2026-10-09 2026-10-05 09:30 09 OCTOBER"
        );
        assert_eq!(
            filled(
                "{{DATE:YYYY-MM-DD|startof:month}} {{DATE:YYYY-MM-DD|endof:month}} \
                 {{DATE:YYYY-MM-DD|startof:isoweek}} {{DATE:YYYY-MM-DD|startof:week}} \
                 {{DATE:YYYY-MM-DD|endof:quarter}} {{DATE:YYYY-MM-DD|startof:year}}",
                &[]
            ),
            "2026-10-01 2026-10-31 2026-10-05 2026-10-04 2026-12-31 2026-01-01"
        );
        let dir = scratch("quickadd-vdate");
        let vault = Vault::open(&dir).unwrap();
        let asked = questions(
            &[&parse(
                "{{VDATE:due, YYYY-MM-DD|tomorrow}} {{VDATE:start|optional}}",
            )],
            &vault,
        );
        assert!(matches!(&asked[0].1, Question::Text { default, .. } if default == "tomorrow"));
        assert!(matches!(&asked[1].1, Question::Text { default, .. } if default.is_empty()));
        assert_eq!(
            filled(
                "[{{VDATE:start, YYYY-MM-DD|optional}}]",
                &[("vdate:start", "")]
            ),
            "[]",
            "optional: nothing"
        );
    }

    #[test]
    fn the_active_note_and_the_note_written_to() {
        assert_eq!(
            filled(
                "{{LINKCURRENT}} {{LINKSECTION}} {{FILENAMECURRENT}} {{FOLDERCURRENT}} {{FOLDERCURRENT|name}} {{TITLE}} {{FOLDER}} {{FOLDER|name}}",
                &[]
            ),
            "[[Plan]] [[Plan#Goals]] Plan Work/Projects Projects Log Logs/2026 2026"
        );
        let r = filled("{{RANDOM:8}}", &[]);
        assert_eq!(r.len(), 8);
        assert!(r.chars().all(|c| c.is_ascii_alphanumeric()), "{r}");
        assert_eq!(filled("a{{CURSOR}}b", &[]), format!("a{CURSOR}b"));
    }

    #[test]
    fn globals_and_templates_are_put_in_first() {
        let globals: HashMap<String, String> =
            [("sig".to_string(), "— {{VALUE:who}}".to_string())].into();
        let read = |path: &str| (path == "Templates/T.md").then(|| "body {{DATE}}".to_string());
        assert_eq!(
            include(
                "a {{GLOBAL_VAR:sig}} {{TEMPLATE:Templates/T}} {{GLOBAL_VAR:nope}}",
                &globals,
                &read
            ),
            "a — {{VALUE:who}} body {{DATE}} {{GLOBAL_VAR:nope}}"
        );
    }

    #[test]
    fn field_values_with_filters_and_inline_fields() {
        let dir = scratch("quickadd-field");
        write(
            &dir,
            &[
                ("Projects/A.md", "---\nstatus: open\ntags: [work]\n---\n"),
                ("Projects/B.md", "---\nstatus: done\n---\n"),
                ("Projects/Old/C.md", "---\nstatus: archived\n---\n"),
                (
                    "Home/D.md",
                    "---\nStatus: someday\n---\n- [status:: waiting]\nstatus:: blocked\n",
                ),
                ("Home/E.md", "---\nstatus: [x, y]\n---\n"),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        let values = |format: &str| match &questions(&[&parse(format)], &vault)[0].1 {
            Question::Suggest { items, .. } => items.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            values("{{FIELD:status}}"),
            ["archived", "done", "open", "someday", "x", "y"]
        );
        assert_eq!(
            values("{{FIELD:status|folder:Projects|exclude-folder:Projects/Old}}"),
            ["done", "open"]
        );
        assert_eq!(values("{{FIELD:status|tag:work}}"), ["open"]);
        assert_eq!(
            values("{{FIELD:status|folder:Home|inline:true|exclude-file:E}}"),
            ["blocked", "someday", "waiting"]
        );
    }

    #[test]
    fn file_choices_link_path_many_and_none() {
        let dir = scratch("quickadd-file");
        write(&dir, &[("People/Ada.md", ""), ("People/Team/Bo.md", "")]);
        let vault = Vault::open(&dir).unwrap();
        let asked = questions(&[&parse("{{FILE:People|optional}}")], &vault);
        assert!(
            matches!(&asked[0].1, Question::Choose { items, .. } if items == &["(none)", "Ada", "Team/Bo"])
        );
        assert_eq!(
            filled("{{FILE:People|link}}", &[("file:People", "Team/Bo")]),
            "[[Bo]]"
        );
        assert_eq!(
            filled("{{FILE:People|path}}", &[("file:People", "Team/Bo")]),
            "People/Team/Bo.md"
        );
        assert_eq!(
            filled(
                "{{FILE:People|multi|link}}",
                &[("file:People", "Ada\nTeam/Bo")]
            ),
            "[[Ada]], [[Bo]]"
        );
        assert_eq!(
            filled("[{{FILE:People}}]", &[("file:People", "(none)")]),
            "[]"
        );
    }
}
