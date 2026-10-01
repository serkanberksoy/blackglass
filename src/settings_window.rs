//! The settings window (Alt+,), in two panes. On the left: a search box
//! and the pages, in groups: **Options** (Editor, Keyboard shortcuts,
//! Plugins) and **Plugin options** (each installed plugin's settings; a
//! Community plugins group can follow). On the right: the chosen page's
//! rows, each with its name, what it does and its value. Typing searches
//! every page; changes are saved at once.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::commands::Host;
use crate::keymap::Chord;
use crate::plugins::settings::{Kind, Setting};
use crate::workspace::{App, Prompt, Target, usable_shortcut};

/// A page of the settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// The editor's settings (mdedit's `config.toml`).
    Editor,
    /// The vault's notes: note IDs.
    Notes,
    /// Every command's keys.
    Shortcuts,
    /// The plugins: install, uninstall, enable, disable.
    Plugins,
    /// A plugin's settings, by its index in the plugins list.
    Plugin(usize),
}

/// The settings window's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsWindow {
    /// The page shown.
    pub page: Page,
    /// Whether the page has the focus (else the list of pages).
    pub on_page: bool,
    /// What's typed in the search box.
    pub search: String,
    /// The highlighted row of the page (an index into its rows now).
    pub row: usize,
    /// The text being typed for a text setting.
    pub editing: Option<String>,
    /// Waiting for a shortcut's new key.
    pub capturing: bool,
}

impl SettingsWindow {
    pub fn new(page: Page, on_page: bool) -> Self {
        SettingsWindow {
            page,
            on_page,
            search: String::new(),
            row: 0,
            editing: None,
            capturing: false,
        }
    }
}

/// The groups of pages.
pub const OPTIONS: &str = "Options";
pub const PLUGIN_OPTIONS: &str = "Plugin options";

/// A page in the list: its group and title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavItem {
    pub group: &'static str,
    pub page: Page,
    pub title: String,
}

/// What a row changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Setting(Setting),
    /// A command's keys, by the command's id.
    Command(String),
    /// A plugin, by its index in the plugins list.
    Plugin(usize),
}

/// A row of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The section it's under (`""` for none).
    pub section: String,
    pub label: String,
    /// What it does (one line; may be empty).
    pub help: String,
    /// Its value as written (a setting's), its keys, or a plugin's state.
    pub value: String,
    pub item: Item,
}

/// Whether every word of `search` is in one of `texts` (any case).
fn matches(search: &str, texts: &[&str]) -> bool {
    let text = texts.join(" ").to_lowercase();
    search
        .split_whitespace()
        .all(|word| text.contains(&word.to_lowercase()))
}

impl App {
    /// Opens the settings window on `page`, with the focus on the page or
    /// on the list of pages.
    pub fn open_settings_window(&mut self, page: Page, on_page: bool) {
        self.prompt = Some(Prompt::Settings(SettingsWindow::new(page, on_page)));
    }

    /// Opens plugin `i`'s settings page, if it's installed and has
    /// settings (else says why and returns false).
    pub(crate) fn open_plugin_page(&mut self, i: usize) -> bool {
        let plugins = self.plugins.borrow();
        let Some(status) = plugins.statuses().get(i).copied() else {
            return false;
        };
        let name = status.manifest.name;
        let message = if !status.installed {
            format!("Install {name} first")
        } else if plugins.settings(i).is_empty() {
            format!("{name} has no settings")
        } else {
            drop(plugins);
            self.open_settings_window(Page::Plugin(i), true);
            return true;
        };
        drop(plugins);
        self.message = message;
        false
    }

    /// The pages, in their groups: those that match `search` (by title,
    /// or by a row).
    pub fn settings_nav(&self, search: &str) -> Vec<NavItem> {
        let mut all = vec![
            NavItem {
                group: OPTIONS,
                page: Page::Editor,
                title: "Editor".into(),
            },
            NavItem {
                group: OPTIONS,
                page: Page::Notes,
                title: "Notes".into(),
            },
            NavItem {
                group: OPTIONS,
                page: Page::Shortcuts,
                title: "Keyboard shortcuts".into(),
            },
            NavItem {
                group: OPTIONS,
                page: Page::Plugins,
                title: "Plugins".into(),
            },
        ];
        let plugins = self.plugins.borrow();
        for (i, status) in plugins.statuses().iter().enumerate() {
            if status.installed && !plugins.settings(i).is_empty() {
                all.push(NavItem {
                    group: PLUGIN_OPTIONS,
                    page: Page::Plugin(i),
                    title: status.manifest.name.to_string(),
                });
            }
        }
        drop(plugins);
        all.retain(|n| search.trim().is_empty() || !self.settings_rows(n.page, search).is_empty());
        all
    }

