//! Keyboard shortcuts: which keys run which command. Every command has
//! default keys; the user's changes are saved in `keys.toml` in the
//! config folder (only what differs from the defaults):
//!
//! ```toml
//! # command id = "keys" (several with " / "; "" for none)
//! new-note = "Alt+N"
//! quit = "Ctrl+Q / F10"
//! ```
//!
//! A key moved to another command leaves its old one, so a key never does
//! two things.

use std::fmt;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A key with its modifiers, normalized: letters lowercase (Shift is a
/// modifier), Shift+Tab as Tab with Shift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// The modifiers a chord can have, in the order they're written.
const MODIFIERS: [(KeyModifiers, &str); 3] = [
    (KeyModifiers::CONTROL, "Ctrl"),
    (KeyModifiers::ALT, "Alt"),
    (KeyModifiers::SHIFT, "Shift"),
];

/// Named keys, as written.
const NAMES: [(KeyCode, &str); 16] = [
    (KeyCode::Enter, "Enter"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::Esc, "Esc"),
    (KeyCode::Backspace, "Backspace"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::Left, "Left"),
    (KeyCode::Right, "Right"),
    (KeyCode::Up, "Up"),
    (KeyCode::Down, "Down"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "PgUp"),
    (KeyCode::PageDown, "PgDn"),
    (KeyCode::Char(' '), "Space"),
    (KeyCode::Null, "Null"),
];

impl Chord {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        let mut chord = Chord { code, modifiers };
        match code {
            KeyCode::BackTab => {
                chord.code = KeyCode::Tab;
                chord.modifiers |= KeyModifiers::SHIFT;
            }
            KeyCode::Char(c) if c.is_ascii_uppercase() => {
                chord.code = KeyCode::Char(c.to_ascii_lowercase());
                chord.modifiers |= KeyModifiers::SHIFT;
            }
            _ => {}
        }
        chord.modifiers &= KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT;
        chord
    }

    /// The chord of a key press.
    pub fn of(key: &KeyEvent) -> Self {
        Chord::new(key.code, key.modifiers)
    }

    /// A key event for the chord (to send an editor command's key).
    pub fn event(&self) -> KeyEvent {
        KeyEvent::new(self.code, self.modifiers)
    }

    /// Reads `Ctrl+Shift+N`, `Alt+,`, `F5`, `Ctrl+Enter` (case doesn't
    /// matter for modifiers and names).
    pub fn parse(text: &str) -> Result<Chord, String> {
        let text = text.trim();
        let mut modifiers = KeyModifiers::NONE;
        let mut rest = text;
        'outer: loop {
            for (m, name) in MODIFIERS {
                let prefix = format!("{name}+");
                if rest.len() > prefix.len() && rest[..prefix.len()].eq_ignore_ascii_case(&prefix) {
                    modifiers |= m;
                    rest = &rest[prefix.len()..];
                    continue 'outer;
                }
            }
            break;
        }
        let code =
            if let Some(&(code, _)) = NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(rest)) {
                code
            } else if let Some(n) = rest
                .strip_prefix(['F', 'f'])
                .and_then(|n| n.parse::<u8>().ok())
                .filter(|n| (1..=12).contains(n))
            {
                KeyCode::F(n)
            } else {
                let mut chars = rest.chars();
                match (chars.next(), chars.next()) {
                    // A written letter is just the letter: Shift is written out.
                    (Some(c), None) => KeyCode::Char(c.to_ascii_lowercase()),
                    _ => return Err(format!("\"{text}\" isn't a key")),
                }
            };
        Ok(Chord::new(code, modifiers))
    }

    /// Several chords written with " / ".
    pub fn parse_list(text: &str) -> Result<Vec<Chord>, String> {
        text.split(" / ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(Chord::parse)
            .collect()
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (m, name) in MODIFIERS {
            if self.modifiers.contains(m) {
                write!(f, "{name}+")?;
            }
        }
        match self.code {
            KeyCode::F(n) => write!(f, "F{n}"),
            KeyCode::Char(c) if c != ' ' => write!(f, "{}", c.to_ascii_uppercase()),
            code => {
                let name = NAMES
                    .iter()
                    .find(|(k, _)| *k == code)
                    .map_or("?", |(_, n)| n);
                f.write_str(name)
            }
        }
    }
}

