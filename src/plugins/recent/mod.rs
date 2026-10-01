//! Recent Files: the notes you visited lately, newest first, in a sidebar
//! tab of its own ("Recent", after Files, Search and Tags). After the
//! Recent Files community plugin. The list is kept per vault, in
//! `.blackglass/plugins/recent-files/recent.toml`; how long it is is a
//! setting (25 by default). Settings also choose whether a note counts
//! when opened or only when changed, leave out paths and tags (regular
//! expressions), and show a note's `title` property.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use regex::Regex;

use super::settings::{Kind, Setting, Values};
use super::{Context, Effect, Manifest, Plugin, PluginCommand, SidebarRow};
use crate::vault::Vault;

/// How many notes the list keeps, unless the settings say otherwise.
pub const DEFAULT_LENGTH: usize = 25;

/// The plugin's id.
const ID: &str = "recent-files";

#[derive(Default)]
pub struct RecentFiles {
    /// The notes visited, newest first, relative to the vault.
    list: Vec<PathBuf>,
    /// How many it keeps.
    length: usize,
    /// Where the list is kept (`recent.toml` beside the settings).
    file: Option<PathBuf>,
    /// Record notes when saved, not when opened.
    on_change: bool,
    /// Paths and tags (without `#`) left out.
    omit_paths: Vec<Regex>,
    omit_tags: Vec<Regex>,
    /// Show the `title` property instead of the name.
    use_title: bool,
}

impl RecentFiles {
    pub fn new() -> Self {
        RecentFiles {
            list: Vec::new(),
            length: DEFAULT_LENGTH,
            file: None,
            on_change: false,
            omit_paths: Vec::new(),
            omit_tags: Vec::new(),
            use_title: false,
        }
    }

    /// The list's notes that are still there (the sidebar's rows), as
    /// indexes into the list.
    fn shown(&self, vault: &Vault) -> Vec<usize> {
        (0..self.list.len())
            .filter(|&i| vault.root.join(&self.list[i]).is_file())
            .collect()
    }

    /// Puts the note at `path` first (unless the settings leave it out).
    fn record(&mut self, path: &Path, vault: &Vault) {
        let Ok(rel) = path.strip_prefix(&vault.root) else {
            return;
        };
        let shown = rel.to_string_lossy().replace('\\', "/");
        if self.omit_paths.iter().any(|r| r.is_match(&shown)) {
            return;
        }
        let tags = vault.note(path).map(|n| n.tags.clone()).unwrap_or_default();
        if tags
            .iter()
            .any(|t| self.omit_tags.iter().any(|r| r.is_match(t)))
        {
            return;
        }
        if self.list.first().is_some_and(|p| p == rel) {
            return;
        }
        self.list.retain(|p| p != rel);
        self.list.insert(0, rel.to_path_buf());
        self.list.truncate(self.length);
        self.save();
    }

    /// Writes the list. If the disk refuses, the list still works for this
    /// session; it just isn't kept for the next one.
    fn save(&self) {
        let Some(file) = &self.file else {
            return;
        };
        let mut text =
            String::from("# Recent Files: the notes visited lately, newest first.\nrecent = [\n");
        for rel in &self.list {
            let rel = rel
                .to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            text.push_str(&format!("    \"{rel}\",\n"));
        }
        text.push_str("]\n");
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = mdedit::files::write_atomic(file, &text);
    }
}

/// The regular expressions in `text`, separated by `;` (bad ones are
/// skipped).
fn patterns(text: &str) -> Vec<Regex> {
    text.split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter_map(|p| Regex::new(p).ok())
        .collect()
}

/// The paths in a saved list.
fn parse_list(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(|l| l.trim().trim_end_matches(','))
        .filter_map(|l| l.strip_prefix('"')?.strip_suffix('"').map(String::from))
        .map(|l| PathBuf::from(l.replace("\\\"", "\"").replace("\\\\", "\\")))
        .collect()
}

