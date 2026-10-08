//! Plugins, like Obsidian's: each has a manifest, is loaded and unloaded,
//! adds commands to the command palette, and can render code blocks of
//! its languages in the editor (Obsidian's
//! `registerMarkdownCodeBlockProcessor`). Plugins are compiled in
//! ([`catalog`]); which ones a vault has installed and enabled is saved in
//! `.blackglass/plugins.toml` in the vault (like `.obsidian/`).

pub mod archiver;
pub mod bases;
pub mod bookmarks;
pub mod citations;
pub mod dataview;
pub mod emoji;
pub mod encrypt;
pub mod freeze;
pub mod git;
pub mod js;
pub mod mermaid;
pub mod moment;
pub mod periodic;
pub mod recent;
pub mod settings;
pub mod tables;
pub mod tasks;
pub mod templater;
pub mod zettelkasten;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mdedit::processor::CodeBlockProcessor;
use ratatui::crossterm::event::KeyEvent;
use ratatui::text::Line;

use crate::vault::Vault;

/// Who a plugin is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Manifest {
    /// Unique and stable: it's saved in the vault.
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub author: &'static str,
    pub description: &'static str,
}

/// A command a plugin adds; the palette shows it as "Plugin: name".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginCommand {
    pub id: &'static str,
    pub name: &'static str,
    /// Its default keys (`"Alt+D"`, several with `" / "`; `""` for none).
    /// A key another command has already stays with that command.
    pub keys: &'static str,
}

impl PluginCommand {
    pub const fn new(id: &'static str, name: &'static str) -> Self {
        PluginCommand { id, name, keys: "" }
    }

    /// With default keys.
    pub const fn keys(mut self, keys: &'static str) -> Self {
        self.keys = keys;
        self
    }
}

/// The command name answers go to after [`Plugin::on_note_created`] asked
/// questions.
pub const NOTE_CREATED: &str = "note-created";

/// What a plugin's command asks the workspace to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Insert `text` at the cursor, then move the cursor `back` characters.
    Insert {
        text: String,
        back: usize,
    },
    /// Show a message in the status bar.
    Message(String),
    /// Open the note at `path` (absolute) in a tab.
    Open {
        path: PathBuf,
    },
    /// Ask the user these questions, one after the other, then call
    /// [`Plugin::answer`] with the answers.
    Ask(Vec<Question>),
    /// Replace the active note's text; put the cursor at char `cursor` of
    /// the new text (else at its start).
    ReplaceNote {
        text: String,
        cursor: Option<usize>,
    },
    /// Create the note `name` (`.md` added) in `folder` with `text`, open
    /// it, and put the cursor at char `cursor`.
    CreateNote {
        folder: PathBuf,
        name: String,
        text: String,
        cursor: Option<usize>,
    },
    /// Show the plugin's own sidebar tab, with the focus.
    ShowSidebarTab,
    /// Files in the vault changed outside blackglass (a Git pull): scan
    /// again, reload open notes without unsaved changes, say this.
    FilesChanged(String),
    /// Nothing to do, but the plugin's status changed: draw again.
    Redraw,
    /// Show `text` (Markdown) in a read-only tab called `title`.
    ShowText {
        title: String,
        text: String,
    },
    /// What to show in the margin beside the note at `path`'s lines
    /// (`width` 0: no margin).
    Margin {
        path: PathBuf,
        width: u16,
        lines: Vec<(usize, Line<'static>)>,
    },
    /// Put the active note's cursor on this line.
    GoToLine(usize),
    /// Open this folder as the vault (a Git clone).
    OpenVault(PathBuf),
    /// Quit blackglass (asking about unsaved notes first).
    Quit,
    /// Put this text on the clipboard.
    CopyText(String),
    /// Open a web address in the browser.
    OpenUrl(String),
    /// Open the file at `path` (absolute) as text, whatever its kind (a
    /// `.base` file's YAML).
    OpenSource(PathBuf),
    /// Create the file at `path` (absolute) with `text`, and open it.
    CreateFile {
        path: PathBuf,
        text: String,
    },
    /// Replace lines `from..to` of the note at `path` (absolute) with
    /// `lines`, if they still are `expect`: in its tab if it's open (saved
    /// if it had no unsaved changes), else in the file.
    EditNote {
        path: PathBuf,
        from: usize,
        to: usize,
        lines: Vec<String>,
        expect: Vec<String>,
    },
    /// Replace the active note's lines `from..to` with `lines` (one undo
    /// step) and put the cursor at (line, char column).
    ReplaceLines {
        from: usize,
        to: usize,
        lines: Vec<String>,
        cursor: (usize, usize),
    },
    /// Create the file at `path` (absolute) with `text`, without opening
    /// it (Templater's `tp.file.create_new`).
    WriteFile {
        path: PathBuf,
        text: String,
    },
    /// Move (or rename) the note at `from` to `to` (both absolute), with
    /// every link to it in the vault (Templater's `tp.file.move`,
    /// `rename`).
    MoveNote {
        from: PathBuf,
        to: PathBuf,
    },
    /// These, one after the other.
    Many(Vec<Effect>),
    /// Search the vault for this (the sidebar's Search tab).
    Search(String),
    /// Move the note at `path` (absolute) to the vault's trash, closing its
    /// tab (ask first: this doesn't).
    DeleteNote(PathBuf),
    /// Point every link to the note at `from` at the note at `to` (both
    /// absolute) instead (a merge).
    Retarget {
        from: PathBuf,
        to: PathBuf,
    },
    /// Show the file or folder at `path` (absolute) in the file explorer.
    Reveal(PathBuf),
    /// Give the plugin's pane above the note the focus (`true`), or give
    /// it back to the note.
    FocusPane(bool),
    /// Do a result row's action (`plugin:<id>:<payload>` reaches
    /// [`Plugin::row_action`]): a suggestion telling its plugin it was
    /// chosen.
    RowAction(String),
    /// Put `line` in today's daily note, at the end of the list under its
    /// `heading` (at the note's end without one); the note is made first
    /// if it isn't there (Periodic Notes, with its template).
    AddToDaily {
        heading: String,
        line: String,
    },
}

