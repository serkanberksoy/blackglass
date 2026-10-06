//! The Emoji Shortcodes plugin (W-129), after the community plugin of
//! that name: typing `:` and a shortcode (`:jo`) suggests emoji (the ones
//! used lately first), and choosing one puts it in (or its shortcode,
//! `:joy: `, when it isn't to be replaced); shortcodes in the text show as
//! their emoji in the live preview. The emoji and their shortcodes
//! (GitHub's, as the original's) are mdedit's ([`mdedit::emoji::all`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use super::settings::{Kind, Setting, Values};
use super::{Context, Effect, Manifest, Plugin, PluginCommand, Suggestions};
use crate::vault::Vault;

const ID: &str = "emoji-shortcodes";

/// The most suggestions shown.
const MOST: usize = 10;

/// Every shortcode and its emoji, in the emoji's order.
static SHORTCODES: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    mdedit::emoji::all()
        .flat_map(|e| e.shortcodes().map(move |code| (code, e.as_str())))
        .collect()
});

/// A shortcode's emoji.
static BY_CODE: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| SHORTCODES.iter().copied().collect());

/// The emoji for `code` (`joy`), if it's a shortcode.
pub fn emoji(code: &str) -> Option<&'static str> {
    BY_CODE.get(code).copied()
}

pub struct EmojiShortcodes {
    /// Put the emoji in (else the shortcode).
    replace: bool,
    suggest: bool,
    /// The emoji used lately first.
    history_first: bool,
    limit: usize,
    /// Shortcodes used, the latest first.
    history: Vec<String>,
    /// Where the history is kept.
    file: Option<PathBuf>,
}

impl EmojiShortcodes {
    pub fn new() -> Self {
        EmojiShortcodes {
            replace: true,
            suggest: true,
            history_first: true,
            limit: 100,
            history: Vec::new(),
            file: None,
        }
    }

    fn history_file(vault: &Vault) -> PathBuf {
        super::settings_file(vault, ID).with_file_name("history")
    }

    fn save_history(&self) -> Effect {
        let Some(file) = &self.file else {
            return Effect::None;
        };
        let text: String = self.history.iter().map(|c| format!("{c}\n")).collect();
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| mdedit::files::write_atomic(file, &text));
        match written {
            Ok(()) => Effect::None,
            Err(e) => Effect::Message(format!("Emoji Shortcodes: cannot save the history: {e}")),
        }
    }

    /// The shortcodes matching `q`: used lately first (if so set), then
    /// the exact one, those starting with it, those with it inside.
    fn matching(&self, q: &str) -> Vec<(&'static str, &'static str)> {
        let recent = |code: &str| {
            self.history_first
                .then(|| self.history.iter().position(|h| h == code))
                .flatten()
                .unwrap_or(usize::MAX)
        };
        let mut found: Vec<(usize, u8, usize, (&str, &str))> = SHORTCODES
            .iter()
            .enumerate()
            .filter(|(_, (code, _))| code.contains(q))
            .map(|(i, &(code, e))| {
                let kind = if code == q {
                    0
                } else if code.starts_with(q) {
                    1
                } else {
                    2
                };
                (recent(code), kind, i, (code, e))
            })
            .collect();
        found.sort_unstable_by_key(|&(r, k, i, _)| (r, k, i));
        found.into_iter().take(MOST).map(|(.., c)| c).collect()
    }
}

impl Default for EmojiShortcodes {
    fn default() -> Self {
        Self::new()
    }
}

/// The shortcode being typed at char `col` of `line`: where its `:` is
/// and what's after it. Not after a letter or digit (`10:30`), not in
/// code.
fn typed(line: &str, col: usize) -> Option<(usize, String)> {
    let before: String = line.chars().take(col).collect();
    let word_start = before.rfind(char::is_whitespace).map_or(0, |i| i + 1);
    let word = &before[word_start..];
    let colon = word.find(':')?;
    let ahead = &before[..word_start + colon];
    if ahead
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '`')
        || ahead.matches('`').count() % 2 == 1
    {
        return None;
    }
    let q = word[colon + 1..].trim_end_matches(':').to_lowercase();
    if q.is_empty() || q.contains(':') {
        return None;
    }
    Some((ahead.chars().count(), q))
}

