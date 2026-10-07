//! Commands, for the command palette (Ctrl+P) and the keyboard shortcuts:
//! blackglass's own, the editor's (run by sending mdedit its key), text to
//! insert, and the enabled plugins'. Each has an id (for `keys.toml`), a
//! name to find it by and default keys ([`crate::keymap`]).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::keymap::Chord;
use crate::switcher::score;

/// Something the workspace does itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// Tab `n` (1 is the first).
    GoToTab(usize),
    LastTab,
    GoToNote,
    NewNote,
    NewFromTemplate,
    CloseTab,
    NextTab,
    PreviousTab,
    SearchVault,
    ShowFiles,
    ShowTags,
    ToggleSidebar,
    ToggleCalendar,
    Back,
    Forward,
    SwitchFocus,
    Rescan,
    OpenPlugins,
    Quit,
    CommandPalette,
    Help,
    Settings,
    /// Extract the selection to a new note (or, without one, close the
    /// tab, as the editor's Ctrl+X does).
    Extract,
    DeleteNote,
    MoveNote,
    ToggleBacklinks,
    OpenVault,
    UniqueNote,
    RenameNote,
    RenameFolder,
    NewFolder,
    ShowProperties,
    RenameProperty,
    PreviewLink,
    PdfToNote,
    RandomNote,
    Outline,
    Footnotes,
    EditProperties,
    ChooseTheme,
    /// Undo the last change to other notes (W-133).
    UndoFiles,
    RedoFiles,
}

/// The natural language dates commands (W-128).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dates {
    /// Today's date.
    Today,
    /// The time now.
    Time,
    /// The date and time now.
    Now,
    /// The selection to a link to its day (`[[2026-10-07]]`).
    Parse,
    /// The selection to a Markdown link (`[tomorrow](2026-10-07)`).
    ParseLink,
    /// The selection to the date.
    ParsePlain,
    /// The selection to a time.
    ParseTime,
    /// Ask for a date in words.
    Picker,
}

/// The editor's modes (mdedit's live preview, source mode and view mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Edit,
    Source,
    View,
}

/// What running a command does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Host(Host),
    /// Send this key to the active editor.
    Key(KeyEvent),
    /// Insert text in the active editor, then move the cursor back.
    Insert {
        text: String,
        back: usize,
    },
    /// Insert today's date (in the Dates settings' format).
    InsertDate,
    /// A natural language dates command.
    Dates(Dates),
    /// Highlight in a color (an index into mdedit's highlight colors;
    /// `None`: no color).
    Highlight(Option<usize>),
    /// A plugin's command: (plugin id, command id).
    Plugin(&'static str, &'static str),
    /// Switch the active note's mode.
    Mode(Mode),
    /// Open a plugin's settings screen (by its id).
    PluginSettings(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// Stable: `keys.toml` uses it (the name as `kebab-case`, or
    /// `plugin:<plugin>:<command>`).
    pub id: String,
    pub name: String,
    pub defaults: Vec<Chord>,
    /// The keys that run it now, as shown (`Ctrl+S`); set from the keymap.
    pub key: String,
    pub action: Action,
}

/// A command's id from its name: `Go to note` → `go-to-note`.
pub fn id_of(name: &str) -> String {
    let mut id = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if !id.ends_with('-') && !id.is_empty() {
            id.push('-');
        }
    }
    id.trim_end_matches('-').to_string()
}