/// What a plugin suggests while typing ([`Plugin::suggestions`]): the
/// char column where the text it replaces starts, and (label, text to put
/// in its place) pairs; an item may also do something once it's in
/// (`effects`: (item, effect), e.g. a block id added to the linked note),
/// and may put in other text with Shift+Enter (`alts`: (item, text)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suggestions {
    pub start: usize,
    pub items: Vec<(String, String)>,
    pub effects: Vec<(usize, Effect)>,
    pub alts: Vec<(usize, String)>,
}

/// How much of the note's side a plugin's pane takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSize {
    /// One line (a summary).
    Minimized,
    /// Half.
    Half,
    /// All but a few rows of the note.
    Maximized,
}

impl PaneSize {
    /// The rows it takes of `height` (with its two rules).
    pub fn rows(self, height: u16) -> u16 {
        match self {
            PaneSize::Minimized => 3,
            PaneSize::Half => (height / 2).max(6),
            PaneSize::Maximized => height.saturating_sub(5).max(6),
        }
        .min(height.saturating_sub(3))
    }

    /// The next size (Alt+M): half, maximized, minimized, half …
    pub fn next(self) -> PaneSize {
        match self {
            PaneSize::Half => PaneSize::Maximized,
            PaneSize::Maximized => PaneSize::Minimized,
            PaneSize::Minimized => PaneSize::Half,
        }
    }
}

/// A plugin's pane above the active note (Bases' filters).
#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    pub title: String,
    pub size: PaneSize,
    /// Its lines, for the rows between its rules
    /// (`size.rows(height) - 2`).
    pub lines: Vec<Line<'static>>,
    /// Where the text cursor is (column, line), while text is typed.
    pub cursor: Option<(u16, u16)>,
}

/// A row of a plugin's sidebar tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarRow {
    pub label: String,
    /// Shown dimmed after the label (a note's folder).
    pub detail: String,
}

/// A question a plugin asks (Templater's `tp.system.prompt`, `suggester`
/// and `multi_suggester`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// Type an answer; Enter with nothing typed takes `default`.
    Text { prompt: String, default: String },
    /// Choose one of `items` (type to filter).
    Choose { prompt: String, items: Vec<String> },
    /// Type several lines (Alt+Enter starts a new one); Enter with nothing
    /// typed takes `default`.
    Lines { prompt: String, default: String },
    /// Choose any of `items` (Tab marks one; Enter takes the marked, or
    /// the highlighted one if none is).
    Many { prompt: String, items: Vec<String> },
    /// A window of fields, every one shown with its default: ↑↓ / Tab move
    /// between them, typing edits a text, ←→ change a choice, Enter takes
    /// them all ([`Answer::Fields`]).
    Form {
        title: String,
        fields: Vec<FormField>,
    },
    /// A text to read (several lines), with buttons under it: ←→ / Tab
    /// choose one, Enter takes it ([`Answer::Choice`]), Esc closes.
    Show {
        title: String,
        text: String,
        buttons: Vec<String>,
    },
}

/// A field of a [`Question::Form`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    pub label: String,
    /// One line saying what it takes (shown for the field being edited).
    pub help: String,
    pub value: FieldValue,
}

impl FormField {
    pub fn text(label: &str, help: &str, value: &str) -> Self {
        FormField {
            label: label.into(),
            help: help.into(),
            value: FieldValue::Text(value.into()),
        }
    }

    /// A date: typed (`2026-10-20`, `tomorrow`), or chosen on a calendar
    /// beside the form (Shift+arrows, PgUp / PgDn, a click).
    pub fn date(label: &str, help: &str, value: &str) -> Self {
        FormField {
            label: label.into(),
            help: help.into(),
            value: FieldValue::Date(value.into()),
        }
    }

    /// A password: typed, shown as dots (answered as [`Answer::Text`]).
    pub fn secret(label: &str, help: &str, value: &str) -> Self {
        FormField {
            label: label.into(),
            help: help.into(),
            value: FieldValue::Secret(value.into()),
        }
    }

    pub fn choice(label: &str, help: &str, items: Vec<String>, chosen: usize) -> Self {
        FormField {
            label: label.into(),
            help: help.into(),
            value: FieldValue::Choice(items, chosen),
        }
    }
}

/// A form field's value: text, or a choice of items (the chosen one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    Text(String),
    Choice(Vec<String>, usize),
    /// A date as typed (answered as [`Answer::Text`]).
    Date(String),
    /// A password, shown as dots (answered as [`Answer::Text`]).
    Secret(String),
}

impl FieldValue {
    /// A date field's day: what's typed, read (`2026-10-20`, `tomorrow`).
    pub fn day(&self) -> Option<chrono::NaiveDate> {
        let FieldValue::Date(text) = self else {
            return None;
        };
        let now = chrono::Local::now().naive_local();
        crate::nldates::parse_date(text, now, chrono::Weekday::Mon)
    }
}

/// An answer, in the order of the questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Text(String),
    /// The index of the chosen item.
    Choice(usize),
    /// The indexes of the chosen items ([`Question::Many`]), in order.
    Choices(Vec<usize>),
    /// A form's fields, in order: [`Answer::Text`] or [`Answer::Choice`].
    Fields(Vec<Answer>),
}