impl Plugin for EmojiShortcodes {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Emoji Shortcodes",
            version: "0.1.0",
            author: "blackglass, after phibr0's Emoji Shortcodes",
            description: "Type : and a shortcode (:joy) for emoji suggestions; the chosen one goes \
                          in (or its shortcode). Shortcodes in the text show as emoji.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        let schema = self.settings();
        let get = |key: &str| {
            values.value(
                schema
                    .iter()
                    .find(|s| s.key == key)
                    .expect("a declared setting"),
            )
        };
        self.replace = get("immediate_replace") != "false";
        self.suggest = get("suggester") != "false";
        self.history_first = get("history_priority") != "false";
        self.limit = get("history_limit").trim().parse().unwrap_or(100);
        let file = Self::history_file(vault);
        self.history = std::fs::read_to_string(&file)
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();
        self.file = Some(file);
    }

    fn on_note_changed(&mut self, _path: &Path, _vault: &Vault) {}

    fn settings(&self) -> Vec<Setting> {
        let toggle = |key: &str, label: &str, help: &str| {
            Setting::new("", key, label, help, Kind::Toggle, "true")
        };
        vec![
            toggle(
                "immediate_replace",
                "Replace with the emoji",
                "A chosen shortcode becomes its emoji; off: the shortcode stays (shown as the emoji)",
            ),
            toggle(
                "suggester",
                "Suggest emoji",
                "Typing : and a letter suggests emoji",
            ),
            toggle(
                "history_priority",
                "Recently used first",
                "Emoji you used lately come first in the suggestions",
            ),
            Setting::new(
                "",
                "history_limit",
                "History size",
                "How many recently used emoji are remembered",
                Kind::Text,
                "100",
            ),
            Setting::new(
                "",
                "clear",
                "Clear the history",
                "Forget the recently used emoji",
                Kind::Action("clear-history"),
                "",
            ),
        ]
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![PluginCommand::new("clear-history", "Clear emoji history")]
    }

    fn run(&mut self, id: &str, _ctx: &Context) -> Effect {
        match id {
            "clear-history" => {
                self.history.clear();
                match self.save_history() {
                    Effect::None => Effect::Message("Emoji Shortcodes: history cleared".into()),
                    other => other,
                }
            }
            _ => Effect::None,
        }
    }

    fn suggestions(&self, line: &str, col: usize, _vault: &Vault) -> Option<Suggestions> {
        if !self.suggest {
            return None;
        }
        let (start, q) = typed(line, col)?;
        let mut out = Suggestions {
            start,
            ..Suggestions::default()
        };
        for (i, (code, e)) in self.matching(&q).into_iter().enumerate() {
            let text = if self.replace {
                e.to_string()
            } else {
                format!(":{code}: ")
            };
            out.items.push((format!("{e}  {code}"), text));
            if self.history_first {
                out.effects
                    .push((i, Effect::RowAction(format!("plugin:{ID}:used:{code}"))));
            }
        }
        (!out.items.is_empty()).then_some(out)
    }

    fn row_action(&mut self, payload: &str, _ctx: &Context) -> Effect {
        let Some(code) = payload.strip_prefix("used:") else {
            return Effect::None;
        };
        self.history.retain(|c| c != code);
        self.history.insert(0, code.to_string());
        self.history.truncate(self.limit);
        self.save_history()
    }

    fn rendered_spans(&self) -> Vec<(&'static str, &'static str)> {
        vec![(":", ":")]
    }

    fn render_span(&self, open: &str, inner: &str) -> Option<String> {
        (open == ":")
            .then(|| emoji(inner))
            .flatten()
            .map(String::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcodes_typed_and_found() {
        assert_eq!(typed("so :jo", 6), Some((3, "jo".into())));
        assert_eq!(typed(":joy:", 5), Some((0, "joy".into())), "closed");
        for (line, col) in [
            ("10:30", 5),
            ("a:b", 3),
            ("`:jo", 4),
            (": x", 3),
            ("http://x", 8),
        ] {
            assert_eq!(typed(line, col), None, "{line}");
        }
        assert_eq!(emoji("joy"), Some("😂"));
        assert_eq!(emoji("+1"), Some("👍"));
        assert_eq!(emoji("nope"), None);
        let mut p = EmojiShortcodes::new();
        let codes = |p: &EmojiShortcodes, q: &str| -> Vec<&str> {
            p.matching(q).into_iter().map(|(c, _)| c).collect()
        };
        assert_eq!(codes(&p, "joy")[0], "joy", "exact first");
        assert!(codes(&p, "smil").iter().all(|c| c.contains("smil")));
        p.history = vec!["cat".into()];
        assert_eq!(codes(&p, "a")[0], "cat", "recently used first");
        p.history_first = false;
        assert_ne!(codes(&p, "a")[0], "cat");
    }
}
