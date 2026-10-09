//! The workspace: the vault, the sidebar, the open notes as tabs (one
//! mdedit [`EditorView`] each), which side has the focus, and the prompts
//! (new note, save as, unsaved changes, quick switcher, command palette,
//! plugins window, settings) and the plugins. Keys and mouse events arrive
//! here first: a key some command has in the [`Keymap`] runs it; what
//! blackglass doesn't use goes to the active editor, and its [`Outcome`]s
//! come back here.

use std::cell::RefCell;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use mdedit::config::HeadingSize;
use mdedit::shared::Shared;
use mdedit::view::{EditorView, Outcome, ViewMode};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use unicode_segmentation::UnicodeSegmentation;

use crate::commands::{self, Command, Host, Palette};
use crate::keymap::{self, Chord, Keymap};
use crate::link_suggest::{self, Suggest};
use crate::plugins::settings::{Setting, Values};
use crate::plugins::{
    ActiveNote, Answer, Blocks, Context, Effect, Place as LinePlace, Plugins, Question,
};
use crate::resolver::VaultResolver;
pub use crate::settings_window::{Page, SettingsWindow};
use crate::sidebar::{Panel, Request, Sidebar};
use crate::switcher::Switcher;
use crate::ui::theme;
use crate::vault::Vault;
use crate::vault_picker::{Picked, VaultPicker};

/// Where keys go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Editor,
    /// A plugin's panel at the bottom of the sidebar (the calendar).
    Panel,
    /// The backlinks under the note.
    Backlinks,
    /// A plugin's pane above the note (Bases' filters).
    Pane,
}

/// What to do after the unsaved-changes question is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Close,
    Quit,
}

/// A prompt over the workspace; it gets every key while it's open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    /// Ctrl+N: the new note's name, in `folder`.
    NewNote { folder: PathBuf, input: String },
    /// Save As (Ctrl+Alt+S): a path relative to the vault.
    SaveAs { input: String },
    /// The date picker: a date in words, put in at the cursor.
    Date { input: String },
    /// "Save changes?" for tab `tab`, before closing it or quitting.
    Unsaved { tab: usize, then: Then },
    /// Ctrl+O: go to a note.
    Switcher(Switcher),
    /// Ctrl+P: run a command.
    Palette(Palette),
    /// A plugin's questions (e.g. Templater's prompts).
    Ask(Ask),
    /// The settings window (Alt+,): the editor's settings, the keyboard
    /// shortcuts, the plugins and their settings.
    Settings(SettingsWindow),
    /// Open another vault (any folder, or a new one).
    OpenVault(VaultPicker),
    /// The open note's properties (Alt+;).
    Properties(PropertyWindow),
}

/// The property editor's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyWindow {
    pub selected: usize,
    /// What's being typed, and for what.
    pub input: Option<(PropertyInput, String)>,
}

/// What the property editor's input is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyInput {
    /// The chosen property's value.
    Value,
    /// A new property, `name: value`.
    Add,
    /// The chosen property's new name.
    Rename,
}

/// Whose settings a settings page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The editor's (mdedit's `config.toml`, in the user's config folder).
    Editor,
    /// The vault's notes (`.blackglass/notes.toml`: note IDs).
    Notes,
    /// The vault's natural language dates (`.blackglass/dates.toml`).
    Dates,
    /// The window's (`window.toml`, in the user's config folder).
    Window,
    /// A plugin's, by its index in the plugins list.
    Plugin(usize),
}

/// A plugin's questions, answered one after the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub plugin: &'static str,
    pub command: &'static str,
    pub questions: Vec<Question>,
    pub answers: Vec<Answer>,
    /// What's typed for the current question.
    pub input: String,
    /// For a choice: the items that match the input, and the highlighted
    /// one.
    pub results: Vec<usize>,
    pub selected: usize,
    /// For a choice of many: the items marked so far (Tab), in order.
    pub marked: Vec<usize>,
    /// For a form: its fields' values as edited, and the field edited.
    pub form: Vec<crate::plugins::FieldValue>,
    pub field: usize,
}

impl Ask {
    fn new(plugin: &'static str, command: &'static str, questions: Vec<Question>) -> Self {
        let mut ask = Ask {
            plugin,
            command,
            questions,
            answers: Vec::new(),
            input: String::new(),
            results: Vec::new(),
            selected: 0,
            marked: Vec::new(),
            form: Vec::new(),
            field: 0,
        };
        ask.update();
        ask
    }

    /// The question being answered.
    pub fn current(&self) -> &Question {
        &self.questions[self.answers.len()]
    }

    /// Filters a choice's items by the input (fuzzy, best first).
    fn update(&mut self) {
        self.selected = 0;
        if let Question::Form { fields, .. } = self.current()
            && self.form.is_empty()
        {
            self.form = fields.iter().map(|f| f.value.clone()).collect();
            self.field = 0;
        }
        let (Question::Choose { items, .. }
        | Question::Many { items, .. }
        | Question::Suggest { items, .. }) = self.current()
        else {
            return;
        };
        let query = self.input.trim();
        let mut scored: Vec<(i64, usize)> = items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                if query.is_empty() {
                    Some((0, i))
                } else {
                    crate::switcher::score(item, query).map(|s| (s, i))
                }
            })
            .collect();
        scored.sort_by_key(|&(s, i)| (std::cmp::Reverse(s), i));
        self.results = scored.into_iter().map(|(_, i)| i).collect();
        // A suggestion's own text: `items.len()` stands for what's typed.
        if let Question::Suggest { items, .. } = self.current()
            && !query.is_empty()
            && !items.iter().any(|i| i.eq_ignore_ascii_case(query))
        {
            let typed = items.len();
            self.results.insert(0, typed);
        }
    }

    /// The answer Enter gives now, if any.
    fn answer(&self) -> Option<Answer> {
        match self.current() {
            Question::Text { default, .. } | Question::Lines { default, .. }
                if self.input.is_empty() =>
            {
                Some(Answer::Text(default.clone()))
            }
            Question::Text { .. } | Question::Lines { .. } => {
                Some(Answer::Text(self.input.clone()))
            }
            Question::Choose { .. } => self.results.get(self.selected).map(|&i| Answer::Choice(i)),
            Question::Suggest { items, default, .. } => {
                Some(Answer::Text(match self.results.get(self.selected) {
                    Some(&i) if i < items.len() => items[i].clone(),
                    Some(_) => self.input.trim().to_string(),
                    None if self.input.trim().is_empty() => default.clone(),
                    None => self.input.trim().to_string(),
                }))
            }
            Question::Many { .. } if !self.marked.is_empty() => {
                Some(Answer::Choices(self.marked.clone()))
            }
            Question::Many { .. } => self
                .results
                .get(self.selected)
                .map(|&i| Answer::Choices(vec![i])),
            Question::Show { .. } => Some(Answer::Choice(self.selected)),
            Question::Form { .. } => Some(Answer::Fields(
                self.form
                    .iter()
                    .map(|v| match v {
                        crate::plugins::FieldValue::Text(t)
                        | crate::plugins::FieldValue::Date(t)
                        | crate::plugins::FieldValue::Secret(t) => Answer::Text(t.clone()),
                        crate::plugins::FieldValue::Choice(_, i) => Answer::Choice(*i),
                        crate::plugins::FieldValue::Pick { chosen, .. } => {
                            Answer::Choices(chosen.clone())
                        }
                    })
                    .collect(),
            )),
        }
    }

    /// A key for a form's fields; false if it isn't one (Enter, Esc).
    fn form_key(&mut self, key: KeyEvent) -> bool {
        use crate::plugins::FieldValue;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let n = self.form.len();
        let Some(value) = self.form.get_mut(self.field) else {
            return false;
        };
        // A date: Shift+arrows a day or a week, PgUp / PgDn a month (from
        // today when there's none).
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let step = match key.code {
            KeyCode::Left if shift => Some((-1i64, 0i32)),
            KeyCode::Right if shift => Some((1, 0)),
            KeyCode::Up if shift => Some((-7, 0)),
            KeyCode::Down if shift => Some((7, 0)),
            KeyCode::PageUp => Some((0, -1)),
            KeyCode::PageDown => Some((0, 1)),
            _ => None,
        };
        if let (Some((days, months)), FieldValue::Date(_)) = (step, &*value) {
            let today = chrono::Local::now().date_naive();
            let day = match value.day() {
                None => today,
                Some(d) => {
                    let d = d + chrono::Duration::days(days);
                    let by = chrono::Months::new(months.unsigned_abs());
                    if months < 0 {
                        d.checked_sub_months(by).unwrap_or(d)
                    } else {
                        d.checked_add_months(by).unwrap_or(d)
                    }
                }
            };
            *value = FieldValue::Date(day.format("%Y-%m-%d").to_string());
            return true;
        }
        // A pick field while something is typed: its matches.
        if let FieldValue::Pick { .. } = value {
            let matches = value.matches(PICK_SHOWN);
            let FieldValue::Pick {
                chosen, query, at, ..
            } = value
            else {
                unreachable!("a pick field")
            };
            match key.code {
                KeyCode::Up if !query.is_empty() => *at = at.saturating_sub(1),
                KeyCode::Down if !query.is_empty() => {
                    *at = (*at + 1).min(matches.len().saturating_sub(1))
                }
                KeyCode::Enter if !query.is_empty() => {
                    if let Some(&i) = matches.get(*at) {
                        chosen.push(i);
                    }
                    query.clear();
                    *at = 0;
                }
                KeyCode::Esc if !query.is_empty() => query.clear(),
                KeyCode::Backspace if query.is_empty() => {
                    chosen.pop();
                }
                KeyCode::Backspace => {
                    query.pop();
                    *at = 0;
                }
                KeyCode::Char('u') if ctrl => query.clear(),
                KeyCode::Char(c) if !ctrl => {
                    query.push(c);
                    *at = 0;
                }
                KeyCode::Up | KeyCode::BackTab => self.field = self.field.saturating_sub(1),
                KeyCode::Down | KeyCode::Tab => self.field = (self.field + 1).min(n - 1),
                KeyCode::Enter | KeyCode::Esc => return false,
                _ => {}
            }
            return true;
        }
        match (key.code, value) {
            (KeyCode::Up | KeyCode::BackTab, _) => self.field = self.field.saturating_sub(1),
            (KeyCode::Down | KeyCode::Tab, _) => self.field = (self.field + 1).min(n - 1),
            (KeyCode::Left, FieldValue::Choice(items, i)) => {
                *i = (*i + items.len() - 1) % items.len()
            }
            (KeyCode::Right | KeyCode::Char(' '), FieldValue::Choice(items, i)) => {
                *i = (*i + 1) % items.len();
            }
            // A letter chooses the next item starting with it.
            (KeyCode::Char(c), FieldValue::Choice(items, i)) if !ctrl => {
                let c = c.to_lowercase().to_string();
                let len = items.len();
                if let Some(k) = (1..=len)
                    .map(|k| (*i + k) % len)
                    .find(|&k| items[k].to_lowercase().starts_with(&c))
                {
                    *i = k;
                }
            }
            (
                KeyCode::Char('u'),
                FieldValue::Text(t) | FieldValue::Date(t) | FieldValue::Secret(t),
            ) if ctrl => t.clear(),
            (
                KeyCode::Char(c),
                FieldValue::Text(t) | FieldValue::Date(t) | FieldValue::Secret(t),
            ) if !ctrl => t.push(c),
            (
                KeyCode::Backspace,
                FieldValue::Text(t) | FieldValue::Date(t) | FieldValue::Secret(t),
            ) => {
                t.pop();
            }
            (KeyCode::Enter | KeyCode::Esc, _) => return false,
            _ => {}
        }
        true
    }
}

/// How many of a pick field's matches show.
pub const PICK_SHOWN: usize = 8;

/// What the program should do after an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Continue,
    Quit,
}

/// Screen areas from the last frame, for mouse clicks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Areas {
    pub panel_tabs: Vec<(Rect, Panel)>,
    /// The sidebar's list of files, results or tags.
    pub list: Rect,
    /// Each tab's title, and its close button.
    pub tabs: Vec<(Rect, usize)>,
    pub closes: Vec<(Rect, usize)>,
    /// The `+` after the tabs.
    pub new_tab: Rect,
    pub editor: Rect,
    /// The open popup's list (switcher, palette, plugins), and the index
    /// of its first row shown.
    pub popup_list: Rect,
    pub popup_scroll: usize,
    /// The link suggestions' rows.
    pub suggest: Rect,
    /// The plugin panel's lines, and whose panel it is.
    pub panel: Rect,
    pub panel_plugin: Option<&'static str>,
    /// The Plugins settings page's ⚙ (settings) marks, by plugin.
    pub gears: Vec<(Rect, usize)>,
    /// The settings window's pages and the rows of the page shown (by
    /// their index in its rows).
    pub settings_nav: Vec<(Rect, Page)>,
    pub settings_rows: Vec<(Rect, usize)>,
    /// The backlinks' rows, by backlink.
    pub backlinks: Vec<(Rect, usize)>,
    /// The plugin pane above the note: whose it is, and its area.
    pub pane: Option<(&'static str, Rect)>,
    /// Where its text cursor is.
    pub pane_cursor: Option<(u16, u16)>,
    /// A form's calendar: each day's cell.
    pub form_days: Vec<(Rect, chrono::NaiveDate)>,
}

/// Opens a file outside blackglass (`Err`: why it couldn't).
pub type Opener = dyn Fn(&Path) -> Result<(), String>;

/// How text is put on the desktop's clipboard (tests catch it).
pub type Copier = dyn FnMut(&str) -> Result<(), String>;

/// Text extracted from a note (Ctrl+X) into a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extract {
    /// The note it comes from.
    pub source: PathBuf,
    pub text: String,
}

/// A note and a cursor position in it, for back / forward.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Place {
    path: PathBuf,
    row: usize,
    col: usize,
}

/// How many places back / forward remembers.
const HISTORY: usize = 100;

/// The plugin id blackglass's own questions (move, delete) are asked as.
const HOST: &str = "blackglass";

/// The embedded help page (F1 / Ctrl+H); the shortcut list is added to it.
const HELP: &str = include_str!("help.md");

/// The help tab's title (the only tab without a file).
pub const HELP_TITLE: &str = "Help";

/// Where deleted notes go, in the vault (hidden, so the scan skips it).
pub const TRASH: &str = ".trash";

/// Said when a key would change the help page.
const HELP_READ_ONLY: &str = "This page is read-only";

/// The user's keyboard shortcuts, in the config folder.
pub const KEYS_FILE: &str = "keys.toml";
/// The chosen theme, in the config folder.
pub const APPEARANCE_FILE: &str = "appearance.toml";

/// What a new note from a template needs.
const TEMPLATER_NEEDED: &str =
    "New notes from templates come with the Templater plugin: install it (Ctrl+P, Open plugins)";

/// The name a new note gets when none is typed (then `Untitled 1` …).
pub const UNTITLED: &str = "Untitled";

/// Rows the mouse wheel scrolls.
const WHEEL_ROWS: usize = 3;

pub struct App {
    pub shared: Shared,
    /// Shared (not copied) with what reads the whole vault while drawing
    /// (```` ```query ```` blocks, through a `Weak`); changed through
    /// `Rc::make_mut`, which doesn't copy while only the App holds it.
    pub vault: Rc<Vault>,
    pub sidebar: Sidebar,
    pub tabs: Vec<EditorView>,
    /// The active tab (meaningless while there are no tabs).
    pub active: usize,
    pub focus: Focus,
    pub prompt: Option<Prompt>,
    /// blackglass's own message for the status bar (cleared on the next
    /// key); the editor has its own.
    pub message: String,
    pub areas: Areas,
    /// Whether the plugin panel (the calendar) is shown (Alt+C).
    pub panel: bool,
    /// The vault's plugins; the editor's code block processor shares them.
    pub plugins: Rc<RefCell<Plugins>>,
    /// Opens a file that isn't a note (an image) in the desktop's viewer;
    /// tests replace it.
    pub opener: Box<Opener>,
    /// Puts text on the clipboard ([`Effect::CopyText`]).
    pub copier: Box<Copier>,
    /// The selection being extracted to a new note (Ctrl+X), while the
    /// new note is made.
    pub extract: Option<Extract>,
    /// The note a followed link names, while "Create it?" is asked.
    linked_note: Option<String>,
    /// A new note and the ID properties it should have once its template
    /// is in (a template that asks questions comes later).
    pending_id: Option<(PathBuf, Vec<(String, String)>)>,
    /// A note being made from a rendered template (its commands mustn't
    /// run again).
    rendered: bool,
    /// The file or folder "Move" asked about.
    moving: Option<PathBuf>,
    /// A calendar's day (`2026-03-05`) whose note "Create it?" asks about.
    creating_day: Option<String>,
    /// A line waiting for today's daily note to be made: the note, the
    /// heading it goes under, the line.
    daily_line: Option<PendingLine>,
    /// The unsaved text last put in the index: its note and a hash.
    synced: Option<(PathBuf, u64)>,
    /// Watches the vault for changes made elsewhere.
    watcher: crate::watch::Watcher,
    /// The session as last written (`.blackglass/workspace.json`).
    session_saved: String,
    /// The note or folder being renamed (or the folder a new one goes in),
    /// while its new name is asked.
    renaming: Option<PathBuf>,
    /// The properties listed in "Show properties" / "Rename property".
    property_names: Vec<String>,
    /// The PDFs listed in "PDF to note" (relative to the vault).
    pdfs: Vec<PathBuf>,
    /// The outline's headings, by line (Alt+O).
    outline: Vec<usize>,
    /// Notes you were on (W-28): before and after the current one, which
    /// is `here`.
    back: Vec<Place>,
    forward: Vec<Place>,
    here: Option<Place>,
    /// Which keys run which command; the user's changes are in
    /// `keys.toml` in `config_dir`.
    pub keymap: Keymap,
    /// A plugin asked to quit (and nothing unsaved is left); the main loop
    /// stops.
    pub quit_requested: bool,
    /// Running in a window of its own (`--gui`): its settings page shows.
    pub windowed: bool,
    /// The window's settings ([`crate::window_settings`]).
    window: crate::window_settings::Settings,
    /// The user's blackglass config folder (`~/.config/blackglass`), where
    /// settings are saved; `None` (as in tests until set) saves nothing.
    pub config_dir: Option<PathBuf>,
    /// The editor's settings, as written in its config file.
    pub editor_values: Values,
    /// The chosen theme's colors (W-69), and its id (saved in
    /// `appearance.toml`).
    pub palette: theme::Palette,
    theme: String,
    /// The themes listed in "Choose theme": (id, file text).
    themes: Vec<(String, String)>,
    /// Whether the backlinks are shown under the note (Alt+L), and the
    /// highlighted one.
    pub backlinks: bool,
    pub backlink: usize,
    /// The active note's backlinks, for its path (cleared when notes
    /// change).
    backlinks_cache: Option<BacklinksCache>,
    /// A linked note shown in a popup (Alt+P), read-only.
    pub preview: Option<Box<EditorView>>,
    /// Link suggestions while a `[[link]]` is typed (W-23).
    pub suggest: Option<Suggest>,
    /// A plugin's suggestions while typing (when no link is typed).
    pub plugin_suggest: Option<link_suggest::PluginSuggest>,
    /// The link (line, name start) whose suggestions were closed with Esc.
    dismissed: Option<(usize, usize)>,
    /// The vault's natural language dates settings (W-128).
    pub dates: crate::nldates::Settings,
    /// Changes made to other notes, to undo and redo (W-133).
    pub journal: crate::journal::Files,
}

impl App {
    /// A workspace on `vault` with no notes open and default settings.
    pub fn new(vault: Vault) -> Self {
        let mut shared = Shared::new();
        shared.resolver = Box::new(VaultResolver::new(&vault));
        // Double-size headings resize whole terminal rows, and the editor
        // shares its rows with the sidebar.
        shared.config.heading_size = HeadingSize::Off;
        let plugins = Rc::new(RefCell::new(Plugins::for_vault(&vault)));
        shared.processor = Some(Box::new(Blocks(Rc::clone(&plugins))));
        let vault_root = vault.root.clone();
        let keymap = Keymap::new(commands::builtin().into_iter().map(|c| (c.id, c.defaults)));
        let mut app = App {
            plugins,
            keymap,
            config_dir: None,
            quit_requested: false,
            windowed: false,
            window: crate::window_settings::Settings::default(),
            backlinks: false,
            backlink: 0,
            backlinks_cache: None,
            preview: None,
            editor_values: Values::default(),
            palette: theme::Palette::default(),
            theme: "default".into(),
            themes: Vec::new(),
            suggest: None,
            plugin_suggest: None,
            dismissed: None,
            dates: crate::nldates::Settings::load(&vault),
            journal: crate::journal::Files::default(),
            back: Vec::new(),
            forward: Vec::new(),
            here: None,
            extract: None,
            linked_note: None,
            pending_id: None,
            rendered: false,
            moving: None,
            creating_day: None,
            daily_line: None,
            synced: None,
            watcher: crate::watch::Watcher::start(&vault_root, crate::watch::INTERVAL),
            session_saved: String::new(),
            renaming: None,
            property_names: Vec::new(),
            pdfs: Vec::new(),
            outline: Vec::new(),
            opener: Box::new(open_outside),
            copier: Box::new(copy_outside),
            shared,
            vault: Rc::new(vault),
            sidebar: Sidebar::new(),
            tabs: Vec::new(),
            active: 0,
            focus: Focus::Sidebar,
            prompt: None,
            message: String::new(),
            areas: Areas::default(),
            panel: false,
        };
        app.plugin_keys();
        app.template_tags();
        app.plugins.borrow_mut().queries.set_vault(&app.vault);
        app.restore_session();
        if !writable(&app.vault.root) {
            app.message =
                "This vault is read-only: notes can be read, but changes can't be saved".into();
        }
        app
    }