/// What a command can see when it runs.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub vault: &'a Vault,
    /// The active note, if one is open.
    pub note: Option<ActiveNote<'a>>,
    /// Where new notes go (the folder selected in the file explorer).
    pub folder: &'a Path,
    /// The new note's name, when it's already typed (the new-note window's
    /// "from a template"); `None` otherwise.
    pub name: Option<&'a str>,
    /// The sidebar's search (`""` for none).
    pub query: &'a str,
}

/// The note in the active tab.
#[derive(Debug, Clone, Copy)]
pub struct ActiveNote<'a> {
    /// `None` for an untitled note.
    pub path: Option<&'a Path>,
    pub text: &'a str,
    pub selection: Option<&'a str>,
    /// Where the selection is: its start and end (line, char column).
    pub selected: Option<((usize, usize), (usize, usize))>,
    /// The cursor's line, and its column (in chars).
    pub row: usize,
    pub col: usize,
    /// View mode: the action of the result row the cursor is on
    /// (`plugin:tasks:toggle:…`), for a command that works on it.
    pub action: Option<&'a str>,
}

/// A plugin. Everything but the manifest is optional.
pub trait Plugin {
    fn manifest(&self) -> Manifest;

    /// The plugin was enabled (or the vault opened with it enabled).
    fn on_load(&mut self, _vault: &Vault) {}

    /// The plugin was disabled or uninstalled: drop what it holds.
    fn on_unload(&mut self) {}

    /// Notes changed (created, moved, scanned again).
    fn on_vault_changed(&mut self, _vault: &Vault) {}

    /// Only the note at `path` changed (it was saved): a plugin with an
    /// index of its own updates just that note. By default, the whole
    /// vault is read again.
    fn on_note_changed(&mut self, _path: &Path, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    /// Suggestions for what's being typed at char column `col` of `line`
    /// (the editor's cursor), shown in a list under it; `None`: none. Runs
    /// on every key in the editor, so return early when it's not yours.
    /// Asked inside a link after `#` or `^` too (`[[Note#`), not for a
    /// note's name (blackglass's own list).
    fn suggestions(&self, _line: &str, _col: usize, _vault: &Vault) -> Option<Suggestions> {
        None
    }

    /// The theme changed: drawn results (which use its colors) are out of
    /// date.
    fn on_theme_changed(&mut self) {}

    /// The daily note for `day`, relative to the vault and without `.md`
    /// (Periodic Notes' folder and name), if the plugin keeps them.
    fn daily_note(&self, _day: chrono::NaiveDate) -> Option<String> {
        None
    }

    /// The name to show for the note at `path` (absolute) wherever notes
    /// are named (the explorer, tabs, the switcher, suggestions, search
    /// results, backlinks): its title, say; `None`: its name. Asked while
    /// drawing: keep it cheap (a cache).
    fn display_name(&self, _path: &Path) -> Option<String> {
        None
    }

    /// A short text shown dimmed after a wiki link to `target` (`Note`,
    /// `Note#Heading`) in the live preview (how many notes link to it).
    /// Asked while drawing: keep it cheap.
    fn link_badge(&self, _target: &str) -> Option<String> {
        None
    }

    /// Spans the plugin shows its own way in the live preview: (opening,
    /// closing) markers (`("[@", "]")`); [`Plugin::render_span`] gives
    /// what's shown.
    fn rendered_spans(&self) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }

    /// Lines the plugin hides in the live preview (and view mode) unless
    /// the cursor is on them, by how they start (Advanced Tables'
    /// `<!-- TBLFM:` formula lines).
    fn hidden_lines(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// Parts of a line's text the plugin styles its own way in the live
    /// preview (Tasks' fields as muted chips): byte ranges with their
    /// style, the text as written. Asked while drawing: keep it cheap.
    fn marks(&self, _text: &str) -> Vec<(std::ops::Range<usize>, ratatui::style::Style)> {
        Vec::new()
    }

    /// A table's lines to show instead of `lines` (its second the
    /// separator), as many: computed cells (Advanced Tables' `=SUM(…)`);
    /// `None`: as written. Asked while drawing, while the cursor isn't in
    /// the table.
    fn table_cells(&self, _lines: &[String]) -> Option<Vec<String>> {
        None
    }

    /// What a span between `open` and its closing marker shows (`inner` is
    /// the text between); `None`: as written. Asked while drawing.
    fn render_span(&self, _open: &str, _inner: &str) -> Option<String> {
        None
    }

    /// Whether the plugin reads the vault's pages (Dataview's index): it's
    /// built once for every plugin that does, and given with
    /// [`Plugin::set_index`].
    fn uses_index(&self) -> bool {
        false
    }

    /// The pages, or `None` while they're being updated (let go of them,
    /// so the update needn't copy them).
    fn set_index(&mut self, _index: Option<Rc<dataview::index::Index>>) {}

    /// An empty note was just created and is the active one
    /// (`ctx.note`): the plugin may fill it (Templater's folder
    /// templates). Its questions are answered to command
    /// [`NOTE_CREATED`].
    fn on_note_created(&mut self, _ctx: &Context) -> Effect {
        Effect::None
    }

    fn commands(&self) -> Vec<PluginCommand> {
        Vec::new()
    }

    /// Runs command `id` (one of [`Plugin::commands`]).
    fn run(&mut self, _id: &str, _ctx: &Context) -> Effect {
        Effect::None
    }

    /// The answers to the questions command `id` asked with
    /// [`Effect::Ask`], in order.
    fn answer(&mut self, _id: &str, _answers: &[Answer], _ctx: &Context) -> Effect {
        Effect::None
    }

    /// The settings the plugin has, for its settings screen; their values
    /// are in its `settings.toml` (read them in `on_vault_changed`, which
    /// runs again after a change).
    fn settings(&self) -> Vec<settings::Setting> {
        Vec::new()
    }

    /// A panel the plugin draws in the sidebar (Obsidian's views, e.g. a
    /// calendar): its lines for `width` columns, or `None` for no panel.
    fn panel(&self, _ctx: &Context, _width: u16) -> Option<Vec<Line<'static>>> {
        None
    }