    /// A page's title.
    pub fn settings_title(&self, page: Page) -> String {
        match page {
            Page::Editor => "Editor".into(),
            Page::Notes => "Notes".into(),
            Page::Shortcuts => "Keyboard shortcuts".into(),
            Page::Plugins => "Plugins".into(),
            Page::Plugin(i) => self
                .plugins
                .borrow()
                .manifest(i)
                .map_or_else(String::new, |m| m.name.to_string()),
        }
    }

    /// A page's rows that match `search` (all of them if its title does).
    pub fn settings_rows(&self, page: Page, search: &str) -> Vec<Row> {
        let settings = |target: Target| {
            let (_, schema, values) = self.settings_of(target);
            schema
                .into_iter()
                .map(|s| Row {
                    section: s.section.clone(),
                    label: s.label.clone(),
                    help: s.help.clone(),
                    value: values.value(&s),
                    item: Item::Setting(s),
                })
                .collect::<Vec<_>>()
        };
        let rows = match page {
            Page::Editor => settings(Target::Editor),
            Page::Notes => settings(Target::Notes),
            Page::Plugin(i) => settings(Target::Plugin(i)),
            Page::Shortcuts => self
                .all_commands()
                .into_iter()
                .map(|c| Row {
                    section: String::new(),
                    label: c.name,
                    help: String::new(),
                    value: c.key,
                    item: Item::Command(c.id),
                })
                .collect(),
            Page::Plugins => self
                .plugins
                .borrow()
                .statuses()
                .iter()
                .enumerate()
                .map(|(i, st)| Row {
                    section: String::new(),
                    label: format!("{}  {}", st.manifest.name, st.manifest.version),
                    help: st.manifest.description.to_string(),
                    value: match (st.installed, st.enabled) {
                        (true, true) => "installed · enabled",
                        (true, false) => "installed · disabled",
                        (false, _) => "not installed",
                    }
                    .into(),
                    item: Item::Plugin(i),
                })
                .collect(),
        };
        if search.trim().is_empty() || matches(search, &[&self.settings_title(page)]) {
            return rows;
        }
        rows.into_iter()
            .filter(|r| matches(search, &[&r.label, &r.help, &r.section]))
            .collect()
    }

    /// After the search changed: the page stays if it still matches, else
    /// the first that does.
    pub(crate) fn settings_search_changed(&self, w: &mut SettingsWindow) {
        let nav = self.settings_nav(&w.search);
        if !nav.iter().any(|n| n.page == w.page)
            && let Some(first) = nav.first()
        {
            w.page = first.page;
        }
        w.row = 0;
    }

