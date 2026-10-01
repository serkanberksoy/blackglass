//! Unique note IDs (a vault setting, for a Zettelkasten): every new note
//! gets an ID made from the time in a format of your choosing
//! (`YYYYMMDDHHmm`, `YYYY-MM` …), in its name (`202610011445 Meeting`), as
//! its name (`202610011445`, the title in a `title` property) or in its
//! properties (`id: 202610011445`). The settings are the vault's, in
//! `.blackglass/notes.toml`, on the settings window's Notes page.

use chrono::NaiveDateTime;

use crate::plugins::settings::{Kind, Setting, Values};
use crate::vault::Vault;

/// The settings file, relative to the vault.
pub const FILE: &str = ".blackglass/notes.toml";

/// The default ID format (moment tokens).
pub const DEFAULT_FORMAT: &str = "YYYYMMDDHHmm";

/// Where a new note's ID goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// No IDs.
    #[default]
    Off,
    /// `202610011445 Meeting`.
    InName,
    /// `202610011445`, with `title: Meeting`.
    AsName,
    /// `Meeting`, with `id: 202610011445`.
    InProperties,
}

/// The modes as the settings window shows them, in order.
const MODES: [(Mode, &str); 4] = [
    (Mode::Off, "off"),
    (Mode::InName, "in the name"),
    (Mode::AsName, "as the name"),
    (Mode::InProperties, "in the properties"),
];

/// A vault's note ID settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteIds {
    pub mode: Mode,
    /// moment.js tokens.
    pub format: String,
}

impl Default for NoteIds {
    fn default() -> Self {
        NoteIds {
            mode: Mode::Off,
            format: DEFAULT_FORMAT.into(),
        }
    }
}

/// What a new note gets: its file name (without `.md`) and the properties
/// to put in its frontmatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNote {
    pub name: String,
    pub properties: Vec<(String, String)>,
}

impl NoteIds {
    /// The settings window's rows.
    pub fn settings() -> Vec<Setting> {
        vec![
            Setting::new(
                "",
                "ids",
                "Note IDs",
                "Give every new note a unique ID: in its name, as its name, or in its properties",
                Kind::Choice(MODES.iter().map(|(_, n)| n.to_string()).collect()),
                "off",
            ),
            Setting::new(
                "",
                "id_format",
                "ID format",
                "A unique ID from the time, in moment.js tokens (YYYYMMDDHHmm, YYYY-MM-DD …)",
                Kind::Text,
                DEFAULT_FORMAT,
            ),
        ]
    }

    /// From the settings file's values.
    pub fn from_values(values: &Values) -> NoteIds {
        let mode = values
            .get("", "ids")
            .and_then(|v| MODES.iter().find(|(_, n)| *n == v))
            .map_or(Mode::Off, |(m, _)| *m);
        let format = values
            .get("", "id_format")
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .unwrap_or(DEFAULT_FORMAT)
            .to_string();
        NoteIds { mode, format }
    }

    /// The vault's settings (the defaults without a file).
    pub fn load(vault: &Vault) -> NoteIds {
        NoteIds::from_values(&values(vault))
    }

    /// An ID for `now` that `taken` doesn't have yet: the formatted time,
    /// else with `-2`, `-3` … after it (a coarse format, several notes a
    /// minute).
    pub fn id(&self, now: NaiveDateTime, taken: impl Fn(&str) -> bool) -> String {
        let id = crate::plugins::moment::format(&now, &self.format);
        (1..)
            .map(|n| {
                if n == 1 {
                    id.clone()
                } else {
                    format!("{id}-{n}")
                }
            })
            .find(|candidate| !taken(candidate))
            .expect("some ID is free")
    }