    /// A key while the panel has the focus.
    fn panel_key(&mut self, _key: KeyEvent, _ctx: &Context) -> Effect {
        Effect::None
    }

    /// A pane above the active note, `width` × `height` being the note's
    /// side (Bases' filters on a `.base` page); `None`: none. Asked while
    /// drawing: keep it cheap.
    fn pane(&self, _ctx: &Context, _width: u16, _height: u16) -> Option<Pane> {
        None
    }

    /// A key while the pane has the focus ([`Effect::FocusPane`] gives
    /// it back).
    fn pane_key(&mut self, _key: KeyEvent, _ctx: &Context) -> Effect {
        Effect::None
    }

    /// A tab of its own in the sidebar, after Files, Search and Tags: its
    /// title. Shown while the plugin is enabled.
    fn sidebar_tab(&self) -> Option<&'static str> {
        None
    }

    /// The sidebar tab's rows.
    fn sidebar_rows(&self, _ctx: &Context) -> Vec<SidebarRow> {
        Vec::new()
    }

    /// The sidebar tab's keys, for the status bar's hints.
    fn sidebar_hint(&self) -> &'static str {
        "⏎ open"
    }

    /// A key on row `row` of the sidebar tab (Enter for a click).
    fn sidebar_key(&mut self, _row: usize, _key: KeyEvent, _ctx: &Context) -> Effect {
        Effect::None
    }

    /// A short line for the status bar (the Git plugin's branch), if any.
    fn status(&self) -> Option<String> {
        None
    }

    /// Called about twice a second (and after events): timers, and work
    /// finished in the background.
    fn tick(&mut self, _ctx: &Context) -> Effect {
        Effect::None
    }

    /// The user went to the note at `path` (opened it, or switched to its
    /// tab).
    fn on_note_opened(&mut self, _path: &Path, _vault: &Vault) {}

    /// The note at `path` was saved (by blackglass).
    fn on_note_saved(&mut self, _path: &Path, _vault: &Vault) {}

    /// The note at `from` was moved or renamed to `to`.
    fn on_note_moved(&mut self, _from: &Path, _to: &Path, _vault: &Vault) {}

    /// A result row's action `plugin:<id>:<payload>` (a click or Enter on
    /// a row [`Plugin::render_block_rows`] gave it).
    fn row_action(&mut self, _payload: &str, _ctx: &Context) -> Effect {
        Effect::None
    }

    /// Whether the plugin may want `key` while the editor has the focus
    /// (then [`Plugin::editor_key`] is asked). Keep it cheap: it runs on
    /// every key.
    fn takes_key(&self, _key: &KeyEvent) -> bool {
        false
    }

    /// A key in the editor that [`Plugin::takes_key`] said it may want:
    /// `Some` takes it (the editor doesn't get it), `None` leaves it to the
    /// editor.
    fn editor_key(&mut self, _key: KeyEvent, _ctx: &Context) -> Option<Effect> {
        None
    }

    /// A click on the panel, at a row and column of its lines.
    fn panel_click(&mut self, _row: u16, _col: u16, _ctx: &Context) -> Effect {
        Effect::None
    }

    /// The code block languages this plugin renders.
    fn block_languages(&self) -> &[&'static str] {
        &[]
    }

    /// The lines for a block in one of its languages (see
    /// [`CodeBlockProcessor::render`]).
    fn render_block(
        &self,
        _lang: &str,
        _source: &[String],
        _from: Option<&Path>,
        _width: usize,
    ) -> Vec<Line<'static>> {
        Vec::new()
    }

    /// [`Plugin::render_block`]'s lines with an action each (see
    /// [`CodeBlockProcessor::render_rows`]); the workspace does
    /// `open:<path>` and `line:<n>:<path>` (paths relative to the vault).
    fn render_block_rows(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<(Line<'static>, Option<String>)> {
        self.render_block(lang, source, from, width)
            .into_iter()
            .map(|l| (l, None))
            .collect()
    }

    /// [`Plugin::render_block_rows`]'s lines with actions on parts of
    /// them (see [`CodeBlockProcessor::render_cells`]: a calendar's days).
    fn render_block_cells(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<mdedit::processor::CellRow> {
        self.render_block_rows(lang, source, from, width)
            .into_iter()
            .map(|(line, action)| {
                let parts = action.map(|a| vec![(0, usize::MAX, a)]);
                (line, parts.unwrap_or_default())
            })
            .collect()
    }
}

/// `/pattern/flags` as a regular expression (flags: `i` any case, `m`
/// lines, `s` dot matches line breaks; others ignored, like `g`).
pub fn slash_regex(text: &str) -> Result<regex::Regex, String> {
    let body = text
        .trim()
        .strip_prefix('/')
        .ok_or("a /regular expression/")?;
    let end = body
        .rfind('/')
        .ok_or("a /regular expression/ (its closing /)")?;
    let (pattern, flags) = (&body[..end], &body[end + 1..]);
    let mut builder = regex::RegexBuilder::new(pattern);
    builder
        .case_insensitive(flags.contains('i'))
        .multi_line(flags.contains('m'))
        .dot_matches_new_line(flags.contains('s'));
    builder
        .build()
        .map_err(|e| format!("the regular expression {text}: {e}"))
}

/// Every plugin blackglass has, installed or not.
pub fn catalog() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(dataview::Dataview::new()),
        Box::new(templater::Templater::new()),
        Box::new(periodic::PeriodicNotes::new()),
        Box::new(recent::RecentFiles::new()),
        Box::new(mermaid::Mermaid::new()),
        Box::new(git::Git::new()),
        Box::new(tables::Tables::new()),
        Box::new(tasks::Tasks::new()),
        Box::new(bases::Bases::new()),
        Box::new(bookmarks::Bookmarks::new()),
        Box::new(zettelkasten::Zettelkasten::new()),
        Box::new(citations::Citations::new()),
        Box::new(archiver::Archiver::new()),
        Box::new(emoji::EmojiShortcodes::new()),
        Box::new(encrypt::Encrypt::new()),
    ]
}