    /// Keys in the settings window. In the list of pages: ↑/↓ choose a
    /// page, Enter (→, Tab) goes to it, typing searches, Esc closes. On a
    /// page: ↑/↓ choose a row, Enter or Space changes it (see the hint),
    /// ←/→ choose a value, Esc (←) goes back to the pages, typing
    /// searches.
    pub(crate) fn settings_window_key(&mut self, mut w: SettingsWindow, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let rows = self.settings_rows(w.page, &w.search);
        let row = rows.get(w.row).cloned();
        if w.capturing {
            let chord = Chord::of(&key);
            if key.code == KeyCode::Esc {
                w.capturing = false;
            } else if !usable_shortcut(&chord) {
                self.message = format!("{chord}: a shortcut needs Ctrl or Alt, or is an F key");
            } else if let Some(Row {
                item: Item::Command(id),
                label,
                ..
            }) = &row
            {
                self.bind(id, label, Some(vec![chord]), &self.all_commands());
                w.capturing = false;
            }
        } else if let Some(input) = &mut w.editing {
            match key.code {
                KeyCode::Esc => w.editing = None,
                KeyCode::Enter => {
                    let value = input.clone();
                    w.editing = None;
                    if let Some(Row {
                        item: Item::Setting(setting),
                        ..
                    }) = &row
                    {
                        self.save_setting(w.page, setting, &value);
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char('u') if ctrl => input.clear(),
                KeyCode::Char(c) if !ctrl => input.push(c),
                _ => {}
            }
        } else if !w.on_page {
            match key.code {
                KeyCode::Esc => return,
                KeyCode::Up | KeyCode::Down => {
                    let nav = self.settings_nav(&w.search);
                    let at = nav.iter().position(|n| n.page == w.page).unwrap_or(0);
                    let next = if key.code == KeyCode::Up {
                        at.saturating_sub(1)
                    } else {
                        (at + 1).min(nav.len().saturating_sub(1))
                    };
                    if let Some(n) = nav.get(next) {
                        w.page = n.page;
                        w.row = 0;
                    }
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Tab if !rows.is_empty() => {
                    w.on_page = true;
                }
                KeyCode::Backspace => {
                    w.search.pop();
                    self.settings_search_changed(&mut w);
                }
                KeyCode::Char(c) if !ctrl => {
                    w.search.push(c);
                    self.settings_search_changed(&mut w);
                }
                _ => {}
            }
        } else {
            let item = row.as_ref().map(|r| &r.item);
            let value = row.as_ref().map_or("", |r| r.value.as_str());
            match (key.code, item) {
                (KeyCode::Esc, _) => w.on_page = false,
                (KeyCode::Up, _) => w.row = w.row.saturating_sub(1),
                (KeyCode::Down, _) => w.row = (w.row + 1).min(rows.len().saturating_sub(1)),
                (KeyCode::Left | KeyCode::Right, Some(Item::Setting(setting)))
                    if matches!(setting.kind, Kind::Choice(_)) =>
                {
                    let next = step(setting, value, key.code == KeyCode::Right);
                    self.save_setting(w.page, setting, &next);
                }
                (KeyCode::Right, Some(&Item::Plugin(i))) => {
                    self.prompt = Some(Prompt::Settings(w.clone()));
                    self.open_plugin_page(i);
                    return;
                }
                (KeyCode::Left, _) => w.on_page = false,
                (KeyCode::Enter | KeyCode::Char(' '), Some(Item::Setting(setting))) => {
                    match &setting.kind {
                        Kind::Text => w.editing = Some(value.to_string()),
                        Kind::Toggle => {
                            let flipped = (value != "true").to_string();
                            self.save_setting(w.page, setting, &flipped);
                        }
                        Kind::Choice(_) => {
                            let next = step(setting, value, true);
                            self.save_setting(w.page, setting, &next);
                        }
                        Kind::Action("choose-theme") => {
                            self.run_host(Host::ChooseTheme);
                            return;
                        }
                        Kind::Action(command) => {
                            // The plugin's command, as from the palette.
                            if let Page::Plugin(i) = w.page {
                                let id = self.plugins.borrow().manifest(i).map(|m| m.id);
                                if let Some(id) = id {
                                    self.call_plugin(id, command, None);
                                }
                            }
                            return;
                        }
                    }
                }
                (KeyCode::Enter, Some(Item::Command(_))) => w.capturing = true,
                (KeyCode::Delete, Some(Item::Command(id))) => {
                    let label = row.as_ref().map_or("", |r| r.label.as_str());
                    self.bind(id, label, Some(Vec::new()), &self.all_commands());
                }
                (KeyCode::Char('d'), Some(Item::Command(id))) if ctrl => {
                    let label = row.as_ref().map_or("", |r| r.label.as_str());
                    self.bind(id, label, None, &self.all_commands());
                }
                (KeyCode::Enter, Some(&Item::Plugin(i))) => {
                    let result = self.plugins.borrow_mut().toggle_installed(i, &self.vault);
                    self.plugin_changed(i, result);
                }
                (KeyCode::Char(' '), Some(&Item::Plugin(i))) => {
                    let result = self.plugins.borrow_mut().toggle_enabled(i, &self.vault);
                    self.plugin_changed(i, result);
                }
                (KeyCode::Backspace, _) => {
                    w.search.pop();
                    self.settings_search_changed(&mut w);
                }
                (KeyCode::Char(c), _) if !ctrl && c != ' ' => {
                    // Typing searches, from the search box.
                    w.on_page = false;
                    w.search.push(c);
                    self.settings_search_changed(&mut w);
                }
                _ => {}
            }
        }
        let count = self.settings_rows(w.page, &w.search).len();
        w.row = w.row.min(count.saturating_sub(1));
        self.prompt = Some(Prompt::Settings(w));
    }

    /// Saves a setting of a page; a problem goes in the status bar.
    fn save_setting(&mut self, page: Page, setting: &Setting, value: &str) {
        let target = match page {
            Page::Plugin(i) => Target::Plugin(i),
            Page::Notes => Target::Notes,
            _ => Target::Editor,
        };
        if let Err(e) = self.set_setting(target, setting, value) {
            self.message = e;
        }
    }
}

/// A choice setting's next (or previous) value after `current`.
fn step(setting: &Setting, current: &str, forward: bool) -> String {
    let Kind::Choice(options) = &setting.kind else {
        return current.to_string();
    };
    if options.is_empty() {
        return current.to_string();
    }
    let at = options.iter().position(|o| o == current).unwrap_or(0);
    let n = options.len();
    let next = if forward {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    };
    options[next].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_words_match_in_any_order_and_case() {
        assert!(matches("folder temp", &["Templates folder", ""]));
        assert!(matches("", &["anything"]));
        assert!(!matches("folder x", &["Templates folder"]));
    }
}
