//! QuickAdd's format syntax (QA-10 … QA-16): `{{VALUE}}`, `{{VALUE:name}}`,
//! `{{VALUE:a,b,c}}`, `{{DATE}}`, `{{DATE:format}}`, `{{VDATE:name,
//! format}}`, `{{LINKCURRENT}}`, `{{SELECTED}}`, and blackglass's
//! `{{NOTE:folder}}` (a note's name). A format becomes its questions ([`questions`]), then
//! with the answers its text ([`fill`]). Unknown `{{…}}` stay as written.

use std::collections::HashMap;

use chrono::NaiveDateTime;

use crate::plugins::Question;
use crate::vault::Vault;

/// A piece of a format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Text(String),
    /// A value to type, by name (`""`: the unnamed `{{VALUE}}`).
    Value(String),
    /// One of these to choose.
    Options(Vec<String>),
    /// Now, in this moment.js format.
    Date(String),
    /// A date to type (in words too), by name, in this format.
    VDate {
        name: String,
        format: String,
    },
    LinkCurrent,
    Selected,
    /// A note in this folder to choose, put in by name (`[[{{NOTE:…}}]]`
    /// makes it a link).
    Note(String),
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
        let (tag, arg) = match inner.split_once(':') {
            Some((tag, arg)) => (tag.trim().to_uppercase(), Some(arg.trim())),
            None => (inner.trim().to_uppercase(), None),
        };
        let part = match (tag.as_str(), arg) {
            ("VALUE" | "NAME", None) => Some(Part::Value(String::new())),
            ("VALUE", Some(arg)) if arg.contains(',') => Some(Part::Options(
                arg.split(',')
                    .map(|o| o.trim().to_string())
                    .filter(|o| !o.is_empty())
                    .collect(),
            )),
            ("VALUE", Some(arg)) => Some(Part::Value(arg.to_string())),
            ("DATE", None) => Some(Part::Date("YYYY-MM-DD".into())),
            ("DATE", Some(arg)) => Some(Part::Date(arg.to_string())),
            ("VDATE", Some(arg)) => {
                let (name, format) = arg.split_once(',').unwrap_or((arg, "YYYY-MM-DD"));
                Some(Part::VDate {
                    name: name.trim().to_string(),
                    format: format.trim().to_string(),
                })
            }
            ("LINKCURRENT", None) => Some(Part::LinkCurrent),
            ("SELECTED", None) => Some(Part::Selected),
            ("NOTE", Some(arg)) => Some(Part::Note(arg.trim_matches('/').to_string())),
            _ => None,
        };
        match part {
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

/// What a part asks, keyed so that it's asked once: (key, question).
fn question(part: &Part, vault: &Vault) -> Option<(String, Question)> {
    match part {
        Part::Value(name) => Some((
            format!("value:{name}"),
            Question::Text {
                prompt: if name.is_empty() {
                    "Value".into()
                } else {
                    name.clone()
                },
                default: String::new(),
            },
        )),
        Part::Options(items) => Some((
            format!("options:{}", items.join(",")),
            Question::Choose {
                prompt: "Choose".into(),
                items: items.clone(),
            },
        )),
        Part::VDate { name, .. } => Some((
            format!("vdate:{name}"),
            Question::Text {
                prompt: format!("{name} (a date: 2026-10-20, tomorrow, next friday)"),
                default: "today".into(),
            },
        )),
        Part::Note(folder) => Some((
            format!("note:{folder}"),
            Question::Choose {
                prompt: format!("A note in {folder}/"),
                items: notes_in(vault, folder),
            },
        )),
        _ => None,
    }
}

/// The names of the notes in `folder` (and the folders in it), sorted.
pub fn notes_in(vault: &Vault, folder: &str) -> Vec<String> {
    let prefix = format!("{}/", folder.to_lowercase());
    let mut names: Vec<String> = vault
        .notes
        .iter()
        .filter(|n| {
            n.rel_name()
                .replace('\\', "/")
                .to_lowercase()
                .starts_with(&prefix)
        })
        .map(|n| n.name())
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
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
    pub now: NaiveDateTime,
    /// The active note's name.
    pub current: Option<String>,
    pub selected: Option<String>,
}

/// The format's text, with `answers` (by question key; a choice as its
/// item's text).
pub fn fill(parts: &[Part], answers: &HashMap<String, String>, env: &Env) -> String {
    let answer = |key: String| answers.get(&key).cloned().unwrap_or_default();
    let mut out = String::new();
    for part in parts {
        match part {
            Part::Text(t) => out.push_str(t),
            Part::Value(name) => out.push_str(&answer(format!("value:{name}"))),
            Part::Options(items) => out.push_str(&answer(format!("options:{}", items.join(",")))),
            Part::Date(format) => out.push_str(&crate::plugins::moment::format(&env.now, format)),
            Part::VDate { name, format } => {
                let typed = answer(format!("vdate:{name}"));
                match crate::nldates::parse_date(&typed, env.now, chrono::Weekday::Mon) {
                    Some(day) => out.push_str(&crate::plugins::moment::format(
                        &day.and_time(chrono::NaiveTime::MIN),
                        format,
                    )),
                    None => out.push_str(&typed),
                }
            }
            Part::LinkCurrent => {
                if let Some(name) = &env.current {
                    out.push_str(&format!("[[{name}]]"));
                }
            }
            Part::Selected => out.push_str(env.selected.as_deref().unwrap_or_default()),
            Part::Note(folder) => out.push_str(&answer(format!("note:{folder}"))),
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
            now: chrono::NaiveDate::from_ymd_opt(2026, 10, 8)
                .unwrap()
                .and_hms_opt(9, 30, 0)
                .unwrap(),
            current: Some("Plan".into()),
            selected: Some("this bit".into()),
        }
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
                "note:Budget/Envelopes",
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
        let answers: HashMap<String, String> = [
            ("note:Budget/Envelopes", "Fun"),
            ("value:amount", "28"),
            ("value:", "cinema"),
            ("options:a,b", "b"),
            ("vdate:due", "tomorrow"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        assert_eq!(
            fill(&parts, &answers, &env()),
            "- [[Fun]] 28 cinema b 28 2026-10-08 09:30 9 Oct [[Plan]] this bit {{MACRO:x}}"
        );
    }

    #[test]
    fn a_path_is_formatted_too() {
        let parts = parse("Logs/{{DATE:YYYY}}/{{DATE:MM}}.md");
        assert_eq!(fill(&parts, &HashMap::new(), &env()), "Logs/2026/10.md");
        assert_eq!(parse("no {{ tokens"), [Part::Text("no {{ tokens".into())]);
    }
}