    /// What a note to be called `title` gets with ID `id`; `None` when IDs
    /// are off.
    pub fn new_note(&self, title: &str, id: &str) -> Option<NewNote> {
        let title = title.trim();
        let (name, properties) = match self.mode {
            Mode::Off => return None,
            Mode::InName if title.is_empty() => (id.to_string(), Vec::new()),
            Mode::InName => (format!("{id} {title}"), Vec::new()),
            Mode::AsName if title.is_empty() => (id.to_string(), Vec::new()),
            Mode::AsName => (
                id.to_string(),
                vec![
                    ("title".to_string(), title.to_string()),
                    ("aliases".to_string(), format!("[{title}]")),
                ],
            ),
            Mode::InProperties => (title.to_string(), vec![("id".to_string(), id.to_string())]),
        };
        Some(NewNote { name, properties })
    }

    /// Whether `id` is some note's already: its name, the start of its
    /// name, or its `id` property.
    pub fn taken(vault: &Vault, id: &str) -> bool {
        vault.notes.iter().any(|n| {
            let name = n.name();
            name == id
                || name.strip_prefix(id).is_some_and(|r| r.starts_with(' '))
                || n.properties
                    .iter()
                    .any(|(k, v)| k == "id" && v.trim() == id)
        })
    }
}

/// The settings file's values.
pub fn values(vault: &Vault) -> Values {
    let text = std::fs::read_to_string(vault.root.join(FILE)).unwrap_or_default();
    Values::parse(&text)
}

/// A note's text with `properties` added to its frontmatter (made if
/// there's none; ones it has already are kept).
pub fn with_properties(text: &str, properties: &[(String, String)]) -> String {
    let (mut props, body) = crate::properties::parse(text);
    for (key, value) in properties {
        if !props.iter().any(|p| p.key == *key) {
            props.push(crate::properties::Property {
                key: key.clone(),
                value: crate::properties::Value::guess(value),
            });
        }
    }
    crate::properties::write(&props, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn ids_in_any_format_and_unique() {
        let ids = NoteIds::default();
        assert_eq!(ids.id(at("2026-10-01 14:45"), |_| false), "202610011445");
        let monthly = NoteIds {
            format: "YYYY-MM".into(),
            ..NoteIds::default()
        };
        let now = at("2026-10-01 14:45");
        assert_eq!(monthly.id(now, |_| false), "2026-10");
        assert_eq!(monthly.id(now, |id| id == "2026-10"), "2026-10-2");
        assert_eq!(
            monthly.id(now, |id| id == "2026-10" || id == "2026-10-2"),
            "2026-10-3"
        );
    }

    #[test]
    fn where_the_id_goes() {
        let with = |mode| NoteIds {
            mode,
            ..NoteIds::default()
        };
        assert_eq!(with(Mode::Off).new_note("Meeting", "1"), None);
        assert_eq!(
            with(Mode::InName).new_note("Meeting", "1").unwrap().name,
            "1 Meeting"
        );
        assert_eq!(with(Mode::InName).new_note("", "1").unwrap().name, "1");
        let as_name = with(Mode::AsName).new_note("Meeting", "1").unwrap();
        assert_eq!(as_name.name, "1");
        assert_eq!(as_name.properties[0], ("title".into(), "Meeting".into()));
        let props = with(Mode::InProperties).new_note("Meeting", "1").unwrap();
        assert_eq!(props.name, "Meeting");
        assert_eq!(props.properties, [("id".into(), "1".into())]);
        let values = Values::parse("ids = \"as the name\"\nid_format = \"YYYY-MM\"\n");
        assert_eq!(
            NoteIds::from_values(&values),
            NoteIds {
                mode: Mode::AsName,
                format: "YYYY-MM".into()
            }
        );
    }

    #[test]
    fn properties_join_a_frontmatter() {
        let id = [("id".to_string(), "7".to_string())];
        assert_eq!(with_properties("text\n", &id), "---\nid: 7\n---\ntext\n");
        let merged = with_properties("---\ntags: [a]\n---\nbody\n", &id);
        assert!(
            merged.contains("tags: [a]") && merged.contains("id: 7"),
            "{merged}"
        );
        let kept = with_properties("---\nid: 3\n---\n", &id);
        assert!(kept.contains("id: 3") && !kept.contains("id: 7"), "{kept}");
    }
}