impl Command {
    /// Whether it works on the active note (then the palette hides it
    /// without one). Moving works on the explorer's choice too.
    pub fn needs_note(&self) -> bool {
        match self.action {
            Action::Host(host) => matches!(
                host,
                Host::CloseTab
                    | Host::NextTab
                    | Host::PreviousTab
                    | Host::Extract
                    | Host::DeleteNote
                    | Host::ToggleBacklinks
                    | Host::PreviewLink
                    | Host::Outline
                    | Host::Footnotes
                    | Host::EditProperties
            ),
            Action::Key(_)
            | Action::Insert { .. }
            | Action::InsertDate
            | Action::Dates(_)
            | Action::Highlight(_)
            | Action::Mode(_) => true,
            Action::Plugin(..) | Action::PluginSettings(_) => false,
        }
    }
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> Action {
    Action::Key(KeyEvent::new(code, modifiers))
}

fn ctrl(c: char) -> Action {
    key(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn insert(text: &str, back: usize) -> Action {
    Action::Insert {
        text: text.into(),
        back,
    }
}

/// blackglass's and the editor's commands, in the palette's order.
pub fn builtin() -> Vec<Command> {
    let c = |name: &str, keys: &str, action: Action| {
        let defaults = Chord::parse_list(keys).expect("the default keys are keys");
        Command {
            id: id_of(name),
            name: name.into(),
            key: crate::keymap::describe(&defaults),
            defaults,
            action,
        }
    };
    let host = |name: &str, keys: &str, h: Host| c(name, keys, Action::Host(h));
    vec![
        host("Command palette", "Ctrl+P", Host::CommandPalette),
        host("Help", "F1 / Ctrl+H", Host::Help),
        host("Settings", "Alt+, / Ctrl+,", Host::Settings),
        host("Open vault", "", Host::OpenVault),
        host("Choose theme", "", Host::ChooseTheme),
        host("Undo last change to other notes", "", Host::UndoFiles),
        host("Redo last change to other notes", "", Host::RedoFiles),
        host("Go to note", "Ctrl+O", Host::GoToNote),
        host("New note", "Ctrl+N", Host::NewNote),
        host("New note from template", "", Host::NewFromTemplate),
        host("New unique note", "", Host::UniqueNote),
        host("Open random note", "", Host::RandomNote),
        host("Outline", "Alt+O", Host::Outline),
        host("Footnotes", "", Host::Footnotes),
        host("Edit properties", "Alt+;", Host::EditProperties),
        host("PDF to note", "", Host::PdfToNote),
        c("Save", "Ctrl+S", ctrl('s')),
        c(
            "Save as",
            "Ctrl+Alt+S",
            key(
                KeyCode::Char('s'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
        ),
        host("Extract selection to a new note", "Ctrl+X", Host::Extract),
        host("Rename note", "F2", Host::RenameNote),
        host("Rename folder", "", Host::RenameFolder),
        host("New folder", "", Host::NewFolder),
        host("Show properties", "", Host::ShowProperties),
        host("Rename property", "", Host::RenameProperty),
        host("Delete note", "", Host::DeleteNote),
        host("Move note to a folder", "", Host::MoveNote),
        host("Close tab", "Ctrl+W", Host::CloseTab),
        host("Next tab", "Alt+Right / Ctrl+PgDn", Host::NextTab),
        host("Previous tab", "Alt+Left / Ctrl+PgUp", Host::PreviousTab),
        host("Go to tab 1", "Alt+1", Host::GoToTab(1)),
        host("Go to tab 2", "Alt+2", Host::GoToTab(2)),
        host("Go to tab 3", "Alt+3", Host::GoToTab(3)),
        host("Go to tab 4", "Alt+4", Host::GoToTab(4)),
        host("Go to tab 5", "Alt+5", Host::GoToTab(5)),
        host("Go to tab 6", "Alt+6", Host::GoToTab(6)),
        host("Go to tab 7", "Alt+7", Host::GoToTab(7)),
        host("Go to tab 8", "Alt+8", Host::GoToTab(8)),
        host("Go to last tab", "Alt+9", Host::LastTab),
        host(
            "Search the vault",
            "Ctrl+G / Ctrl+Shift+F",
            Host::SearchVault,
        ),
        host("Show files", "", Host::ShowFiles),
        host("Show tags", "", Host::ShowTags),
        host("Toggle sidebar", "Alt+B", Host::ToggleSidebar),
        host("Toggle calendar", "Alt+C", Host::ToggleCalendar),
        host("Toggle backlinks", "Alt+L", Host::ToggleBacklinks),
        host("Go back", "Ctrl+Alt+Left", Host::Back),
        host("Go forward", "Ctrl+Alt+Right", Host::Forward),
        host("Focus sidebar / editor", "Ctrl+B", Host::SwitchFocus),
        host("Scan the vault again", "F5", Host::Rescan),
        host("Open plugins", "", Host::OpenPlugins),
        c("Find in note", "Ctrl+F", ctrl('f')),
        c("Find and replace", "Ctrl+R", ctrl('r')),
        c("Find next", "F3", key(KeyCode::F(3), KeyModifiers::NONE)),
        c(
            "Find previous",
            "Shift+F3",
            key(KeyCode::F(3), KeyModifiers::SHIFT),
        ),
        c("Undo", "Ctrl+Z", ctrl('z')),
        c("Redo", "Ctrl+Y", ctrl('y')),
        c("Select all", "Ctrl+A", ctrl('a')),
        c("Paste", "Ctrl+V", ctrl('v')),
        c(
            "Cycle modes: live preview, source, view",
            "Alt+V",
            key(KeyCode::Char('v'), KeyModifiers::ALT),
        ),
        c("View mode (read-only)", "", Action::Mode(Mode::View)),
        c("Edit mode (live preview)", "", Action::Mode(Mode::Edit)),
        c("Source mode", "", Action::Mode(Mode::Source)),
        c("Toggle task", "Ctrl+T", ctrl('t')),
        c("Close / reopen task", "Ctrl+L", ctrl('l')),
        c("Fold / unfold", "Ctrl+K", ctrl('k')),
        c(
            "Indent list item",
            "Tab",
            key(KeyCode::Tab, KeyModifiers::NONE),
        ),
        c(
            "Unindent list item",
            "Shift+Tab",
            key(KeyCode::BackTab, KeyModifiers::SHIFT),
        ),
        c("Insert emoji", "Ctrl+E", ctrl('e')),
        c(
            "Follow link under cursor",
            "Ctrl+Enter",
            key(KeyCode::Enter, KeyModifiers::CONTROL),
        ),
        host("Preview link under cursor", "Alt+P", Host::PreviewLink),
        c("Insert hyperlink", "", insert("[]()", 3)),
        c("Insert wiki link", "", insert("[[]]", 2)),
        c("Insert embed", "", insert("![[]]", 2)),
        c("Insert tag", "", insert("#", 0)),
        c("Insert callout", "", insert("> [!note] \n> ", 3)),
        c(
            "Insert table",
            "",
            insert(
                "| Column | Column |\n| ------ | ------ |\n|        |        |",
                17,
            ),
        ),
        c("Insert code block", "", insert("```\n\n```", 4)),
        c("Insert horizontal rule", "", insert("---\n", 0)),
        c("Insert task", "", insert("- [ ] ", 0)),
        c("Insert today's date", "", Action::InsertDate),
        c("Insert the current time", "", Action::Dates(Dates::Time)),
        c(
            "Insert the current date and time",
            "",
            Action::Dates(Dates::Now),
        ),
        c(
            "Parse natural language date",
            "",
            Action::Dates(Dates::Parse),
        ),
        c(
            "Parse natural language date as link",
            "",
            Action::Dates(Dates::ParseLink),
        ),
        c(
            "Parse natural language date as plain text",
            "",
            Action::Dates(Dates::ParsePlain),
        ),
        c(
            "Parse natural language time",
            "",
            Action::Dates(Dates::ParseTime),
        ),
        c("Date picker", "", Action::Dates(Dates::Picker)),
        c("Highlight in red", "", Action::Highlight(Some(0))),
        c("Highlight in orange", "", Action::Highlight(Some(1))),
        c("Highlight in yellow", "", Action::Highlight(Some(2))),
        c("Highlight in green", "", Action::Highlight(Some(3))),
        c("Highlight in blue", "", Action::Highlight(Some(4))),
        c("Highlight in purple", "", Action::Highlight(Some(5))),
        c("Remove highlight color", "", Action::Highlight(None)),
        host("Quit", "Ctrl+Q", Host::Quit),
    ]
}

/// The command palette's state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Palette {
    pub input: String,
    /// Every command it can show.
    pub commands: Vec<Command>,
    /// Indexes into `commands`, best first.
    pub results: Vec<usize>,
    pub selected: usize,
}

impl Palette {
    pub fn new(commands: Vec<Command>) -> Self {
        let mut p = Palette {
            commands,
            ..Palette::default()
        };
        p.update();
        p
    }

    /// Ranks the commands for the input (fuzzy, like the quick switcher).
    pub fn update(&mut self) {
        self.selected = 0;
        let query = self.input.trim();
        if query.is_empty() {
            self.results = (0..self.commands.len()).collect();
            return;
        }
        let mut scored: Vec<(i64, usize)> = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(i, c)| score(&c.name, query).map(|s| (s, i)))
            .collect();
        scored.sort_by_key(|&(s, i)| (std::cmp::Reverse(s), i));
        self.results = scored.into_iter().map(|(_, i)| i).collect();
    }

    /// The highlighted command.
    pub fn chosen(&self) -> Option<&Command> {
        self.results.get(self.selected).map(|&i| &self.commands[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_filters_by_name() {
        let mut p = Palette::new(builtin());
        assert_eq!(p.results.len(), builtin().len());
        p.input = "ins hyp".into();
        p.update();
        assert_eq!(
            p.chosen().map(|c| c.name.as_str()),
            Some("Insert hyperlink")
        );
        p.input = "save".into();
        p.update();
        let names: Vec<_> = p
            .results
            .iter()
            .map(|&i| p.commands[i].name.as_str())
            .collect();
        assert_eq!(names[..2], ["Save", "Save as"]);
    }

    #[test]
    fn every_command_has_a_unique_name() {
        let mut names: Vec<_> = builtin().into_iter().map(|c| c.name).collect();
        names.sort();
        let len = names.len();
        names.dedup();
        assert_eq!(names.len(), len);
    }
}