impl Plugin for RecentFiles {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Recent Files",
            version: "0.1.0",
            author: "blackglass, after Tony Grosinger's Recent Files",
            description: "The notes you visited lately, newest first, in a Recent tab of the sidebar: \
                          Enter opens one, Delete takes it off the list. Kept per vault; 25 notes by \
                          default (a setting).",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        let settings = super::settings_file(vault, ID);
        self.file = Some(settings.with_file_name("recent.toml"));
        let text = self
            .file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .unwrap_or_default();
        self.list = parse_list(&text);
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        self.length = values
            .get("", "list_length")
            .and_then(|v| v.trim().parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_LENGTH);
        self.list.truncate(self.length);
        let get = |key: &str| values.get("", key).unwrap_or_default().to_string();
        self.on_change = get("update_on") == "change";
        self.omit_paths = patterns(&get("omit_paths"));
        self.omit_tags = patterns(&get("omit_tags"));
        self.use_title = get("use_title") == "true";
    }

    fn settings(&self) -> Vec<Setting> {
        let text =
            |key: &str, label: &str, help: &str| Setting::new("", key, label, help, Kind::Text, "");
        vec![
            Setting::new(
                "",
                "update_on",
                "Update the list when a note is",
                "opened, or changed (saved): notes only looked at don't count",
                Kind::Choice(vec!["open".into(), "change".into()]),
                "open",
            ),
            text(
                "omit_paths",
                "Leave out paths",
                "Regular expressions, separated by ; (e.g. ^Journal/)",
            ),
            text(
                "omit_tags",
                "Leave out tags",
                "Regular expressions for the note's tags, separated by ; (e.g. ^private)",
            ),
            Setting::new(
                "",
                "use_title",
                "Show titles",
                "Show a note's title property instead of its name",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "list_length",
                "List length",
                "How many notes the list keeps",
                Kind::Text,
                &DEFAULT_LENGTH.to_string(),
            ),
        ]
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("open", "Open"),
            PluginCommand::new("clear", "Clear list"),
        ]
    }

    fn run(&mut self, id: &str, _ctx: &Context) -> Effect {
        match id {
            "open" => Effect::ShowSidebarTab,
            "clear" => {
                self.list.clear();
                self.save();
                Effect::Message("Recent Files: the list is cleared".into())
            }
            _ => Effect::None,
        }
    }

    fn sidebar_tab(&self) -> Option<&'static str> {
        Some("Recent")
    }

    fn sidebar_hint(&self) -> &'static str {
        "⏎ open  l link  Delete remove"
    }

    fn sidebar_rows(&self, ctx: &Context) -> Vec<SidebarRow> {
        self.shown(ctx.vault)
            .into_iter()
            .map(|i| {
                let rel = &self.list[i];
                let title = self
                    .use_title
                    .then(|| ctx.vault.note(&ctx.vault.root.join(rel)))
                    .flatten()
                    .and_then(|n| {
                        n.properties
                            .iter()
                            .find(|(k, _)| k == "title")
                            .map(|(_, v)| v.trim_matches(['"', '\'']).to_string())
                    })
                    .filter(|t| !t.is_empty());
                SidebarRow {
                    label: title.unwrap_or_else(|| {
                        rel.file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    }),
                    detail: rel
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                }
            })
            .collect()
    }

    fn sidebar_key(&mut self, row: usize, key: KeyEvent, ctx: &Context) -> Effect {
        let Some(&i) = self.shown(ctx.vault).get(row) else {
            return Effect::None;
        };
        match key.code {
            KeyCode::Enter => Effect::Open {
                path: ctx.vault.root.join(&self.list[i]),
            },
            KeyCode::Delete => {
                self.list.remove(i);
                self.save();
                Effect::None
            }
            // A link to it at the editor's cursor (for dragging upstream).
            KeyCode::Char('l') => {
                let name = self.list[i]
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Effect::Insert {
                    text: format!("[[{name}]]"),
                    back: 0,
                }
            }
            _ => Effect::None,
        }
    }

    fn on_note_opened(&mut self, path: &Path, vault: &Vault) {
        if !self.on_change {
            self.record(path, vault);
        }
    }

    fn on_note_saved(&mut self, path: &Path, vault: &Vault) {
        if self.on_change {
            self.record(path, vault);
        }
    }

    fn on_note_moved(&mut self, from: &Path, to: &Path, vault: &Vault) {
        let (Ok(from), Ok(to)) = (from.strip_prefix(&vault.root), to.strip_prefix(&vault.root))
        else {
            return;
        };
        for rel in self.list.iter_mut().filter(|p| *p == from) {
            *rel = to.to_path_buf();
        }
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn the_list_keeps_the_newest_visits_once() {
        let dir = scratch("recent-list");
        let files: Vec<(String, String)> = (0..30)
            .map(|i| (format!("n{i}.md"), String::new()))
            .collect();
        let files: Vec<(&str, &str)> = files
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        write(&dir, &files);
        let vault = Vault::open(&dir).unwrap();
        let mut recent = RecentFiles::new();
        recent.on_load(&vault);
        for i in 0..30 {
            recent.on_note_opened(&vault.root.join(format!("n{i}.md")), &vault);
        }
        recent.on_note_opened(&vault.root.join("n10.md"), &vault);
        assert_eq!(recent.list.len(), 25, "25 by default");
        assert_eq!(
            recent.list[0],
            PathBuf::from("n10.md"),
            "again: to the top, once"
        );
        assert_eq!(recent.list[1], PathBuf::from("n29.md"));
        assert!(
            !recent.list.contains(&PathBuf::from("n4.md")),
            "the oldest dropped"
        );
        // A move keeps its place.
        recent.on_note_moved(
            &vault.root.join("n29.md"),
            &vault.root.join("sub/n29.md"),
            &vault,
        );
        assert_eq!(recent.list[1], PathBuf::from("sub/n29.md"));
    }

    #[test]
    fn settings_choose_what_is_kept_and_shown() {
        let dir = scratch("recent-settings");
        write(
            &dir,
            &[
                ("A.md", "---\ntitle: The A note\n---\n"),
                ("Journal/d.md", ""),
                ("Secret.md", "---\ntags: [private]\n---\n"),
                (
                    ".blackglass/plugins/recent-files/settings.toml",
                    "update_on = \"change\"\nomit_paths = \"^Journal/\"\nomit_tags = \"^priv\"\nuse_title = \"true\"\n",
                ),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        let mut recent = RecentFiles::new();
        recent.on_load(&vault);
        let a = vault.root.join("A.md");
        recent.on_note_opened(&a, &vault);
        assert!(recent.list.is_empty(), "only when changed");
        recent.on_note_saved(&a, &vault);
        recent.on_note_saved(&vault.root.join("Journal/d.md"), &vault);
        recent.on_note_saved(&vault.root.join("Secret.md"), &vault);
        assert_eq!(
            recent.list,
            [PathBuf::from("A.md")],
            "paths and tags left out"
        );
        let ctx = Context {
            vault: &vault,
            note: None,
            folder: &vault.root,
            name: None,
            query: "",
        };
        assert_eq!(
            recent.sidebar_rows(&ctx)[0].label,
            "The A note",
            "its title"
        );
    }
}