struct Entry {
    plugin: Box<dyn Plugin>,
    installed: bool,
    enabled: bool,
}

/// One plugin as the plugins window shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub manifest: Manifest,
    pub installed: bool,
    pub enabled: bool,
}

/// The plugins of a vault, with what's installed and enabled.
pub struct Plugins {
    entries: Vec<Entry>,
    /// ```` ```query ```` blocks (core, not a plugin).
    pub queries: crate::query_embed::QueryBlocks,
    /// The pages the plugins that use them share (`None` while none is
    /// enabled).
    index: Option<Rc<dataview::index::Index>>,
    /// `.blackglass/plugins.toml`; `None` keeps the state in memory.
    file: Option<PathBuf>,
}

/// The state file, relative to the vault.
pub const STATE_FILE: &str = ".blackglass/plugins.toml";

/// Each plugin keeps its data in its own folder in the vault,
/// `.blackglass/plugins/<id>/` (as Obsidian's `.obsidian/plugins/<id>/`);
/// its settings are `settings.toml` there.
pub fn settings_file(vault: &Vault, id: &str) -> PathBuf {
    vault
        .root
        .join(".blackglass")
        .join("plugins")
        .join(id)
        .join("settings.toml")
}

impl Plugins {
    /// `plugins`, installed and enabled as `file` says (none if it's
    /// missing); enabled ones are loaded.
    pub fn new(plugins: Vec<Box<dyn Plugin>>, file: Option<PathBuf>, vault: &Vault) -> Plugins {
        let text = file
            .as_deref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .unwrap_or_default();
        let (installed, enabled) = (list(&text, "installed"), list(&text, "enabled"));
        let mut entries: Vec<Entry> = plugins
            .into_iter()
            .map(|plugin| {
                let id = plugin.manifest().id;
                let installed = installed.iter().any(|i| i == id);
                Entry {
                    enabled: installed && enabled.iter().any(|i| i == id),
                    installed,
                    plugin,
                }
            })
            .collect();
        for e in entries.iter_mut().filter(|e| e.enabled) {
            e.plugin.on_load(vault);
        }
        let mut plugins = Plugins {
            entries,
            file,
            queries: crate::query_embed::QueryBlocks::default(),
            index: None,
        };
        plugins.refresh_index(vault, None);
        plugins
    }

    /// Builds the shared pages again (or updates the note at `changed`
    /// alone), if an enabled plugin uses them, and hands them out.
    fn refresh_index(&mut self, vault: &Vault, changed: Option<&Path>) {
        let users = |e: &Entry| e.enabled && e.plugin.uses_index();
        for e in &mut self.entries {
            e.plugin.set_index(None);
        }
        if !self.entries.iter().any(users) {
            self.index = None;
            return;
        }
        match (&mut self.index, changed) {
            // Nobody else holds it now: updated in place.
            (Some(index), Some(path)) => Rc::make_mut(index).update(vault, path),
            _ => self.index = Some(Rc::new(dataview::index::Index::build(vault))),
        }
        let index = self.index.clone();
        for e in self.entries.iter_mut().filter(|e| users(e)) {
            e.plugin.set_index(index.clone());
        }
    }

    /// The vault's plugins, from its state file.
    pub fn for_vault(vault: &Vault) -> Plugins {
        Plugins::new(catalog(), Some(vault.root.join(STATE_FILE)), vault)
    }

    pub fn statuses(&self) -> Vec<Status> {
        self.entries
            .iter()
            .map(|e| Status {
                manifest: e.plugin.manifest(),
                installed: e.installed,
                enabled: e.enabled,
            })
            .collect()
    }

    /// Installs plugin `i` (and enables it) or uninstalls it.
    pub fn toggle_installed(&mut self, i: usize, vault: &Vault) -> Result<(), String> {
        let Some(e) = self.entries.get_mut(i) else {
            return Ok(());
        };
        if e.installed {
            if e.enabled {
                e.plugin.on_unload();
            }
            e.installed = false;
            e.enabled = false;
        } else {
            e.installed = true;
            e.enabled = true;
            e.plugin.on_load(vault);
        }
        self.refresh_index(vault, None);
        self.save()
    }

    /// Enables or disables installed plugin `i`.
    pub fn toggle_enabled(&mut self, i: usize, vault: &Vault) -> Result<(), String> {
        let Some(e) = self.entries.get_mut(i).filter(|e| e.installed) else {
            return Ok(());
        };
        e.enabled = !e.enabled;
        if e.enabled {
            e.plugin.on_load(vault);
        } else {
            e.plugin.on_unload();
        }
        self.refresh_index(vault, None);
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        let ids = |f: fn(&Entry) -> bool| {
            self.entries
                .iter()
                .filter(|e| f(e))
                .map(|e| format!("\"{}\"", e.plugin.manifest().id))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let text = format!(
            "# blackglass plugins for this vault\ninstalled = [{}]\nenabled = [{}]\n",
            ids(|e| e.installed),
            ids(|e| e.enabled)
        );
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Cannot save plugins: {e}"))?;
        }
        mdedit::files::write_atomic(file, &text).map_err(|e| format!("Cannot save plugins: {e}"))
    }