    /// The workspace to remember: the tabs on notes and files in the vault
    /// (not help or base pages), the active one, the open folders.
    fn session(&self) -> crate::session::Session {
        let mut tabs = Vec::new();
        let mut active = 0;
        for (i, view) in self.tabs.iter().enumerate() {
            let Some(rel) = view.path.as_deref().and_then(|p| self.vault.rel(p)) else {
                continue;
            };
            if i == self.active {
                active = tabs.len();
            }
            tabs.push(crate::session::Tab {
                path: rel.to_path_buf(),
                cursor: (view.editor.row, view.editor.col),
            });
        }
        let expanded = self
            .sidebar
            .expanded
            .iter()
            .filter_map(|p| self.vault.rel(p))
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .collect();
        crate::session::Session {
            tabs,
            active,
            expanded,
        }
    }

    /// Writes the session if it changed since it was last written.
    pub fn save_session(&mut self) {
        let json = self.session().to_json();
        if json == self.session_saved {
            return;
        }
        match crate::session::save(&self.vault.root, &self.session()) {
            Ok(()) => self.session_saved = json,
            Err(e) => self.message = format!("Cannot remember the open tabs: {e}"),
        }
    }

    /// Opens the vault's last session: its tabs that are still there,
    /// with their cursors, and its open folders.
    fn restore_session(&mut self) {
        let Some(session) = crate::session::load(&self.vault.root) else {
            return;
        };
        let mut active = None;
        for (i, tab) in session.tabs.iter().enumerate() {
            let path = self.vault.root.join(&tab.path);
            if !path.is_file() || !self.show(&path) {
                continue;
            }
            if i <= session.active {
                active = Some(self.active);
            }
            let view = self.tabs.get_mut(self.active).expect("just opened");
            let last = view.editor.lines.len() - 1;
            view.editor.row = tab.cursor.0.min(last);
            view.editor.col = tab.cursor.1;
            view.editor.snap_col();
        }
        if let Some(i) = active {
            self.active = i;
        }
        for folder in &session.expanded {
            let dir = self.vault.root.join(folder);
            if dir.is_dir() {
                self.sidebar.expanded.insert(dir);
            }
        }
        self.session_saved = self.session().to_json();
    }

    /// The vault changed: the plugins and query blocks see it as it is.
    fn vault_changed(&mut self) {
        let mut plugins = self.plugins.borrow_mut();
        plugins.queries.set_vault(&self.vault);
        plugins.vault_changed(&self.vault);
    }

    /// Gives the enabled plugins' commands their default keys.
    fn plugin_keys(&mut self) {
        for (plugin, _, command) in self.plugins.borrow().commands() {
            let defaults =
                Chord::parse_list(command.keys).expect("a plugin's default keys are keys");
            self.keymap
                .add_defaults(&format!("plugin:{plugin}:{}", command.id), defaults);
        }
    }

    /// The enabled plugins' commands that have default keys, with their
    /// keys now: (name, keys), for the empty editor side.
    pub fn keyed_plugin_commands(&self) -> Vec<(String, String, String)> {
        self.plugins
            .borrow()
            .commands()
            .into_iter()
            .filter(|(_, _, c)| !c.keys.is_empty())
            .map(|(plugin, _, c)| {
                let id = format!("plugin:{plugin}:{}", c.id);
                let keys = keymap::describe(self.keymap.keys(&id));
                (id, c.name.to_string(), keys)
            })
            .collect()
    }

    /// The active tab's editor, if a note is open.
    pub fn view(&self) -> Option<&EditorView> {
        self.tabs.get(self.active)
    }

    fn view_mut(&mut self) -> Option<&mut EditorView> {
        self.tabs.get_mut(self.active)
    }

    /// Shows the file at `path` in a tab: the tab it's already in, or a new
    /// one after the active tab. Gives the editor the focus. The note left
    /// goes in the back / forward history.
    pub fn open(&mut self, path: &Path) -> bool {
        self.track();
        let opened = self.show(path);
        self.track();
        opened
    }

    /// [`App::open`] without touching the history (back / forward use it).
    fn show(&mut self, path: &Path) -> bool {
        if path.extension().is_some_and(|e| e == "base")
            && self.plugins.borrow().is_enabled(crate::plugins::bases::ID)
        {
            return self.show_base(path);
        }
        self.show_file(path)
    }

    /// A `.base` file, rendered, in a read-only tab named after it.
    fn show_base(&mut self, path: &Path) -> bool {
        match self.base_page(path) {
            Ok((title, text)) => {
                self.show_page(&title, &text);
                self.focus = Focus::Editor;
                true
            }
            Err(e) => {
                self.message = e;
                false
            }
        }
    }

    /// A `.base` file's page: its title and text (the YAML in a block).
    fn base_page(&self, path: &Path) -> Result<(String, String), String> {
        let yaml =
            std::fs::read_to_string(path).map_err(|e| format!("Cannot open the base: {e}"))?;
        let rel = self
            .vault
            .rel(path)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = format!(
            "{}\n```base\n{}\n```\n",
            crate::plugins::bases::page_marker(&rel),
            yaml.trim_end()
        );
        Ok((title, text))
    }

    /// A saved `.base` file's page, drawn again if it's open.
    fn refresh_base_page(&mut self, path: &Path) {
        let Ok((title, text)) = self.base_page(path) else {
            return;
        };
        if let Some(view) = self
            .tabs
            .iter_mut()
            .find(|v| is_help(v) && v.name.as_deref() == Some(title.as_str()))
        {
            let mut page = EditorView::new(&text, None);
            page.name = Some(title);
            page.enter_reading();
            page.status.clear();
            *view = page;
        }
    }

    /// [`App::show`] for any file, as text.
    fn show_file(&mut self, path: &Path) -> bool {
        let same = |view: &EditorView| {
            view.path
                .as_deref()
                .is_some_and(|p| p == path || same_file(p, path))
        };
        if let Some(i) = self.tabs.iter().position(same) {
            self.active = i;
        } else {
            match EditorView::open(path.to_path_buf()) {
                Ok(mut view) => {
                    view.status.clear();
                    let at = if self.tabs.is_empty() {
                        0
                    } else {
                        self.active + 1
                    };
                    self.tabs.insert(at, view);
                    self.active = at;
                }
                Err(e) => {
                    self.message = e;
                    return false;
                }
            }
        }
        self.focus = Focus::Editor;
        true
    }

    /// Opens `path` and puts the cursor at `at` (line, char column), with
    /// `search` as the note's last search (for F3).
    fn open_at(&mut self, path: &Path, at: Option<(usize, usize)>, search: Option<String>) {
        if !self.open(path) {
            return;
        }
        let view = self.view_mut().expect("a tab was just opened");
        if let Some((row, col)) = at {
            let row = row.min(view.editor.lines.len().saturating_sub(1));
            view.editor.row = row;
            view.editor.col = col.min(view.editor.lines[row].chars().count());
            view.editor.snap_col();
            // A few lines of context above the match.
            view.scroll = row.saturating_sub(3);
        }
        if let Some(search) = search {
            view.last_search = search;
        }
    }

    /// Scans the vault again, after files were created or renamed.
    pub fn rescan(&mut self) {
        if let Err(e) = Rc::make_mut(&mut self.vault).rescan() {
            self.message = format!("Cannot scan the vault: {e}");
            return;
        }
        self.shared.resolver = Box::new(VaultResolver::new(&self.vault));
        self.sidebar.update_search(&self.vault);
        self.vault_changed();
        self.backlinks_cache = None;
    }

    /// Updates the index after the note at `path` was saved.
    fn saved(&mut self, path: &Path, text: &str) {
        if path.extension().is_some_and(|e| e == "base") {
            self.refresh_base_page(path);
        }
        if Rc::make_mut(&mut self.vault).update_note(path, text) {
            self.backlinks_cache = None;
            self.sidebar.update_search(&self.vault);
            let mut plugins = self.plugins.borrow_mut();
            plugins.queries.set_vault(&self.vault);
            // Only this note: the plugins update it alone.
            plugins.note_changed(path, &self.vault);
            plugins.note_saved(path, &self.vault);
        } else if path.starts_with(&self.vault.root) {
            self.rescan();
        }
    }