/// Chords as written in a list.
pub fn describe(chords: &[Chord]) -> String {
    chords
        .iter()
        .map(Chord::to_string)
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Every command's keys: the defaults, with the user's changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keymap {
    /// (command id, default keys, current keys), in command order.
    entries: Vec<(String, Vec<Chord>, Vec<Chord>)>,
}

impl Keymap {
    /// The defaults for `commands`: (id, default keys).
    pub fn new(commands: impl IntoIterator<Item = (String, Vec<Chord>)>) -> Self {
        Keymap {
            entries: commands
                .into_iter()
                .map(|(id, keys)| (id, keys.clone(), keys))
                .collect(),
        }
    }

    /// Adds commands (e.g. a plugin's) that aren't known yet, with no keys.
    pub fn add(&mut self, id: &str) {
        if !self.entries.iter().any(|(i, _, _)| i == id) {
            self.entries.push((id.to_string(), Vec::new(), Vec::new()));
        }
    }

    /// Adds a command that comes later (a plugin's) with its default keys:
    /// those no other command has now (the others aren't its defaults, so
    /// they're never saved as a change). If it's known already (the user's
    /// `keys.toml` named it), only its defaults are set.
    pub fn add_defaults(&mut self, id: &str, defaults: Vec<Chord>) {
        if let Some(entry) = self.entries.iter_mut().find(|(i, _, _)| i == id) {
            entry.1 = defaults;
            return;
        }
        let free: Vec<Chord> = defaults
            .into_iter()
            .filter(|k| self.command_for(k).is_none())
            .collect();
        self.entries.push((id.to_string(), free.clone(), free));
    }

    /// Applies the user's `keys.toml`; returns a warning per line it can't
    /// use (unknown command ids are kept, for commands of plugins not
    /// loaded yet).
    pub fn apply(&mut self, text: &str) -> Vec<String> {
        let mut warnings = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((id, value)) = line.split_once('=') else {
                warnings.push(format!("keys.toml line {}: expected id = \"keys\"", n + 1));
                continue;
            };
            let id = id.trim().trim_matches('"');
            let value = value.trim().trim_matches('"');
            match Chord::parse_list(value) {
                Ok(chords) => {
                    self.add(id);
                    self.set(id, chords);
                }
                Err(e) => warnings.push(format!("keys.toml line {}: {e}", n + 1)),
            }
        }
        warnings
    }

    /// The user's changes, for `keys.toml`.
    pub fn to_text(&self) -> String {
        let mut out = String::from(
            "# blackglass keyboard shortcuts: only the changes from the defaults.\n# command id = \"keys\" (several with \" / \", \"\" for none)\n",
        );
        for (id, default, keys) in &self.entries {
            if default != keys {
                out.push_str(&format!("{id} = \"{}\"\n", describe(keys)));
            }
        }
        out
    }

    /// A command's keys.
    pub fn keys(&self, id: &str) -> &[Chord] {
        self.entries
            .iter()
            .find(|(i, _, _)| i == id)
            .map_or(&[], |(_, _, keys)| keys.as_slice())
    }

    /// The command a key runs.
    pub fn command_for(&self, chord: &Chord) -> Option<&str> {
        self.entries
            .iter()
            .find(|(_, _, keys)| keys.contains(chord))
            .map(|(id, _, _)| id.as_str())
    }

    /// Whether `chord` is some command's default key that the user took
    /// away and gave to nothing: then it does nothing (not the editor's
    /// own meaning of it either).
    pub fn is_freed(&self, chord: &Chord) -> bool {
        self.command_for(chord).is_none()
            && self
                .entries
                .iter()
                .any(|(_, default, _)| default.contains(chord))
    }

    /// Sets a command's keys; each is taken from any other command that had
    /// it. Returns those commands' ids.
    pub fn set(&mut self, id: &str, chords: Vec<Chord>) -> Vec<String> {
        let mut moved = Vec::new();
        for (other, _, keys) in &mut self.entries {
            if other != id && keys.iter().any(|k| chords.contains(k)) {
                keys.retain(|k| !chords.contains(k));
                moved.push(other.clone());
            }
        }
        if let Some((_, _, keys)) = self.entries.iter_mut().find(|(i, _, _)| i == id) {
            *keys = chords;
        }
        moved
    }

    /// Back to a command's default keys (taken from others if needed).
    pub fn reset(&mut self, id: &str) -> Vec<String> {
        let default = self
            .entries
            .iter()
            .find(|(i, _, _)| i == id)
            .map(|(_, d, _)| d.clone())
            .unwrap_or_default();
        self.set(id, default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> Chord {
        Chord::parse(text).unwrap()
    }

    #[test]
    fn chords_read_write_and_normalize() {
        for text in [
            "Ctrl+N",
            "Ctrl+Shift+N",
            "Alt+,",
            "F5",
            "Ctrl+Alt+Left",
            "Ctrl+Enter",
            "Shift+Tab",
            "Ctrl+PgDn",
        ] {
            assert_eq!(chord(text).to_string(), text);
        }
        assert_eq!(chord("ctrl+shift+n"), chord("Ctrl+Shift+N"));
        let typed = KeyEvent::new(
            KeyCode::Char('N'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(
            Chord::of(&typed),
            chord("Ctrl+Shift+N"),
            "a capital is Shift"
        );
        let backtab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(Chord::of(&backtab), chord("Shift+Tab"));
        assert_eq!(
            Chord::parse_list("F1 / Ctrl+H").unwrap(),
            [chord("F1"), chord("Ctrl+H")]
        );
        assert!(Chord::parse("Ctrl+Nope").is_err());
        assert!(Chord::parse("F13").is_err());
    }

    fn keymap() -> Keymap {
        Keymap::new([
            ("new-note".to_string(), vec![chord("Ctrl+N")]),
            (
                "search".to_string(),
                vec![chord("Ctrl+G"), chord("Ctrl+Shift+F")],
            ),
            ("quit".to_string(), vec![chord("Ctrl+Q")]),
        ])
    }

    #[test]
    fn keys_move_between_commands_and_defaults_come_back() {
        let mut k = keymap();
        assert_eq!(k.command_for(&chord("Ctrl+Shift+F")), Some("search"));
        let moved = k.set("new-note", vec![chord("Ctrl+G")]);
        assert_eq!(moved, ["search"]);
        assert_eq!(k.keys("search"), [chord("Ctrl+Shift+F")]);
        assert_eq!(k.command_for(&chord("Ctrl+G")), Some("new-note"));
        assert!(
            k.is_freed(&chord("Ctrl+N")),
            "new-note's default, now nobody's"
        );
        assert!(!k.is_freed(&chord("Ctrl+X")), "never a default");
        k.reset("search");
        assert_eq!(k.keys("search"), [chord("Ctrl+G"), chord("Ctrl+Shift+F")]);
        assert_eq!(k.keys("new-note"), [], "it gave Ctrl+G back");
    }

    #[test]
    fn later_commands_get_the_default_keys_that_are_free() {
        let mut k = keymap();
        k.add_defaults("plugin:p:today", vec![chord("Alt+D"), chord("Ctrl+Q")]);
        assert_eq!(
            k.keys("plugin:p:today"),
            [chord("Alt+D")],
            "Ctrl+Q is quit's"
        );
        assert!(!k.to_text().contains("plugin"), "defaults aren't saved");
        let mut user = keymap();
        user.apply("plugin:p:today = \"F9\"\n");
        user.add_defaults("plugin:p:today", vec![chord("Alt+D")]);
        assert_eq!(
            user.keys("plugin:p:today"),
            [chord("F9")],
            "the user's keys stay"
        );
        assert!(user.is_freed(&chord("Alt+D")));
    }

    #[test]
    fn only_changes_are_saved_and_read_back() {
        let mut k = keymap();
        k.set("new-note", vec![chord("Alt+N")]);
        k.set("quit", Vec::new());
        let text = k.to_text();
        assert!(text.contains("new-note = \"Alt+N\"\n"), "{text}");
        assert!(text.contains("quit = \"\"\n"), "{text}");
        assert!(!text.contains("search"), "unchanged: {text}");
        let mut again = keymap();
        assert!(again.apply(&text).is_empty());
        assert_eq!(again, k);
        let warnings = again.apply("new-note = \"Ctrl+Nope\"\nplugin:x = \"F9\"\nbad line");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert_eq!(again.keys("plugin:x"), [chord("F9")], "kept for later");
    }
}