    /// Plugin `i`'s id and name.
    pub fn manifest(&self, i: usize) -> Option<Manifest> {
        self.entries.get(i).map(|e| e.plugin.manifest())
    }

    /// Plugin `i`'s settings.
    pub fn settings(&self, i: usize) -> Vec<settings::Setting> {
        self.entries
            .get(i)
            .map(|e| e.plugin.settings())
            .unwrap_or_default()
    }

    /// Plugin `i`'s setting values, from its settings file.
    pub fn values(&self, i: usize, vault: &Vault) -> settings::Values {
        let Some(m) = self.manifest(i) else {
            return settings::Values::default();
        };
        let text = std::fs::read_to_string(settings_file(vault, m.id)).unwrap_or_default();
        settings::Values::parse(&text)
    }

    /// Saves a setting of plugin `i`, then lets the plugin read it.
    pub fn set(
        &mut self,
        i: usize,
        setting: &settings::Setting,
        value: &str,
        vault: &Vault,
    ) -> Result<(), String> {
        let (Some(m), schema) = (self.manifest(i), self.settings(i)) else {
            return Ok(());
        };
        let mut values = self.values(i, vault);
        values.set(&setting.section, &setting.key, value);
        let file = settings_file(vault, m.id);
        let dir = file.parent().expect("a settings file is in a folder");
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot save settings: {e}"))?;
        mdedit::files::write_atomic(&file, &values.to_text(&schema))
            .map_err(|e| format!("Cannot save settings: {e}"))?;
        let e = &mut self.entries[i];
        if e.enabled {
            e.plugin.on_vault_changed(vault);
        }
        Ok(())
    }

    /// The first enabled plugin's panel: its id and lines.
    pub fn panel(&self, ctx: &Context, width: u16) -> Option<(&'static str, Vec<Line<'static>>)> {
        self.entries.iter().filter(|e| e.enabled).find_map(|e| {
            e.plugin
                .panel(ctx, width)
                .map(|lines| (e.plugin.manifest().id, lines))
        })
    }

    /// The first enabled plugin's pane above the note: (plugin id, pane).
    pub fn pane(&self, ctx: &Context, width: u16, height: u16) -> Option<(&'static str, Pane)> {
        self.entries.iter().filter(|e| e.enabled).find_map(|e| {
            e.plugin
                .pane(ctx, width, height)
                .map(|pane| (e.plugin.manifest().id, pane))
        })
    }

    /// A key for plugin `id`'s pane.
    pub fn pane_key(&mut self, id: &str, key: KeyEvent, ctx: &Context) -> Effect {
        match self.enabled_mut(id) {
            Some(plugin) => plugin.pane_key(key, ctx),
            None => Effect::None,
        }
    }

