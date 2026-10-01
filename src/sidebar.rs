//! The left sidebar and its panels: the file explorer (folders you open
//! and close, sorted by name or date), vault search, the tags with their
//! note counts, and the enabled plugins' tabs (Recent Files'). Keys and
//! clicks here return a [`Request`] for what only the workspace can do
//! (open a note, give the editor focus, pass a key to a plugin's tab).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::vault::search::{self, NoteHits, Query};
use crate::vault::{File, Folder, Vault};

/// The sidebar's panels, in the order of their tabs: the core ones, then
/// the enabled plugins' tabs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Panel {
    #[default]
    Files,
    Search,
    Tags,
    /// A plugin's tab (Recent Files'), by the plugin's id.
    Plugin(&'static str),
}

impl Panel {
    /// The core panels.
    pub const CORE: [Panel; 3] = [Panel::Files, Panel::Search, Panel::Tags];

    /// The core panels' titles (a plugin's tab has its own).
    pub fn title(self) -> &'static str {
        match self {
            Panel::Files => "Files",
            Panel::Search => "Search",
            Panel::Tags => "Tags",
            Panel::Plugin(_) => "",
        }
    }

    /// Where its scroll position is kept (plugin tabs share one).
    fn index(self) -> usize {
        match self {
            Panel::Files => 0,
            Panel::Search => 1,
            Panel::Tags => 2,
            Panel::Plugin(_) => 3,
        }
    }
}

/// How the file explorer orders files (`s` cycles through them).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    NameAscending,
    NameDescending,
    /// Newest first.
    ModifiedNewest,
    ModifiedOldest,
}

impl Sort {
    pub fn label(self) -> &'static str {
        match self {
            Sort::NameAscending => "A→Z",
            Sort::NameDescending => "Z→A",
            Sort::ModifiedNewest => "newest",
            Sort::ModifiedOldest => "oldest",
        }
    }

    fn next(self) -> Sort {
        match self {
            Sort::NameAscending => Sort::NameDescending,
            Sort::NameDescending => Sort::ModifiedNewest,
            Sort::ModifiedNewest => Sort::ModifiedOldest,
            Sort::ModifiedOldest => Sort::NameAscending,
        }
    }
}

/// What a file explorer row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Folder {
        expanded: bool,
    },
    Note,
    /// Any other file (images, PDFs …).
    Attachment,
}

/// One row of the file explorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    pub path: PathBuf,
    pub name: String,
    /// 0 for the vault's top level.
    pub depth: usize,
    pub kind: RowKind,
    /// The top-level folder this row is in (or is), by its index in the
    /// vault's folders: it picks the row's color.
    pub color: Option<usize>,
}

/// One row of the search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchRow {
    /// A matching note; `hits` indexes [`Sidebar::results`].
    Note { hits: usize },
    /// A matching line of that note.
    Line {
        hits: usize,
        line: usize,
        col: usize,
    },
    /// "N more" matches not listed.
    More { hits: usize, count: usize },
}