    /// Puts the active note's unsaved text in the vault's index (results,
    /// backlinks and search follow edits before they're saved), when it
    /// changed since the last time.
    fn sync_unsaved(&mut self) {
        let Some(view) = self.view().filter(|v| v.is_dirty()) else {
            return;
        };
        let Some(path) = view.path.clone() else {
            return;
        };
        let text = view.editor.to_text();
        let stamp = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut h);
            h.finish()
        };
        if self.synced.as_ref() == Some(&(path.clone(), stamp)) {
            return;
        }
        self.synced = Some((path.clone(), stamp));
        self.index_text(&path, &text);
    }

    /// The index has the note at `path` as `text` (saved or not).
    fn index_text(&mut self, path: &Path, text: &str) {
        if self
            .vault
            .note(path)
            .is_some_and(|n| n.lines == crate::vault::split_lines(text))
        {
            return;
        }
        if Rc::make_mut(&mut self.vault).update_note(path, text) {
            self.backlinks_cache = None;
            self.sidebar.update_search(&self.vault);
            let mut plugins = self.plugins.borrow_mut();
            plugins.queries.set_vault(&self.vault);
            plugins.note_changed(path, &self.vault);
        }
    }

    /// Creates the note `name` (`.md` added; `/` makes folders) in `folder`
    /// and opens it. An empty name picks `Untitled`, `Untitled 1` …; an
    /// existing note is just opened.
    pub fn create_note(&mut self, folder: &Path, name: &str) -> Result<PathBuf, String> {
        self.create_note_with(folder, name, "")
    }

    /// A new note called `title` (a typed name, a link's), with the
    /// vault's note IDs if they're on (`note_ids`): its name, or its
    /// properties. Returns its path.
    fn create_new_note(
        &mut self,
        folder: &Path,
        title: &str,
        text: &str,
    ) -> Result<PathBuf, String> {
        let ids = crate::note_ids::NoteIds::load(&self.vault);
        // A path typed (`Folder/Name`): the ID goes on the name part.
        let (dir, title) = match title.trim().rsplit_once('/') {
            Some((d, t)) => (format!("{d}/"), t),
            None => (String::new(), title.trim()),
        };
        let vault = Rc::clone(&self.vault);
        let id = ids.id(chrono::Local::now().naive_local(), |id| {
            crate::note_ids::NoteIds::taken(&vault, id)
        });
        let Some(new) = ids.new_note(title, &id) else {
            return self.create_note_with(folder, &format!("{dir}{title}"), text);
        };
        let name = format!("{dir}{}", new.name);
        let text = if new.properties.is_empty() || text.is_empty() {
            text.to_string()
        } else {
            crate::note_ids::with_properties(text, &new.properties)
        };
        if !new.properties.is_empty() {
            let path = self.vault_path(folder, &name)?;
            self.pending_id = Some((path, new.properties.clone()));
        }
        let path = self.create_note_with(folder, &name, &text)?;
        self.add_pending_id();
        Ok(path)
    }

    /// Puts a new note's ID properties in (after its template, if any).
    fn add_pending_id(&mut self) {
        let Some((path, properties)) = self.pending_id.take() else {
            return;
        };
        let Some(view) = self
            .tabs
            .iter_mut()
            .find(|v| v.path.as_deref() == Some(path.as_path()))
        else {
            return;
        };
        if self.prompt.is_some() {
            // A template's questions are open: after their answers.
            self.pending_id = Some((path, properties));
            return;
        }
        let text = view.editor.to_text();
        let new = crate::note_ids::with_properties(&text, &properties);
        if new == text {
            return;
        }
        let mut lines: Vec<String> = new.lines().map(String::from).collect();
        let empty = text.trim().is_empty();
        if empty {
            // A line to write on, below the properties.
            lines.push(String::new());
        }
        let (row, col) = (view.editor.row, view.editor.col);
        let before = view.editor.lines.len();
        replace_lines(view, 0, before, &lines, &mut self.shared);
        // The cursor where it was (a template's), moved down by the lines
        // added above it; in an empty note, below the properties.
        let last = view.editor.lines.len().saturating_sub(1);
        if empty {
            view.editor.row = last;
            view.editor.col = 0;
        } else {
            view.editor.row = (row + view.editor.lines.len())
                .saturating_sub(before)
                .min(last);
            view.editor.col = col;
            view.editor.snap_col();
        }
        let saved = view.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            &mut self.shared,
        );
        let _ = saved;
        let text = view.editor.to_text();
        self.saved(&path, &text);
    }

    /// [`App::create_note`] with `text` in the new note; then the name
    /// must be new.
    fn create_note_with(
        &mut self,
        folder: &Path,
        name: &str,
        text: &str,
    ) -> Result<PathBuf, String> {
        let name = name.trim();
        let path = if name.is_empty() {
            (0..)
                .map(|n| match n {
                    0 => format!("{UNTITLED}.md"),
                    n => format!("{UNTITLED} {n}.md"),
                })
                .map(|file| folder.join(file))
                .find(|p| !p.exists())
                .expect("some Untitled N is free")
        } else {
            self.vault_path(folder, name)?
        };
        if path.exists() && !text.is_empty() {
            return Err(format!("{name} already exists"));
        }
        let created = !path.exists();
        if created {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create note: {e}"))?;
            }
            self.journal.touch(&path);
            mdedit::files::write_atomic(&path, text)
                .map_err(|e| format!("Cannot create note: {e}"))?;
            self.rescan();
        }
        self.open(&path);
        self.sidebar.reveal(&path, &self.vault);
        if created && (text.is_empty() || (text.contains("<%") && !self.rendered)) {
            self.note_created();
        }
        Ok(path)
    }

    /// The active note was just made: plugins may fill it (a folder's
    /// template) or run the commands in it (Templater's trigger on new
    /// files).
    fn note_created(&mut self) {
        let found = self.with_context(|ctx| self.plugins.borrow_mut().note_created(ctx));
        if let Some((id, effect)) = found {
            self.apply_effect(id, crate::plugins::NOTE_CREATED, effect);
        }
    }

    /// `name` in `folder` as a note path (`.md` added unless it has an
    /// extension); `Err` if it would leave the vault.
    fn vault_path(&self, folder: &Path, name: &str) -> Result<PathBuf, String> {
        let rel = Path::new(name);
        if rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err("A note name can't leave the vault".into());
        }
        let mut path = folder.join(rel);
        if path.extension().is_none() {
            path.as_mut_os_string().push(".md");
        }
        Ok(path)
    }

    /// Closes tab `i`, asking first if it has unsaved changes.
    pub fn close(&mut self, i: usize) {
        if self.tabs.get(i).is_some_and(EditorView::is_dirty) {
            self.active = i;
            self.prompt = Some(Prompt::Unsaved {
                tab: i,
                then: Then::Close,
            });
            return;
        }
        self.remove_tab(i);
    }

    fn remove_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        let view = self.tabs.remove(i);
        // Its unsaved changes are gone: the index goes back to the file.
        if view.is_dirty()
            && let Some(path) = view.path.as_deref()
            && let Ok(text) = std::fs::read_to_string(path)
        {
            self.index_text(path, &text);
            self.synced = None;
        }
        if self.active > i || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        if self.tabs.is_empty() {
            self.focus = Focus::Sidebar;
        }
    }

    /// Quits, first asking about each tab with unsaved changes.
    pub fn quit(&mut self) -> Action {
        match self.tabs.iter().position(EditorView::is_dirty) {
            Some(i) => {
                self.active = i;
                self.prompt = Some(Prompt::Unsaved {
                    tab: i,
                    then: Then::Quit,
                });
                Action::Continue
            }
            None => {
                self.save_session();
                Action::Quit
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        self.track();
        let action = self.dispatch_key(key);
        self.refresh_suggest();
        self.track();
        action
    }

    fn dispatch_key(&mut self, key: KeyEvent) -> Action {
        self.message.clear();
        if self.preview.is_some() {
            self.preview_key(key);
            return Action::Continue;
        }
        if let Some(prompt) = self.prompt.take() {
            return self.prompt_key(prompt, key);
        }
        if let Some(action) = self.host_key(key) {
            return action;
        }
        if self.focus == Focus::Editor && self.suggest_key(key) {
            return Action::Continue;
        }
        if self.focus == Focus::Editor && self.plugin_editor_key(key) {
            return Action::Continue;
        }
        match self.focus {
            Focus::Sidebar => {
                self.refresh_plugin_tabs();
                let request = self.sidebar.handle_key(key, &self.vault);
                self.request(request);
            }
            Focus::Editor => {
                let chord = Chord::of(&key);
                if self.keymap.is_freed(&chord) {
                    // An editor key taken off its command.
                    self.message =
                        format!("{chord} has no command now (Settings, Keyboard shortcuts)");
                } else {
                    self.editor_key(key);
                }
            }
            Focus::Panel => self.panel_key(key),
            Focus::Backlinks => self.backlinks_key(key),
            Focus::Pane => self.pane_key(key),
        }
        Action::Continue
    }

    /// A key for the plugin pane above the note; without one, the note's.
    fn pane_key(&mut self, key: KeyEvent) {
        let id = self.with_context(|ctx| self.plugins.borrow().pane(ctx, 80, 24).map(|(id, _)| id));
        let Some(id) = id else {
            self.focus = Focus::Editor;
            return;
        };
        let effect = self.with_context(|ctx| self.plugins.borrow_mut().pane_key(id, key, ctx));
        self.apply_effect(id, "pane", effect);
    }

    /// The link suggestion list's keys (W-23): ↑/↓ choose, Enter or Tab
    /// inserts, Esc closes. Returns false for keys the editor gets.
    fn suggest_key(&mut self, key: KeyEvent) -> bool {
        if self.plugin_suggest.is_some()
            && key.code == KeyCode::Enter
            && key.modifiers == KeyModifiers::SHIFT
        {
            self.accept_plugin_suggestion(true);
            return true;
        }
        if let Some(p) = &mut self.plugin_suggest
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Up => p.selected = p.selected.saturating_sub(1),
                KeyCode::Down => p.selected = (p.selected + 1).min(p.items.len().saturating_sub(1)),
                KeyCode::Enter | KeyCode::Tab => self.accept_plugin_suggestion(false),
                KeyCode::Esc => {
                    self.dismissed = Some((p.row, p.start));
                    self.plugin_suggest = None;
                }
                _ => return false,
            }
            return true;
        }
        let Some(s) = &mut self.suggest else {
            return false;
        };
        if !key.modifiers.is_empty() {
            return false;
        }
        let rows = s.switcher.results.len();
        match key.code {
            KeyCode::Up => s.switcher.selected = s.switcher.selected.saturating_sub(1),
            KeyCode::Down => {
                s.switcher.selected = (s.switcher.selected + 1).min(rows.saturating_sub(1));
            }
            KeyCode::Enter | KeyCode::Tab if rows > 0 => {
                let selected = s.switcher.selected;
                self.accept_suggestion(selected);
            }
            KeyCode::Esc => {
                self.dismissed = Some((s.row, s.start));
                self.suggest = None;
            }
            _ => return false,
        }
        true
    }

    /// Opens, updates or closes the link suggestions for where the cursor
    /// is now.
    fn refresh_suggest(&mut self) {
        let typing = self
            .view()
            .filter(|_| self.focus == Focus::Editor && self.prompt.is_none())
            .filter(|v| matches!(v.mode, ViewMode::Edit) && v.editor.selection().is_none())
            .and_then(|v| {
                let (row, col) = (v.editor.row, v.editor.col);
                link_suggest::context(&v.editor.lines[row], col)
                    .map(|(start, name)| (row, start, name))
            });
        let Some((row, start, name)) = typing else {
            self.suggest = None;
            self.refresh_plugin_suggest();
            return;
        };
        self.plugin_suggest = None;
        if self.dismissed == Some((row, start)) {
            self.suggest = None;
            return;
        }
        let mut s = match self.suggest.take() {
            Some(s) if (s.row, s.start) == (row, start) => s,
            _ => Suggest {
                row,
                start,
                switcher: Switcher {
                    input: name.clone(),
                    ..Switcher::default()
                },
            },
        };
        if s.switcher.input != name || s.switcher.results.is_empty() {
            s.switcher.input = name;
            s.switcher.update(&self.vault);
            // Not the note being written.
            let this = self.view().and_then(|v| v.path.as_deref());
            s.switcher
                .results
                .retain(|&i| Some(self.vault.notes[i].path.as_path()) != this);
            let most = if s.switcher.input.trim().is_empty() {
                link_suggest::RECENT
            } else {
                link_suggest::MATCHES
            };
            s.switcher.results.truncate(most);
        }
        self.suggest = Some(s);
    }

    /// A plugin's suggestions for where the cursor is (no link being
    /// typed).
    fn refresh_plugin_suggest(&mut self) {
        let at = self
            .view()
            .filter(|_| self.focus == Focus::Editor && self.prompt.is_none())
            .filter(|v| matches!(v.mode, ViewMode::Edit) && v.editor.selection().is_none())
            .map(|v| {
                (
                    v.editor.row,
                    v.editor.col,
                    v.editor.lines[v.editor.row].clone(),
                )
            });
        let found = at.and_then(|(row, col, line)| {
            // Tags and properties first, then the plugins'.
            let lines = &self.view()?.editor.lines;
            let now = chrono::Local::now().naive_local();
            let s = match crate::tag_suggest::suggestions(&self.vault, lines, row, col)
                .or_else(|| crate::nldates::suggestions(&line, col, &self.dates, now))
                .or_else(|| crate::highlights::suggestions(&line, col))
            {
                Some(s) => (None, s),
                None => {
                    let (id, s) = self.plugins.borrow().suggestions(&line, col, &self.vault)?;
                    (Some(id), s)
                }
            };
            Some((row, s))
        });
        let Some((row, (plugin, s))) = found.filter(|(_, (_, s))| !s.items.is_empty()) else {
            self.plugin_suggest = None;
            self.dismissed = None;
            return;
        };
        if self.dismissed == Some((row, s.start)) {
            self.plugin_suggest = None;
            return;
        }
        let selected = match &self.plugin_suggest {
            Some(p) if (p.row, p.start) == (row, s.start) => p.selected.min(s.items.len() - 1),
            _ => 0,
        };
        self.plugin_suggest = Some(link_suggest::PluginSuggest {
            plugin,
            row,
            start: s.start,
            items: s.items,
            effects: s.effects,
            alts: s.alts,
            selected,
        });
    }

    /// Puts the highlighted plugin suggestion in place of what was typed.
    fn accept_plugin_suggestion(&mut self, alt: bool) {
        let Some(p) = self.plugin_suggest.take() else {
            return;
        };
        let Some((_, text)) = p.items.get(p.selected) else {
            return;
        };
        // Shift+Enter: the item's other text, if it has one.
        let text = match p.alts.iter().find(|(i, _)| alt && *i == p.selected) {
            Some((_, other)) => other,
            None => text,
        };
        let Some(view) = self.tabs.get_mut(self.active) else {
            return;
        };
        if p.start < view.editor.col {
            view.editor.anchor = Some((p.row, p.start));
        }
        view.handle_paste(text, &mut self.shared);
        view.editor.anchor = None;
        // What choosing it does besides (a block id in the linked note).
        if let Some(id) = p.plugin
            && let Some((_, effect)) = p.effects.into_iter().find(|(i, _)| *i == p.selected)
        {
            self.apply_effect(id, "suggestion", effect);
        }
        self.refresh_plugin_suggest();
    }

    /// Replaces the typed name with suggestion `i`'s link and puts the
    /// cursor after the closing `]]`.
    fn accept_suggestion(&mut self, i: usize) {
        let Some(s) = self.suggest.take() else {
            return;
        };
        let Some(&note) = s.switcher.results.get(i) else {
            return;
        };
        let target = match s.switcher.aliases.get(&note) {
            // Found by an alias: the link shows it.
            Some(alias) => format!("{}|{alias}", self.link_target(note)),
            None => self.link_target(note),
        };
        let Some(view) = self.tabs.get_mut(self.active) else {
            return;
        };
        let press = |view: &mut EditorView, shared: &mut Shared, code: KeyCode| {
            view.handle_key(KeyEvent::new(code, KeyModifiers::NONE), shared);
        };
        for _ in s.switcher.input.graphemes(true) {
            press(view, &mut self.shared, KeyCode::Backspace);
        }
        view.handle_paste(&target, &mut self.shared);
        let line = &view.editor.lines[view.editor.row];
        let after: String = line.chars().skip(view.editor.col).take(2).collect();
        if after == "]]" {
            press(view, &mut self.shared, KeyCode::Right);
            press(view, &mut self.shared, KeyCode::Right);
        } else {
            view.handle_paste("]]", &mut self.shared);
        }
    }

    /// How a link names note `i`: its name, or its path if another note
    /// has the same name.
    fn link_target(&self, i: usize) -> String {
        let note = &self.vault.notes[i];
        let name = note.name();
        let shared = self
            .vault
            .notes
            .iter()
            .filter(|n| n.name().eq_ignore_ascii_case(&name))
            .count();
        if shared > 1 { note.rel_name() } else { name }
    }

    /// A key some command has (outside prompts): runs it. The editor's own
    /// commands on their own keys go on to where keys go (the sidebar's
    /// search box, or the editor), so nothing changes for them.
    fn host_key(&mut self, key: KeyEvent) -> Option<Action> {
        let chord = Chord::of(&key);
        let Some(id) = self.keymap.command_for(&chord).map(str::to_string) else {
            // An editor key no command has now: only the editor loses it
            // ([`App::editor_key`]); the sidebar, suggestions and plugins
            // still get it (Tab).
            if self.keymap.is_freed(&chord) && !self.editor_default(&chord) {
                self.message = format!("{chord} has no command now (Settings, Keyboard shortcuts)");
                return Some(Action::Continue);
            }
            return None;
        };
        let command = self.all_commands().into_iter().find(|c| c.id == id)?;
        match command.action {
            // With no note open, Ctrl+T (make the line a task) adds a quick
            // task to today's note.
            commands::Action::Key(_)
                if id == "toggle-task"
                    && self.view().is_none()
                    && self.plugins.borrow().is_enabled("tasks") =>
            {
                self.call_plugin("tasks", "quick-task", None);
                Some(Action::Continue)
            }
            commands::Action::Key(native) if Chord::of(&native) == chord => None,
            // Without a selection, Ctrl+X is the editor's (close the tab).
            commands::Action::Host(Host::Extract) => {
                let extracted = self.focus == Focus::Editor && self.start_extract();
                extracted.then_some(Action::Continue)
            }
            action => Some(self.run_command(action)),
        }
    }

    /// The name to show for the note at `path`: a plugin's (its title),
    /// else its name. Safe while drawing.
    pub fn shown_name(&self, path: &Path) -> String {
        self.plugins
            .try_borrow()
            .ok()
            .and_then(|p| p.display_name(path))
            .unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
    }

    /// A tab's title: [`App::shown_name`] for a note, else its page's.
    pub fn tab_name(&self, view: &EditorView) -> String {
        match view.path.as_deref() {
            Some(p) if crate::vault::is_note(p) => self.shown_name(p),
            _ => tab_title(view),
        }
    }

    /// Whether `chord` is the default key of one of the editor's own
    /// commands (it sends the editor that key).
    fn editor_default(&self, chord: &Chord) -> bool {
        commands::builtin().iter().any(|c| {
            matches!(c.action, commands::Action::Key(native) if Chord::of(&native) == *chord)
                && c.defaults.contains(chord)
        })
    }

    /// The active note and cursor, as a place in the history.
    fn place(&self) -> Option<Place> {
        let view = self.view()?;
        Some(Place {
            path: view.path.clone()?,
            row: view.editor.row,
            col: view.editor.col,
        })
    }

    /// Records a move to another note in the history (W-28): the note left
    /// goes on the back list and the forward list is cleared; on the same
    /// note, the place's cursor is kept up to date. Runs around every key,
    /// click and paste, so every way of changing notes is covered.
    fn track(&mut self) {
        let Some(now) = self.place() else {
            return;
        };
        if self.here.as_ref().is_none_or(|here| here.path != now.path) {
            // Plugins hear of every note visited (Recent Files).
            self.plugins
                .borrow_mut()
                .note_opened(&now.path, &self.vault);
        }
        match self.here.take() {
            Some(here) if here.path != now.path => {
                self.back.push(here);
                if self.back.len() > HISTORY {
                    self.back.remove(0);
                }
                self.forward.clear();
            }
            _ => {}
        }
        self.here = Some(now);
    }

    /// Back (or forward) to the note you were on before (after), where the
    /// cursor was; a closed note opens again.
    fn go(&mut self, forward: bool) {
        self.track();
        // Notes deleted since (and not open in a tab) are skipped.
        let mut skipped = Vec::new();
        let place = loop {
            let next = if forward {
                self.forward.pop()
            } else {
                self.back.pop()
            };
            let Some(place) = next else { break None };
            let open = self
                .tabs
                .iter()
                .any(|t| t.path.as_deref() == Some(place.path.as_path()));
            if open || place.path.is_file() {
                break Some(place);
            }
            let name = place.path.file_name().unwrap_or_default();
            skipped.push(name.to_string_lossy().into_owned());
        };
        if !skipped.is_empty() {
            self.message = format!("Skipped {} (no longer there)", skipped.join(", "));
        }
        let Some(place) = place else {
            if skipped.is_empty() {
                self.message = if forward {
                    "Nothing to go forward to"
                } else {
                    "Nothing to go back to"
                }
                .into();
            }
            return;
        };
        if let Some(here) = self.here.take() {
            if forward {
                self.back.push(here);
            } else {
                self.forward.push(here);
            }
        }
        if !self.show(&place.path) {
            return;
        }
        let view = self.view_mut().expect("the note is open");
        let row = place.row.min(view.editor.lines.len().saturating_sub(1));
        view.editor.row = row;
        view.editor.col = place.col.min(view.editor.lines[row].chars().count());
        view.editor.snap_col();
        self.here = self.place();
    }

    /// Tab `n` (1 is the first), like a browser's Alt+1 … Alt+9.
    fn go_to_tab(&mut self, n: usize) {
        if (1..=self.tabs.len()).contains(&n) {
            self.active = n - 1;
            self.focus = Focus::Editor;
        }
    }

    fn step_tab(&mut self, forward: bool) {
        let n = self.tabs.len();
        if n == 0 {
            return;
        }
        self.active = if forward {
            (self.active + 1) % n
        } else {
            (self.active + n - 1) % n
        };
        self.focus = Focus::Editor;
    }

    /// Ctrl+B: the next open pane, in screen order: the sidebar, the
    /// plugin panel under it (the calendar), the note, the backlinks under
    /// it; then the sidebar again.
    fn toggle_focus(&mut self) {
        let has_note = !self.tabs.is_empty();
        let open: Vec<Focus> = [
            (Focus::Sidebar, true),
            (Focus::Panel, self.panel),
            (Focus::Pane, self.areas.pane.is_some()),
            (Focus::Editor, has_note),
            (Focus::Backlinks, self.backlinks && has_note),
        ]
        .into_iter()
        .filter_map(|(focus, open)| open.then_some(focus))
        .collect();
        let at = open.iter().position(|f| *f == self.focus).unwrap_or(0);
        self.focus = open[(at + 1) % open.len()];
        if matches!(self.focus, Focus::Sidebar | Focus::Panel) {
            self.sidebar.visible = true;
        }
    }

    fn focus_search(&mut self) {
        self.sidebar.show(Panel::Search);
        self.focus = Focus::Sidebar;
    }

    fn new_note_prompt(&mut self) {
        self.prompt = Some(Prompt::NewNote {
            folder: self.sidebar.target_folder(&self.vault),
            input: String::new(),
        });
    }

    fn open_switcher(&mut self) {
        self.prompt = Some(Prompt::Switcher(Switcher::new(&self.vault)));
    }

    /// Every command: blackglass's and the editor's, the enabled plugins'
    /// and their settings screens, with the keys they have now.
    pub fn all_commands(&self) -> Vec<Command> {
        let mut all = commands::builtin();
        let quit = all.pop().expect("Quit is the last command");
        let plugin = |id: String, name: String, action| Command {
            id,
            name,
            defaults: Vec::new(),
            key: String::new(),
            action,
        };
        for (id, name, command) in self.plugins.borrow().commands() {
            all.push(Command {
                defaults: Chord::parse_list(command.keys).unwrap_or_default(),
                ..plugin(
                    format!("plugin:{id}:{}", command.id),
                    format!("{name}: {}", command.name),
                    commands::Action::Plugin(id, command.id),
                )
            });
        }
        let plugins = self.plugins.borrow();
        for status in plugins.statuses() {
            let m = status.manifest;
            let has = plugins
                .index(m.id)
                .is_some_and(|i| !plugins.settings(i).is_empty());
            if status.installed && has {
                all.push(plugin(
                    format!("plugin:{}:settings", m.id),
                    format!("{}: Settings", m.name),
                    commands::Action::PluginSettings(m.id),
                ));
            }
        }
        all.push(quit);
        for command in &mut all {
            command.key = keymap::describe(self.keymap.keys(&command.id));
        }
        all
    }

    /// The command palette, with the commands that can run now: those for
    /// the active note only when there is one.
    fn open_palette(&mut self) {
        let has_note = !self.tabs.is_empty();
        let all = self
            .all_commands()
            .into_iter()
            .filter(|c| has_note || !c.needs_note())
            // It's open already.
            .filter(|c| c.action != commands::Action::Host(Host::CommandPalette))
            .collect();
        self.prompt = Some(Prompt::Palette(Palette::new(all)));
    }

    /// Runs a command from the palette.
    pub fn run_command(&mut self, action: commands::Action) -> Action {
        use commands::Action as Run;
        match action {
            Run::Host(host) => return self.run_host(host),
            Run::Key(_) | Run::Mode(_) if self.tabs.is_empty() => {
                self.message = "Open a note first".into();
            }
            Run::Key(key) => {
                self.focus = Focus::Editor;
                self.editor_key(key);
            }
            Run::Insert { text, back } => self.insert(&text, back),
            Run::InsertDate => self.date_command(commands::Dates::Today),
            Run::Dates(command) => self.date_command(command),
            Run::Highlight(color) => self.highlight(color),
            Run::Plugin(id, command) => self.call_plugin(id, command, None),
            Run::Mode(_) if self.view().is_some_and(is_help) => {
                self.message = HELP_READ_ONLY.into();
            }
            Run::Mode(mode) => {
                let view = self.tabs.get_mut(self.active).expect("a tab is open");
                self.focus = Focus::Editor;
                match mode {
                    commands::Mode::View => view.enter_reading(),
                    commands::Mode::Edit | commands::Mode::Source => {
                        view.reading = false;
                        view.source_mode = mode == commands::Mode::Source;
                    }
                }
            }
            Run::PluginSettings(id) => {
                let i = self.plugins.borrow().index(id);
                if let Some(i) = i {
                    self.open_plugin_page(i);
                }
            }
        }
        Action::Continue
    }

    /// Runs a plugin's command (`answers`: `None`), or gives it the
    /// answers to its questions, then does what it asks.
    pub(crate) fn call_plugin(
        &mut self,
        id: &'static str,
        command: &'static str,
        answers: Option<&[Answer]>,
    ) {
        let effect = self.with_context(|ctx| match answers {
            None => self.plugins.borrow_mut().run(id, command, ctx),
            Some(answers) => self.plugins.borrow_mut().answer(id, command, answers, ctx),
        });
        self.apply_effect(id, command, effect);
        if command == crate::plugins::NOTE_CREATED {
            self.add_pending_id();
        }
    }

    /// Runs `f` with what plugins see: the vault, the active note and the
    /// folder new notes go to.
    pub fn with_context<R>(&self, f: impl FnOnce(&Context) -> R) -> R {
        self.with_context_named(None, f)
    }

    /// [`App::with_context`] with the new note's name already typed.
    fn with_context_named<R>(&self, name: Option<&str>, f: impl FnOnce(&Context) -> R) -> R {
        let view = self.tabs.get(self.active);
        // Plugins read lines ending in `\n` (a `\r\n` note's too).
        let text = view.map(|v| {
            let text = v.editor.to_text();
            if v.editor.crlf {
                text.replace("\r\n", "\n")
            } else {
                text
            }
        });
        let selection = view.and_then(|v| v.editor.selected_text());
        let selected = view.and_then(|v| v.editor.selection());
        let action = view.and_then(|v| v.read_action(&self.shared));
        let folder = self.sidebar.target_folder(&self.vault);
        let ctx = Context {
            vault: &self.vault,
            note: view.zip(text.as_deref()).map(|(v, text)| ActiveNote {
                path: v.path.as_deref(),
                text,
                selection: selection.as_deref(),
                selected,
                row: v.editor.row,
                col: v.editor.col,
                action: action.as_deref(),
            }),
            folder: &folder,
            name,
            query: &self.sidebar.query,
        };
        f(&ctx)
    }

    /// The plugin panel's id and lines, `width` columns wide (`None`: no
    /// enabled plugin has one).
    pub fn panel_lines(
        &self,
        width: u16,
    ) -> Option<(&'static str, Vec<ratatui::text::Line<'static>>)> {
        self.with_context(|ctx| self.plugins.borrow().panel(ctx, width))
    }

    /// Alt+C: shows the panel (the calendar) with the focus, or hides it.
    fn toggle_panel(&mut self) {
        if self.panel {
            self.panel = false;
            if self.focus == Focus::Panel {
                self.focus = if self.tabs.is_empty() {
                    Focus::Sidebar
                } else {
                    Focus::Editor
                };
            }
            return;
        }
        if self.panel_lines(30).is_none() {
            self.message = "The calendar comes with the Periodic Notes plugin: install it (Ctrl+P, Open plugins)".into();
            return;
        }
        self.panel = true;
        self.sidebar.visible = true;
        self.focus = Focus::Panel;
    }

    /// Runs the plugins' timers and collects their background work; true
    /// if anything happened (the screen needs drawing again).
    pub fn tick(&mut self) -> bool {
        self.save_session();
        self.sync_unsaved();
        let outside = self.outside_changes();
        let effects = self.with_context(|ctx| self.plugins.borrow_mut().tick(ctx));
        let waiting = self.daily_line.is_some();
        let happened = outside || !effects.is_empty();
        for (id, effect) in effects {
            match effect {
                // Not through `apply_effect`: a tick mustn't end an extract.
                Effect::Message(m) => self.message = m,
                Effect::FilesChanged(m) => {
                    self.files_changed();
                    self.message = m;
                }
                Effect::Redraw => {}
                Effect::Margin { path, width, lines } => self.set_margin(&path, width, lines),
                effect => self.apply_effect(id, "tick", effect),
            }
        }
        self.flush_daily_line();
        happened || (waiting && self.daily_line.is_none())
    }

    /// Takes in what the watcher saw changed outside blackglass: the vault
    /// is scanned again and open notes without unsaved changes reloaded;
    /// a note with unsaved changes is kept, with a warning. Changes the
    /// vault knows already (its own saves, notes it made, moved or deleted)
    /// are skipped. Whether something changed.
    fn outside_changes(&mut self) -> bool {
        use crate::watch::Change;
        let known = |app: &App, path: &Path| {
            app.vault
                .rel(path)
                .is_some_and(|rel| app.vault.files.iter().any(|f| f == rel))
        };
        let real: Vec<Change> = self
            .watcher
            .poll()
            .into_iter()
            .filter(|change| match change {
                Change::Added(p) => !known(self, p),
                Change::Removed(p) => known(self, p),
                Change::Modified(p) => match self.vault.note(p) {
                    Some(note) => std::fs::read_to_string(p)
                        .is_ok_and(|disk| crate::vault::split_lines(&disk) != note.lines),
                    // An attachment: only notes are read.
                    None => false,
                },
            })
            .collect();
        if real.is_empty() {
            return false;
        }
        let unsaved: Vec<String> = self
            .tabs
            .iter()
            .filter(|t| t.is_dirty())
            .filter_map(|t| t.path.as_deref())
            .filter(|p| real.iter().any(|c| c.path() == *p))
            .map(file_name)
            .collect();
        self.files_changed();
        let mut names: Vec<String> = real.iter().map(|c| file_name(c.path())).collect();
        names.dedup();
        let more = names.len().saturating_sub(3);
        names.truncate(3);
        let mut shown = names.join(", ");
        if more > 0 {
            shown.push_str(&format!(" and {more} more"));
        }
        self.message = if unsaved.is_empty() {
            format!("Changed outside blackglass: {shown}")
        } else {
            format!(
                "Changed outside blackglass: {shown}; {} has unsaved changes here (kept; save to overwrite)",
                unsaved.join(", ")
            )
        };
        true
    }

    /// Sets the margin beside the note at `path` (in its tabs).
    fn set_margin(
        &mut self,
        path: &Path,
        width: u16,
        lines: Vec<(usize, ratatui::text::Line<'static>)>,
    ) {
        for view in self
            .tabs
            .iter_mut()
            .filter(|v| v.path.as_deref() == Some(path))
        {
            view.margin_width = width;
            view.margin = lines.iter().cloned().collect();
        }
    }

    /// The plugins' status bar lines, side by side.
    pub fn plugin_status(&self) -> String {
        self.plugins.borrow().status_lines().join("  ")
    }

    /// Files changed on disk (a pull): scan again, and show the new text in
    /// the tabs that have no unsaved changes.
    fn files_changed(&mut self) {
        self.rescan();
        let changed: Vec<PathBuf> = self
            .tabs
            .iter()
            .filter(|t| !t.is_dirty())
            .filter_map(|t| {
                let path = t.path.clone()?;
                let disk = std::fs::read_to_string(&path).ok()?;
                (disk != t.editor.to_text()).then_some(path)
            })
            .collect();
        self.reload_tabs(&changed);
    }

    /// Brings the sidebar's plugin tabs up to date (the enabled plugins'),
    /// and the shown one's row count; a tab whose plugin went away gives
    /// way to the file explorer.
    pub fn refresh_plugin_tabs(&mut self) {
        self.sidebar.plugin_tabs = self.plugins.borrow().sidebar_tabs();
        match self.sidebar.panel {
            Panel::Plugin(id) if self.sidebar.plugin_tabs.iter().any(|t| t.0 == id) => {
                self.sidebar.plugin_rows = self.sidebar_plugin_rows().len();
                let last = self.sidebar.plugin_rows.saturating_sub(1);
                self.sidebar.plugin_cursor = self.sidebar.plugin_cursor.min(last);
            }
            Panel::Plugin(_) => self.sidebar.panel = Panel::Files,
            _ => {}
        }
    }

    /// The shown plugin tab's rows.
    pub fn sidebar_plugin_rows(&self) -> Vec<crate::plugins::SidebarRow> {
        let Panel::Plugin(id) = self.sidebar.panel else {
            return Vec::new();
        };
        self.with_context(|ctx| self.plugins.borrow().sidebar_rows(id, ctx))
    }

    /// Opens the vault at `path` in place of this one (not while a note has
    /// unsaved changes): its notes, plugins and settings; no tabs.
    pub fn switch_vault(&mut self, path: &Path) {
        if self.tabs.iter().any(EditorView::is_dirty) {
            self.message = "Save or close the notes with unsaved changes first".into();
            return;
        }
        let vault = match Vault::open(path) {
            Ok(vault) => vault,
            Err(e) => {
                self.message = format!("Cannot open {}: {e}", path.display());
                return;
            }
        };
        self.save_session();
        self.watcher = crate::watch::Watcher::start(&vault.root, crate::watch::INTERVAL);
        self.dates = crate::nldates::Settings::load(&vault);
        self.vault = Rc::new(vault);
        self.plugins = Rc::new(RefCell::new(Plugins::for_vault(&self.vault)));
        self.plugins.borrow_mut().queries.set_vault(&self.vault);
        self.shared.processor = Some(Box::new(Blocks(Rc::clone(&self.plugins))));
        self.shared.resolver = Box::new(VaultResolver::new(&self.vault));
        self.sidebar = Sidebar::new();
        self.tabs.clear();
        self.active = 0;
        self.focus = Focus::Sidebar;
        (self.back, self.forward, self.here) = (Vec::new(), Vec::new(), None);
        (self.suggest, self.dismissed, self.extract) = (None, None, None);
        (self.panel, self.backlinks, self.backlinks_cache) = (false, false, None);
        self.plugin_keys();
        self.template_tags();
        self.remember_vault();
        self.session_saved.clear();
        self.restore_session();
        self.message = format!("Opened the vault {}", self.vault.name());
    }

    /// Puts this vault first in the user's recent vaults.
    pub fn remember_vault(&mut self) {
        if let Some(dir) = &self.config_dir
            && let Err(e) = crate::config::remember_vault(dir, &self.vault.root)
        {
            self.message = format!("Cannot save the recent vaults: {e}");
        }
    }

    /// The pane under the note (Alt+L): the active note's backlinks, its
    /// unlinked mentions (both worked out again only when the note or the
    /// vault changed) and its outgoing links (from its text now).
    pub fn link_pane(&mut self) -> Vec<PaneItem> {
        let Some(view) = self.view() else {
            return Vec::new();
        };
        let Some(path) = view.path.clone() else {
            return Vec::new();
        };
        let outgoing = crate::backlinks::outgoing(&view.editor.lines, &path, &self.vault);
        if self.backlinks_cache.as_ref().is_none_or(|c| c.0 != path) {
            let back = crate::backlinks::backlinks(&self.vault, &path);
            let mentions = crate::backlinks::mentions(&self.vault, &path);
            self.backlinks_cache = Some((path, back, mentions));
        }
        let (_, back, mentions) = self.backlinks_cache.as_ref().expect("just set");
        back.iter()
            .cloned()
            .map(PaneItem::Backlink)
            .chain(mentions.iter().cloned().map(PaneItem::Mention))
            .chain(outgoing.into_iter().map(PaneItem::Outgoing))
            .collect()
    }

    /// Alt+L: shows the backlinks under the note, with the focus, or hides
    /// them.
    fn toggle_backlinks(&mut self) {
        if self.backlinks {
            self.backlinks = false;
            if self.focus == Focus::Backlinks {
                self.focus = Focus::Editor;
            }
            return;
        }
        if self.tabs.is_empty() {
            self.message = "Open a note first".into();
            return;
        }
        self.backlinks = true;
        self.backlink = 0;
        self.focus = Focus::Backlinks;
    }

    /// Keys in the pane: ↑/↓ choose, Enter opens (a backlink or mention at
    /// its line, an outgoing link's note, or offers to make a missing one),
    /// `l` links a mention, Esc goes back to the note.
    fn backlinks_key(&mut self, key: KeyEvent) {
        let count = self.link_pane().len();
        match key.code {
            KeyCode::Esc => self.focus = Focus::Editor,
            KeyCode::Up => self.backlink = self.backlink.saturating_sub(1),
            KeyCode::Down => self.backlink = (self.backlink + 1).min(count.saturating_sub(1)),
            KeyCode::Enter => self.open_backlink(self.backlink),
            KeyCode::Char('l') => self.link_mention(self.backlink),
            _ => {}
        }
    }

    /// Opens item `i` of the pane.
    fn open_backlink(&mut self, i: usize) {
        let Some(item) = self.link_pane().get(i).cloned() else {
            return;
        };
        match item {
            PaneItem::Backlink(link) | PaneItem::Mention(link) => {
                self.open_at(&link.note, Some((link.line, 0)), None);
            }
            PaneItem::Outgoing(out) => match out.path {
                Some(path) => {
                    self.open(&path);
                }
                None => self.ask_create_linked(out.name),
            },
        }
        self.backlink = 0;
    }

    /// `l` on an unlinked mention: its word becomes a link (`[[word]]`), in
    /// that note's file (not if it's open with unsaved changes).
    fn link_mention(&mut self, i: usize) {
        self.journal.begin();
        self.link_mention_now(i);
        self.journal.end();
    }

    fn link_mention_now(&mut self, i: usize) {
        let Some(PaneItem::Mention(mention)) = self.link_pane().get(i).cloned() else {
            self.message = "Choose an unlinked mention to link it".into();
            return;
        };
        let Some(note) = self.note_path().and_then(|p| self.vault.note(&p).cloned()) else {
            return;
        };
        if self
            .tabs
            .iter()
            .any(|t| t.path.as_deref() == Some(mention.note.as_path()) && t.is_dirty())
        {
            self.message = format!(
                "{} has unsaved changes: save it first",
                file_name(&mention.note)
            );
            return;
        }
        let mut names = vec![note.name()];
        names.extend(note.aliases.iter().cloned());
        let Ok(text) = std::fs::read_to_string(&mention.note) else {
            return;
        };
        let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
        let Some(line) = lines.get_mut(mention.line) else {
            return;
        };
        let Some((start, end)) = crate::backlinks::mention_at(line, &names) else {
            return;
        };
        let word = line[start..end].to_string();
        line.replace_range(start..end, &format!("[[{word}]]"));
        let text = lines.join("\n");
        self.journal.touch(&mention.note);
        if let Err(e) = mdedit::files::write_atomic(&mention.note, &text) {
            self.message = format!("Cannot link it: {e}");
            return;
        }
        self.saved(&mention.note, &text);
        self.backlinks_cache = None;
        self.reload_tabs(std::slice::from_ref(&mention.note));
        self.message = format!("Linked {word} in {}", file_name(&mention.note));
    }

    /// A key while the panel has the focus: Esc leaves it, the rest are
    /// the plugin's.
    fn panel_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.focus = if self.tabs.is_empty() {
                Focus::Sidebar
            } else {
                Focus::Editor
            };
            return;
        }
        let Some(id) = self
            .areas
            .panel_plugin
            .or_else(|| self.panel_lines(30).map(|(id, _)| id))
        else {
            return;
        };
        let effect = self.with_context(|ctx| self.plugins.borrow_mut().panel_key(id, key, ctx));
        self.apply_effect(id, "panel", effect);
    }

    fn apply_effect(&mut self, id: &'static str, command: &'static str, effect: Effect) {
        // What it does to files can be undone (W-133).
        self.journal.begin();
        self.apply_effect_now(id, command, effect);
        self.journal.end();
    }

    fn apply_effect_now(&mut self, id: &'static str, command: &'static str, effect: Effect) {
        // An extract waits for its note only while it's being made.
        if !matches!(effect, Effect::Ask(_) | Effect::CreateNote { .. }) {
            self.extract = None;
        }
        match effect {
            Effect::None => {}
            Effect::RowAction(action) => self.row_action(&action),
            Effect::FocusPane(true) => self.focus = Focus::Pane,
            Effect::FocusPane(false) => {
                if self.focus == Focus::Pane {
                    self.focus = Focus::Editor;
                }
            }
            Effect::Message(m) => self.message = m,
            Effect::FilesChanged(message) => {
                self.files_changed();
                self.message = message;
            }
            Effect::Redraw => {}
            Effect::ShowText { title, text } => self.show_page(&title, &text),
            Effect::Margin { path, width, lines } => self.set_margin(&path, width, lines),
            Effect::OpenVault(path) => self.switch_vault(&path),
            Effect::GoToLine(line) => {
                if let Some(view) = self.tabs.get_mut(self.active) {
                    let line = line.min(view.editor.lines.len().saturating_sub(1));
                    view.editor.row = line;
                    view.editor.col = 0;
                    view.editor.anchor = None;
                    self.focus = Focus::Editor;
                }
            }
            Effect::ShowSidebarTab => {
                self.refresh_plugin_tabs();
                self.sidebar.show(Panel::Plugin(id));
                self.focus = Focus::Sidebar;
            }
            Effect::Open { path } => {
                if self.open(&path) {
                    self.sidebar.reveal(&path, &self.vault);
                }
            }
            Effect::Insert { text, back } => self.insert(&text, back),
            Effect::Ask(questions) if questions.is_empty() => {
                self.call_plugin(id, command, Some(&[]))
            }
            Effect::Ask(questions) => {
                self.prompt = Some(Prompt::Ask(Ask::new(id, command, questions)))
            }
            Effect::ReplaceNote { text, cursor } => {
                let Some(view) = self.tabs.get_mut(self.active) else {
                    self.message = "Open a note first".into();
                    return;
                };
                self.focus = Focus::Editor;
                // Select everything, then paste over it: one undo step.
                view.handle_key(
                    KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
                    &mut self.shared,
                );
                // The editor ends the file with a line break itself.
                view.handle_paste(text.strip_suffix('\n').unwrap_or(&text), &mut self.shared);
                place_cursor(view, &text, cursor.unwrap_or(0));
            }
            Effect::ReplaceLines {
                from,
                to,
                lines,
                cursor,
            } => {
                let Some(view) = self.tabs.get_mut(self.active) else {
                    self.message = "Open a note first".into();
                    return;
                };
                replace_lines(view, from, to, &lines, &mut self.shared);
                let row = cursor.0.min(view.editor.lines.len().saturating_sub(1));
                view.editor.row = row;
                view.editor.col = cursor.1.min(view.editor.lines[row].chars().count());
                view.editor.snap_col();
                self.focus = Focus::Editor;
            }
            Effect::CopyText(text) => {
                self.message = match (self.copier)(&text) {
                    Ok(()) => format!("Copied {} lines", text.lines().count()),
                    Err(e) => format!("Cannot copy: {e}"),
                };
            }
            Effect::Quit => {
                if self.quit() == Action::Quit {
                    self.quit_requested = true;
                }
            }
            Effect::OpenUrl(url) => {
                self.message = match (self.opener)(Path::new(&url)) {
                    Ok(()) => format!("Opened {url} in the browser"),
                    Err(e) => format!("Cannot open {url}: {e}"),
                };
            }
            Effect::OpenSource(path) => {
                self.track();
                self.show_file(&path);
                self.track();
            }
            Effect::CreateFile { path, text } => {
                self.journal.touch(&path);
                let written = path
                    .parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| mdedit::files::write_atomic(&path, &text));
                match written {
                    Ok(()) => {
                        self.rescan();
                        if self.open(&path) {
                            self.sidebar.reveal(&path, &self.vault);
                            // A template's commands in it run (Templater).
                            if text.contains("<%") && crate::vault::is_note(&path) {
                                self.note_created();
                            }
                        }
                    }
                    Err(e) => self.message = format!("Cannot create the file: {e}"),
                }
            }
            Effect::WriteFile { path, text } => {
                if !path.starts_with(&self.vault.root) {
                    self.message = format!("{} is outside the vault", path.display());
                } else if path.exists() {
                    self.message = format!("{} already exists", file_name(&path));
                } else {
                    self.journal.touch(&path);
                    let written = path
                        .parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|()| mdedit::files::write_atomic(&path, &text));
                    match written {
                        Ok(()) => self.rescan(),
                        Err(e) => self.message = format!("Cannot create the file: {e}"),
                    }
                }
            }
            Effect::MoveNote { from, to } => {
                if let Err(e) = self.move_note_to(&from, &to) {
                    self.message = e;
                }
            }
            Effect::DeleteNote(path) => {
                if let Err(e) = self.delete_note(&path) {
                    self.message = e;
                }
            }
            Effect::Retarget { from, to } => {
                if let Err(e) = self.retarget_links(&from, &to) {
                    self.message = e;
                }
            }
            Effect::Search(query) => {
                self.sidebar.search(&query, &self.vault);
                self.focus = Focus::Sidebar;
            }
            Effect::Reveal(path) => {
                if path.exists() {
                    self.sidebar.reveal(&path, &self.vault);
                    self.sidebar.show(Panel::Files);
                    self.sidebar.selected = Some(path);
                    self.focus = Focus::Sidebar;
                } else {
                    self.message = format!("{} isn't in the vault any more", file_name(&path));
                }
            }
            Effect::Many(effects) => {
                for effect in effects {
                    self.apply_effect(id, command, effect);
                }
            }
            Effect::AddToDaily { heading, line } => self.add_to_daily(heading, line),
            Effect::AddToNote {
                path,
                day,
                place,
                line,
                new_text,
                open,
                link,
            } => self.add_to_note(path, day, place, line, new_text, (link, open)),
            Effect::EditNote {
                path,
                from,
                to,
                lines,
                expect,
            } => {
                if let Err(e) = self.edit_note(&path, from, to, &lines, &expect) {
                    self.message = e;
                }
            }
            Effect::CreateNote {
                folder,
                name,
                text,
                cursor,
            } => {
                // A template's text was rendered already: its commands don't
                // run again when the note is made.
                self.rendered = matches!(id, "templater" | "periodic-notes");
                // Periodic notes are named by their date already.
                let made = if id == "periodic-notes" {
                    self.create_note_with(&folder, &name, &text)
                } else {
                    self.create_new_note(&folder, &name, &text)
                };
                self.rendered = false;
                match made {
                    Ok(path) => {
                        let view = self.view_mut().expect("the new note is open");
                        // Properties put before the text move the cursor on.
                        let now = view.editor.to_text();
                        let added = now.chars().count().saturating_sub(text.chars().count());
                        place_cursor(view, &now, cursor.unwrap_or(0) + added);
                        if let Some(extract) = self.extract.take() {
                            self.fill_extract(&extract, cursor.is_some());
                            self.finish_extract(&extract, &path);
                        }
                    }
                    Err(e) => {
                        self.extract = None;
                        self.message = e;
                    }
                }
            }
        }
    }

    /// Ctrl+X with text selected: opens the new-note window for the
    /// extract. False (nothing happens) without a selection.
    fn start_extract(&mut self) -> bool {
        let Some(view) = self.view().filter(|v| !v.reading) else {
            return false;
        };
        let (Some(text), Some(source)) = (view.editor.selected_text(), view.path.clone()) else {
            return false;
        };
        if text.is_empty() {
            return false;
        }
        self.extract = Some(Extract { source, text });
        self.new_note_prompt();
        true
    }

    /// Puts the extracted text in the new note made from a template: at
    /// its cursor if the template set one, else after its text; then
    /// saves it.
    fn fill_extract(&mut self, extract: &Extract, at_cursor: bool) {
        let view = self
            .tabs
            .get_mut(self.active)
            .expect("the new note is open");
        if !at_cursor {
            let last = view.editor.lines.len() - 1;
            view.editor.row = last;
            view.editor.col = view.editor.lines[last].chars().count();
        }
        let blank_end = view.editor.lines[view.editor.row].trim().is_empty();
        let text = if at_cursor || blank_end {
            extract.text.clone()
        } else {
            format!("\n\n{}", extract.text)
        };
        view.handle_paste(&text, &mut self.shared);
        if view.save().is_ok() {
            let (path, text) = (view.path.clone(), view.editor.to_text());
            if let Some(path) = path {
                self.saved(&path, &text);
            }
        }
    }

    /// Replaces the extracted selection in its note with a link to the new
    /// note (one undo step; the note isn't saved). The new note's tab stays
    /// active.
    fn finish_extract(&mut self, extract: &Extract, new: &Path) {
        let target = match self.vault.notes.iter().position(|n| n.path == new) {
            Some(i) => self.link_target(i),
            None => new
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        };
        let Some(source) = self
            .tabs
            .iter_mut()
            .find(|t| t.path.as_deref() == Some(extract.source.as_path()))
        else {
            self.message = "The extract's note was closed: its text is only in the new note".into();
            return;
        };
        source.handle_paste(&format!("[[{target}]]"), &mut self.shared);
        self.message = format!("Extracted to [[{target}]]");
    }

    /// Whether Templater's "create from template" can run.
    /// What plugins show in the live preview, through mdedit's hooks (for
    /// the drawing thread): Templater's tags (`<% … %>`) as written while
    /// it's enabled, link badges (link counts) and rendered spans
    /// (citations).
    fn template_tags(&self) {
        let on = self.plugins.borrow().is_enabled("templater");
        mdedit::markdown::set_verbatim(if on { &[("<%", "%>")] } else { &[] });
        let plugins = Rc::clone(&self.plugins);
        mdedit::markdown::set_link_badge(Some(Rc::new(move |target: &str| {
            plugins.try_borrow().ok()?.link_badge(target)
        })));
        let spans = self.plugins.borrow().rendered_spans();
        let rendered = spans
            .into_iter()
            .map(|(open, close)| {
                let plugins = Rc::clone(&self.plugins);
                let render: mdedit::markdown::HostText =
                    Rc::new(move |inner: &str| plugins.try_borrow().ok()?.render_span(open, inner));
                (open.to_string(), close.to_string(), render)
            })
            .collect();
        mdedit::markdown::set_rendered(rendered);
        let plugins = Rc::clone(&self.plugins);
        mdedit::markdown::set_marks(Some(Rc::new(move |text: &str| {
            plugins
                .try_borrow()
                .map(|p| p.marks(text))
                .unwrap_or_default()
        })));
        let plugins = Rc::clone(&self.plugins);
        mdedit::markdown::set_table_cells(Some(Rc::new(move |lines: &[String]| {
            plugins.try_borrow().ok()?.table_cells(lines)
        })));
        let starts = self.plugins.borrow().hidden_lines();
        mdedit::markdown::set_hidden_lines((!starts.is_empty()).then(|| {
            let hide: mdedit::markdown::HostLines = Rc::new(move |line: &str| {
                let line = line.trim_start();
                starts.iter().any(|s| line.starts_with(s))
            });
            hide
        }));
    }

    fn templater_ready(&self) -> bool {
        self.plugins
            .borrow()
            .commands()
            .iter()
            .any(|(id, _, c)| *id == "templater" && c.id == "create-from-template")
    }

    pub(crate) fn run_host(&mut self, host: Host) -> Action {
        match host {
            Host::GoToNote => self.open_switcher(),
            Host::NewNote => self.new_note_prompt(),
            Host::NewFromTemplate => {
                if self.templater_ready() {
                    self.call_plugin("templater", "create-from-template", None);
                } else {
                    self.message = TEMPLATER_NEEDED.into();
                }
            }
            Host::CloseTab => self.close(self.active),
            Host::NextTab => self.step_tab(true),
            Host::PreviousTab => self.step_tab(false),
            Host::GoToTab(n) => self.go_to_tab(n),
            Host::LastTab => self.go_to_tab(self.tabs.len()),
            Host::SearchVault => self.focus_search(),
            Host::ShowFiles | Host::ShowTags => {
                let panel = if host == Host::ShowFiles {
                    Panel::Files
                } else {
                    Panel::Tags
                };
                self.sidebar.show(panel);
                self.focus = Focus::Sidebar;
            }
            Host::ToggleCalendar => self.toggle_panel(),
            Host::Back => self.go(false),
            Host::Forward => self.go(true),
            Host::ToggleSidebar => {
                self.sidebar.visible = !self.sidebar.visible;
                if !self.sidebar.visible && !self.tabs.is_empty() {
                    self.focus = Focus::Editor;
                }
            }
            Host::SwitchFocus => self.toggle_focus(),
            Host::Rescan => {
                self.rescan();
                self.message = "Vault scanned again".into();
            }
            Host::OpenPlugins => self.open_settings_window(Page::Plugins, true),
            Host::Quit => return self.quit(),
            Host::CommandPalette => self.open_palette(),
            Host::Help => self.open_help(),
            Host::Settings => self.open_settings_window(Page::Editor, false),
            Host::Extract => {
                if !self.start_extract() {
                    self.message = "Select text in a note to extract it".into();
                }
            }
            Host::ToggleBacklinks => self.toggle_backlinks(),
            Host::UniqueNote => self.unique_note(),
            Host::PreviewLink => self.preview_link(),
            Host::PdfToNote => self.ask_pdf(),
            Host::RandomNote => self.random_note(),
            Host::Outline => self.ask_outline(),
            Host::Footnotes => self.ask_footnotes(),
            Host::EditProperties => {
                if self.view().is_some_and(|v| v.path.is_some()) {
                    self.prompt = Some(Prompt::Properties(PropertyWindow {
                        selected: 0,
                        input: None,
                    }));
                } else {
                    self.message = "Open a note first".into();
                }
            }
            Host::ShowProperties | Host::RenameProperty => self.ask_property(host),
            Host::RenameNote => self.ask_rename_note(),
            Host::RenameFolder => self.ask_rename_folder(),
            Host::NewFolder => {
                let folder = self.sidebar.target_folder(&self.vault);
                let shown = self
                    .vault
                    .rel(&folder)
                    .map(|r| format!("{}/", r.display()))
                    .filter(|r| r != "/")
                    .unwrap_or_else(|| format!("{}/", self.vault.name()));
                self.renaming = Some(folder);
                self.ask_text(
                    "new-folder",
                    format!("New folder in {shown}"),
                    String::new(),
                );
            }
            Host::OpenVault => {
                let recent = self
                    .config_dir
                    .as_deref()
                    .map(crate::config::recent_vaults)
                    .unwrap_or_default();
                let picker = VaultPicker::new(recent, crate::config::home());
                self.prompt = Some(Prompt::OpenVault(picker));
            }
            Host::DeleteNote => self.ask_delete(),
            Host::MoveNote => self.ask_move(),
            Host::ChooseTheme => self.ask_theme(),
            Host::UndoFiles => self.undo_files(true),
            Host::RedoFiles => self.undo_files(false),
        }
        Action::Continue
    }

    /// Inserts `text` at the cursor of the active note (as one undo step),
    /// then moves the cursor `back` characters.
    fn insert(&mut self, text: &str, back: usize) {
        let Some(view) = self.tabs.get_mut(self.active) else {
            self.message = "Open a note first".into();
            return;
        };
        self.focus = Focus::Editor;
        view.handle_paste(text, &mut self.shared);
        for _ in 0..back {
            view.handle_key(
                KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                &mut self.shared,
            );
        }
    }

    fn request(&mut self, request: Request) {
        match request {
            Request::None => {}
            Request::Open { path, at, search } => self.open_at(&path, at, search),
            Request::PluginKey { id, row, key } => {
                let effect = self
                    .with_context(|ctx| self.plugins.borrow_mut().sidebar_key(id, row, key, ctx));
                if matches!(effect, Effect::Open { .. }) {
                    // The list changes with the visit: the top is the note.
                    self.sidebar.plugin_cursor = 0;
                }
                // An edit from a sidebar tab (the table controls) keeps
                // the focus there, for the next one.
                let stay = matches!(effect, Effect::ReplaceLines { .. });
                self.apply_effect(id, "sidebar", effect);
                if stay {
                    self.focus = Focus::Sidebar;
                }
                self.refresh_plugin_tabs();
            }
            Request::FocusEditor if !self.tabs.is_empty() => self.focus = Focus::Editor,
            Request::FocusEditor => {}
        }
    }

    fn editor_key(&mut self, key: KeyEvent) {
        // Undo / redo: of changes to other notes when the note has none of
        // its own (a read-only page, or nothing left).
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let undo = ctrl
            && matches!(key.code, KeyCode::Char('z'))
            && !key.modifiers.contains(KeyModifiers::SHIFT);
        let redo = ctrl
            && (matches!(key.code, KeyCode::Char('y'))
                || (matches!(key.code, KeyCode::Char('z' | 'Z'))
                    && key.modifiers.contains(KeyModifiers::SHIFT)));
        if (undo || redo)
            && let Some(view) = self.tabs.get(self.active)
        {
            let own = if undo {
                view.editor.history.can_undo()
            } else {
                view.editor.history.can_redo()
            };
            let others = if undo {
                self.journal.can_undo()
            } else {
                self.journal.can_redo()
            };
            if others && (view.reading || !own) {
                self.undo_files(undo);
                return;
            }
        }
        let Some(view) = self.tabs.get_mut(self.active) else {
            return;
        };
        if is_help(view) && leaves_view_mode(&key) {
            self.message = HELP_READ_ONLY.into();
            return;
        }
        let was_dirty = view.is_dirty();
        let outcome = view.handle_key(key, &mut self.shared);
        if was_dirty
            && !view.is_dirty()
            && let Some(path) = view.path.clone()
        {
            let text = view.editor.to_text();
            self.saved(&path, &text);
        }
        self.outcome(outcome);
    }

    /// Undoes (`back`) or redoes the last change to other notes (W-133);
    /// open notes without unsaved changes show it.
    pub(crate) fn undo_files(&mut self, back: bool) {
        match self.journal.step(back) {
            Ok((paths, message)) => {
                self.files_changed();
                for p in paths
                    .iter()
                    .filter(|p| p.extension().is_some_and(|e| e == "base"))
                {
                    self.refresh_base_page(p);
                }
                self.message = message;
            }
            Err(e) => self.message = e,
        }
    }

    /// [`Effect::EditNote`]: lines of a note, open or not, if they still
    /// read `expect`.
    /// Puts `line` in today's daily note under `heading` ([`Effect::AddToDaily`]),
    /// making the note first if it isn't there: through Periodic Notes
    /// (its folder, name and template; it may take a moment, then
    /// [`App::tick`] finishes), else as `YYYY-MM-DD.md` in the vault.
    fn add_to_daily(&mut self, heading: String, line: String) {
        let place = LinePlace::Heading {
            name: heading,
            first: false,
            make_at_top: false,
        };
        let today = chrono::Local::now().date_naive();
        self.add_to_note(None, today, place, line, String::new(), (false, false));
    }

    /// Puts `line` in the note at `path` (`None`: `day`'s daily note) at
    /// `place` ([`Effect::AddToNote`]), making the note first if it isn't
    /// there; `(link, open)`: a link to it at the cursor first, the note
    /// opened after.
    fn add_to_note(
        &mut self,
        path: Option<PathBuf>,
        day: chrono::NaiveDate,
        place: LinePlace,
        line: String,
        new_text: String,
        (link, open): (bool, bool),
    ) {
        let daily = path
            .is_none()
            .then(|| self.plugins.borrow().daily_note(day))
            .flatten();
        let path = path.unwrap_or_else(|| {
            let rel = daily
                .clone()
                .unwrap_or_else(|| day.format("%Y-%m-%d").to_string());
            self.vault.root.join(format!("{rel}.md"))
        });
        let Ok(rel) = path.strip_prefix(&self.vault.root).map(Path::to_path_buf) else {
            self.message = format!("{} isn't in the vault", path.display());
            return;
        };
        if link {
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            self.insert(&format!("[[{name}]]"), 0);
        }
        self.daily_line = Some(PendingLine {
            path: path.clone(),
            place,
            line,
            open,
        });
        let is_open = self.tabs.iter().any(|v| v.path.as_deref() == Some(&path));
        if !path.is_file() && !is_open {
            if daily.is_some() {
                let day = day.format("%Y-%m-%d");
                self.row_action(&format!("plugin:periodic-notes:open-day:{day}"));
            } else {
                let name = rel.with_extension("").to_string_lossy().replace('\\', "/");
                let root = self.vault.root.clone();
                if let Err(e) = self.create_note_with(&root, &name, &new_text) {
                    self.message = e;
                    self.daily_line = None;
                    return;
                }
            }
        }
        self.flush_daily_line();
    }

    /// The line waiting for its note goes in once the note is there.
    fn flush_daily_line(&mut self) {
        let Some(PendingLine { path, .. }) = &self.daily_line else {
            return;
        };
        let tab = self
            .tabs
            .iter()
            .position(|v| v.path.as_deref() == Some(path));
        let lines = match tab {
            Some(i) => self.tabs[i].editor.lines.clone(),
            None => match std::fs::read_to_string(path) {
                Ok(text) => crate::vault::split_lines(&text),
                // Not made yet (a template still fetching): at a tick.
                Err(_) => return,
            },
        };
        let PendingLine {
            path,
            place,
            line,
            open,
        } = self.daily_line.take().expect("checked above");
        let (at, mut added, after) = note_spot(&lines, &place);
        added.extend(line.lines().map(String::from));
        added.extend(after);
        let name = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // An empty note's one empty line is replaced.
        let empty = lines.len() == 1 && lines[0].is_empty();
        let (to, expect) = if empty {
            (1, &lines[..])
        } else {
            (at, &[][..])
        };
        match self.edit_note(&path, at, to, &added, expect) {
            Ok(()) => {
                self.message = format!(
                    "Added to {name}: {}",
                    line.lines().next().unwrap_or_default()
                )
            }
            Err(e) => self.message = e,
        }
        if open && self.open(&path) {
            self.sidebar.reveal(&path, &self.vault);
        }
    }

    fn edit_note(
        &mut self,
        path: &Path,
        from: usize,
        to: usize,
        lines: &[String],
        expect: &[String],
    ) -> Result<(), String> {
        const CHANGED: &str = "The note changed since it was read; try again";
        let tab = self.tabs.iter().position(|v| {
            v.path
                .as_deref()
                .is_some_and(|p| p == path || same_file(p, path))
        });
        if let Some(i) = tab {
            let view = &mut self.tabs[i];
            if view.editor.lines.get(from..to) != Some(expect) {
                return Err(CHANGED.into());
            }
            let was_dirty = view.is_dirty();
            let (row, col) = (view.editor.row, view.editor.col);
            replace_lines(view, from, to, lines, &mut self.shared);
            // The cursor stays where it was, below moved lines.
            let row = if row >= to {
                (row + lines.len()).saturating_sub(to - from)
            } else {
                row
            };
            view.editor.row = row.min(view.editor.lines.len().saturating_sub(1));
            view.editor.col = col;
            view.editor.snap_col();
            if !was_dirty {
                self.journal.touch(path);
                let view = &mut self.tabs[i];
                view.handle_key(
                    KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                    &mut self.shared,
                );
                let text = view.editor.to_text();
                self.saved(path, &text);
            }
            return Ok(());
        }
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("Cannot read the note: {e}"))?;
        let mut all = crate::vault::split_lines(&text);
        if all.get(from..to) != Some(expect) {
            return Err(CHANGED.into());
        }
        all.splice(from..to, lines.iter().cloned());
        // The note's own line ends (`\r\n` from Windows).
        let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut text = all.join(ending);
        if !text.is_empty() {
            text.push_str(ending);
        }
        self.journal.touch(path);
        mdedit::files::write_atomic(path, &text)
            .map_err(|e| format!("Cannot save the note: {e}"))?;
        self.saved(path, &text);
        Ok(())
    }

    /// Offers an editor key to the plugins (Advanced Tables' Tab); true if
    /// one took it.
    fn plugin_editor_key(&mut self, key: KeyEvent) -> bool {
        // A `.base` page (read-only) too: its board's keys.
        let editable = self.view().is_some_and(|v| {
            !v.reading
                || v.editor
                    .lines
                    .first()
                    .is_some_and(|l| l.starts_with("%% base:"))
        });
        if !editable || !self.plugins.borrow().takes_key(&key) {
            return false;
        }
        let taken = self.with_context(|ctx| self.plugins.borrow_mut().editor_key(key, ctx));
        let Some((id, effect)) = taken else {
            return false;
        };
        self.apply_effect(id, "editor-key", effect);
        true
    }

    /// Does what the editor asked for (after a key or a click).
    fn outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::OpenLink { path, .. } if !crate::vault::is_note(&path) => {
                // An image or another file: the desktop's viewer opens it.
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.message = match (self.opener)(&path) {
                    Ok(()) => format!("Opened {name} in the system viewer"),
                    Err(e) => format!("Cannot open {name}: {e}"),
                };
            }
            Outcome::OpenLink { path, heading } => {
                if self.open(&path)
                    && let Some(heading) = heading
                {
                    self.view_mut()
                        .expect("a tab was just opened")
                        .go_to_heading(&heading);
                }
            }
            Outcome::Action(action) => self.row_action(&action),
            // A web link: the desktop's browser opens it.
            Outcome::OpenUrl(url) => {
                self.message = match (self.opener)(Path::new(&url)) {
                    Ok(()) => format!("Opened {url} in the browser"),
                    Err(e) => format!("Cannot open {url}: {e}"),
                };
            }
            Outcome::MissingLink { target, .. } => self.ask_create_linked(target),
            Outcome::RequestOpen => self.open_switcher(),
            Outcome::RequestSaveAs if self.view().is_some_and(is_help) => {
                self.message = HELP_READ_ONLY.into();
            }
            Outcome::RequestSaveAs => {
                let input = self
                    .view()
                    .and_then(|v| v.path.as_deref())
                    .and_then(|p| self.vault.rel(p))
                    .map(|rel| {
                        let rel = rel.to_string_lossy();
                        rel.strip_suffix(".md").unwrap_or(&rel).to_string()
                    })
                    .unwrap_or_default();
                self.prompt = Some(Prompt::SaveAs { input });
            }
            Outcome::RequestClose => self.close(self.active),
            _ => {}
        }
    }

    /// A result row's action (a plugin's code block, view mode or a
    /// click): `open:<path>` opens a note, `line:<n>:<path>` opens it at a
    /// line. Paths are relative to the vault.
    fn row_action(&mut self, action: &str) {
        if let Some((id, payload)) = action
            .strip_prefix("plugin:")
            .and_then(|r| r.split_once(':'))
        {
            let id = id.to_string();
            let effect =
                self.with_context(|ctx| self.plugins.borrow_mut().row_action(&id, payload, ctx));
            let id = self
                .plugins
                .borrow()
                .statuses()
                .iter()
                .map(|s| s.manifest.id)
                .find(|i| *i == id)
                .unwrap_or("blackglass");
            self.apply_effect(id, "row-action", effect);
            return;
        }
        // A calendar's day without notes: its daily note (Periodic Notes
        // asks and uses its template), or a note named after it.
        if let Some(day) = action.strip_prefix("day:") {
            if self.plugins.borrow().is_enabled("periodic-notes") {
                self.row_action(&format!("plugin:periodic-notes:day:{day}"));
                return;
            }
            let question = Question::Choose {
                prompt: format!("Create {day}?"),
                items: vec![format!("Create {day}.md"), "Don't create it".into()],
            };
            self.creating_day = Some(day.to_string());
            self.prompt = Some(Prompt::Ask(Ask::new(HOST, "create-day", vec![question])));
            return;
        }
        let (line, rel) = if let Some(rel) = action.strip_prefix("open:") {
            (None, rel)
        } else if let Some((n, rel)) = action.strip_prefix("line:").and_then(|r| r.split_once(':'))
        {
            (n.parse::<usize>().ok(), rel)
        } else {
            self.message = format!("Unknown action {action}");
            return;
        };
        let path = self.vault.root.join(rel);
        if !path.is_file() {
            self.message = format!("{rel} doesn't exist");
            return;
        }
        self.open_at(&path, line.map(|l| (l, 0)), None);
    }

    fn prompt_key(&mut self, mut prompt: Prompt, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if let Prompt::Settings(window) = prompt {
            self.settings_window_key(window, key);
            return Action::Continue;
        }
        if let Prompt::Properties(window) = prompt {
            self.properties_key(window, key);
            return Action::Continue;
        }
        if let Prompt::OpenVault(mut picker) = prompt {
            match picker.key(key) {
                Picked::Open(path) => self.switch_vault(&path),
                Picked::Cancel => {}
                Picked::None => self.prompt = Some(Prompt::OpenVault(picker)),
            }
            return Action::Continue;
        }
        if key.code == KeyCode::Esc {
            self.extract = None;
            if matches!(&prompt, Prompt::Ask(a) if a.plugin == HOST && a.command == "theme") {
                // Back to the theme before the preview.
                let theme = self.theme.clone();
                self.use_theme(&theme);
            }
            return Action::Continue;
        }
        match &mut prompt {
            Prompt::Unsaved { tab, then } => return self.unsaved_key(*tab, *then, key),
            Prompt::Switcher(s) => match key.code {
                KeyCode::Up => s.selected = s.selected.saturating_sub(1),
                KeyCode::Down => {
                    s.selected = (s.selected + 1).min(s.rows(&self.vault).saturating_sub(1));
                }
                KeyCode::Enter => {
                    let s = s.clone();
                    self.switch_to(&s, s.selected);
                    return Action::Continue;
                }
                KeyCode::Backspace => {
                    s.input.pop();
                    s.update(&self.vault);
                }
                KeyCode::Char(c) if !ctrl => {
                    s.input.push(c);
                    s.update(&self.vault);
                }
                _ => {}
            },
            Prompt::Palette(p) => match key.code {
                KeyCode::Up => p.selected = p.selected.saturating_sub(1),
                KeyCode::Down => {
                    p.selected = (p.selected + 1).min(p.results.len().saturating_sub(1))
                }
                KeyCode::Enter => {
                    let Some(command) = p.chosen() else {
                        return Action::Continue;
                    };
                    let action = command.action.clone();
                    return self.run_command(action);
                }
                KeyCode::Backspace => {
                    p.input.pop();
                    p.update();
                }
                KeyCode::Char(c) if !ctrl => {
                    p.input.push(c);
                    p.update();
                }
                _ => {}
            },
            Prompt::Ask(ask) => {
                // A form's fields take their keys (Enter and Esc aside).
                if matches!(ask.current(), Question::Form { .. }) && ask.form_key(key) {
                    self.prompt = Some(prompt);
                    return Action::Continue;
                }
                // A text's buttons: ←→ / Tab choose one.
                if let Question::Show { buttons, .. } = ask.current() {
                    let n = buttons.len().max(1);
                    match key.code {
                        KeyCode::Left | KeyCode::Up | KeyCode::BackTab => {
                            ask.selected = (ask.selected + n - 1) % n;
                        }
                        KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                            ask.selected = (ask.selected + 1) % n;
                        }
                        KeyCode::Enter | KeyCode::Esc => {}
                        _ => {
                            self.prompt = Some(prompt);
                            return Action::Continue;
                        }
                    }
                    if !matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                        self.prompt = Some(prompt);
                        return Action::Continue;
                    }
                }
                match key.code {
                    KeyCode::Up => ask.selected = ask.selected.saturating_sub(1),
                    KeyCode::Down => {
                        ask.selected = (ask.selected + 1).min(ask.results.len().saturating_sub(1));
                    }
                    // A new line in a text of several lines.
                    KeyCode::Enter
                        if matches!(ask.current(), Question::Lines { .. })
                            && key
                                .modifiers
                                .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
                    {
                        ask.input.push('\n');
                    }
                    // Mark (or unmark) an item of a choice of many.
                    KeyCode::Tab if matches!(ask.current(), Question::Many { .. }) => {
                        if let Some(&i) = ask.results.get(ask.selected) {
                            match ask.marked.iter().position(|&m| m == i) {
                                Some(at) => {
                                    ask.marked.remove(at);
                                }
                                None => ask.marked.push(i),
                            }
                        }
                    }
                    KeyCode::Enter => {
                        let Some(answer) = ask.answer() else {
                            self.prompt = Some(prompt);
                            return Action::Continue;
                        };
                        ask.answers.push(answer);
                        if ask.answers.len() < ask.questions.len() {
                            ask.input.clear();
                            ask.marked.clear();
                            ask.update();
                        } else {
                            let (id, command, answers) =
                                (ask.plugin, ask.command, ask.answers.clone());
                            if id == HOST {
                                self.host_answer(command, &answers);
                            } else {
                                self.call_plugin(id, command, Some(&answers));
                            }
                            return Action::Continue;
                        }
                    }
                    KeyCode::Backspace => {
                        ask.input.pop();
                        ask.update();
                    }
                    KeyCode::Char('u') if ctrl => {
                        ask.input.clear();
                        ask.update();
                    }
                    KeyCode::Char(c) if !ctrl => {
                        ask.input.push(c);
                        ask.update();
                    }
                    _ => {}
                }
                if ask.plugin == HOST && ask.command == "theme" {
                    // The highlighted theme, at once.
                    if let Some(Answer::Choice(i)) = ask.answer()
                        && let Some((_, text)) = self.themes.get(i)
                    {
                        self.set_palette(theme::Palette::parse(text).0);
                    }
                }
            }
            Prompt::Settings(_) | Prompt::OpenVault(_) | Prompt::Properties(_) => {
                unreachable!("settings and vault picker keys are handled first")
            }
            Prompt::NewNote { input, .. } if key.code == KeyCode::Tab => {
                // From a template: Templater's create command, with this name.
                if self.templater_ready() {
                    let name = input.clone();
                    let effect = self.with_context_named(Some(&name), |ctx| {
                        self.plugins
                            .borrow_mut()
                            .run("templater", "create-from-template", ctx)
                    });
                    self.apply_effect("templater", "create-from-template", effect);
                    return Action::Continue;
                }
                self.message = TEMPLATER_NEEDED.into();
            }
            Prompt::NewNote { input, .. } | Prompt::SaveAs { input } | Prompt::Date { input } => {
                match key.code {
                    KeyCode::Enter => {
                        self.submit(prompt);
                        return Action::Continue;
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Char(c) if !ctrl => input.push(c),
                    _ => {}
                }
            }
        }
        self.prompt = Some(prompt);
        Action::Continue
    }

    /// A settings page's title, settings and their values.
    pub fn settings_of(&self, target: Target) -> (String, Vec<Setting>, Values) {
        match target {
            Target::Editor => (
                "Editor".into(),
                crate::config::editor_settings(),
                self.editor_values.clone(),
            ),
            Target::Notes => (
                "Notes".into(),
                crate::note_ids::NoteIds::settings(),
                crate::note_ids::values(&self.vault),
            ),
            Target::Dates => (
                "Dates".into(),
                crate::nldates::Settings::schema(),
                crate::nldates::Settings::values(&self.vault),
            ),
            Target::Window => (
                "Window".into(),
                crate::window_settings::Settings::schema(),
                crate::window_settings::Settings::values(self.config_dir.as_deref()),
            ),
            Target::Plugin(i) => {
                let plugins = self.plugins.borrow();
                let name = plugins.manifest(i).map_or("", |m| m.name);
                (
                    name.to_string(),
                    plugins.settings(i),
                    plugins.values(i, &self.vault),
                )
            }
        }
    }

    /// Saves a setting and puts it to use.
    pub(crate) fn set_setting(
        &mut self,
        target: Target,
        setting: &Setting,
        value: &str,
    ) -> Result<(), String> {
        match target {
            Target::Notes => {
                let mut values = crate::note_ids::values(&self.vault);
                values.set("", &setting.key, value);
                let path = self.vault.root.join(crate::note_ids::FILE);
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)
                        .map_err(|e| format!("Cannot save notes.toml: {e}"))?;
                }
                let text = values.to_text(&crate::note_ids::NoteIds::settings());
                mdedit::files::write_atomic(&path, &text)
                    .map_err(|e| format!("Cannot save notes.toml: {e}"))
            }
            Target::Window => {
                use crate::window_settings::{FILE, Settings};
                let Some(dir) = self.config_dir.clone() else {
                    return Err("No config folder to save the window's settings in".into());
                };
                let mut values = Settings::values(Some(&dir));
                values.set("", &setting.key, value);
                std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot save {FILE}: {e}"))?;
                mdedit::files::write_atomic(&dir.join(FILE), &values.to_text(&Settings::schema()))
                    .map_err(|e| format!("Cannot save {FILE}: {e}"))?;
                self.window = Settings::load(Some(&dir));
                Ok(())
            }
            Target::Dates => {
                let mut values = crate::nldates::Settings::values(&self.vault);
                values.set("", &setting.key, value);
                let path = self.vault.root.join(crate::nldates::FILE);
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)
                        .map_err(|e| format!("Cannot save dates.toml: {e}"))?;
                }
                let text = values.to_text(&crate::nldates::Settings::schema());
                mdedit::files::write_atomic(&path, &text)
                    .map_err(|e| format!("Cannot save dates.toml: {e}"))?;
                self.dates = crate::nldates::Settings::load(&self.vault);
                Ok(())
            }
            Target::Plugin(i) => {
                let result = self
                    .plugins
                    .borrow_mut()
                    .set(i, setting, value, &self.vault);
                // A setting can turn a command on (e.g. daily notes).
                self.plugin_keys();
                self.template_tags();
                result
            }
            Target::Editor => {
                let line = crate::config::editor_text_line(&setting.key, value);
                let (_, warnings) = mdedit::config::Config::parse(&line);
                if let Some(warning) = warnings.into_iter().next() {
                    return Err(warning.replace("config line 1: ", ""));
                }
                let mut values = self.editor_values.clone();
                values.set("", &setting.key, value);
                let text = crate::config::editor_text(&values);
                self.use_editor_config(&text);
                self.save_config("config.toml", &text)
            }
        }
    }

    /// Uses the editor settings in `text` (a `config.toml`); returns its
    /// warnings.
    pub fn use_editor_config(&mut self, text: &str) -> Vec<String> {
        let (config, warnings) = mdedit::config::Config::parse(text);
        self.shared.config = mdedit::config::Config {
            // The editor shares its rows with the sidebar (see App::new).
            heading_size: self.shared.config.heading_size,
            ..config
        };
        self.editor_values = Values::parse(text);
        self.set_palette(self.palette);
        warnings
    }

    /// Reads the user's keyboard shortcuts (`keys.toml` in the config
    /// folder); problems go in the status bar.
    pub fn load_user_config(&mut self) {
        let Some(dir) = self.config_dir.clone() else {
            return;
        };
        self.window = crate::window_settings::Settings::load(Some(&dir));
        if let Some(id) = theme::saved(&dir.join(APPEARANCE_FILE)) {
            let warnings = self.use_theme(&id);
            self.theme = id;
            if !warnings.is_empty() {
                self.message = format!("Theme: {}", warnings.join("; "));
            }
        }
        match std::fs::read_to_string(dir.join(KEYS_FILE)) {
            Ok(text) => {
                let warnings = self.keymap.apply(&text);
                self.plugin_keys();
                self.template_tags();
                if !warnings.is_empty() {
                    self.message = warnings.join("; ");
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => self.message = format!("Cannot read {KEYS_FILE}: {e}"),
        }
    }

    /// Writes `name` in the user's config folder (nothing without one).
    fn save_config(&self, name: &str, text: &str) -> Result<(), String> {
        let Some(dir) = &self.config_dir else {
            return Ok(());
        };
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot save {name}: {e}"))?;
        mdedit::files::write_atomic(&dir.join(name), text)
            .map_err(|e| format!("Cannot save {name}: {e}"))
    }

    /// Gives command `id` the keys `keys` (`None`: its defaults), taking
    /// them from other commands, and saves the shortcuts.
    pub(crate) fn bind(&mut self, id: &str, name: &str, keys: Option<Vec<Chord>>, all: &[Command]) {
        self.keymap.add(id);
        let moved = match keys {
            Some(keys) => self.keymap.set(id, keys),
            None => self.keymap.reset(id),
        };
        let keys = keymap::describe(self.keymap.keys(id));
        let from: Vec<&str> = moved
            .iter()
            .filter_map(|m| all.iter().find(|c| c.id == *m))
            .map(|c| c.name.as_str())
            .collect();
        self.message = match (keys.is_empty(), from.is_empty()) {
            (true, _) => format!("{name} has no key now"),
            (false, true) => format!("{name}: {keys}"),
            (false, false) => format!("{name}: {keys} (moved from {})", from.join(", ")),
        };
        if let Err(e) = self.save_config(KEYS_FILE, &self.keymap.to_text()) {
            self.message = e;
        }
    }

    /// The help page: the embedded text with each `{command-id}` replaced
    /// by the command's keys now, then every command and its keys.
    fn help_text(&self) -> String {
        let commands = self.all_commands();
        let shown = |c: &Command| {
            if c.key.is_empty() {
                "no key".to_string()
            } else {
                c.key.clone()
            }
        };
        let mut text = HELP.trim_end().to_string();
        for c in &commands {
            text = text.replace(&format!("{{{}}}", c.id), &shown(c));
        }
        text.push_str("\n\n## Keyboard shortcuts\n\n");
        text.push_str(&format!(
            "Every command, with its keys now. Change them in Settings ({}), Keyboard shortcuts.\n\n",
            keymap::describe(self.keymap.keys("settings"))
        ));
        text.push_str("| Command | Keys |\n| --- | --- |\n");
        for c in &commands {
            let keys = c.key.replace('|', "\\|");
            text.push_str(&format!("| {} | {keys} |\n", c.name));
        }
        text
    }

    /// Opens the help page in a read-only tab (again: shows it, updated).
    fn open_help(&mut self) {
        self.show_page(HELP_TITLE, &self.help_text());
    }

    /// Shows `text` in a read-only tab called `title` (a page without a
    /// file: help, a diff); a page of that title is replaced.
    pub fn show_page(&mut self, title: &str, text: &str) {
        let mut view = EditorView::new(text, None);
        view.name = Some(title.to_string());
        view.enter_reading();
        view.status.clear();
        let same = |v: &EditorView| is_help(v) && v.name.as_deref() == Some(title);
        match self.tabs.iter().position(same) {
            Some(i) => {
                self.tabs[i] = view;
                self.active = i;
            }
            None => {
                let at = if self.tabs.is_empty() {
                    0
                } else {
                    self.active + 1
                };
                self.tabs.insert(at, view);
                self.active = at;
            }
        }
        self.focus = Focus::Editor;
    }

    /// The active note's file (not the help page's).
    fn note_path(&self) -> Option<PathBuf> {
        self.view().and_then(|v| v.path.clone())
    }

    /// The vault's properties, each with how many notes have it (most
    /// first), for "Show properties" (then searched) or "Rename property"
    /// (then its new name is asked).
    fn ask_property(&mut self, host: Host) {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for note in &self.vault.notes {
            for (key, _) in &note.properties {
                match counts.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((key.clone(), 1)),
                }
            }
        }
        if counts.is_empty() {
            self.message = "No note has properties yet".into();
            return;
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let items = counts
            .iter()
            .map(|(k, n)| format!("{k}  {n} note{}", if *n == 1 { "" } else { "s" }))
            .collect();
        self.property_names = counts.into_iter().map(|(k, _)| k).collect();
        let (command, questions) = if host == Host::ShowProperties {
            (
                "show-properties",
                vec![Question::Choose {
                    prompt: "Properties (Enter finds the notes)".into(),
                    items,
                }],
            )
        } else {
            (
                "rename-property",
                vec![
                    Question::Choose {
                        prompt: "Rename property".into(),
                        items,
                    },
                    Question::Text {
                        prompt: "Its new name".into(),
                        default: String::new(),
                    },
                ],
            )
        };
        self.prompt = Some(Prompt::Ask(Ask::new(HOST, command, questions)));
    }

    /// Renames the property `old` to `new` in every note's frontmatter
    /// (notes open with unsaved changes are left out, and named).
    fn rename_property(&mut self, old: &str, new: &str) {
        self.journal.begin();
        self.rename_property_now(old, new);
        self.journal.end();
    }

    fn rename_property_now(&mut self, old: &str, new: &str) {
        if new.is_empty() || new.contains(':') {
            self.message = "A property's name can't be empty or have a colon".into();
            return;
        }
        let (mut changed, mut skipped) = (Vec::new(), Vec::new());
        for note in &self.vault.notes {
            if !note
                .properties
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case(old))
            {
                continue;
            }
            if self
                .tabs
                .iter()
                .any(|t| t.path.as_deref() == Some(note.path.as_path()) && t.is_dirty())
            {
                skipped.push(note.name());
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&note.path) else {
                continue;
            };
            let text = rename_key(&text, old, new);
            self.journal.touch(&note.path);
            match mdedit::files::write_atomic(&note.path, &text) {
                Ok(()) => changed.push(note.path.clone()),
                Err(e) => skipped.push(format!("{} ({e})", note.name())),
            }
        }
        self.reload_tabs(&changed);
        self.rescan();
        let n = changed.len();
        self.message = format!(
            "Renamed the property {old} to {new} in {n} note{}",
            if n == 1 { "" } else { "s" }
        );
        if !skipped.is_empty() {
            self.message.push_str(&format!(
                "; not in (unsaved changes): {}",
                skipped.join(", ")
            ));
        }
    }

    /// Opens a note chosen at random (not the open one).
    fn random_note(&mut self) {
        let here = self.note_path();
        let others: Vec<PathBuf> = self
            .vault
            .notes
            .iter()
            .filter(|n| Some(&n.path) != here.as_ref())
            .map(|n| n.path.clone())
            .collect();
        if others.is_empty() {
            self.message = "No other note to open".into();
            return;
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos() as usize);
        let path = others[nanos % others.len()].clone();
        self.open(&path);
    }

    /// Alt+O: the note's headings (indented by level) to jump to.
    /// "Choose theme": every theme, the current one highlighted; moving
    /// the highlight previews it.
    fn ask_theme(&mut self) {
        self.themes = theme::themes(self.config_dir.as_deref());
        let items = self
            .themes
            .iter()
            .map(|(id, text)| theme::name(id, text))
            .collect();
        let mut ask = Ask::new(
            HOST,
            "theme",
            vec![Question::Choose {
                prompt: "Choose theme".into(),
                items,
            }],
        );
        ask.selected = self
            .themes
            .iter()
            .position(|(id, _)| *id == self.theme)
            .unwrap_or(0);
        self.prompt = Some(Prompt::Ask(ask));
    }

    /// Uses theme `id` (the user's file or a built-in; the default if
    /// there's none); returns its file's problems.
    fn use_theme(&mut self, id: &str) -> Vec<String> {
        let Some((palette, warnings)) = theme::load(self.config_dir.as_deref(), id) else {
            self.set_palette(theme::Palette::default());
            return vec![format!("no theme {id:?}")];
        };
        self.set_palette(palette);
        warnings
    }

    /// Paints with `palette`: blackglass's colors, and the editor's
    /// headings where the editor settings don't set them.
    fn set_palette(&mut self, palette: theme::Palette) {
        self.palette = palette;
        // The editor's colors, and the plugins' results.
        self.shared.palette = palette.editor;
        theme::set_current(palette);
        self.plugins.borrow_mut().theme_changed();
        for (level, color) in palette.headings.iter().enumerate() {
            let key = format!("heading{}_color", level + 1);
            if self.editor_values.get("", &key).is_none() {
                self.shared.config.heading_colors[level] = *color;
            }
        }
    }

    fn ask_outline(&mut self) {
        let Some(view) = self.view() else {
            self.message = "Open a note first".into();
            return;
        };
        let mut rows = Vec::new();
        let mut items = Vec::new();
        let mut fence = false;
        for (i, line) in view.editor.lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                fence = !fence;
                continue;
            }
            let level = line.chars().take_while(|&c| c == '#').count();
            if fence || !(1..=6).contains(&level) || !line[level..].starts_with(' ') {
                continue;
            }
            rows.push(i);
            items.push(format!(
                "{}{}",
                "  ".repeat(level - 1),
                line[level..].trim()
            ));
        }
        if items.is_empty() {
            self.message = "This note has no headings".into();
            return;
        }
        self.outline = rows;
        let question = Question::Choose {
            prompt: "Outline".into(),
            items,
        };
        self.prompt = Some(Prompt::Ask(Ask::new(HOST, "outline", vec![question])));
    }

    /// The open note's properties (from its text now).
    pub fn note_properties(&self) -> Vec<crate::properties::Property> {
        let Some(view) = self.view() else {
            return Vec::new();
        };
        crate::properties::parse(&view.editor.to_text()).0
    }

    /// Writes `props` as the open note's frontmatter: one undo step, the
    /// cursor staying on its text.
    fn set_properties(&mut self, props: &[crate::properties::Property]) {
        let Some(view) = self.tabs.get_mut(self.active) else {
            return;
        };
        let text = view.editor.to_text();
        let (_, body) = crate::properties::parse(&text);
        let old_rows = text[..text.len() - body.len()].matches('\n').count();
        let front = crate::properties::write(props, "");
        let new_rows = front.matches('\n').count();
        let (row, col) = (view.editor.row, view.editor.col);
        // Select the old frontmatter and paste the new one over it.
        view.editor.anchor = Some((0, 0));
        view.editor.row = old_rows.min(view.editor.lines.len() - 1);
        view.editor.col = 0;
        if front.is_empty() {
            if old_rows > 0 {
                view.handle_key(
                    KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
                    &mut self.shared,
                );
            }
        } else {
            view.handle_paste(&front, &mut self.shared);
        }
        view.editor.anchor = None;
        let row = if row >= old_rows {
            row + new_rows - old_rows
        } else {
            row.min(new_rows.saturating_sub(1))
        };
        view.editor.row = row.min(view.editor.lines.len() - 1);
        view.editor.col = col.min(view.editor.lines[view.editor.row].chars().count());
        view.status.clear();
    }

    /// Keys in the property editor: ↑/↓ choose, Enter (Space) edits the
    /// value or switches a checkbox, `a` adds one (`name: value`), `r`
    /// renames it, Delete removes it, Esc closes (or cancels a typing).
    fn properties_key(&mut self, mut w: PropertyWindow, key: KeyEvent) {
        use crate::properties::{Property, Value};
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let mut props = self.note_properties();
        if let Some((what, input)) = &mut w.input {
            match key.code {
                KeyCode::Esc => w.input = None,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char('u') if ctrl => input.clear(),
                KeyCode::Char(c) if !ctrl => input.push(c),
                KeyCode::Enter => {
                    let text = input.trim().to_string();
                    match what {
                        PropertyInput::Value => {
                            if let Some(p) = props.get_mut(w.selected) {
                                p.value = p.value.edited(&text);
                            }
                        }
                        PropertyInput::Rename if !text.is_empty() && !text.contains(':') => {
                            if let Some(p) = props.get_mut(w.selected) {
                                p.key = text;
                            }
                        }
                        PropertyInput::Add => {
                            let (name, value) = text.split_once(':').unwrap_or((&text, ""));
                            let name = name.trim();
                            if !name.is_empty() {
                                props.push(Property {
                                    key: name.to_string(),
                                    value: Value::guess(value),
                                });
                                w.selected = props.len() - 1;
                            }
                        }
                        PropertyInput::Rename => {}
                    }
                    w.input = None;
                    self.set_properties(&props);
                }
                _ => {}
            }
            self.prompt = Some(Prompt::Properties(w));
            return;
        }
        match key.code {
            KeyCode::Esc => return,
            KeyCode::Up => w.selected = w.selected.saturating_sub(1),
            KeyCode::Down => w.selected = (w.selected + 1).min(props.len().saturating_sub(1)),
            KeyCode::Home => w.selected = 0,
            KeyCode::End => w.selected = props.len().saturating_sub(1),
            KeyCode::Char('a') => w.input = Some((PropertyInput::Add, String::new())),
            KeyCode::Enter | KeyCode::Char(' ') => match props.get_mut(w.selected) {
                Some(Property {
                    value: Value::Checkbox(b),
                    ..
                }) => {
                    *b = !*b;
                    self.set_properties(&props);
                }
                Some(Property {
                    value: Value::Nested(_),
                    ..
                }) => self.message = "Nested properties are edited in the note".into(),
                Some(p) => w.input = Some((PropertyInput::Value, p.value.edit_text())),
                None => {}
            },
            KeyCode::Char('r') => {
                if let Some(p) = props.get(w.selected) {
                    w.input = Some((PropertyInput::Rename, p.key.clone()));
                }
            }
            KeyCode::Delete if w.selected < props.len() => {
                props.remove(w.selected);
                w.selected = w.selected.min(props.len().saturating_sub(1));
                self.set_properties(&props);
            }
            _ => {}
        }
        self.prompt = Some(Prompt::Properties(w));
    }

    /// The note's footnote definitions (`[^label]: text`) to jump to.
    fn ask_footnotes(&mut self) {
        let Some(view) = self.view() else {
            self.message = "Open a note first".into();
            return;
        };
        let (mut rows, mut items) = (Vec::new(), Vec::new());
        for (i, line) in view.editor.lines.iter().enumerate() {
            let Some(rest) = line.trim_start().strip_prefix("[^") else {
                continue;
            };
            if let Some((label, text)) = rest.split_once("]:") {
                rows.push(i);
                items.push(format!("[{label}] {}", text.trim()));
            }
        }
        if items.is_empty() {
            self.message = "This note has no footnotes".into();
            return;
        }
        self.outline = rows;
        let question = Question::Choose {
            prompt: "Footnotes".into(),
            items,
        };
        self.prompt = Some(Prompt::Ask(Ask::new(HOST, "footnotes", vec![question])));
    }

    /// Asks which of the vault's PDFs to turn into a note.
    fn ask_pdf(&mut self) {
        let mut pdfs: Vec<PathBuf> = self
            .vault
            .files
            .iter()
            .filter(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
            .cloned()
            .collect();
        pdfs.sort();
        if pdfs.is_empty() {
            self.message = "The vault has no PDF".into();
            return;
        }
        let items = pdfs
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        self.pdfs = pdfs;
        let question = Question::Choose {
            prompt: "PDF to note".into(),
            items,
        };
        self.prompt = Some(Prompt::Ask(Ask::new(HOST, "pdf-to-note", vec![question])));
    }

    /// Writes the text of the PDF at `rel` into a new note beside it
    /// (`Name`, else `Name (PDF text)` …), linking back to the PDF.
    fn pdf_to_note(&mut self, rel: &Path) -> Result<(), String> {
        let pdf = self.vault.root.join(rel);
        let text = crate::pdf::text(&pdf)?;
        let folder = pdf.parent().expect("a file is in a folder").to_path_buf();
        let stem = pdf
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let name = [stem.clone(), format!("{stem} (PDF text)")]
            .into_iter()
            .chain((2..).map(|n| format!("{stem} (PDF text {n})")))
            .find(|n| !folder.join(format!("{n}.md")).exists())
            .expect("some name is free");
        let link = rel.to_string_lossy().replace('\\', "/");
        let body = if text.is_empty() {
            "(no text: the PDF may be scanned images)".to_string()
        } else {
            text
        };
        let note = format!("# {stem}\n\nFrom ![[{link}]]\n\n{body}\n");
        self.create_note_with(&folder, &name, &note)?;
        self.message = format!("{link}'s text is in {name}");
        Ok(())
    }

    /// Reads the notes at `paths` again into their tabs (those without
    /// unsaved changes), keeping the cursor.
    fn reload_tabs(&mut self, paths: &[PathBuf]) {
        for view in &mut self.tabs {
            let Some(path) = view.path.clone() else {
                continue;
            };
            if paths.contains(&path)
                && !view.is_dirty()
                && let Ok(mut fresh) = EditorView::open(path)
            {
                let row = view.editor.row.min(fresh.editor.lines.len() - 1);
                fresh.editor.row = row;
                fresh.editor.col = view.editor.col.min(fresh.editor.lines[row].chars().count());
                fresh.status.clear();
                *view = fresh;
            }
        }
    }

    /// Asks one line of text for blackglass's own `command`, starting
    /// from `input`.
    fn ask_text(&mut self, command: &'static str, prompt: String, input: String) {
        let question = Question::Text {
            prompt,
            default: String::new(),
        };
        let mut ask = Ask::new(HOST, command, vec![question]);
        ask.input = input;
        self.prompt = Some(Prompt::Ask(ask));
    }

    /// What the file explorer has chosen, while it has the focus.
    fn explorer_choice(&self) -> Option<PathBuf> {
        self.sidebar
            .selected
            .clone()
            .filter(|p| p.exists() && *p != self.vault.root)
            .filter(|_| self.focus == Focus::Sidebar && self.sidebar.panel == Panel::Files)
    }

    /// F2: the new name for the file (a note, an image …) or folder chosen
    /// in the file explorer (while it has the focus), else the open note.
    fn ask_rename_note(&mut self) {
        let chosen = self.explorer_choice();
        if chosen.as_ref().is_some_and(|p| p.is_dir()) {
            self.ask_rename_folder();
            return;
        }
        let Some(path) = chosen.or_else(|| self.note_path()) else {
            self.message = "Open or choose a note first".into();
            return;
        };
        // A note by its name; another file with its extension.
        let stem = if crate::vault::is_note(&path) {
            path.file_stem()
        } else {
            path.file_name()
        }
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
        self.renaming = Some(path);
        self.ask_text("rename-note", format!("Rename {stem}"), stem);
    }

    /// The new name for the folder chosen in the file explorer (a note's
    /// folder if a note is chosen).
    fn ask_rename_folder(&mut self) {
        let folder = self.sidebar.target_folder(&self.vault);
        if folder == self.vault.root {
            self.message = "Choose a folder in the file explorer first".into();
            return;
        }
        let name = file_name(&folder);
        self.renaming = Some(folder);
        self.ask_text("rename-folder", format!("Rename folder {name}"), name);
    }

    /// Renames the note (or other file) at `path` to `name` (in its
    /// folder; `/` makes folders), with the links to it. A file that isn't
    /// a note keeps its extension if `name` has none.
    fn rename_note(&mut self, path: &Path, name: &str) -> Result<(), String> {
        let note = crate::vault::is_note(path);
        let name = name.trim();
        let name = if note {
            name.trim_end_matches(".md")
        } else {
            name
        };
        if name.is_empty() {
            return Ok(());
        }
        let folder = path.parent().expect("a note is in a folder").to_path_buf();
        let ext = path.extension().map(|e| e.to_string_lossy().into_owned());
        let new = match ext {
            Some(ext) if !note && Path::new(name).extension().is_none() => {
                self.vault_path(&folder, &format!("{name}.{ext}"))?
            }
            _ => self.vault_path(&folder, name)?,
        };
        if new == path {
            return Ok(());
        }
        if new.exists() {
            return Err(format!("{name} already exists"));
        }
        let old = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let links = self.apply_moves(vec![(path.to_path_buf(), new.clone())], path, &new)?;
        self.message = format!("Renamed {old} to {name}{links}");
        Ok(())
    }

    /// Moves the note at `from` to `to` (anywhere in the vault, folders
    /// made as needed), with the links to it.
    fn move_note_to(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        if from == to {
            return Ok(());
        }
        let inside = |p: &Path| {
            p.starts_with(&self.vault.root)
                && p.strip_prefix(&self.vault.root)
                    .is_ok_and(|rel| rel.components().all(|c| matches!(c, Component::Normal(_))))
        };
        if !inside(from) || !inside(to) {
            return Err(format!(
                "Cannot move {}: outside the vault",
                file_name(from)
            ));
        }
        if !from.exists() {
            return Err(format!("Cannot move {}: no such note", file_name(from)));
        }
        if to.exists() {
            return Err(format!("{} already exists", file_name(to)));
        }
        let links = self.apply_moves(vec![(from.to_path_buf(), to.to_path_buf())], from, to)?;
        let rel = to.strip_prefix(&self.vault.root).unwrap_or(to);
        self.message = format!(
            "Moved {} to {}{links}",
            file_name(from),
            crate::vault::slash(rel)
        );
        Ok(())
    }

    /// Renames the folder at `dir` to `name` (beside it), with the links to
    /// the notes in it.
    fn rename_folder(&mut self, dir: &Path, name: &str) -> Result<(), String> {
        let name = name.trim().trim_matches('/');
        if name.is_empty() {
            return Ok(());
        }
        if name.contains('/') || name == ".." || name == "." {
            return Err("A folder's new name is one name (no /)".into());
        }
        let new = dir
            .parent()
            .expect("a folder in the vault has a parent")
            .join(name);
        if new.exists() {
            return Err(format!("{name} already exists"));
        }
        let moves = self.files_under(dir, &new);
        let links = self.apply_moves(moves, dir, &new)?;
        self.message = format!("Renamed the folder {} to {name}{links}", file_name(dir));
        Ok(())
    }

    /// Every file (notes, attachments) in the folder `dir`, with where it
    /// goes when the folder becomes `new`.
    fn files_under(&self, dir: &Path, new: &Path) -> Vec<(PathBuf, PathBuf)> {
        self.vault
            .files
            .iter()
            .map(|rel| self.vault.root.join(rel))
            .filter_map(|path| {
                let inside = path.strip_prefix(dir).ok()?.to_path_buf();
                Some((path, new.join(inside)))
            })
            .collect()
    }

    /// Makes the folder `name` in `parent` and shows it in the explorer.
    fn new_folder(&mut self, parent: &Path, name: &str) -> Result<(), String> {
        let name = name.trim().trim_matches('/');
        if name.is_empty() {
            return Ok(());
        }
        let rel = Path::new(name);
        if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err("A folder can't leave the vault".into());
        }
        let dir = parent.join(rel);
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot make {name}: {e}"))?;
        self.rescan();
        self.sidebar.reveal(&dir, &self.vault);
        self.sidebar.show(Panel::Files);
        self.message = format!("Made the folder {name}");
        Ok(())
    }

    /// Moves `from` (a note or folder) to `to`, and every note in `moves`
    /// with it: the links to them are rewritten in every note (open tabs
    /// too, unless they have unsaved changes), tabs, history and plugins
    /// follow. Returns " (N links in M notes)" for the message.
    fn apply_moves(
        &mut self,
        moves: Vec<(PathBuf, PathBuf)>,
        from: &Path,
        to: &Path,
    ) -> Result<String, String> {
        self.journal.begin();
        let done = self.apply_moves_now(moves, from, to);
        self.journal.end();
        done
    }

    fn apply_moves_now(
        &mut self,
        moves: Vec<(PathBuf, PathBuf)>,
        from: &Path,
        to: &Path,
    ) -> Result<String, String> {
        for (a, b) in &moves {
            self.journal.touch(a);
            self.journal.touch(b);
        }
        let updates = crate::backlinks::link_updates(&self.vault, &moves);
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Cannot rename: {e}"))?;
        }
        std::fs::rename(from, to).map_err(|e| format!("Cannot rename: {e}"))?;
        let moved = |p: &Path| {
            moves
                .iter()
                .find(|(old, _)| old == p)
                .map_or_else(|| p.to_path_buf(), |(_, new)| new.clone())
        };
        let (mut links, mut notes, mut skipped) = (0, 0, Vec::new());
        for update in &updates {
            let path = moved(&update.note);
            let dirty = self
                .tabs
                .iter()
                .any(|t| t.path.as_deref() == Some(update.note.as_path()) && t.is_dirty());
            if dirty {
                skipped.push(file_name(&path));
                continue;
            }
            self.journal.touch(&path);
            if let Err(e) = mdedit::files::write_atomic(&path, &update.text) {
                skipped.push(format!("{} ({e})", file_name(&path)));
                continue;
            }
            links += update.links;
            notes += 1;
        }
        // Tabs follow their files; the updated notes show their new links.
        let updated: Vec<PathBuf> = updates.iter().map(|u| u.note.clone()).collect();
        for view in &mut self.tabs {
            let Some(old) = view.path.clone() else {
                continue;
            };
            let new = moved(&old);
            if updated.contains(&old)
                && !view.is_dirty()
                && let Ok(mut fresh) = EditorView::open(new.clone())
            {
                let row = view.editor.row.min(fresh.editor.lines.len() - 1);
                fresh.editor.row = row;
                fresh.editor.col = view.editor.col.min(fresh.editor.lines[row].chars().count());
                fresh.status.clear();
                *view = fresh;
            }
            view.path = Some(new);
        }
        let places = self
            .back
            .iter_mut()
            .chain(&mut self.forward)
            .chain(&mut self.here);
        for place in places {
            place.path = moved(&place.path);
        }
        for (old, new) in &moves {
            self.plugins.borrow_mut().note_moved(old, new, &self.vault);
        }
        self.rescan();
        self.sidebar.reveal(to, &self.vault);
        let mut note = match (links, notes) {
            (0, _) => String::new(),
            (1, _) => " (1 link)".to_string(),
            (l, 1) => format!(" ({l} links in 1 note)"),
            (l, n) => format!(" ({l} links in {n} notes)"),
        };
        if !skipped.is_empty() {
            note.push_str(&format!(
                "; not updated (unsaved changes): {}",
                skipped.join(", ")
            ));
        }
        Ok(note)
    }

    /// Points every link to the note at `from` at the note at `to` (both
    /// still there; a merge), in every note but `from` without unsaved
    /// changes.
    fn retarget_links(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        let updates =
            crate::backlinks::link_updates(&self.vault, &[(from.to_path_buf(), to.to_path_buf())]);
        let (mut links, mut changed, mut skipped) = (0, Vec::new(), Vec::new());
        for update in updates.iter().filter(|u| u.note != from) {
            let dirty = self
                .tabs
                .iter()
                .any(|t| t.path.as_deref() == Some(update.note.as_path()) && t.is_dirty());
            if dirty {
                skipped.push(file_name(&update.note));
                continue;
            }
            self.journal.touch(&update.note);
            mdedit::files::write_atomic(&update.note, &update.text)
                .map_err(|e| format!("Cannot update {}: {e}", file_name(&update.note)))?;
            links += update.links;
            changed.push(update.note.clone());
        }
        self.reload_tabs(&changed);
        self.rescan();
        self.message = format!(
            "{links} link{} now point to {}",
            if links == 1 { "" } else { "s" },
            file_name(to)
        );
        if !skipped.is_empty() {
            self.message.push_str(&format!(
                "; not updated (unsaved changes): {}",
                skipped.join(", ")
            ));
        }
        Ok(())
    }

    /// Alt+P: the note the link under the cursor points to, in a popup
    /// (read-only, at the link's heading).
    fn preview_link(&mut self) {
        let Some(view) = self.view() else {
            self.message = "Open a note first".into();
            return;
        };
        let line = &view.editor.lines[view.editor.row];
        let link = mdedit::links::link_at(line, view.editor.col);
        let (target, heading) = match link {
            Some(mdedit::links::Link::File { path, heading }) => (path, heading),
            Some(mdedit::links::Link::Web(url)) => {
                self.message = format!("{url} is a web page: Ctrl+Enter opens it");
                return;
            }
            None => {
                self.message = "Put the cursor on a link to preview its note".into();
                return;
            }
        };
        let from = view.path.clone();
        let path = if target.is_empty() {
            from.clone()
        } else {
            self.shared.resolver.resolve(from.as_deref(), &target).ok()
        };
        let Some(path) = path.filter(|p| crate::vault::is_note(p)) else {
            self.message = format!("{target} isn't a note in the vault");
            return;
        };
        match EditorView::open(path) {
            Ok(mut view) => {
                view.enter_reading();
                view.status.clear();
                if let Some(heading) = heading {
                    view.go_to_heading(&heading);
                    view.scroll = view.editor.row;
                    view.enter_reading();
                    view.status.clear();
                }
                self.preview = Some(Box::new(view));
            }
            Err(e) => self.message = e,
        }
    }

    /// Keys in the preview: ↑/↓, PgUp / PgDn scroll, Enter opens the note
    /// (where the preview is), Esc closes it; nothing edits it.
    fn preview_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.preview = None,
            KeyCode::Enter => {
                let preview = self.preview.take().expect("a preview is open");
                if let Some(path) = preview.path.clone() {
                    self.open_at(&path, Some((preview.editor.row, 0)), None);
                }
            }
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                let view = self.preview.as_mut().expect("a preview is open");
                view.handle_key(key, &mut self.shared);
                view.status.clear();
            }
            _ => {}
        }
    }

    /// A new note named by the time (`YYYYMMDDHHmm`, then ` 1`, ` 2` …
    /// within the same minute), in the folder new notes go to.
    fn unique_note(&mut self) {
        let folder = self.sidebar.target_folder(&self.vault);
        // The vault's ID format (`YYYYMMDDHHmm` by default), the next free.
        let ids = crate::note_ids::NoteIds::load(&self.vault);
        let vault = Rc::clone(&self.vault);
        let name = ids.id(chrono::Local::now().naive_local(), |id| {
            crate::note_ids::NoteIds::taken(&vault, id) || folder.join(format!("{id}.md")).exists()
        });
        if let Err(e) = self.create_note(&folder, &name) {
            self.message = e;
        }
    }

    /// A followed link names a note that isn't there: asks whether to make
    /// it (at the top of the vault, or where the link's path says).
    fn ask_create_linked(&mut self, target: String) {
        let name = target.trim().trim_end_matches(".md").to_string();
        if name.is_empty() {
            return;
        }
        let question = Question::Choose {
            prompt: format!("Create {name}?"),
            items: vec![format!("Create {name}.md"), "Don't create it".into()],
        };
        self.linked_note = Some(name);
        self.prompt = Some(Prompt::Ask(Ask::new(
            HOST,
            "create-linked-note",
            vec![question],
        )));
    }

    /// Asks whether to delete the active note.
    fn ask_delete(&mut self) {
        let Some(path) = self.note_path() else {
            self.message = "Open a note first".into();
            return;
        };
        let name = file_name(&path);
        let question = Question::Choose {
            prompt: format!("Delete {name}?"),
            items: vec![
                format!("Delete: move it to the vault's {TRASH}/ folder"),
                "Keep it".into(),
            ],
        };
        self.prompt = Some(Prompt::Ask(Ask::new(HOST, "delete-note", vec![question])));
    }

    /// Asks which folder to move the file or folder chosen in the file
    /// explorer (while it has the focus) to, else the active note.
    fn ask_move(&mut self) {
        let Some(path) = self.explorer_choice().or_else(|| self.note_path()) else {
            self.message = "Open a note first".into();
            return;
        };
        self.moving = Some(path.clone());
        let question = Question::Choose {
            prompt: format!("Move {} to", file_name(&path)),
            items: self.move_targets(&path),
        };
        self.prompt = Some(Prompt::Ask(Ask::new(HOST, "move-note", vec![question])));
    }

    /// The vault's folders (as `Books/`, the top one as `/`), without the
    /// one `note` (a file or folder) is in, and a folder's own.
    fn move_targets(&self, note: &Path) -> Vec<String> {
        fn walk(folder: &crate::vault::Folder, root: &Path, out: &mut Vec<(PathBuf, String)>) {
            let rel = folder.path.strip_prefix(root).unwrap_or(&folder.path);
            let shown = if rel.as_os_str().is_empty() {
                "/".to_string()
            } else {
                format!("{}/", crate::vault::slash(rel))
            };
            out.push((folder.path.clone(), shown));
            for sub in &folder.folders {
                walk(sub, root, out);
            }
        }
        let mut all = Vec::new();
        walk(&self.vault.tree, &self.vault.root, &mut all);
        all.into_iter()
            .filter(|(dir, _)| Some(dir.as_path()) != note.parent() && !dir.starts_with(note))
            .map(|(_, shown)| shown)
            .collect()
    }

    /// The answers to blackglass's own questions.
    fn host_answer(&mut self, command: &str, answers: &[Answer]) {
        if command == "create-day" {
            let day = self.creating_day.take();
            if let (Some(day), [Answer::Choice(0)]) = (day, answers) {
                let folder = self.sidebar.target_folder(&self.vault);
                if let Err(e) = self.create_note_with(&folder, &day, "") {
                    self.message = e;
                }
            }
            return;
        }
        if let ("rename-note" | "rename-folder" | "new-folder", [Answer::Text(name)]) =
            (command, answers)
        {
            let Some(path) = self.renaming.take() else {
                return;
            };
            let result = match command {
                "rename-note" => self.rename_note(&path, name),
                "rename-folder" => self.rename_folder(&path, name),
                _ => self.new_folder(&path, name),
            };
            if let Err(e) = result {
                self.message = e;
            }
            return;
        }
        if let ("theme", [Answer::Choice(i)]) = (command, answers) {
            let Some((id, _)) = self.themes.get(*i).cloned() else {
                return;
            };
            let warnings = self.use_theme(&id);
            self.theme = id;
            let text = format!(
                "# blackglass: the chosen theme (\"Choose theme\").\ntheme = \"{}\"\n",
                self.theme
            );
            let saved = self.save_config(APPEARANCE_FILE, &text);
            self.message = match (saved, warnings.is_empty()) {
                (Err(e), _) => e,
                (Ok(()), false) => format!("Theme: {}", warnings.join("; ")),
                (Ok(()), true) => String::new(),
            };
            return;
        }
        if let ("outline" | "footnotes", [Answer::Choice(i)]) = (command, answers) {
            if let (Some(&row), Some(view)) = (self.outline.get(*i), self.tabs.get_mut(self.active))
            {
                view.editor.row = row;
                view.editor.col = 0;
                view.scroll = row.saturating_sub(3);
                self.focus = Focus::Editor;
            }
            return;
        }
        if let ("pdf-to-note", [Answer::Choice(i)]) = (command, answers) {
            if let Some(rel) = self.pdfs.get(*i).cloned()
                && let Err(e) = self.pdf_to_note(&rel)
            {
                self.message = e;
            }
            return;
        }
        if let ("show-properties", [Answer::Choice(i)]) = (command, answers) {
            if let Some(name) = self.property_names.get(*i).cloned() {
                self.sidebar.search(&format!("[{name}]"), &self.vault);
                self.sidebar.visible = true;
                self.focus = Focus::Sidebar;
            }
            return;
        }
        if let ("rename-property", [Answer::Choice(i), Answer::Text(new)]) = (command, answers) {
            if let Some(old) = self.property_names.get(*i).cloned() {
                self.rename_property(&old, new.trim());
            }
            return;
        }
        if command == "create-linked-note" {
            let name = self.linked_note.take();
            if let (Some(name), [Answer::Choice(0)]) = (name, answers) {
                let root = self.vault.root.clone();
                let source = self.active;
                match self.create_new_note(&root, &name, "") {
                    Ok(path) => self.follow_new_name(source, &name, &path),
                    Err(e) => self.message = e,
                }
            }
            return;
        }
        let moving = self.moving.take().filter(|_| command == "move-note");
        let Some(path) = moving.or_else(|| self.note_path()) else {
            return;
        };
        let result = match (command, answers) {
            ("delete-note", [Answer::Choice(0)]) => self.delete_note(&path),
            ("move-note", [Answer::Choice(i)]) => match self.move_targets(&path).get(*i) {
                Some(folder) => self.move_note(&path, &folder.clone()),
                None => Ok(()),
            },
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.message = e;
        }
    }

    /// Moves the note at `path` to the vault's trash folder and closes its
    /// tab (unsaved changes go with it).
    fn delete_note(&mut self, path: &Path) -> Result<(), String> {
        self.journal.begin();
        let done = self.delete_note_now(path);
        self.journal.end();
        done
    }

    fn delete_note_now(&mut self, path: &Path) -> Result<(), String> {
        self.journal.touch(path);
        let name = file_name(path);
        let trash = self.vault.root.join(TRASH);
        std::fs::create_dir_all(&trash).map_err(|e| format!("Cannot delete {name}: {e}"))?;
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let target = (0..)
            .map(|n| match n {
                0 => trash.join(&name),
                n => trash.join(format!("{stem} {n}{ext}")),
            })
            .find(|p| !p.exists())
            .expect("some name in the trash is free");
        self.journal.touch(&target);
        std::fs::rename(path, &target).map_err(|e| format!("Cannot delete {name}: {e}"))?;
        if let Some(i) = self
            .tabs
            .iter()
            .position(|t| t.path.as_deref() == Some(path))
        {
            self.remove_tab(i);
        }
        self.rescan();
        self.message = format!("{name} moved to {TRASH}/ in the vault");
        Ok(())
    }

    /// Moves the note, file or folder at `path` to `folder` (as
    /// [`App::move_targets`] shows it), with the links to everything that
    /// moves; tabs and history follow.
    fn move_note(&mut self, path: &Path, folder: &str) -> Result<(), String> {
        let name = file_name(path);
        let dir = self.vault.root.join(folder.trim_matches('/'));
        let new = dir.join(&name);
        if new.exists() {
            return Err(format!("{folder} already has a {name}"));
        }
        let moves = if path.is_dir() {
            self.files_under(path, &new)
        } else {
            vec![(path.to_path_buf(), new.clone())]
        };
        let links = self.apply_moves(moves, path, &new)?;
        self.message = format!("Moved {name} to {folder}{links}");
        Ok(())
    }

    /// Reports an install / enable change in the status bar.
    pub(crate) fn plugin_changed(&mut self, i: usize, result: Result<(), String>) {
        self.plugin_keys();
        self.template_tags();
        let status = self.plugins.borrow().statuses()[i];
        let name = status.manifest.name;
        self.message = match result {
            Err(e) => e,
            Ok(()) if !status.installed => format!("{name} uninstalled"),
            Ok(()) if status.enabled => format!("{name} installed and enabled"),
            Ok(()) => format!("{name} disabled"),
        };
    }

    /// A link `[[name]]` in tab `source` made a note named otherwise (its
    /// ID): the link points to it and still reads `name`.
    fn follow_new_name(&mut self, source: usize, name: &str, path: &Path) {
        let Some(new) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            return;
        };
        let shown = name.rsplit('/').next().unwrap_or(name);
        if new == name || new == shown {
            return;
        }
        let Some(view) = self.tabs.get_mut(source) else {
            return;
        };
        let from = [
            format!("[[{name}]]"),
            format!("[[{name}|"),
            format!("[[{name}#"),
        ];
        let to = [
            format!("[[{new}|{shown}]]"),
            format!("[[{new}|"),
            format!("[[{new}#"),
        ];
        let changed: Vec<(usize, String)> = view
            .editor
            .lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| {
                let mut out = line.clone();
                for (a, b) in from.iter().zip(&to) {
                    out = out.replace(a.as_str(), b);
                }
                (out != *line).then_some((i, out))
            })
            .collect();
        let (row, col) = (view.editor.row, view.editor.col);
        for (i, line) in changed {
            replace_lines(view, i, i + 1, &[line], &mut self.shared);
        }
        view.editor.row = row.min(view.editor.lines.len().saturating_sub(1));
        view.editor.col = col;
        view.editor.snap_col();
    }

    /// Opens row `row` of the quick switcher, or creates the note it offers.
    fn switch_to(&mut self, s: &Switcher, row: usize) {
        match s.results.get(row) {
            Some(&i) => {
                let path = self.vault.notes[i].path.clone();
                self.open(&path);
            }
            None if s.offers_create(&self.vault) => {
                let root = self.vault.root.clone();
                if let Err(e) = self.create_new_note(&root, &s.input, "") {
                    self.message = e;
                }
            }
            None => {}
        }
    }

    /// Enter in the new-note or save-as prompt.
    fn submit(&mut self, prompt: Prompt) {
        let result = match prompt {
            Prompt::NewNote { folder, input } => match self.extract.take() {
                Some(extract) => self
                    .create_new_note(&folder, &input, &extract.text)
                    .map(|path| self.finish_extract(&extract, &path)),
                None => self.create_new_note(&folder, &input, "").map(|_| ()),
            },
            Prompt::SaveAs { input } => self.save_as(&input),
            Prompt::Date { input } => self.insert_date_from(&input),
            _ => unreachable!("only text prompts are submitted"),
        };
        if let Err(e) = result {
            self.message = e;
        }
    }

    /// The date picker's Enter: the date in words put in at the cursor (a
    /// link if the settings say so).
    fn insert_date_from(&mut self, words: &str) -> Result<(), String> {
        let now = chrono::Local::now().naive_local();
        let day = crate::nldates::parse_date(words, now, self.dates.week_start)
            .ok_or_else(|| format!("No date in \"{words}\""))?;
        let text = crate::nldates::date_text(day, None, &self.dates);
        self.insert(&text, 0);
        Ok(())
    }

    /// Highlights the selection in color `color` (an index into mdedit's
    /// highlight colors; `None`: no color), or gives the highlight at the
    /// cursor that color (W-130).
    fn highlight(&mut self, color: Option<usize>) {
        let emoji = color.map(|i| mdedit::markdown::HIGHLIGHT_COLORS[i].0);
        let Some(view) = self.view() else {
            return;
        };
        if let Some(text) = view.editor.selected_text().filter(|t| !t.contains('\n')) {
            self.insert(&format!("=={}{text}==", emoji.unwrap_or_default()), 0);
            return;
        }
        let (row, col) = (view.editor.row, view.editor.col);
        match crate::highlights::recolor(&view.editor.lines[row], col, emoji) {
            Some(line) => {
                // The cursor keeps its place in the text.
                let old = view.editor.lines[row].chars().count();
                let new = line.chars().count();
                let effect = Effect::ReplaceLines {
                    from: row,
                    to: row + 1,
                    lines: vec![line],
                    cursor: (row, (col + new).saturating_sub(old).min(new)),
                };
                self.apply_effect("blackglass", "highlight", effect);
            }
            None => self.message = "Select text or put the cursor in a highlight".into(),
        }
    }

    /// Runs a natural language dates command (W-128).
    fn date_command(&mut self, command: commands::Dates) {
        use commands::Dates;
        let now = chrono::Local::now().naive_local();
        let format = |f: &str| crate::plugins::moment::format(&now, f);
        let s = self.dates.clone();
        let text = match command {
            Dates::Today => format(&s.format),
            Dates::Time => format(&s.time_format),
            Dates::Now => format!(
                "{}{}{}",
                format(&s.format),
                s.separator,
                format(&s.time_format)
            ),
            Dates::Picker => {
                self.prompt = Some(Prompt::Date {
                    input: String::new(),
                });
                return;
            }
            Dates::Parse | Dates::ParseLink | Dates::ParsePlain | Dates::ParseTime => {
                let Some(words) = self
                    .view()
                    .and_then(|v| v.editor.selected_text())
                    .filter(|w| !w.trim().is_empty())
                else {
                    self.message = "Select a date in words first".into();
                    return;
                };
                let parsed = if command == Dates::ParseTime {
                    crate::nldates::parse_time(&words, now)
                        .map(|t| crate::plugins::moment::format(&t, &s.time_format))
                } else {
                    crate::nldates::parse_date(&words, now, s.week_start).map(|day| {
                        let date = crate::nldates::format_date(day, &s);
                        match command {
                            Dates::ParseLink => format!("[{words}]({date})"),
                            Dates::ParsePlain => date,
                            _ => format!("[[{date}]]"),
                        }
                    })
                };
                match parsed {
                    Some(text) => text,
                    None => {
                        self.message = format!("No date in \"{words}\"");
                        return;
                    }
                }
            }
        };
        self.insert(&text, 0);
    }

    /// Saves the active note under `name` (relative to the vault).
    fn save_as(&mut self, name: &str) -> Result<(), String> {
        if name.trim().is_empty() || self.tabs.is_empty() {
            return Ok(());
        }
        let root = self.vault.root.clone();
        let path = self.vault_path(&root, name.trim())?;
        if path.exists() {
            return Err(format!("{} already exists", name.trim()));
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Save failed: {e}"))?;
        }
        let view = self.view_mut().expect("there is an active tab");
        view.save_as(path.clone())?;
        self.rescan();
        self.sidebar.reveal(&path, &self.vault);
        Ok(())
    }

    fn unsaved_key(&mut self, tab: usize, then: Then, key: KeyEvent) -> Action {
        let prompt = Prompt::Unsaved { tab, then };
        match key.code {
            KeyCode::Char('y' | 'Y') => {
                let Some(view) = self.tabs.get_mut(tab) else {
                    return Action::Continue;
                };
                match view.save() {
                    Ok(()) => {
                        let path = view.path.clone().expect("a saved note has a path");
                        let text = view.editor.to_text();
                        self.saved(&path, &text);
                    }
                    Err(e) => {
                        self.message = e;
                        return Action::Continue;
                    }
                }
            }
            KeyCode::Char('n' | 'N') => {
                if then == Then::Quit {
                    // Discarded: only this tab, before asking about the next.
                    self.remove_tab(tab);
                }
            }
            KeyCode::Char('c' | 'C') => return Action::Continue,
            _ => {
                self.prompt = Some(prompt);
                return Action::Continue;
            }
        }
        match then {
            Then::Close => {
                self.remove_tab(tab);
                Action::Continue
            }
            Then::Quit => self.quit(),
        }
    }

    /// Pasted text: into the prompt, the search input or the editor.
    pub fn handle_paste(&mut self, text: &str) {
        self.track();
        self.paste(text);
        self.refresh_suggest();
        self.track();
    }

    fn paste(&mut self, text: &str) {
        let line = text.replace(['\r', '\n'], " ");
        match &mut self.prompt {
            Some(
                Prompt::NewNote { input, .. } | Prompt::SaveAs { input } | Prompt::Date { input },
            ) => {
                input.push_str(&line);
            }
            Some(Prompt::Switcher(s)) => {
                s.input.push_str(&line);
                s.update(&self.vault);
            }
            Some(Prompt::Palette(p)) => {
                p.input.push_str(&line);
                p.update();
            }
            Some(Prompt::Ask(ask)) if matches!(ask.current(), Question::Form { .. }) => {
                if let Some(
                    crate::plugins::FieldValue::Text(t)
                    | crate::plugins::FieldValue::Date(t)
                    | crate::plugins::FieldValue::Secret(t),
                ) = ask.form.get_mut(ask.field)
                {
                    t.push_str(&line);
                }
            }
            Some(Prompt::Ask(ask)) => {
                ask.input.push_str(&line);
                ask.update();
            }
            Some(Prompt::Settings(SettingsWindow {
                editing: Some(input),
                ..
            })) => input.push_str(&line),
            Some(Prompt::Settings(w)) if !w.capturing => {
                w.search.push_str(&line);
                let mut w = w.clone();
                self.settings_search_changed(&mut w);
                self.prompt = Some(Prompt::Settings(w));
            }
            Some(Prompt::OpenVault(picker)) => picker.paste(&line),
            Some(Prompt::Properties(PropertyWindow {
                input: Some((_, input)),
                ..
            })) => input.push_str(&line),
            Some(Prompt::Properties(_)) => {}
            Some(Prompt::Unsaved { .. } | Prompt::Settings(_)) => {}
            None => match self.focus {
                Focus::Sidebar => self.sidebar.paste(text, &self.vault),
                Focus::Panel | Focus::Backlinks => {}
                Focus::Pane => {
                    // Typed text: one key at a time.
                    for c in line.chars() {
                        self.pane_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
                    }
                }
                Focus::Editor => {
                    if let Some(view) = self.tabs.get_mut(self.active) {
                        view.handle_paste(text, &mut self.shared);
                    }
                }
            },
        }
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) -> Action {
        self.track();
        let action = self.mouse(event);
        self.refresh_suggest();
        self.track();
        action
    }

    /// The mouse moved (no button): the note's rendered block under it
    /// gets its frame and source button. True if the screen changed (draw
    /// again); other moves draw nothing.
    pub fn mouse_moved(&mut self, event: MouseEvent) -> bool {
        let at = Position::new(event.column, event.row);
        let over = (self.prompt.is_none() && self.preview.is_none())
            .then_some(at)
            .filter(|&at| self.areas.editor.contains(at));
        self.tabs
            .get_mut(self.active)
            .is_some_and(|view| view.hover(over))
    }

    fn mouse(&mut self, event: MouseEvent) -> Action {
        // Over a preview: the wheel scrolls it, a click closes it.
        if let Some(preview) = self.preview.as_mut() {
            match event.kind {
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                    let code = if event.kind == MouseEventKind::ScrollDown {
                        KeyCode::Down
                    } else {
                        KeyCode::Up
                    };
                    for _ in 0..WHEEL_ROWS {
                        preview
                            .handle_key(KeyEvent::new(code, KeyModifiers::NONE), &mut self.shared);
                    }
                    preview.status.clear();
                }
                MouseEventKind::Down(_) => self.preview = None,
                _ => {}
            }
            return Action::Continue;
        }
        let at = Position::new(event.column, event.row);
        let inside = |r: Rect| r.contains(at);
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => self.click(at),
            // The mouse's side buttons (xterm buttons 8 and 9).
            MouseEventKind::Down(MouseButton::Back) if self.prompt.is_none() => self.go(false),
            MouseEventKind::Down(MouseButton::Forward) if self.prompt.is_none() => self.go(true),
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let down = event.kind == MouseEventKind::ScrollDown;
                if self.prompt.is_some() {
                    return Action::Continue;
                }
                if inside(self.areas.list) {
                    let rows = WHEEL_ROWS as isize;
                    self.sidebar
                        .scroll_by(if down { rows } else { -rows }, &self.vault);
                } else if inside(self.areas.editor)
                    && let Some(view) = self.tabs.get_mut(self.active)
                {
                    if view.reading {
                        // View mode: its row cursor moves.
                        let code = if down { KeyCode::Down } else { KeyCode::Up };
                        for _ in 0..WHEEL_ROWS {
                            view.handle_key(
                                KeyEvent::new(code, KeyModifiers::NONE),
                                &mut self.shared,
                            );
                        }
                    } else {
                        // The live preview: the view moves, not the cursor,
                        // so a query's long result scrolls instead of
                        // opening its source.
                        let rows = WHEEL_ROWS as isize;
                        view.scroll_rows(if down { rows } else { -rows });
                    }
                }
            }
            _ => {}
        }
        Action::Continue
    }

    fn click(&mut self, at: Position) {
        let hit = |areas: &[(Rect, usize)]| areas.iter().find(|(r, _)| r.contains(at)).map(|a| a.1);
        if self.suggest.is_some() && self.areas.suggest.contains(at) {
            self.accept_suggestion((at.y - self.areas.suggest.y) as usize);
            return;
        }
        if let Some(Prompt::Settings(w)) = &self.prompt {
            // The settings window: a gear, a page or a row.
            let mut w = w.clone();
            if let Some(&(_, i)) = self.areas.gears.iter().find(|(r, _)| r.contains(at)) {
                self.open_plugin_page(i);
                return;
            }
            if let Some(&(_, page)) = self.areas.settings_nav.iter().find(|(r, _)| r.contains(at)) {
                (w.page, w.row, w.on_page) = (page, 0, false);
                w.editing = None;
                w.capturing = false;
            } else if let Some(row) = hit(&self.areas.settings_rows) {
                (w.row, w.on_page) = (row, true);
            }
            self.prompt = Some(Prompt::Settings(w));
            return;
        }
        // A form's calendar: the day for its date field.
        if let Some(Prompt::Ask(ask)) = &mut self.prompt
            && matches!(ask.current(), Question::Form { .. })
        {
            if let Some(&(_, day)) = self.areas.form_days.iter().find(|(r, _)| r.contains(at))
                && let Some(value @ crate::plugins::FieldValue::Date(_)) =
                    ask.form.get_mut(ask.field)
            {
                *value = crate::plugins::FieldValue::Date(day.format("%Y-%m-%d").to_string());
            }
            return;
        }
        if self.prompt.is_some() {
            // In a popup, only its list's rows can be clicked.
            if !self.areas.popup_list.contains(at) {
                return;
            }
            let row = self.areas.popup_scroll + (at.y - self.areas.popup_list.y) as usize;
            match self.prompt.take() {
                Some(Prompt::Switcher(s)) => self.switch_to(&s, row),
                Some(Prompt::Palette(mut p)) if row < p.results.len() => {
                    p.selected = row;
                    let action = p.chosen().expect("the row exists").action.clone();
                    self.run_command(action);
                }
                Some(Prompt::Ask(mut ask))
                    if matches!(ask.current(), Question::Many { .. })
                        && row < ask.results.len() =>
                {
                    // A click marks or unmarks.
                    ask.selected = row;
                    self.prompt_key(
                        Prompt::Ask(ask),
                        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
                    );
                }
                Some(Prompt::Ask(mut ask))
                    if matches!(
                        ask.current(),
                        Question::Choose { .. } | Question::Suggest { .. }
                    ) && row < ask.results.len() =>
                {
                    // As if chosen with the arrows and Enter.
                    ask.selected = row;
                    self.prompt_key(
                        Prompt::Ask(ask),
                        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                    );
                }
                other => self.prompt = other,
            }
            return;
        }
        if let Some(i) = hit(&self.areas.backlinks) {
            self.open_backlink(i);
            return;
        }
        if let Some(&(_, panel)) = self.areas.panel_tabs.iter().find(|(r, _)| r.contains(at)) {
            self.sidebar.show(panel);
            self.focus = Focus::Sidebar;
        } else if self.areas.panel.contains(at)
            && let Some(id) = self.areas.panel_plugin
        {
            self.focus = Focus::Panel;
            let (row, col) = (at.y - self.areas.panel.y, at.x - self.areas.panel.x);
            let effect =
                self.with_context(|ctx| self.plugins.borrow_mut().panel_click(id, row, col, ctx));
            self.apply_effect(id, "panel", effect);
        } else if self.areas.list.contains(at) {
            self.focus = Focus::Sidebar;
            let request = self
                .sidebar
                .click((at.y - self.areas.list.y) as usize, &self.vault);
            self.request(request);
        } else if let Some(i) = hit(&self.areas.closes) {
            self.close(i);
        } else if let Some(i) = hit(&self.areas.tabs) {
            self.active = i;
            self.focus = Focus::Editor;
        } else if self.areas.new_tab.contains(at) {
            self.new_note_prompt();
        } else if self.areas.editor.contains(at) && !self.tabs.is_empty() {
            self.focus = Focus::Editor;
            let view = self.tabs.get_mut(self.active).expect("a tab is open");
            let outcome = view.click(at, &mut self.shared);
            self.outcome(outcome);
        }
    }
}