    /// The enabled plugins' sidebar tabs: (plugin id, title).
    pub fn sidebar_tabs(&self) -> Vec<(&'static str, &'static str)> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .filter_map(|e| e.plugin.sidebar_tab().map(|t| (e.plugin.manifest().id, t)))
            .collect()
    }

    /// Plugin `id`'s sidebar tab key hints.
    pub fn sidebar_hint(&self, id: &str) -> &'static str {
        self.entries
            .iter()
            .find(|e| e.enabled && e.plugin.manifest().id == id)
            .map_or("⏎ open", |e| e.plugin.sidebar_hint())
    }

    /// Plugin `id`'s sidebar tab rows.
    pub fn sidebar_rows(&self, id: &str, ctx: &Context) -> Vec<SidebarRow> {
        self.entries
            .iter()
            .find(|e| e.enabled && e.plugin.manifest().id == id)
            .map(|e| e.plugin.sidebar_rows(ctx))
            .unwrap_or_default()
    }

    /// A key on a row of plugin `id`'s sidebar tab.
    pub fn sidebar_key(&mut self, id: &str, row: usize, key: KeyEvent, ctx: &Context) -> Effect {
        match self.enabled_mut(id) {
            Some(plugin) => plugin.sidebar_key(row, key, ctx),
            None => Effect::None,
        }
    }

    /// Whether plugin `id` is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.entries
            .iter()
            .any(|e| e.enabled && e.plugin.manifest().id == id)
    }

    /// Plugin `id`'s result row action.
    pub fn row_action(&mut self, id: &str, payload: &str, ctx: &Context) -> Effect {
        match self.enabled_mut(id) {
            Some(plugin) => plugin.row_action(payload, ctx),
            None => Effect::Message(format!("The {id} plugin isn't enabled")),
        }
    }

    /// Whether an enabled plugin may want `key` in the editor.
    pub fn takes_key(&self, key: &KeyEvent) -> bool {
        self.entries
            .iter()
            .any(|e| e.enabled && e.plugin.takes_key(key))
    }

    /// The first enabled plugin that takes editor key `key`: its id and
    /// effect.
    pub fn editor_key(&mut self, key: KeyEvent, ctx: &Context) -> Option<(&'static str, Effect)> {
        self.entries
            .iter_mut()
            .filter(|e| e.enabled && e.plugin.takes_key(&key))
            .find_map(|e| {
                let id = e.plugin.manifest().id;
                e.plugin.editor_key(key, ctx).map(|effect| (id, effect))
            })
    }

    /// The enabled plugins' status bar lines.
    pub fn status_lines(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .filter_map(|e| e.plugin.status())
            .collect()
    }

    /// Every enabled plugin's tick, and what each wants done.
    pub fn tick(&mut self, ctx: &Context) -> Vec<(&'static str, Effect)> {
        self.entries
            .iter_mut()
            .filter(|e| e.enabled)
            .filter_map(|e| match e.plugin.tick(ctx) {
                Effect::None => None,
                effect => Some((e.plugin.manifest().id, effect)),
            })
            .collect()
    }

    /// Tells the enabled plugins the user went to the note at `path`.
    pub fn note_opened(&mut self, path: &Path, vault: &Vault) {
        for e in self.entries.iter_mut().filter(|e| e.enabled) {
            e.plugin.on_note_opened(path, vault);
        }
    }

    /// Tells the enabled plugins a note was moved.
    pub fn note_moved(&mut self, from: &Path, to: &Path, vault: &Vault) {
        for e in self.entries.iter_mut().filter(|e| e.enabled) {
            e.plugin.on_note_moved(from, to, vault);
        }
    }

    /// A key for plugin `id`'s panel.
    pub fn panel_key(&mut self, id: &str, key: KeyEvent, ctx: &Context) -> Effect {
        match self.enabled_mut(id) {
            Some(plugin) => plugin.panel_key(key, ctx),
            None => Effect::None,
        }
    }

    /// A click on plugin `id`'s panel.
    pub fn panel_click(&mut self, id: &str, row: u16, col: u16, ctx: &Context) -> Effect {
        match self.enabled_mut(id) {
            Some(plugin) => plugin.panel_click(row, col, ctx),
            None => Effect::None,
        }
    }

    /// The index of the plugin with this id.
    pub fn index(&self, id: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.plugin.manifest().id == id)
    }

    /// Tells the enabled plugins the notes changed.
    /// Tells the enabled plugins the note at `path` was saved.
    pub fn note_saved(&mut self, path: &Path, vault: &Vault) {
        for e in self.entries.iter_mut().filter(|e| e.enabled) {
            e.plugin.on_note_saved(path, vault);
        }
    }

    pub fn vault_changed(&mut self, vault: &Vault) {
        self.refresh_index(vault, None);
        for e in self.entries.iter_mut().filter(|e| e.enabled) {
            e.plugin.on_vault_changed(vault);
        }
    }

    /// The first enabled plugin's badge for a link to `target`.
    pub fn link_badge(&self, target: &str) -> Option<String> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .find_map(|e| e.plugin.link_badge(target))
    }

    /// The first enabled plugin's lines for a table ([`Plugin::table_cells`]).
    pub fn table_cells(&self, lines: &[String]) -> Option<Vec<String>> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .find_map(|e| e.plugin.table_cells(lines))
    }

    /// How the lines the enabled plugins hide start.
    pub fn hidden_lines(&self) -> Vec<&'static str> {
        let mut all = Vec::new();
        for e in self.entries.iter().filter(|e| e.enabled) {
            all.extend(e.plugin.hidden_lines());
        }
        all
    }

    /// The enabled plugins' marks in `text`.
    pub fn marks(&self, text: &str) -> Vec<(std::ops::Range<usize>, ratatui::style::Style)> {
        let mut all = Vec::new();
        for e in self.entries.iter().filter(|e| e.enabled) {
            all.extend(e.plugin.marks(text));
        }
        all
    }

    /// The enabled plugins' rendered spans' markers.
    pub fn rendered_spans(&self) -> Vec<(&'static str, &'static str)> {
        let mut all = Vec::new();
        for e in self.entries.iter().filter(|e| e.enabled) {
            all.extend(e.plugin.rendered_spans());
        }
        all
    }

    /// The first enabled plugin's text for a span.
    pub fn render_span(&self, open: &str, inner: &str) -> Option<String> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .find_map(|e| e.plugin.render_span(open, inner))
    }

    /// The first enabled plugin's name for the note at `path`.
    /// The daily note for `day` ([`Plugin::daily_note`]), if a plugin
    /// keeps them.
    pub fn daily_note(&self, day: chrono::NaiveDate) -> Option<String> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .find_map(|e| e.plugin.daily_note(day))
    }

    pub fn display_name(&self, path: &Path) -> Option<String> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .find_map(|e| e.plugin.display_name(path))
    }

    /// The first enabled plugin's suggestions for `line` at `col`, with
    /// its id.
    pub fn suggestions(
        &self,
        line: &str,
        col: usize,
        vault: &Vault,
    ) -> Option<(&'static str, Suggestions)> {
        self.entries.iter().filter(|e| e.enabled).find_map(|e| {
            e.plugin
                .suggestions(line, col, vault)
                .map(|s| (e.plugin.manifest().id, s))
        })
    }

    /// The theme changed: results are drawn again in its colors.
    pub fn theme_changed(&mut self) {
        self.queries.clear_cache();
        for e in self.entries.iter_mut().filter(|e| e.enabled) {
            e.plugin.on_theme_changed();
        }
    }

    /// Only the note at `path` changed (a save).
    pub fn note_changed(&mut self, path: &Path, vault: &Vault) {
        self.refresh_index(vault, Some(path));
        for e in self.entries.iter_mut().filter(|e| e.enabled) {
            e.plugin.on_note_changed(path, vault);
        }
    }

    /// The enabled plugins' commands: (plugin id, plugin name, command).
    pub fn commands(&self) -> Vec<(&'static str, &'static str, PluginCommand)> {
        self.entries
            .iter()
            .filter(|e| e.enabled)
            .flat_map(|e| {
                let m = e.plugin.manifest();
                e.plugin
                    .commands()
                    .into_iter()
                    .map(move |c| (m.id, m.name, c))
            })
            .collect()
    }

    /// Runs `command` of plugin `id`, if it's enabled.
    pub fn run(&mut self, id: &str, command: &str, ctx: &Context) -> Effect {
        match self.enabled_mut(id) {
            Some(plugin) => plugin.run(command, ctx),
            None => Effect::None,
        }
    }

    /// Tells the enabled plugins an empty note was created; the first that
    /// does something says what (its id and effect).
    pub fn note_created(&mut self, ctx: &Context) -> Option<(&'static str, Effect)> {
        self.entries.iter_mut().filter(|e| e.enabled).find_map(|e| {
            match e.plugin.on_note_created(ctx) {
                Effect::None => None,
                effect => Some((e.plugin.manifest().id, effect)),
            }
        })
    }

    /// Gives `command` of plugin `id` the answers to its questions.
    pub fn answer(&mut self, id: &str, command: &str, answers: &[Answer], ctx: &Context) -> Effect {
        match self.enabled_mut(id) {
            Some(plugin) => plugin.answer(command, answers, ctx),
            None => Effect::None,
        }
    }

    fn enabled_mut(&mut self, id: &str) -> Option<&mut Box<dyn Plugin>> {
        self.entries
            .iter_mut()
            .find(|e| e.enabled && e.plugin.manifest().id == id)
            .map(|e| &mut e.plugin)
    }

    fn renderer(&self, lang: &str) -> Option<&dyn Plugin> {
        self.entries
            .iter()
            .find(|e| e.enabled && e.plugin.block_languages().contains(&lang))
            .map(|e| e.plugin.as_ref())
    }
}