/// What the workspace should do after a sidebar key or click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    None,
    /// Open (or show) a file, at a position, with a search to remember
    /// for F3.
    Open {
        path: PathBuf,
        at: Option<(usize, usize)>,
        search: Option<String>,
    },
    /// Give the editor the focus.
    FocusEditor,
    /// A key on row `row` of a plugin's tab (Enter for a click).
    PluginKey {
        id: &'static str,
        row: usize,
        key: KeyEvent,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Sidebar {
    pub panel: Panel,
    pub visible: bool,
    /// Open folders (absolute paths).
    pub expanded: HashSet<PathBuf>,
    pub sort: Sort,
    /// The file explorer's selected row, by path so it survives changes
    /// to the tree.
    pub selected: Option<PathBuf>,
    /// The search input and its results.
    pub query: String,
    pub results: Vec<NoteHits>,
    /// The selected row in the search results and the tags.
    pub search_cursor: usize,
    pub tags_cursor: usize,
    /// The open tags of the tag tree (lowercase: `project`).
    pub tags_open: HashSet<String>,
    /// First row shown, per panel (plugin tabs share the last).
    pub scroll: [usize; 4],
    /// The enabled plugins' tabs: (plugin id, title); and the shown
    /// plugin tab's selected row and row count (the workspace keeps them
    /// up to date).
    pub plugin_tabs: Vec<(&'static str, &'static str)>,
    pub plugin_cursor: usize,
    pub plugin_rows: usize,
    /// Rows the list had room for in the last frame (for Page Up/Down and
    /// keeping the selection on screen).
    pub height: usize,
}

impl Sidebar {
    pub fn new() -> Self {
        Sidebar {
            visible: true,
            height: 20,
            ..Sidebar::default()
        }
    }

    /// The file explorer's rows: open folders show their contents.
    pub fn file_rows(&self, vault: &Vault) -> Vec<FileRow> {
        let mut rows = Vec::new();
        self.walk(&vault.tree, 0, None, &mut rows);
        rows
    }

    fn walk(&self, folder: &Folder, depth: usize, color: Option<usize>, rows: &mut Vec<FileRow>) {
        // Colors follow the top-level folders' alphabetical order, so a
        // folder keeps its color whatever the sort.
        let mut folders: Vec<(usize, &Folder)> = folder.folders.iter().enumerate().collect();
        if self.sort == Sort::NameDescending {
            folders.reverse();
        }
        for (i, sub) in folders {
            let expanded = self.expanded.contains(&sub.path);
            let color = color.or(Some(i));
            rows.push(FileRow {
                path: sub.path.clone(),
                name: sub.name.clone(),
                depth,
                kind: RowKind::Folder { expanded },
                color,
            });
            if expanded {
                self.walk(sub, depth + 1, color, rows);
            }
        }
        let mut files: Vec<&File> = folder.files.iter().collect();
        match self.sort {
            Sort::NameAscending => {}
            Sort::NameDescending => files.reverse(),
            Sort::ModifiedNewest => files.sort_by_key(|f| std::cmp::Reverse(f.modified)),
            Sort::ModifiedOldest => files.sort_by_key(|f| f.modified),
        }
        rows.extend(files.into_iter().map(|f| FileRow {
            path: f.path.clone(),
            name: f.display_name().to_string(),
            depth,
            kind: if f.is_note() {
                RowKind::Note
            } else {
                RowKind::Attachment
            },
            color,
        }));
    }

    /// The search results' rows.
    pub fn search_rows(&self) -> Vec<SearchRow> {
        let mut rows = Vec::new();
        for (i, h) in self.results.iter().enumerate() {
            rows.push(SearchRow::Note { hits: i });
            rows.extend(
                h.lines
                    .iter()
                    .map(|&(line, col)| SearchRow::Line { hits: i, line, col }),
            );
            if h.total > h.lines.len() {
                rows.push(SearchRow::More {
                    hits: i,
                    count: h.total - h.lines.len(),
                });
            }
        }
        rows
    }

    /// Runs the search again (after the query or the vault changed).
    pub fn update_search(&mut self, vault: &Vault) {
        self.results = search::run(vault, &Query::parse(&self.query));
        self.search_cursor = self
            .search_cursor
            .min(self.search_rows().len().saturating_sub(1));
    }

    /// Shows the search panel with `query` (e.g. `tag:work`).
    pub fn search(&mut self, query: &str, vault: &Vault) {
        self.visible = true;
        self.panel = Panel::Search;
        self.query = query.to_string();
        self.search_cursor = 0;
        self.scroll[Panel::Search.index()] = 0;
        self.update_search(vault);
    }

    /// The selected row of the current panel, and how many rows it has.
    pub fn cursor(&self, vault: &Vault) -> (usize, usize) {
        match self.panel {
            Panel::Files => {
                let rows = self.file_rows(vault);
                let at = self
                    .selected
                    .as_ref()
                    .and_then(|p| rows.iter().position(|r| &r.path == p))
                    .unwrap_or(0);
                (at, rows.len())
            }
            Panel::Search => (self.search_cursor, self.search_rows().len()),
            Panel::Tags => (self.tags_cursor, self.tag_rows(vault).len()),
            Panel::Plugin(_) => (self.plugin_cursor, self.plugin_rows),
        }
    }

    /// The Tags panel's rows: the vault's tags as a tree.
    pub fn tag_rows(&self, vault: &Vault) -> Vec<crate::vault::tags::TagRow> {
        crate::vault::tags::tree(
            vault.notes.iter().map(|n| n.tags.as_slice()),
            &self.tags_open,
        )
    }

    /// The first row shown in the current panel.
    pub fn scroll(&self) -> usize {
        self.scroll[self.panel.index()]
    }

    fn select(&mut self, at: usize, vault: &Vault) {
        match self.panel {
            Panel::Files => {
                self.selected = self.file_rows(vault).get(at).map(|r| r.path.clone());
            }
            Panel::Search => self.search_cursor = at,
            Panel::Tags => self.tags_cursor = at,
            Panel::Plugin(_) => self.plugin_cursor = at,
        }
        // Keep the selection on screen.
        let height = self.height.max(1);
        let scroll = &mut self.scroll[self.panel.index()];
        if at < *scroll {
            *scroll = at;
        } else if at >= *scroll + height {
            *scroll = at + 1 - height;
        }
    }

    /// Scrolls the current panel by `delta` rows (mouse wheel), without
    /// moving the selection.
    pub fn scroll_by(&mut self, delta: isize, vault: &Vault) {
        let (_, len) = self.cursor(vault);
        let max = len.saturating_sub(self.height.max(1));
        let scroll = &mut self.scroll[self.panel.index()];
        *scroll = scroll.saturating_add_signed(delta).min(max);
    }

    /// The folder new notes go to: the selected folder, or the folder of
    /// the selected file, or the vault root.
    pub fn target_folder(&self, vault: &Vault) -> PathBuf {
        match &self.selected {
            Some(p) if p.is_dir() => p.clone(),
            Some(p) => p
                .parent()
                .filter(|d| d.starts_with(&vault.root))
                .map_or(vault.root.clone(), Path::to_path_buf),
            None => vault.root.clone(),
        }
    }

    /// Opens the folders around `path` and selects it in the file explorer.
    pub fn reveal(&mut self, path: &Path, vault: &Vault) {
        for dir in path.ancestors().skip(1) {
            if dir == vault.root || !dir.starts_with(&vault.root) {
                break;
            }
            self.expanded.insert(dir.to_path_buf());
        }
        self.selected = Some(path.to_path_buf());
        let panel = self.panel;
        self.panel = Panel::Files;
        let (at, _) = self.cursor(vault);
        self.select(at, vault);
        self.panel = panel;
    }

    /// The tabs, in order: the core panels, then the plugins'.
    pub fn panels(&self) -> Vec<Panel> {
        let plugins = self.plugin_tabs.iter().map(|&(id, _)| Panel::Plugin(id));
        Panel::CORE.into_iter().chain(plugins).collect()
    }

    /// A tab's title.
    pub fn title(&self, panel: Panel) -> &'static str {
        match panel {
            Panel::Plugin(id) => self
                .plugin_tabs
                .iter()
                .find(|&&(i, _)| i == id)
                .map_or("", |&(_, title)| title),
            core => core.title(),
        }
    }

    /// The next (or previous) tab.
    fn step(&self, forward: bool) -> Panel {
        let panels = self.panels();
        let n = panels.len();
        let at = panels.iter().position(|&p| p == self.panel).unwrap_or(0);
        let i = if forward { at + 1 } else { at + n - 1 };
        panels[i % n]
    }

    /// Shows `panel` (from its tab).
    pub fn show(&mut self, panel: Panel) {
        self.visible = true;
        self.panel = panel;
    }

    pub fn handle_key(&mut self, key: KeyEvent, vault: &Vault) -> Request {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let (at, len) = self.cursor(vault);
        let last = len.saturating_sub(1);
        let page = self.height.max(2) - 1;
        let to = match key.code {
            KeyCode::Tab => {
                self.panel = self.step(true);
                return Request::None;
            }
            KeyCode::BackTab => {
                self.panel = self.step(false);
                return Request::None;
            }
            KeyCode::Esc => return Request::FocusEditor,
            KeyCode::Up => at.saturating_sub(1),
            KeyCode::Down => (at + 1).min(last),
            KeyCode::PageUp => at.saturating_sub(page),
            KeyCode::PageDown => (at + page).min(last),
            KeyCode::Home if self.panel != Panel::Search => 0,
            KeyCode::End if self.panel != Panel::Search => last,
            KeyCode::Enter => return self.activate(at, vault),
            _ => return self.panel_key(key, ctrl, at, vault),
        };
        self.select(to, vault);
        Request::None
    }

    /// Keys only one panel uses (a plugin's tab gets the rest).
    fn panel_key(&mut self, key: KeyEvent, ctrl: bool, at: usize, vault: &Vault) -> Request {
        if let Panel::Plugin(id) = self.panel {
            return Request::PluginKey { id, row: at, key };
        }
        match (self.panel, key.code) {
            (Panel::Files, KeyCode::Char(' ')) => return self.activate(at, vault),
            (Panel::Files, KeyCode::Char('s')) => self.sort = self.sort.next(),
            (Panel::Files, KeyCode::Right | KeyCode::Left) => {
                let expand = key.code == KeyCode::Right;
                let rows = self.file_rows(vault);
                let Some(row) = rows.get(at) else {
                    return Request::None;
                };
                match row.kind {
                    RowKind::Folder { expanded } if expanded != expand => {
                        self.toggle(&row.path);
                    }
                    // ← on a file or a closed folder: go to its folder.
                    _ if !expand => {
                        if let Some(up) = rows[..at].iter().rposition(|r| r.depth < row.depth) {
                            self.select(up, vault);
                        }
                    }
                    _ => {}
                }
            }
            (Panel::Tags, KeyCode::Right | KeyCode::Left) => {
                let open = key.code == KeyCode::Right;
                let rows = self.tag_rows(vault);
                let Some(row) = rows.get(at) else {
                    return Request::None;
                };
                if row.nested && row.open != open {
                    let key = row.name.to_lowercase();
                    if open {
                        self.tags_open.insert(key);
                    } else {
                        self.tags_open.remove(&key);
                    }
                } else if !open
                    && let Some(up) = rows[..at].iter().rposition(|r| r.depth < row.depth)
                {
                    // ← on a tag without nested ones open: its parent.
                    self.select(up, vault);
                }
            }
            (Panel::Search, KeyCode::Char('u')) if ctrl => {
                self.query.clear();
                self.update_search(vault);
            }
            (Panel::Search, KeyCode::Char(c)) if !ctrl => {
                self.query.push(c);
                self.search_cursor = 0;
                self.update_search(vault);
            }
            (Panel::Search, KeyCode::Backspace) => {
                self.query.pop();
                self.search_cursor = 0;
                self.update_search(vault);
            }
            _ => {}
        }
        Request::None
    }

    /// Adds typed or pasted text to the search input.
    pub fn paste(&mut self, text: &str, vault: &Vault) {
        if self.panel == Panel::Search {
            self.query.push_str(&text.replace(['\r', '\n'], " "));
            self.update_search(vault);
        }
    }

    fn toggle(&mut self, folder: &Path) {
        if !self.expanded.remove(folder) {
            self.expanded.insert(folder.to_path_buf());
        }
    }

    /// Enter (or a click) on row `at`: opens a folder or file, a search
    /// result, or searches for a tag.
    fn activate(&mut self, at: usize, vault: &Vault) -> Request {
        self.select(at, vault);
        match self.panel {
            Panel::Files => {
                let Some(row) = self.file_rows(vault).into_iter().nth(at) else {
                    return Request::None;
                };
                if let RowKind::Folder { .. } = row.kind {
                    self.toggle(&row.path);
                    return Request::None;
                }
                Request::Open {
                    path: row.path,
                    at: None,
                    search: None,
                }
            }
            Panel::Search => {
                let Some(&row) = self.search_rows().get(at) else {
                    return Request::None;
                };
                let (hits, at) = match row {
                    SearchRow::Note { hits } | SearchRow::More { hits, .. } => (hits, None),
                    SearchRow::Line { hits, line, col } => (hits, Some((line, col))),
                };
                Request::Open {
                    path: vault.notes[self.results[hits].note].path.clone(),
                    at,
                    search: Query::parse(&self.query).highlight(),
                }
            }
            Panel::Tags => {
                if let Some(tag) = self.tag_rows(vault).get(at) {
                    let query = format!("tag:#{}", tag.name);
                    self.search(&query, vault);
                }
                Request::None
            }
            Panel::Plugin(id) => Request::PluginKey {
                id,
                row: at,
                key: KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            },
        }
    }

    /// A click on the list's row `row` (counted from the first row shown).
    pub fn click(&mut self, row: usize, vault: &Vault) -> Request {
        let at = self.scroll() + row;
        if at >= self.cursor(vault).1 {
            return Request::None;
        }
        self.activate(at, vault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn vault(name: &str) -> Vault {
        let dir = scratch(name);
        write(
            &dir,
            &[
                ("Articles/one.md", "#idea"),
                ("Journal/2026-08-09.md", "Pasta #food"),
                ("Journal/2026-08-10.md", "Pasta again"),
                ("Journal/Old/2025.md", ""),
                ("assets/pic.png", ""),
                ("Home.md", "#food"),
            ],
        );
        Vault::open(&dir).unwrap()
    }

    fn names(rows: &[FileRow]) -> Vec<String> {
        rows.iter()
            .map(|r| format!("{}{}", "  ".repeat(r.depth), r.name))
            .collect()
    }

    #[test]
    fn folders_open_and_close_and_their_rows_take_the_folder_color() {
        let v = vault("sidebar-tree");
        let mut s = Sidebar::new();
        assert_eq!(
            names(&s.file_rows(&v)),
            ["Articles", "assets", "Journal", "Home"]
        );
        s.handle_key(key(KeyCode::Down), &v);
        s.handle_key(key(KeyCode::Down), &v);
        assert_eq!(s.handle_key(key(KeyCode::Enter), &v), Request::None);
        let rows = s.file_rows(&v);
        assert_eq!(
            names(&rows),
            [
                "Articles",
                "assets",
                "Journal",
                "  Old",
                "  2026-08-09",
                "  2026-08-10",
                "Home"
            ]
        );
        assert_eq!(rows[4].color, Some(2), "Journal's color");
        assert_eq!(rows[6].color, None, "a note at the top level");
        s.handle_key(key(KeyCode::Down), &v);
        s.handle_key(key(KeyCode::Right), &v);
        assert_eq!(s.file_rows(&v)[4].name, "2025", "→ opens a folder");
        s.handle_key(key(KeyCode::Down), &v);
        s.handle_key(key(KeyCode::Left), &v);
        assert_eq!(s.cursor(&v).0, 3, "← on a file goes to its folder");
        s.handle_key(key(KeyCode::Left), &v);
        assert_eq!(s.file_rows(&v).len(), 7, "← closes it");
    }

    #[test]
    fn enter_on_a_note_asks_to_open_it() {
        let v = vault("sidebar-open");
        let mut s = Sidebar::new();
        s.handle_key(key(KeyCode::End), &v);
        assert_eq!(
            s.handle_key(key(KeyCode::Enter), &v),
            Request::Open {
                path: v.root.join("Home.md"),
                at: None,
                search: None
            }
        );
        assert_eq!(s.handle_key(key(KeyCode::Esc), &v), Request::FocusEditor);
    }

    #[test]
    fn sorting_cycles_and_keeps_folder_colors() {
        let v = vault("sidebar-sort");
        let mut s = Sidebar::new();
        s.expanded.insert(v.root.join("Journal"));
        s.handle_key(key(KeyCode::Char('s')), &v);
        assert_eq!(s.sort, Sort::NameDescending);
        let rows = s.file_rows(&v);
        assert_eq!(
            names(&rows)[..4],
            ["Journal", "  Old", "  2026-08-10", "  2026-08-09"]
        );
        assert_eq!(rows[0].color, Some(2));
        for _ in 0..3 {
            s.handle_key(key(KeyCode::Char('s')), &v);
        }
        assert_eq!(s.sort, Sort::NameAscending);
    }

    #[test]
    fn typing_in_the_search_panel_searches_the_vault() {
        let v = vault("sidebar-search");
        let mut s = Sidebar::new();
        s.handle_key(key(KeyCode::Tab), &v);
        assert_eq!(s.panel, Panel::Search);
        for c in "pasta".chars() {
            s.handle_key(key(KeyCode::Char(c)), &v);
        }
        assert_eq!(s.results.len(), 2);
        assert_eq!(
            s.search_rows()[..2],
            [
                SearchRow::Note { hits: 0 },
                SearchRow::Line {
                    hits: 0,
                    line: 0,
                    col: 0
                }
            ]
        );
        s.handle_key(key(KeyCode::Down), &v);
        assert_eq!(
            s.handle_key(key(KeyCode::Enter), &v),
            Request::Open {
                path: v.root.join("Journal/2026-08-09.md"),
                at: Some((0, 0)),
                search: Some("pasta".into())
            }
        );
        s.handle_key(key(KeyCode::Backspace), &v);
        assert_eq!(s.query, "past");
    }

    #[test]
    fn a_tag_opens_a_tag_search() {
        let v = vault("sidebar-tags");
        let mut s = Sidebar::new();
        s.show(Panel::Tags);
        assert_eq!(s.cursor(&v), (0, 2));
        assert_eq!(s.handle_key(key(KeyCode::Enter), &v), Request::None);
        assert_eq!(s.panel, Panel::Search);
        assert_eq!(s.query, "tag:#food");
        assert_eq!(s.results.len(), 2);
    }

    #[test]
    fn reveal_opens_the_folders_around_a_file() {
        let v = vault("sidebar-reveal");
        let mut s = Sidebar::new();
        let file = v.root.join("Journal/Old/2025.md");
        s.reveal(&file, &v);
        assert!(s.expanded.contains(&v.root.join("Journal")));
        assert!(s.expanded.contains(&v.root.join("Journal/Old")));
        assert_eq!(s.selected.as_deref(), Some(file.as_path()));
        assert_eq!(s.target_folder(&v), v.root.join("Journal/Old"));
    }

    #[test]
    fn the_selection_stays_on_screen_and_the_wheel_scrolls() {
        let v = vault("sidebar-scroll");
        let mut s = Sidebar::new();
        s.height = 2;
        s.handle_key(key(KeyCode::End), &v);
        assert_eq!(s.scroll(), 2);
        s.scroll_by(-5, &v);
        assert_eq!(s.scroll(), 0);
        s.scroll_by(10, &v);
        assert_eq!(s.scroll(), 2, "no further than the last page");
        assert_eq!(
            s.click(1, &v),
            Request::Open {
                path: v.root.join("Home.md"),
                at: None,
                search: None
            }
        );
    }
}
