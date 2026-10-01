//! Plugin settings: each plugin declares its settings ([`Setting`]) and
//! blackglass shows them in one settings screen, saving them to the
//! plugin's `settings.toml` (see [`super::settings_file`]):
//!
//! ```toml
//! templates_folder = "Templates"
//!
//! [daily]
//! enabled = true
//! folder = "Journal"
//! ```

/// What a setting holds, and how the screen edits it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Typed text (Enter edits it).
    Text,
    /// On or off (Enter switches it).
    Toggle,
    /// One of these values (Enter, ← / → choose).
    Choice(Vec<String>),
    /// Not a value: Enter runs this command of the plugin's (e.g. to add
    /// an entry to a list of settings).
    Action(&'static str),
}

/// A setting a plugin declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// The `[section]` it's in; `""` for none.
    pub section: String,
    pub key: String,
    pub label: String,
    /// One line saying what it does.
    pub help: String,
    pub kind: Kind,
    pub default: String,
}

impl Setting {
    pub fn new(
        section: &str,
        key: &str,
        label: &str,
        help: &str,
        kind: Kind,
        default: &str,
    ) -> Self {
        Setting {
            section: section.into(),
            key: key.into(),
            label: label.into(),
            help: help.into(),
            kind,
            default: default.into(),
        }
    }
}

/// The values in a settings file, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Values {
    /// (section, key, value) in file order.
    entries: Vec<(String, String, String)>,
}

impl Values {
    /// Reads `key = "value"` lines, under `[section]` headers.
    pub fn parse(text: &str) -> Values {
        let mut entries = Vec::new();
        let mut section = String::new();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_string();
                continue;
            }
            if let Some((key, value)) = split_key(line) {
                let value = value.trim();
                let value = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .map_or_else(|| value.to_string(), unescape);
                entries.push((section.clone(), key, value));
            }
        }
        Values { entries }
    }

    /// The (key, value)s in `section`, in file order.
    pub fn section<'a>(&'a self, section: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.entries
            .iter()
            .filter(move |(s, _, _)| s == section)
            .map(|(_, k, v)| (k.as_str(), v.as_str()))
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(s, k, _)| s == section && k == key)
            .map(|(_, _, v)| v.as_str())
    }

    /// A setting's value, or its default.
    pub fn value(&self, setting: &Setting) -> String {
        self.get(&setting.section, &setting.key)
            .unwrap_or(&setting.default)
            .to_string()
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        match self
            .entries
            .iter_mut()
            .find(|(s, k, _)| s == section && k == key)
        {
            Some(entry) => entry.2 = value.to_string(),
            None => self
                .entries
                .push((section.into(), key.into(), value.into())),
        }
    }

    /// The file's text: every declared setting (in `schema` order, grouped
    /// by section), then anything else the file had.
    pub fn to_text(&self, schema: &[Setting]) -> String {
        let mut sections: Vec<&str> = Vec::new();
        let declared = |s: &str, k: &str| schema.iter().any(|x| x.section == s && x.key == k);
        for s in schema
            .iter()
            .map(|x| x.section.as_str())
            .chain(self.entries.iter().map(|e| e.0.as_str()))
        {
            if !sections.contains(&s) {
                sections.push(s);
            }
        }
        // Keys without a section must come before the first header.
        sections.sort_by_key(|s| !s.is_empty());
        let mut out = String::new();
        for section in sections {
            let mut lines = Vec::new();
            for setting in schema
                .iter()
                .filter(|x| x.section == section && !matches!(x.kind, Kind::Action(_)))
            {
                let value = self.value(setting);
                lines.push(format_line(&setting.key, &value, &setting.kind));
            }
            for (_, key, value) in self
                .entries
                .iter()
                .filter(|(s, k, _)| s == section && !declared(s, k))
            {
                lines.push(format_line(key, value, &Kind::Text));
            }
            if lines.is_empty() {
                continue;
            }
            if !section.is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&format!("[{section}]\n"));
            }
            for line in lines {
                out.push_str(&line);
                out.push('\n');
            }
        }
        out
    }
}

/// `key = "value"`, or `key = true` for an on / off setting. A key that
/// isn't a plain word (a folder: `"Books/Sci-fi"`) is quoted.
fn format_line(key: &str, value: &str, kind: &Kind) -> String {
    let quote = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
    let bare = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let key = if bare { key.to_string() } else { quote(key) };
    match kind {
        Kind::Toggle => format!("{key} = {}", value == "true"),
        _ => format!("{key} = {}", quote(value)),
    }
}

/// A `key = value` line's key (unquoted) and the rest after the `=`.
fn split_key(line: &str) -> Option<(String, &str)> {
    if let Some(rest) = line.strip_prefix('"') {
        let mut key = String::new();
        let mut chars = rest.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => key.extend(chars.next().map(|(_, c)| c)),
                '"' => {
                    let value = rest[i + 1..].trim_start().strip_prefix('=')?;
                    return Some((key, value));
                }
                c => key.push(c),
            }
        }
        return None;
    }
    let (key, value) = line.split_once('=')?;
    Some((key.trim().to_string(), value))
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Vec<Setting> {
        vec![
            Setting::new("", "folder", "Folder", "", Kind::Text, "Templates"),
            Setting::new("daily", "enabled", "Enabled", "", Kind::Toggle, "true"),
            Setting::new("daily", "format", "Format", "", Kind::Text, "YYYY-MM-DD"),
        ]
    }

    #[test]
    fn values_read_set_and_write_back() {
        let mut v = Values::parse(
            "# comment\nfolder = \"T \\\"x\\\"\"\n[daily]\nenabled = false\nkept = \"yes\"\n[other]\na = 1",
        );
        assert_eq!(v.get("", "folder"), Some("T \"x\""));
        assert_eq!(v.get("daily", "enabled"), Some("false"));
        assert_eq!(v.value(&schema()[2]), "YYYY-MM-DD", "the default");
        v.set("daily", "format", "YYYY");
        v.set("", "folder", "Tpl");
        assert_eq!(
            v.to_text(&schema()),
            "folder = \"Tpl\"\n\n[daily]\nenabled = false\nformat = \"YYYY\"\nkept = \"yes\"\n\n[other]\na = \"1\"\n"
        );
        assert_eq!(
            Values::parse(&v.to_text(&schema())).get("daily", "format"),
            Some("YYYY"),
            "round trip"
        );
    }

    #[test]
    fn keys_that_are_not_words_are_quoted() {
        let schema = vec![
            Setting::new("folders", "Books/Sci-fi", "", "", Kind::Text, ""),
            Setting::new("folders", "add", "Add", "", Kind::Action("add"), ""),
        ];
        let mut values = Values::default();
        values.set("folders", "Books/Sci-fi", "Book");
        values.set("folders", "/", "Note");
        let text = values.to_text(&schema);
        assert!(text.contains("\"Books/Sci-fi\" = \"Book\""), "{text}");
        assert!(text.contains("\"/\" = \"Note\""), "{text}");
        assert!(!text.contains("add"), "an action isn't a value: {text}");
        assert_eq!(Values::parse(&text), values);
    }
}