/// `key = ["a", "b"]` in the state file.
fn list(text: &str, key: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| k.trim() == key)
        .flat_map(|(_, v)| {
            v.trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The editor's code block processor: the enabled plugins' blocks.
pub struct Blocks(pub Rc<RefCell<Plugins>>);

impl CodeBlockProcessor for Blocks {
    fn handles(&self, lang: &str) -> bool {
        lang == crate::query_embed::LANGUAGE || self.0.borrow().renderer(lang).is_some()
    }

    fn render(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<Line<'static>> {
        let plugins = self.0.borrow();
        if lang == crate::query_embed::LANGUAGE {
            return plugins
                .queries
                .render(source, from, width)
                .into_iter()
                .map(|(l, _)| l)
                .collect();
        }
        plugins
            .renderer(lang)
            .map(|p| p.render_block(lang, source, from, width))
            .unwrap_or_default()
    }

    fn render_rows(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<(Line<'static>, Option<String>)> {
        let plugins = self.0.borrow();
        if lang == crate::query_embed::LANGUAGE {
            return plugins.queries.render(source, from, width);
        }
        plugins
            .renderer(lang)
            .map(|p| p.render_block_rows(lang, source, from, width))
            .unwrap_or_default()
    }

    fn render_cells(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<mdedit::processor::CellRow> {
        let plugins = self.0.borrow();
        if lang == crate::query_embed::LANGUAGE {
            return plugins
                .queries
                .render(source, from, width)
                .into_iter()
                .map(|(line, action)| {
                    let parts = action.map(|a| vec![(0, usize::MAX, a)]);
                    (line, parts.unwrap_or_default())
                })
                .collect();
        }
        plugins
            .renderer(lang)
            .map(|p| p.render_block_cells(lang, source, from, width))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn each_plugin_has_its_own_folder() {
        let dir = scratch("plugins-folders");
        let vault = Vault::open(&dir).unwrap();
        assert_eq!(
            settings_file(&vault, "templater"),
            vault
                .root
                .join(".blackglass/plugins/templater/settings.toml")
        );
        let ids: Vec<_> = catalog().iter().map(|p| p.manifest().id).collect();
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/plugins");
        for id in ids {
            // The source folder is the id's first word (periodic-notes: periodic).
            let folder = id.split('-').next().expect("an id");
            assert!(
                src.join(folder).join("mod.rs").is_file(),
                "src/plugins/{folder}/mod.rs for {id}"
            );
        }
    }

    #[test]
    fn install_enable_and_the_state_file() {
        let dir = scratch("plugins-state");
        write(&dir, &[("a.md", "")]);
        let vault = Vault::open(&dir).unwrap();
        let mut plugins = Plugins::for_vault(&vault);
        assert_eq!(plugins.statuses()[0].manifest.id, "dataview");
        assert!(
            !plugins.statuses()[0].installed,
            "nothing installed at first"
        );
        assert!(plugins.commands().is_empty());

        plugins.toggle_installed(0, &vault).unwrap();
        let s = plugins.statuses()[0];
        assert!(s.installed && s.enabled);
        assert_eq!(plugins.commands().len(), 2);
        let saved = std::fs::read_to_string(vault.root.join(STATE_FILE)).unwrap();
        assert!(saved.contains("installed = [\"dataview\"]"), "{saved}");

        // Another session reads it back.
        let mut again = Plugins::for_vault(&vault);
        assert!(again.statuses()[0].enabled);
        again.toggle_enabled(0, &vault).unwrap();
        assert!(again.commands().is_empty(), "disabled");
        let blocks = Blocks(Rc::new(RefCell::new(again)));
        assert!(
            !blocks.handles("dataview"),
            "a disabled plugin renders nothing"
        );
        blocks.0.borrow_mut().toggle_enabled(0, &vault).unwrap();
        assert!(blocks.handles("dataview"));
        blocks.0.borrow_mut().toggle_installed(0, &vault).unwrap();
        assert!(!blocks.0.borrow().statuses()[0].installed, "uninstalled");
        assert!(!blocks.handles("dataview"));
    }
}