/// Puts `text` on the clipboard with the first tool that works:
/// `wl-copy` (Wayland), `xclip` or `xsel` (X11), `pbcopy` (macOS),
/// PowerShell (Windows) ([`mdedit::platform::copy_tools`]).
fn copy_outside(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let tools = mdedit::platform::copy_tools(mdedit::platform::Os::this());
    for tool in &tools {
        let Ok(mut child) = Command::new(&tool[0])
            .args(&tool[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        let written = child
            .stdin
            .take()
            .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        if written && child.wait().is_ok_and(|s| s.success()) {
            return Ok(());
        }
    }
    Err(if cfg!(windows) {
        "no clipboard tool (PowerShell)".into()
    } else {
        "no clipboard tool (install wl-clipboard or xclip)".into()
    })
}

/// Opens `path` with the desktop's default program (`xdg-open`; `open` on
/// macOS; `start` on Windows), without waiting for it.
fn open_outside(path: &Path) -> Result<(), String> {
    let mut command = mdedit::platform::open(&path.to_string_lossy());
    let program = command.get_program().to_string_lossy().into_owned();
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{program}: {e}"))
}

/// Replaces lines `from..to` of `view`'s note with `lines` (none: the
/// lines go), as one undo step: selects them and pastes over them.
fn replace_lines(
    view: &mut EditorView,
    from: usize,
    to: usize,
    lines: &[String],
    shared: &mut Shared,
) {
    let count = view.editor.lines.len();
    let to = to.min(count);
    if from >= to {
        // No lines replaced: `lines` go in before line `from` (after the
        // last line when it's past the end).
        if lines.is_empty() || from > count {
            return;
        }
        let text = lines.join("\n");
        view.editor.anchor = None;
        if from < count {
            (view.editor.row, view.editor.col) = (from, 0);
            view.handle_paste(&format!("{text}\n"), shared);
        } else {
            let last = count - 1;
            let end = view.editor.lines[last].chars().count();
            (view.editor.row, view.editor.col) = (last, end);
            view.handle_paste(&format!("\n{text}"), shared);
        }
        return;
    }
    let len = |view: &EditorView, row: usize| view.editor.lines[row].chars().count();
    let (start, end) = if !lines.is_empty() {
        ((from, 0), (to - 1, len(view, to - 1)))
    } else if to < count {
        // The lines and their line breaks.
        ((from, 0), (to, 0))
    } else if from > 0 {
        ((from - 1, len(view, from - 1)), (to - 1, len(view, to - 1)))
    } else {
        ((0, 0), (to - 1, len(view, to - 1)))
    };
    view.editor.anchor = Some(start);
    view.editor.row = end.0;
    view.editor.col = end.1;
    if start == end {
        view.editor.anchor = None;
    }
    view.handle_paste(&lines.join("\n"), shared);
    view.editor.anchor = None;
}

/// Puts the cursor at char `offset` of `text`, the note's new text.
fn place_cursor(view: &mut EditorView, text: &str, offset: usize) {
    let before: String = text.chars().take(offset).collect();
    let row = before.matches('\n').count();
    let col = before
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .chars()
        .count();
    let last = view.editor.lines.len().saturating_sub(1);
    if row > last {
        // Past a final line break (the file's ending, not a line of its
        // own): the end of the last line.
        view.editor.row = last;
        view.editor.col = view.editor.lines[last].chars().count();
    } else {
        view.editor.row = row;
        view.editor.col = col;
    }
    view.editor.snap_col();
}

/// Whether two paths are the same file (e.g. through a symbolic link).
/// Where new lines go for `place` (headings at any level, any case):
/// under a heading, after the list right under it (or right under the
/// heading, `first`); a missing heading is made (with an empty line
/// before it) at the end, or at the top; the end (before the file's last
/// empty line); the top (after the frontmatter). The line's index, and
/// the lines to put before and after the new ones.
fn note_spot(lines: &[String], place: &LinePlace) -> (usize, Vec<String>, Vec<String>) {
    let end = match lines.last() {
        Some(last) if last.is_empty() => lines.len() - 1,
        _ => lines.len(),
    };
    let top = crate::vault::properties::frontmatter_end(lines).map_or(0, |e| e + 1);
    let (name, first, make_at_top) = match place {
        LinePlace::Top => return (top, Vec::new(), Vec::new()),
        LinePlace::Bottom => return (end, Vec::new(), Vec::new()),
        LinePlace::Heading {
            name,
            first,
            make_at_top,
        } => (name.as_str(), *first, *make_at_top),
    };
    let is_heading = |l: &str| l.starts_with('#') && l.trim_start_matches('#').starts_with(' ');
    let Some(h) = lines
        .iter()
        .position(|l| is_heading(l) && l.trim_start_matches('#').trim().eq_ignore_ascii_case(name))
    else {
        if name.is_empty() {
            return (end, Vec::new(), Vec::new());
        }
        if make_at_top {
            let after = match lines.get(top) {
                Some(l) if !l.trim().is_empty() && top < end => vec![String::new()],
                _ => Vec::new(),
            };
            return (top, vec![format!("## {name}")], after);
        }
        let mut before = Vec::new();
        if end > 0 && !lines[end - 1].trim().is_empty() {
            before.push(String::new());
        }
        before.push(format!("## {name}"));
        return (end, before, Vec::new());
    };
    if first {
        return (h + 1, Vec::new(), Vec::new());
    }
    let mut at = h + 1;
    for (i, l) in lines.iter().enumerate().skip(h + 1) {
        if is_heading(l) {
            break;
        }
        let item = l.trim_start();
        if item.starts_with("- ") || item.starts_with("* ") || item.starts_with("+ ") {
            at = i + 1;
        }
    }
    (at, Vec::new(), Vec::new())
}

/// A line waiting for its note ([`Effect::AddToNote`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingLine {
    path: PathBuf,
    place: LinePlace,
    line: String,
    /// Opened once the line is in.
    open: bool,
}

/// Whether files can be made in `folder` (a test file, made and removed).
fn writable(folder: &Path) -> bool {
    let probe = folder.join(format!(".blackglass-write-check-{}", std::process::id()));
    let made = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .is_ok();
    if made {
        let _ = std::fs::remove_file(&probe);
    }
    made
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (
        mdedit::platform::canonical(a),
        mdedit::platform::canonical(b),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Whether a tab is a read-only page without a file (the help page, a
/// plugin's page such as a diff).
pub fn is_help(view: &EditorView) -> bool {
    view.path.is_none()
}

/// A tab's title: its file's name, or its page's name.
pub fn tab_title(view: &EditorView) -> String {
    view.title()
}

/// Keys that would take a note out of view mode, or save it.
fn leaves_view_mode(key: &KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Esc => true,
        KeyCode::Char('v' | 'V') => alt && !ctrl,
        KeyCode::Char('s' | 'S') => ctrl,
        _ => false,
    }
}

/// Whether a key can be a shortcut: with Ctrl or Alt, or an F key (the
/// others are typed text or move the cursor).
pub(crate) fn usable_shortcut(chord: &Chord) -> bool {
    matches!(chord.code, KeyCode::F(_))
        || chord
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// A path's file name, for messages.
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// `text` with the frontmatter's top-level key `old` (any case) named
/// `new`.
fn rename_key(text: &str, old: &str, new: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_front = false;
    for (i, piece) in text.split_inclusive('\n').enumerate() {
        let line = piece.trim_end_matches(['\n', '\r']);
        if i == 0 && line == "---" {
            in_front = true;
        } else if in_front && (line == "---" || line == "...") {
            in_front = false;
        } else if in_front
            && !line.starts_with([' ', '\t'])
            && let Some((key, rest)) = line.split_once(':')
            && key.trim().eq_ignore_ascii_case(old)
        {
            out.push_str(new);
            out.push(':');
            out.push_str(rest);
            out.push_str(&piece[line.len()..]);
            continue;
        }
        out.push_str(piece);
    }
    out
}

/// The backlinks pane's cache: the note, its backlinks, its unlinked
/// mentions.
type BacklinksCache = (
    PathBuf,
    Vec<crate::backlinks::Backlink>,
    Vec<crate::backlinks::Backlink>,
);

/// A row of the pane under the note that can be chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneItem {
    Backlink(crate::backlinks::Backlink),
    Mention(crate::backlinks::Backlink),
    Outgoing(crate::backlinks::Outgoing),
}

impl App {
    /// The window's settings (its font and text size).
    pub fn window_settings(&self) -> crate::window_settings::Settings {
        self.window.clone()
    }
}
