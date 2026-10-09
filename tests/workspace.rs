//! The workspace as a user drives it: keys and clicks in, the screen out
//! (ratatui's `TestBackend`), on a copy of a small vault.

use std::fs;
use std::path::{Path, PathBuf};

use blackglass::plugins::STATE_FILE;
use blackglass::sidebar::Panel;
use blackglass::ui;
use blackglass::vault::Vault;
use blackglass::workspace::{Action, App, Focus, Page, Prompt};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// A fresh vault under Cargo's temp dir with `files` (path, text).
fn vault(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("workspace")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    for (rel, text) in files {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    mdedit::platform::canonical(&dir).unwrap()
}

const NOTES: &[(&str, &str)] = &[
    ("Welcome.md", "# Welcome\nSee [[Dune]].\n#start"),
    ("Books/Dune.md", "---\ntags: [reading]\n---\n# Dune\nspice"),
    (
        "Journal/2026-08-09.md",
        "<< [[Dune|yesterday]] >>\n- read #reading",
    ),
    ("Journal/2026-08-10.md", "- more spice"),
];

fn app(name: &str) -> App {
    App::new(Vault::open(&vault(name, NOTES)).unwrap())
}

fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Action {
    app.handle_key(KeyEvent::new(code, modifiers))
}

fn key(app: &mut App, code: KeyCode) -> Action {
    press(app, code, KeyModifiers::NONE)
}

fn ctrl(app: &mut App, c: char) -> Action {
    press(app, KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn alt(app: &mut App, code: KeyCode) -> Action {
    press(app, code, KeyModifiers::ALT)
}

fn typing(app: &mut App, text: &str) {
    for c in text.chars() {
        key(app, KeyCode::Char(c));
    }
}

fn click(app: &mut App, x: u16, y: u16) {
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    });
}

/// Draws the workspace and returns the screen's rows.
fn screen(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    let buf = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let mut row = String::new();
            let mut skip = 0;
            for x in 0..width {
                if skip > 0 {
                    skip -= 1;
                    continue;
                }
                let symbol = buf[(x, y)].symbol();
                skip = unicode_width::UnicodeWidthStr::width(symbol).saturating_sub(1);
                row.push_str(symbol);
            }
            row.trim_end().to_string()
        })
        .collect()
}

fn find(rows: &[String], text: &str) -> Option<(usize, usize)> {
    rows.iter().enumerate().find_map(|(y, r)| {
        r.find(text)
            .map(|b| (y, unicode_width::UnicodeWidthStr::width(&r[..b])))
    })
}

fn note(app: &App, rel: &str) -> PathBuf {
    app.vault.root.join(rel)
}

fn active_path(app: &App) -> Option<PathBuf> {
    app.view().and_then(|v| v.path.clone())
}

#[test]
fn the_screen_is_laid_out_like_obsidian() {
    let mut app = app("layout");
    app.sidebar.expanded.insert(note(&app, "Journal"));
    let journal = note(&app, "Journal/2026-08-09.md");
    app.open(&journal);
    let rows = screen(&mut app, 90, 16);
    // The sidebar's panels, then the tab bar.
    assert!(rows[0].starts_with(" Files  Search  Tags"), "{:?}", rows[0]);
    assert!(rows[0].contains("│ 2026-08-09 ×  +"), "{:?}", rows[0]);
    // The vault's name and sort; the note's folder path.
    assert!(rows[1].starts_with(" layout"), "{:?}", rows[1]);
    assert!(
        rows[1].contains("A→Z │ Journal / 2026-08-09"),
        "{:?}",
        rows[1]
    );
    // Folders with their color bar; notes inside an open folder.
    assert!(rows[2].starts_with("▌▸ Books"), "{:?}", rows[2]);
    assert!(rows[3].starts_with("▌▾ Journal"), "{:?}", rows[3]);
    assert!(rows[4].starts_with("▏    2026-08-09"), "{:?}", rows[4]);
    assert!(rows[6].starts_with("   Welcome"), "{:?}", rows[6]);
    // The note's name as its title, then its text, rendered by mdedit.
    assert!(rows[3].ends_with("│  2026-08-09"), "{:?}", rows[3]);
    assert!(
        rows[5].ends_with("<< [[Dune|yesterday]] >>"),
        "cursor line, raw: {:?}",
        rows[5]
    );
    assert!(rows[6].ends_with("• read #reading"), "{:?}", rows[6]);
    // The status bar.
    assert!(
        rows[15].starts_with(" Journal/2026-08-09.md  Ln 1, Col 1"),
        "{:?}",
        rows[15]
    );
    assert!(rows[15].ends_with("^N new  ^W close"), "{:?}", rows[15]);
    // The cursor is in the editor.
    assert_eq!(app.view().unwrap().cursor.map(|p| p.y), Some(5));
}

#[test]
fn without_a_note_the_editor_side_says_what_to_do() {
    let mut app = app("empty");
    let rows = screen(&mut app, 80, 14);
    assert!(find(&rows, "No file is open").is_some(), "{rows:#?}");
    assert!(find(&rows, "Go to file").is_some());
    let (y, _) = find(&rows, "Help").expect("how to get help");
    assert!(rows[y].contains("F1 / Ctrl+H"), "{:?}", rows[y]);
    assert_eq!(app.focus, Focus::Sidebar);
    // The keys shown are the keys now.
    app.keymap.apply("help = \"F2\"\n");
    let rows = screen(&mut app, 80, 14);
    let (y, _) = find(&rows, "Help").unwrap();
    assert!(
        rows[y].contains("F2") && !rows[y].contains("F1"),
        "{:?}",
        rows[y]
    );
}

#[test]
fn notes_open_from_the_file_explorer_in_tabs() {
    let mut app = app("explorer");
    // Books is the first row: open it, then its note.
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Books/Dune.md")));
    assert_eq!(app.focus, Focus::Editor);
    // Back to the sidebar; Welcome opens in a second tab.
    ctrl(&mut app, 'b');
    assert_eq!(app.focus, Focus::Sidebar);
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(active_path(&app), Some(note(&app, "Welcome.md")));
    // Opening an open note shows its tab.
    let dune = note(&app, "Books/Dune.md");
    app.open(&dune);
    assert_eq!((app.tabs.len(), app.active), (2, 0));
}

#[test]
fn links_find_notes_anywhere_in_the_vault() {
    let mut app = app("links");
    let welcome = note(&app, "Welcome.md");
    app.open(&welcome);
    key(&mut app, KeyCode::Down);
    for _ in 0..6 {
        key(&mut app, KeyCode::Right);
    }
    press(&mut app, KeyCode::Enter, KeyModifiers::CONTROL);
    assert_eq!(app.tabs.len(), 2, "{}", app.view().unwrap().status);
    assert_eq!(active_path(&app), Some(note(&app, "Books/Dune.md")));
}

#[test]
fn the_quick_switcher_opens_and_creates_notes() {
    let mut app = app("switcher");
    ctrl(&mut app, 'o');
    typing(&mut app, "dune");
    let rows = screen(&mut app, 90, 20);
    assert!(find(&rows, "Go to note").is_some(), "{rows:#?}");
    assert!(find(&rows, "Dune  Books").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    assert!(app.prompt.is_none());
    assert_eq!(active_path(&app), Some(note(&app, "Books/Dune.md")));

    // Again, from the editor (where Ctrl+O is mdedit's "open"); a new
    // name offers "Create".
    ctrl(&mut app, 'o');
    assert!(matches!(app.prompt, Some(Prompt::Switcher(_))));
    typing(&mut app, "Ideas");
    let rows = screen(&mut app, 90, 20);
    assert!(find(&rows, "+ Create “Ideas”").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    let ideas = note(&app, "Ideas.md");
    assert!(ideas.is_file());
    assert_eq!(active_path(&app), Some(ideas));
    assert!(app.vault.note(&note(&app, "Ideas.md")).is_some(), "indexed");
}

#[test]
fn new_notes_go_to_the_selected_folder() {
    let mut app = app("new-note");
    key(&mut app, KeyCode::Down); // Journal
    ctrl(&mut app, 'n');
    let rows = screen(&mut app, 90, 20);
    assert!(find(&rows, "In Journal/").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    assert!(note(&app, "Journal/Untitled.md").is_file());
    assert_eq!(active_path(&app), Some(note(&app, "Journal/Untitled.md")));
    // The explorer shows it, selected; the next one is Untitled 1.
    assert_eq!(
        app.sidebar.selected,
        Some(note(&app, "Journal/Untitled.md"))
    );
    ctrl(&mut app, 'n');
    key(&mut app, KeyCode::Enter);
    assert!(note(&app, "Journal/Untitled 1.md").is_file());
    // A typed name, with a new folder.
    ctrl(&mut app, 'n');
    typing(&mut app, "Trips/Rome");
    key(&mut app, KeyCode::Enter);
    assert!(note(&app, "Journal/Trips/Rome.md").is_file());
    // Names can't leave the vault.
    ctrl(&mut app, 'n');
    typing(&mut app, "../../escape");
    key(&mut app, KeyCode::Enter);
    assert!(
        app.message.contains("can't leave the vault"),
        "{}",
        app.message
    );
}

#[test]
fn unsaved_changes_are_asked_about() {
    let mut app = app("unsaved");
    let path = note(&app, "Journal/2026-08-10.md");
    app.open(&path);
    typing(&mut app, "x");
    ctrl(&mut app, 'w');
    assert!(matches!(app.prompt, Some(Prompt::Unsaved { .. })));
    let rows = screen(&mut app, 90, 20);
    assert!(
        find(&rows, "Save 2026-08-10.md before closing?").is_some(),
        "{rows:#?}"
    );
    key(&mut app, KeyCode::Char('c'));
    assert_eq!(app.tabs.len(), 1, "cancelled");
    ctrl(&mut app, 'w');
    key(&mut app, KeyCode::Char('n'));
    assert!(app.tabs.is_empty(), "closed without saving");
    assert_eq!(fs::read_to_string(&path).unwrap(), "- more spice");

    app.open(&path);
    typing(&mut app, "y");
    assert_eq!(ctrl(&mut app, 'q'), Action::Continue, "asks first");
    assert_eq!(key(&mut app, KeyCode::Char('y')), Action::Quit);
    assert_eq!(fs::read_to_string(&path).unwrap(), "y- more spice");
}

#[test]
fn saving_a_note_updates_tags_and_search() {
    let mut app = app("save-index");
    let path = note(&app, "Journal/2026-08-10.md");
    app.open(&path);
    typing(&mut app, "#fresh ");
    ctrl(&mut app, 's');
    assert!(!app.view().unwrap().is_dirty());
    assert!(
        app.vault.tags.iter().any(|t| t.name == "fresh"),
        "{:?}",
        app.vault.tags
    );
    ctrl(&mut app, 'g');
    typing(&mut app, "tag:fresh");
    assert_eq!(app.sidebar.results.len(), 1);
}

#[test]
fn a_search_result_opens_at_the_match() {
    let mut app = app("search");
    ctrl(&mut app, 'g');
    assert_eq!(
        (app.focus, app.sidebar.panel),
        (Focus::Sidebar, Panel::Search)
    );
    typing(&mut app, "spice");
    let rows = screen(&mut app, 90, 20);
    assert!(rows[1].starts_with(" ⌕ spice"), "{:?}", rows[1]);
    assert!(rows[2].starts_with(" Books/Dune"), "{:?}", rows[2]);
    assert!(rows[3].starts_with("   spice"), "{:?}", rows[3]);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    let view = app.view().unwrap();
    assert_eq!(view.path, Some(note(&app, "Books/Dune.md")));
    assert_eq!((view.editor.row, view.editor.col), (4, 0));
    assert_eq!(view.last_search, "spice", "F3 finds the next one");
}

#[test]
fn tabs_switch_with_alt_keys() {
    let mut app = app("tab-keys");
    for rel in ["Welcome.md", "Books/Dune.md", "Journal/2026-08-10.md"] {
        let path = note(&app, rel);
        app.open(&path);
    }
    assert_eq!(app.active, 2);
    alt(&mut app, KeyCode::Char('1'));
    assert_eq!(app.active, 0);
    alt(&mut app, KeyCode::Left);
    assert_eq!(app.active, 2, "wraps around");
    alt(&mut app, KeyCode::Char('2'));
    alt(&mut app, KeyCode::Char('9'));
    assert_eq!(app.active, 2, "9: the last tab");
    press(&mut app, KeyCode::PageUp, KeyModifiers::CONTROL);
    assert_eq!(app.active, 1);
    // Ctrl+X (mdedit's close) closes the tab too.
    ctrl(&mut app, 'x');
    assert_eq!(app.tabs.len(), 2);
}

#[test]
fn the_mouse_opens_files_switches_tabs_and_panels() {
    let mut app = app("mouse");
    let rows = screen(&mut app, 90, 20);
    let (y, x) = find(&rows, "Welcome").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(active_path(&app), Some(note(&app, "Welcome.md")));
    let rows = screen(&mut app, 90, 20);
    let (y, x) = find(&rows, "Books").unwrap();
    click(&mut app, x as u16, y as u16);
    let rows = screen(&mut app, 90, 20);
    let (y, x) = find(&rows, "Dune").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(app.tabs.len(), 2);
    // The tabs: click the first, then close it.
    let rows = screen(&mut app, 90, 20);
    let (y, x) = find(&rows, "Welcome ×").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(app.active, 0);
    click(&mut app, (x + "Welcome ".len()) as u16, y as u16);
    assert_eq!(app.tabs.len(), 1);
    // The panel tabs.
    let (y, x) = find(&rows, "Tags").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(app.sidebar.panel, Panel::Tags);
}

#[test]
fn save_as_moves_the_note_within_the_vault() {
    let mut app = app("save-as");
    let path = note(&app, "Welcome.md");
    app.open(&path);
    press(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    assert_eq!(
        app.prompt,
        Some(Prompt::SaveAs {
            input: "Welcome".into()
        })
    );
    for _ in 0.."Welcome".len() {
        key(&mut app, KeyCode::Backspace);
    }
    typing(&mut app, "Archive/Old welcome");
    key(&mut app, KeyCode::Enter);
    let moved = note(&app, "Archive/Old welcome.md");
    assert!(moved.is_file(), "{}", app.message);
    assert_eq!(active_path(&app), Some(moved));
    assert!(
        app.vault
            .note(&note(&app, "Archive/Old welcome.md"))
            .is_some()
    );
}

#[test]
fn every_screen_size_draws_without_panicking() {
    let mut app = app("sizes");
    let path = note(&app, "Books/Dune.md");
    app.open(&path);
    app.sidebar.expanded.insert(note(&app, "Journal"));
    for (w, h) in [
        (0, 0),
        (0, 5),
        (5, 0),
        (1, 1),
        (10, 3),
        (39, 5),
        (40, 4),
        (45, 9),
        (60, 12),
        (200, 60),
    ] {
        screen(&mut app, w, h);
        ctrl(&mut app, 'p');
        screen(&mut app, w, h);
        key(&mut app, KeyCode::Esc);
        ctrl(&mut app, 'n');
        screen(&mut app, w, h);
        key(&mut app, KeyCode::Esc);
        for panel in [Panel::Search, Panel::Tags, Panel::Files] {
            app.sidebar.show(panel);
            screen(&mut app, w, h);
        }
    }
}

#[test]
fn the_quick_switcher_opens_from_the_sidebar_too() {
    let mut app = app("switcher-sidebar");
    assert_eq!(app.focus, Focus::Sidebar);
    ctrl(&mut app, 'o');
    assert!(matches!(app.prompt, Some(Prompt::Switcher(_))), "Ctrl+O");
    key(&mut app, KeyCode::Esc);
    ctrl(&mut app, 'p');
    assert!(
        matches!(app.prompt, Some(Prompt::Palette(_))),
        "Ctrl+P: commands"
    );
}

#[test]
fn the_command_palette_lists_and_runs_commands() {
    let mut app = app("palette");
    ctrl(&mut app, 'p');
    let rows = screen(&mut app, 90, 40);
    assert!(find(&rows, "Commands").is_some(), "{rows:#?}");
    assert!(find(&rows, "Go to note").is_some(), "{rows:#?}");
    assert!(find(&rows, "Open vault").is_some(), "{rows:#?}");
    assert!(
        find(&rows, "Save").is_none(),
        "no note open: no editor commands"
    );
    typing(&mut app, "new note");
    key(&mut app, KeyCode::Enter);
    assert!(
        matches!(app.prompt, Some(Prompt::NewNote { .. })),
        "ran New note"
    );

    // With a note: editor commands, with their keys.
    key(&mut app, KeyCode::Esc);
    let path = note(&app, "Journal/2026-08-10.md");
    app.open(&path);
    ctrl(&mut app, 'p');
    typing(&mut app, "undo");
    let rows = screen(&mut app, 90, 30);
    let (y, _) = find(&rows, "Undo").expect("Undo is listed");
    assert!(rows[y].contains("Ctrl+Z"), "{:?}", rows[y]);
    key(&mut app, KeyCode::Esc);

    // Insert hyperlink: the cursor lands between the brackets.
    ctrl(&mut app, 'p');
    typing(&mut app, "insert hyperlink");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "site");
    let view = app.view().unwrap();
    assert_eq!(view.editor.lines[0], "[site]()- more spice");
    // Save runs mdedit's Ctrl+S.
    ctrl(&mut app, 'p');
    typing(&mut app, "save");
    key(&mut app, KeyCode::Enter);
    assert!(!app.view().unwrap().is_dirty());
    assert_eq!(fs::read_to_string(&path).unwrap(), "[site]()- more spice");
}

#[test]
fn plugins_are_installed_and_uninstalled_in_the_plugins_window() {
    let mut app = app("plugins-window");
    ctrl(&mut app, 'p');
    typing(&mut app, "open plugins");
    key(&mut app, KeyCode::Enter);
    assert_eq!(settings_page(&app), Some((Page::Plugins, true)));
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Dataview  0.1.0").is_some(), "{rows:#?}");
    assert!(find(&rows, "not installed").is_some(), "{rows:#?}");
    assert!(
        find(&rows, "Treat your vault as a database").is_some(),
        "details"
    );

    key(&mut app, KeyCode::Enter);
    assert_eq!(app.message, "Dataview installed and enabled");
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "installed · enabled").is_some(), "{rows:#?}");
    let state = fs::read_to_string(app.vault.root.join(STATE_FILE)).unwrap();
    assert!(state.contains("enabled = [\"dataview\"]"), "{state}");
    // The plugin's commands are in the palette now.
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    ctrl(&mut app, 'p');
    typing(&mut app, "dataview");
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Dataview: Insert query").is_some(), "{rows:#?}");
    assert!(find(&rows, "Dataview: Refresh views").is_some());
    key(&mut app, KeyCode::Esc);

    // Space disables it, Enter uninstalls it.
    ctrl(&mut app, 'p');
    typing(&mut app, "open plugins");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char(' '));
    assert_eq!(app.message, "Dataview disabled");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.message, "Dataview uninstalled");
    assert!(!app.plugins.borrow().statuses()[0].installed);
}

const DATAVIEW: &[(&str, &str)] = &[
    (
        "Reading.md",
        "# Reading\n```dataview\nTABLE author, rating FROM #reading SORT rating DESC\n```\nafter",
    ),
    (
        "Books/Dune.md",
        "---\nauthor: Frank Herbert\nrating: 5\ntags: [reading]\n---\n- [ ] finish",
    ),
    (
        "Books/Emma.md",
        "---\nauthor: Jane Austen\nrating: 3\n---\n#reading",
    ),
    (
        ".blackglass/plugins.toml",
        "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
    ),
];

#[test]
fn a_dataview_block_shows_its_query_result() {
    let dir = vault("dataview", DATAVIEW);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let path = note(&app, "Reading.md");
    app.open(&path);
    let rows = screen(&mut app, 100, 20);
    let at = |text: &str| find(&rows, text).unwrap_or_else(|| panic!("no {text:?} in {rows:#?}"));
    let (top, _) = at("╭─ dataview");
    assert_eq!(at("File (2) │ author        │ rating").0, top + 1);
    assert_eq!(at("Dune     │ Frank Herbert │ 5").0, top + 3);
    assert_eq!(at("Emma     │ Jane Austen   │ 3").0, top + 4);
    assert!(rows[top + 5].ends_with("│  ╰─"), "{:?}", rows[top + 5]);
    assert!(rows[top + 6].ends_with("after"), "{:?}", rows[top + 6]);

    // In the block: its query, to edit it.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    let rows = screen(&mut app, 100, 20);
    assert!(
        find(&rows, "TABLE author, rating FROM #reading").is_some(),
        "{rows:#?}"
    );
    assert!(find(&rows, "Frank Herbert").is_none());
}

#[test]
fn dataview_results_follow_saved_changes_and_plugin_state() {
    let dir = vault("dataview-live", DATAVIEW);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let emma = note(&app, "Books/Emma.md");
    app.open(&emma);
    // Rate Emma higher, and save.
    for _ in 0..2 {
        key(&mut app, KeyCode::Down);
    }
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Backspace);
    typing(&mut app, "9");
    ctrl(&mut app, 's');
    let reading = note(&app, "Reading.md");
    app.open(&reading);
    key(&mut app, KeyCode::End); // stay on the heading line
    let rows = screen(&mut app, 100, 20);
    let emma_row = find(&rows, "Emma     │").expect("Emma listed").0;
    let dune_row = find(&rows, "Dune     │").expect("Dune listed").0;
    assert!(emma_row < dune_row, "sorted by the saved rating: {rows:#?}");

    // Uninstalled, the block is code again.
    app.plugins
        .borrow_mut()
        .toggle_installed(0, &app.vault)
        .unwrap();
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "Emma     │").is_none());
    assert!(find(&rows, "TABLE author, rating").is_some(), "{rows:#?}");
}

#[test]
fn a_dataview_error_is_shown_in_the_block() {
    let dir = vault(
        "dataview-error",
        &[
            ("Bad.md", "top\n```dataview\nLIST WHERE\n```"),
            (
                ".blackglass/plugins.toml",
                "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let path = note(&app, "Bad.md");
    app.open(&path);
    let rows = screen(&mut app, 100, 12);
    assert!(
        find(&rows, "Dataview: expected a value, not end of query").is_some(),
        "{rows:#?}"
    );
}

#[test]
fn popup_rows_can_be_clicked() {
    let mut app = app("popup-clicks");
    // A palette row runs its command.
    ctrl(&mut app, 'p');
    typing(&mut app, "plugins");
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "Open plugins").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(settings_page(&app), Some((Page::Plugins, true)));
    // A plugin's row selects it; a click elsewhere does nothing.
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "Templater  0.2.0").unwrap();
    click(&mut app, x as u16, y as u16);
    click(&mut app, 0, 0);
    assert!(
        matches!(&app.prompt, Some(Prompt::Settings(w)) if w.row == 1 && w.on_page),
        "{:?}",
        app.prompt
    );
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    // A switcher row opens its note.
    ctrl(&mut app, 'o');
    typing(&mut app, "dune");
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "Dune  Books").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(active_path(&app), Some(note(&app, "Books/Dune.md")));
}

/// A vault of notes with known ages (minutes ago), for link suggestions.
/// The notes are in a folder, so the sidebar doesn't show their names.
fn aged_vault(name: &str, notes: &[(&str, u64)]) -> App {
    let files: Vec<(String, &str)> = notes
        .iter()
        .map(|(rel, _)| (rel.to_string(), ""))
        .chain([("Home.md".to_string(), "")])
        .collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(r, t)| (r.as_str(), *t)).collect();
    let dir = vault(name, &files);
    let now = std::time::SystemTime::now();
    for (rel, minutes) in notes {
        let file = fs::File::options().write(true).open(dir.join(rel)).unwrap();
        file.set_modified(now - std::time::Duration::from_secs(60 * minutes))
            .unwrap();
    }
    let mut app = App::new(Vault::open(&dir).unwrap());
    let home = note(&app, "Home.md");
    app.open(&home);
    app
}

const AGED: &[(&str, u64)] = &[
    ("Notes/Alpha.md", 1),
    ("Notes/Bravo.md", 2),
    ("Notes/Charlie.md", 3),
    ("Notes/Delta.md", 4),
    ("Notes/Echo.md", 5),
    ("Notes/Foxtrot.md", 6),
    ("Notes/Dune.md", 50),
];

fn line(app: &App) -> String {
    let view = app.view().unwrap();
    view.editor.lines[view.editor.row].clone()
}

#[test]
fn double_brackets_suggest_the_five_latest_notes() {
    let mut app = aged_vault("suggest-latest", AGED);
    typing(&mut app, "see [[");
    assert_eq!(line(&app), "see [[]]", "mdedit pairs the brackets");
    let rows = screen(&mut app, 100, 24);
    let ys: Vec<usize> = ["Alpha", "Bravo", "Charlie", "Delta", "Echo"]
        .iter()
        .map(|n| {
            find(&rows, n)
                .unwrap_or_else(|| panic!("{n} missing: {rows:#?}"))
                .0
        })
        .collect();
    assert!(
        ys.windows(2).all(|w| w[1] == w[0] + 1),
        "newest first: {ys:?}"
    );
    assert!(find(&rows, "Foxtrot").is_none(), "only five: {rows:#?}");
    // Right under the line being typed.
    let cursor = app.view().unwrap().cursor.unwrap();
    assert!(ys[0] > cursor.y as usize, "{ys:?} {cursor:?}");
}

#[test]
fn typing_searches_and_enter_inserts_the_link() {
    let mut app = aged_vault("suggest-search", AGED);
    typing(&mut app, "[[dun");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Dune").is_some(), "{rows:#?}");
    assert!(find(&rows, "Alpha").is_none(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    assert_eq!(line(&app), "[[Dune]]");
    assert!(app.suggest.is_none(), "closed");
    typing(&mut app, " next");
    assert_eq!(line(&app), "[[Dune]] next", "the cursor is after the link");
}

#[test]
fn arrows_choose_tab_inserts_and_esc_closes() {
    let mut app = aged_vault("suggest-keys", AGED);
    typing(&mut app, "[[");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Up);
    key(&mut app, KeyCode::Tab);
    assert_eq!(line(&app), "[[Bravo]]", "the second newest");

    key(&mut app, KeyCode::Enter);
    typing(&mut app, "[[");
    assert!(app.suggest.is_some());
    key(&mut app, KeyCode::Esc);
    assert!(app.suggest.is_none());
    typing(&mut app, "x");
    assert!(app.suggest.is_none(), "stays closed for this link");
    assert_eq!(line(&app), "[[x]]");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.view().unwrap().editor.lines.len(),
        3,
        "Enter is the editor's again"
    );
}

#[test]
fn a_name_used_twice_is_linked_by_its_path() {
    let mut app = aged_vault(
        "suggest-paths",
        &[("Notes/Home.md", 1), ("Archive/Dune.md", 2)],
    );
    typing(&mut app, "[[home");
    // The note being written (the top folder's Home) isn't suggested.
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Home  Notes").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        line(&app),
        "[[Notes/Home]]",
        "Home.md is in the vault's top folder too"
    );
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "![[dune");
    key(&mut app, KeyCode::Enter);
    assert_eq!(line(&app), "![[Dune]]", "a unique name, in an embed");
}

#[test]
fn a_suggestion_can_be_clicked() {
    let mut app = aged_vault("suggest-click", AGED);
    typing(&mut app, "[[");
    let rows = screen(&mut app, 100, 24);
    let (y, x) = find(&rows, "Charlie").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(line(&app), "[[Charlie]]");
}

const TEMPLATER: &[(&str, &str)] = &[
    (
        "Templates/Daily.md",
        "# <% tp.file.title %>\nCreated <% tp.date.now(\"YYYY\") %> in <% tp.file.folder() %>\n<% tp.file.cursor() %>",
    ),
    (
        "Templates/Greeting.md",
        "Hi <% tp.system.prompt(\"Who?\", \"there\") %>, <% tp.system.suggester([\"Red\", \"Blue\"], [\"r\", \"b\"]) %>",
    ),
    ("Journal/2026-08-09.md", ""),
    ("Draft.md", "Year <% tp.date.now(\"YYYY\") %>!"),
    (
        ".blackglass/plugins.toml",
        "installed = [\"templater\"]\nenabled = [\"templater\"]\n",
    ),
];

fn run_palette(app: &mut App, command: &str) {
    ctrl(app, 'p');
    typing(app, command);
    key(app, KeyCode::Enter);
}

fn year() -> String {
    chrono::Local::now().format("%Y").to_string()
}

#[test]
fn templater_inserts_a_template_at_the_cursor() {
    let dir = vault("templater-insert", TEMPLATER);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let path = note(&app, "Journal/2026-08-09.md");
    app.open(&path);
    run_palette(&mut app, "templater insert template");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Insert template").is_some(), "{rows:#?}");
    assert!(find(&rows, "Greeting").is_some(), "{rows:#?}");
    typing(&mut app, "dai");
    key(&mut app, KeyCode::Enter);
    let view = app.view().unwrap();
    assert_eq!(
        view.editor.lines,
        [
            "# 2026-08-09",
            &format!("Created {} in Journal", year()),
            ""
        ]
    );
    assert_eq!(
        (view.editor.row, view.editor.col),
        (2, 0),
        "at tp.file.cursor()"
    );
    assert!(view.is_dirty());
}

#[test]
fn templater_asks_the_templates_questions() {
    let dir = vault("templater-prompts", TEMPLATER);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let path = note(&app, "Journal/2026-08-09.md");
    app.open(&path);
    run_palette(&mut app, "templater insert template");
    typing(&mut app, "greet");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Who?").is_some(), "{rows:#?}");
    assert!(find(&rows, "there").is_some(), "the default: {rows:#?}");
    typing(&mut app, "Ann");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Blue").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.view().unwrap().editor.lines, ["Hi Ann, b"]);
    assert!(app.prompt.is_none());
}

#[test]
fn templater_creates_a_note_from_a_template() {
    let dir = vault("templater-create", TEMPLATER);
    let mut app = App::new(Vault::open(&dir).unwrap());
    // Select Journal (the first row): the new note's folder.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Up);
    run_palette(&mut app, "create new note from template");
    typing(&mut app, "daily");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Name of the new note").is_some(), "{rows:#?}");
    typing(&mut app, "Trip");
    key(&mut app, KeyCode::Enter);
    let trip = note(&app, "Journal/Trip.md");
    let expected = format!("# Trip\nCreated {} in Journal\n", year());
    assert_eq!(fs::read_to_string(&trip).unwrap(), expected, "written");
    assert_eq!(active_path(&app), Some(trip), "and opened");
    let view = app.view().unwrap();
    let end = view.editor.lines[1].chars().count();
    assert_eq!(
        (view.editor.row, view.editor.col),
        (1, end),
        "tp.file.cursor() after the last line break: the end of the text"
    );
}

#[test]
fn templater_runs_the_commands_in_the_active_note() {
    let dir = vault("templater-replace", TEMPLATER);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let path = note(&app, "Draft.md");
    app.open(&path);
    run_palette(&mut app, "replace templates");
    assert_eq!(
        app.view().unwrap().editor.lines,
        [format!("Year {}!", year())]
    );
    ctrl(&mut app, 'z');
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["Year <% tp.date.now(\"YYYY\") %>!"],
        "one undo step"
    );
}

#[test]
fn templater_explains_what_it_cannot_do() {
    let dir = vault(
        "templater-errors",
        &[
            ("Templates/App.md", "<% tp.app.vault.getName() %>"),
            ("Note.md", ""),
            (
                ".blackglass/plugins.toml",
                "installed = [\"templater\"]\nenabled = [\"templater\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "templater insert template");
    assert_eq!(app.message, "Templater: open a note first");
    let path = note(&app, "Note.md");
    app.open(&path);
    run_palette(&mut app, "templater insert template");
    key(&mut app, KeyCode::Enter);
    assert!(
        app.message.contains("tp.app is the original app's own API"),
        "{}",
        app.message
    );
    assert_eq!(app.view().unwrap().editor.lines, [""], "nothing inserted");
}

#[test]
fn templater_choices_can_be_clicked() {
    let dir = vault("templater-click", TEMPLATER);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let path = note(&app, "Journal/2026-08-09.md");
    app.open(&path);
    run_palette(&mut app, "templater insert template");
    let rows = screen(&mut app, 100, 24);
    let (y, x) = find(&rows, "Greeting").unwrap();
    click(&mut app, x as u16, y as u16);
    let rows = screen(&mut app, 100, 24);
    assert!(
        find(&rows, "Who?").is_some(),
        "the template's question: {rows:#?}"
    );
}

/// A clipboard with fixed text, for tests.
struct Clip(&'static str);

impl mdedit::clipboard::Clipboard for Clip {
    fn read(&self) -> Option<String> {
        Some(self.0.to_string())
    }
}

#[test]
fn ctrl_v_and_ctrl_shift_v_paste_and_alt_v_changes_the_mode() {
    let mut app = app("paste-keys");
    let path = note(&app, "Journal/2026-08-10.md");
    app.open(&path);
    app.shared.clipboard = Box::new(Clip("pasted "));
    ctrl(&mut app, 'v');
    assert_eq!(line(&app), "pasted - more spice");
    // Ctrl+Shift+V is the terminal's paste; where it reports it as a key,
    // it pastes too.
    press(
        &mut app,
        KeyCode::Char('V'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert_eq!(line(&app), "pasted pasted - more spice");
    alt(&mut app, KeyCode::Char('v'));
    assert!(app.view().unwrap().source_mode, "Alt+V: source mode");
    alt(&mut app, KeyCode::Char('v'));
    assert!(app.view().unwrap().reading, "Alt+V again: view mode");
    // The palette has the modes, with the key.
    ctrl(&mut app, 'p');
    typing(&mut app, "cycle modes");
    let rows = screen(&mut app, 100, 24);
    let (y, _) = find(&rows, "Cycle modes").unwrap();
    assert!(rows[y].contains("Alt+V"), "{:?}", rows[y]);
    key(&mut app, KeyCode::Enter);
    assert!(
        !app.view().unwrap().reading,
        "run from the palette: live preview"
    );
    run_palette(&mut app, "paste");
    assert_eq!(line(&app), "pasted pasted pasted - more spice");
}

const PERIODIC: &[(&str, &str)] = &[
    (
        "Templates/Daily.md",
        "# {{date:dddd, MMMM Do}}\n<< {{yesterday}} | {{tomorrow}} >>\ntitle <% tp.file.title %>\n",
    ),
    (
        "Templates/Mood.md",
        "Mood: <% tp.system.prompt(\"Mood?\", \"ok\") %>",
    ),
    ("Journal/2026-08-08.md", "eight"),
    ("Journal/2026-08-09.md", "nine"),
    ("Journal/2026-08-12.md", "twelve"),
    (
        ".blackglass/plugins/periodic-notes/settings.toml",
        "[daily]\nfolder = \"Journal\"\ntemplate = \"Templates/Daily\"\n\n[monthly]\nformat = \"YYYY/MM\"\nfolder = \"Months\"\ntemplate = \"Templates/Mood\"\n\n[yearly]\nenabled = false\n",
    ),
    (
        ".blackglass/plugins.toml",
        "installed = [\"periodic-notes\"]\nenabled = [\"periodic-notes\"]\n",
    ),
];

fn today(format: &str) -> String {
    chrono::Local::now().format(format).to_string()
}

#[test]
fn todays_daily_note_is_created_from_its_template_then_reopened() {
    let dir = vault("periodic-daily", PERIODIC);
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "open daily note");
    let path = note(&app, &format!("Journal/{}.md", today("%Y-%m-%d")));
    assert_eq!(active_path(&app), Some(path.clone()), "{}", app.message);
    let now = chrono::Local::now().date_naive();
    let day = |d: chrono::NaiveDate| d.format("%Y-%m-%d").to_string();
    let expected = format!(
        "# {}\n<< {} | {} >>\ntitle {}\n",
        today("%A, %B ") + &ordinal(now.format("%-d").to_string()),
        day(now.pred_opt().unwrap()),
        day(now.succ_opt().unwrap()),
        day(now),
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    // Again: the same note, not a new one.
    typing(&mut app, "x");
    ctrl(&mut app, 's');
    run_palette(&mut app, "open daily note");
    assert_eq!(app.tabs.len(), 1);
    assert!(fs::read_to_string(&path).unwrap().starts_with('x'));
}

fn ordinal(day: String) -> String {
    let n: u32 = day.parse().unwrap();
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

#[test]
fn weekly_and_monthly_notes_use_their_formats_and_folders() {
    let dir = vault("periodic-names", PERIODIC);
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "open weekly note");
    use chrono::Datelike;
    let iso = chrono::Local::now().date_naive().iso_week();
    let weekly = note(&app, &format!("{}-W{:02}.md", iso.year(), iso.week()));
    assert_eq!(
        active_path(&app),
        Some(weekly.clone()),
        "default: the top folder"
    );
    assert_eq!(fs::read_to_string(&weekly).unwrap(), "", "no template");
    // Monthly: a format with a folder in it, and a template that asks.
    run_palette(&mut app, "open monthly note");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Mood?").is_some(), "{rows:#?}");
    typing(&mut app, "great");
    key(&mut app, KeyCode::Enter);
    let monthly = note(&app, &format!("Months/{}.md", today("%Y/%m")));
    assert_eq!(fs::read_to_string(&monthly).unwrap(), "Mood: great");
    // Yearly is turned off in the settings.
    ctrl(&mut app, 'p');
    typing(&mut app, "open yearly note");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Open yearly note").is_none(), "{rows:#?}");
}

#[test]
fn next_and_previous_jump_to_the_closest_existing_note() {
    let dir = vault("periodic-jump", PERIODIC);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let nine = note(&app, "Journal/2026-08-09.md");
    app.open(&nine);
    run_palette(&mut app, "next periodic note");
    assert_eq!(
        active_path(&app),
        Some(note(&app, "Journal/2026-08-12.md")),
        "skips missing days"
    );
    run_palette(&mut app, "previous periodic note");
    assert_eq!(active_path(&app), Some(nine.clone()));
    run_palette(&mut app, "previous periodic note");
    let eight = note(&app, "Journal/2026-08-08.md");
    assert_eq!(active_path(&app), Some(eight));
    run_palette(&mut app, "previous periodic note");
    assert_eq!(app.message, "Periodic Notes: no earlier daily note");
    // Not a periodic note: nothing to jump from.
    let tmpl = note(&app, "Templates/Mood.md");
    app.open(&tmpl);
    run_palette(&mut app, "next periodic note");
    assert!(
        app.message.contains("isn't a periodic note"),
        "{}",
        app.message
    );
}

const ALL_PLUGINS: &str = "installed = [\"dataview\", \"templater\", \"periodic-notes\"]\nenabled = [\"dataview\", \"templater\", \"periodic-notes\"]\n";

/// Opens the plugins window with plugin `i` (catalog order: Dataview,
/// Templater, Periodic Notes) highlighted.
/// The settings window's page, and whether the page has the focus.
fn settings_page(app: &App) -> Option<(Page, bool)> {
    match &app.prompt {
        Some(Prompt::Settings(w)) => Some((w.page, w.on_page)),
        _ => None,
    }
}

/// The settings window on its Plugins page, plugin `i` chosen.
fn plugins_window(app: &mut App, i: usize) {
    run_palette(app, "open plugins");
    for _ in 0..i {
        key(app, KeyCode::Down);
    }
}

fn settings_text(app: &App, id: &str) -> String {
    fs::read_to_string(
        app.vault
            .root
            .join(format!(".blackglass/plugins/{id}/settings.toml")),
    )
    .unwrap_or_default()
}

#[test]
fn a_plugins_settings_open_from_the_plugins_window() {
    let dir = vault(
        "settings-open",
        &[("Home.md", ""), (".blackglass/plugins.toml", ALL_PLUGINS)],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    plugins_window(&mut app, 2);
    let rows = screen(&mut app, 100, 30);
    assert!(
        find(&rows, "→ settings").is_some(),
        "the hint says how: {rows:#?}"
    );
    key(&mut app, KeyCode::Right);
    let rows = screen(&mut app, 100, 30);
    assert_eq!(settings_page(&app), Some((Page::Plugin(2), true)));
    assert!(find(&rows, "Daily").is_some(), "section: {rows:#?}");
    assert!(
        find(&rows, "YYYY-MM-DD").is_some(),
        "the default format: {rows:#?}"
    );
    // Esc goes back to the list of pages, then closes.
    key(&mut app, KeyCode::Esc);
    assert_eq!(settings_page(&app), Some((Page::Plugin(2), false)));
    key(&mut app, KeyCode::Esc);
    assert!(app.prompt.is_none());
    // The palette opens them too.
    run_palette(&mut app, "templater settings");
    let rows = screen(&mut app, 100, 30);
    assert_eq!(settings_page(&app), Some((Page::Plugin(1), true)));
    assert!(find(&rows, "Templates folder").is_some(), "{rows:#?}");
}

#[test]
fn changed_settings_are_saved_and_used_at_once() {
    let dir = vault(
        "settings-change",
        &[("Home.md", ""), (".blackglass/plugins.toml", ALL_PLUGINS)],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    plugins_window(&mut app, 2);
    key(&mut app, KeyCode::Right);
    // Daily: enabled, format, folder …: edit the folder.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "Days");
    key(&mut app, KeyCode::Enter);
    // Weekly notes off (the fifth setting).
    for _ in 0..2 {
        key(&mut app, KeyCode::Down);
    }
    key(&mut app, KeyCode::Enter);
    let saved = settings_text(&app, "periodic-notes");
    assert!(
        saved.contains("[daily]\nenabled = true\nformat = \"YYYY-MM-DD\"\nfolder = \"Days\""),
        "{saved}"
    );
    assert!(saved.contains("[weekly]\nenabled = false"), "{saved}");
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    run_palette(&mut app, "open daily note");
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(
        active_path(&app),
        Some(note(&app, &format!("Days/{day}.md"))),
        "{}",
        app.message
    );
    ctrl(&mut app, 'p');
    typing(&mut app, "open weekly note");
    let rows = screen(&mut app, 100, 30);
    assert!(
        find(&rows, "Open weekly note").is_none(),
        "weekly is off: {rows:#?}"
    );
}

#[test]
fn text_settings_can_be_cleared_and_edits_cancelled() {
    let dir = vault(
        "settings-edit",
        &[("Home.md", ""), (".blackglass/plugins.toml", ALL_PLUGINS)],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    plugins_window(&mut app, 1);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Enter);
    ctrl(&mut app, 'u');
    typing(&mut app, "Tpl");
    key(&mut app, KeyCode::Esc);
    assert_eq!(settings_text(&app, "templater"), "", "Esc cancels the edit");
    assert!(
        matches!(app.prompt, Some(Prompt::Settings(_))),
        "still in the settings"
    );
    key(&mut app, KeyCode::Enter);
    ctrl(&mut app, 'u');
    typing(&mut app, "Tpl");
    key(&mut app, KeyCode::Enter);
    assert!(
        settings_text(&app, "templater").starts_with("templates_folder = \"Tpl\"\n"),
        "{}",
        settings_text(&app, "templater")
    );
}

#[test]
fn dataview_display_settings_change_the_results() {
    let files = [
        (
            "Q.md",
            "top\n```dataview\nTABLE due FROM \"Tasks\"\n```\n```dataview\nLIST FROM \"Nothing\"\n```",
        ),
        ("Tasks/A.md", "---\ndue: 2026-08-09\n---"),
        (".blackglass/plugins.toml", ALL_PLUGINS),
        (
            ".blackglass/plugins/dataview/settings.toml",
            "date_format = \"D MMMM YYYY\"\nno_results = \"(nothing here)\"\nid_header = \"Note\"\n",
        ),
    ];
    let dir = vault("settings-dataview", &files);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "Note (1) │ due").is_some(), "{rows:#?}");
    assert!(find(&rows, "9 August 2026").is_some(), "{rows:#?}");
    assert!(find(&rows, "(nothing here)").is_some(), "{rows:#?}");
}

#[test]
fn only_installed_plugins_have_settings_to_open() {
    let dir = vault("settings-installed", &[("Home.md", "")]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    plugins_window(&mut app, 0);
    key(&mut app, KeyCode::Right);
    assert_eq!(app.message, "Install Dataview first");
    assert_eq!(settings_page(&app), Some((Page::Plugins, true)));
}

#[test]
fn the_gear_opens_a_plugins_settings() {
    let dir = vault(
        "settings-gear",
        &[("Home.md", ""), (".blackglass/plugins.toml", ALL_PLUGINS)],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    plugins_window(&mut app, 0);
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "⚙").expect("a gear on the installed plugins");
    click(&mut app, x as u16, y as u16);
    assert_eq!(settings_page(&app), Some((Page::Plugin(0), true)));
}

const CALENDAR: &[(&str, &str)] = &[
    ("Home.md", ""),
    (
        ".blackglass/plugins.toml",
        "installed = [\"periodic-notes\"]\nenabled = [\"periodic-notes\"]\n",
    ),
    (
        ".blackglass/plugins/periodic-notes/settings.toml",
        "[daily]\nfolder = \"Journal\"\n",
    ),
];

fn month_title(date: chrono::NaiveDate) -> String {
    date.format("%B %Y").to_string()
}

#[test]
fn alt_c_shows_and_hides_the_calendar() {
    let dir = vault("calendar-toggle", CALENDAR);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let today = chrono::Local::now().date_naive();
    alt(&mut app, KeyCode::Char('c'));
    assert_eq!(app.focus, Focus::Panel);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, &month_title(today)).is_some(), "{rows:#?}");
    assert!(
        find(&rows, "Wk Mo Tu We Th Fr Sa Su").is_some(),
        "{rows:#?}"
    );
    alt(&mut app, KeyCode::Char('c'));
    let rows = screen(&mut app, 100, 30);
    assert!(
        find(&rows, &month_title(today)).is_none(),
        "hidden: {rows:#?}"
    );
    // The palette toggles it too.
    run_palette(&mut app, "toggle calendar");
    assert!(app.panel);
}

#[test]
fn the_calendar_opens_and_creates_daily_notes_by_key() {
    let dir = vault("calendar-keys", CALENDAR);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let today = chrono::Local::now().date_naive();
    let day = |d: chrono::NaiveDate| format!("Journal/{}.md", d.format("%Y-%m-%d"));
    alt(&mut app, KeyCode::Char('c'));
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter); // Create it?
    assert_eq!(
        active_path(&app),
        Some(note(&app, &day(today))),
        "{}",
        app.message
    );
    alt(&mut app, KeyCode::Char('c')); // hide
    alt(&mut app, KeyCode::Char('c')); // show again, focused
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Up);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    let back = today - chrono::Duration::days(8);
    assert_eq!(active_path(&app), Some(note(&app, &day(back))));
    // PgUp shows the month before, and t comes back to today.
    alt(&mut app, KeyCode::Char('c'));
    alt(&mut app, KeyCode::Char('c'));
    // From today's month (the day chosen above may be in the month before).
    key(&mut app, KeyCode::Char('t'));
    key(&mut app, KeyCode::PageUp);
    let rows = screen(&mut app, 100, 30);
    let before = today.checked_sub_months(chrono::Months::new(1)).unwrap();
    assert!(find(&rows, &month_title(before)).is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Char('t'));
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, &month_title(today)).is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Editor);
}

#[test]
fn calendar_days_and_arrows_can_be_clicked() {
    use chrono::Datelike;
    let dir = vault("calendar-click", CALENDAR);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let today = chrono::Local::now().date_naive();
    alt(&mut app, KeyCode::Char('c'));
    let rows = screen(&mut app, 100, 30);
    let (title_y, _) = find(&rows, &month_title(today)).unwrap();
    // The 1st is in the first week row, in its weekday's column.
    let first = today.with_day(1).unwrap();
    let column = first.weekday().num_days_from_monday() as u16;
    let (_, wk_x) = find(&rows, "Wk Mo").unwrap();
    let x = wk_x as u16 + 3 + 3 * column + 1;
    click(&mut app, x, title_y as u16 + 2);
    key(&mut app, KeyCode::Enter);
    let expected = format!("Journal/{}.md", first.format("%Y-%m-%d"));
    assert_eq!(active_path(&app), Some(note(&app, &expected)), "{rows:#?}");
    // ‹ goes to the month before.
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "‹").unwrap();
    click(&mut app, x as u16, y as u16);
    let rows = screen(&mut app, 100, 30);
    let before = today.checked_sub_months(chrono::Months::new(1)).unwrap();
    assert!(find(&rows, &month_title(before)).is_some(), "{rows:#?}");
}

#[test]
fn the_calendar_asks_before_creating_a_note() {
    let dir = vault("calendar-ask", CALENDAR);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let today = chrono::Local::now().date_naive();
    let name = today.format("%Y-%m-%d").to_string();
    let path = dir.join(format!("Journal/{name}.md"));
    alt(&mut app, KeyCode::Char('c'));
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    assert!(
        find(&rows, &format!("Create {name}?")).is_some(),
        "{rows:#?}"
    );
    assert!(!path.exists(), "not yet");
    // Esc: nothing is made.
    key(&mut app, KeyCode::Esc);
    assert!(!path.exists() && app.tabs.is_empty());
    // "Don't create" neither.
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert!(!path.exists() && app.tabs.is_empty());
    // Create it.
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    assert!(path.is_file());
    assert_eq!(
        active_path(&app),
        Some(mdedit::platform::canonical(&path).unwrap())
    );
    // A day that has its note opens it without asking.
    ctrl(&mut app, 'w');
    alt(&mut app, KeyCode::Char('c'));
    alt(&mut app, KeyCode::Char('c'));
    key(&mut app, KeyCode::Enter);
    assert!(app.prompt.is_none(), "{:?}", app.prompt);
    assert_eq!(
        active_path(&app),
        Some(mdedit::platform::canonical(&path).unwrap())
    );
}

#[test]
fn without_a_calendar_plugin_alt_c_says_so() {
    let mut app = app("calendar-none");
    alt(&mut app, KeyCode::Char('c'));
    assert!(!app.panel);
    assert!(app.message.contains("Periodic Notes"), "{}", app.message);
}

#[test]
fn a_dataviewjs_block_runs_its_script_unless_javascript_is_off() {
    let files = [
        (
            "Q.md",
            "top\n```dataviewjs\ndv.list(dv.pages('\"Books\"').map(p => p.file.name + ': ' + '★'.repeat(p.rating)))\n```",
        ),
        ("Books/Dune.md", "---\nrating: 5\n---"),
        ("Books/Emma.md", "---\nrating: 3\n---"),
        (".blackglass/plugins.toml", ALL_PLUGINS),
    ];
    let dir = vault("dataviewjs-block", &files);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 100, 16);
    assert!(find(&rows, "• Dune: ★★★★★").is_some(), "{rows:#?}");
    assert!(find(&rows, "• Emma: ★★★").is_some(), "{rows:#?}");
    // Dataview settings: the fifth one turns JavaScript off.
    plugins_window(&mut app, 0);
    key(&mut app, KeyCode::Right);
    for _ in 0..4 {
        key(&mut app, KeyCode::Down);
    }
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    let rows = screen(&mut app, 100, 16);
    assert!(
        find(
            &rows,
            "Dataview: JavaScript queries are off in the settings"
        )
        .is_some(),
        "{rows:#?}"
    );
}

const RESULTS: &[(&str, &str)] = &[
    (
        "Q.md",
        "top\n```dataview\nLIST FROM \"Books\"\n```\n```dataview\nTASK\n```\n```dataviewjs\ndv.list(dv.pages('\"Books\"'))\n```\nsee [[Emma]] here\n![[pic.png]]",
    ),
    ("Books/Dune.md", "# Dune\n\n- [ ] finish part two"),
    ("Books/Emma.md", "# Emma"),
    ("pic.png", "not really a picture"),
    (".blackglass/plugins.toml", ALL_PLUGINS),
    // Enter on a task opens it at its line (not checks it off).
    (
        ".blackglass/plugins/dataview/settings.toml",
        "task_click = \"open\"\n",
    ),
];

fn view_mode(app: &mut App) {
    run_palette(app, "view mode");
    assert!(app.view().unwrap().reading, "{}", app.message);
}

#[test]
fn view_mode_follows_dataview_results() {
    let dir = vault("view-results", RESULTS);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let q = note(&app, "Q.md");
    app.open(&q);
    view_mode(&mut app);
    let rows = screen(&mut app, 100, 30);
    let status = rows.last().unwrap();
    assert!(status.contains("VIEW │"), "{status:?}");
    // Tab: the first result (Dune, from LIST); Enter opens it.
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        active_path(&app),
        Some(note(&app, "Books/Dune.md")),
        "{}",
        app.message
    );
    // Back to Q (still in view mode): the task opens at its line.
    app.open(&q);
    screen(&mut app, 100, 30);
    press(&mut app, KeyCode::Home, KeyModifiers::CONTROL);
    // LIST: Dune, Emma; TASK: the page, then its task.
    for _ in 0..4 {
        key(&mut app, KeyCode::Tab);
    }
    key(&mut app, KeyCode::Enter);
    let view = app.view().unwrap();
    assert_eq!(
        view.path,
        Some(note(&app, "Books/Dune.md")),
        "{}",
        app.message
    );
    assert_eq!(view.editor.row, 2, "the task's line");
}

#[test]
fn dataviewjs_pages_are_followable() {
    let dir = vault("view-js", RESULTS);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let q = note(&app, "Q.md");
    app.open(&q);
    view_mode(&mut app);
    screen(&mut app, 100, 30);
    // LIST: 2 rows, TASK: page and task, then the JS list's first row.
    for _ in 0..5 {
        key(&mut app, KeyCode::Tab);
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        active_path(&app),
        Some(note(&app, "Books/Dune.md")),
        "{}",
        app.message
    );
}

#[test]
fn links_are_clickable_in_the_live_preview_and_images_open_outside() {
    let dir = vault("click-links", RESULTS);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = std::rc::Rc::clone(&opened);
    app.opener = Box::new(move |path| {
        seen.borrow_mut().push(path.to_path_buf());
        Ok(())
    });
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "Emma").expect("the link is drawn");
    click(&mut app, x as u16, y as u16);
    assert_eq!(
        active_path(&app),
        Some(note(&app, "Books/Emma.md")),
        "{}",
        app.message
    );
    app.open(&q);
    let rows = screen(&mut app, 100, 30);
    // The embed's frame (not the file explorer's pic.png).
    let (y, x) =
        find(&rows, "⚠ pic.png").unwrap_or_else(|| panic!("the image embed is drawn: {rows:#?}"));
    click(&mut app, x as u16, y as u16);
    assert_eq!(*opened.borrow(), [note(&app, "pic.png")]);
    assert!(app.message.contains("system viewer"), "{}", app.message);
}

fn mouse_button(app: &mut App, button: MouseButton) {
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(button),
        column: 50,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });
}

fn back(app: &mut App) {
    press(
        app,
        KeyCode::Left,
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
}

fn forward(app: &mut App) {
    press(
        app,
        KeyCode::Right,
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
}

#[test]
fn back_and_forward_return_to_the_notes_you_were_on() {
    let mut app = app("history-keys");
    let welcome = note(&app, "Welcome.md");
    let dune = note(&app, "Books/Dune.md");
    let journal = note(&app, "Journal/2026-08-10.md");
    app.open(&welcome);
    key(&mut app, KeyCode::Down);
    app.open(&dune);
    back(&mut app);
    assert_eq!(active_path(&app), Some(welcome.clone()));
    assert_eq!(app.view().unwrap().editor.row, 1, "where the cursor was");
    forward(&mut app);
    assert_eq!(active_path(&app), Some(dune.clone()));
    // A new place clears what's forward.
    back(&mut app);
    app.open(&journal);
    forward(&mut app);
    assert_eq!(active_path(&app), Some(journal.clone()));
    assert_eq!(app.message, "Nothing to go forward to");
    // Back reopens a closed note.
    back(&mut app);
    assert_eq!(active_path(&app), Some(welcome.clone()));
    ctrl(&mut app, 'w');
    back(&mut app);
    back(&mut app);
    assert!(
        app.tabs
            .iter()
            .any(|t| t.path.as_deref() == Some(welcome.as_path())),
        "reopened"
    );
    // The palette has them too.
    run_palette(&mut app, "go forward");
    assert!(
        app.message.is_empty() || app.message.contains("forward"),
        "{}",
        app.message
    );
}

#[test]
fn the_mouses_back_and_forward_buttons_navigate() {
    let mut app = app("history-mouse");
    let welcome = note(&app, "Welcome.md");
    let dune = note(&app, "Books/Dune.md");
    app.open(&welcome);
    app.open(&dune);
    screen(&mut app, 100, 24);
    mouse_button(&mut app, MouseButton::Back);
    assert_eq!(active_path(&app), Some(welcome));
    mouse_button(&mut app, MouseButton::Forward);
    assert_eq!(active_path(&app), Some(dune));
}

#[test]
fn back_skips_notes_that_were_deleted() {
    let mut app = app("history-deleted");
    let welcome = note(&app, "Welcome.md");
    let dune = note(&app, "Books/Dune.md");
    let journal = note(&app, "Journal/2026-08-10.md");
    app.open(&welcome);
    app.open(&dune);
    app.open(&journal);
    // Dune's tab is closed and its file deleted. (With its tab still open,
    // back would show the tab: its text is still there.)
    let i = app
        .tabs
        .iter()
        .position(|t| t.path.as_deref() == Some(dune.as_path()))
        .unwrap();
    app.close(i);
    fs::remove_file(&dune).unwrap();
    back(&mut app);
    assert_eq!(
        active_path(&app),
        Some(welcome),
        "Dune is skipped: {}",
        app.message
    );
    assert!(app.message.contains("Dune.md"), "{}", app.message);
}

#[test]
fn tab_in_the_new_note_window_picks_a_template() {
    let dir = vault("new-from-template", TEMPLATER);
    let mut app = App::new(Vault::open(&dir).unwrap());
    ctrl(&mut app, 'n');
    let rows = screen(&mut app, 100, 24);
    assert!(
        find(&rows, "Tab from a template").is_some(),
        "the hint: {rows:#?}"
    );
    typing(&mut app, "Plan");
    key(&mut app, KeyCode::Tab);
    let rows = screen(&mut app, 100, 24);
    assert!(
        find(&rows, "Create a note from template").is_some(),
        "{rows:#?}"
    );
    typing(&mut app, "daily");
    key(&mut app, KeyCode::Enter);
    // The name typed before is used: no second question.
    assert_eq!(
        active_path(&app),
        Some(note(&app, "Plan.md")),
        "{}",
        app.message
    );
    assert!(
        fs::read_to_string(note(&app, "Plan.md"))
            .unwrap()
            .starts_with("# Plan")
    );
    // Without a name typed, the template flow asks for one.
    ctrl(&mut app, 'n');
    key(&mut app, KeyCode::Tab);
    typing(&mut app, "daily");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Name of the new note").is_some(), "{rows:#?}");
    // Ctrl+Shift+N is no longer a shortcut (it's Ctrl+N in most terminals).
    key(&mut app, KeyCode::Esc);
    press(
        &mut app,
        KeyCode::Char('N'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    assert!(
        !matches!(app.prompt, Some(Prompt::Ask(_))),
        "{:?}",
        app.prompt
    );
}

#[test]
fn without_templater_tab_in_the_new_note_window_says_how_to_get_templates() {
    let mut app = app("new-from-template-none");
    ctrl(&mut app, 'n');
    typing(&mut app, "Plan");
    key(&mut app, KeyCode::Tab);
    assert!(
        matches!(app.prompt, Some(Prompt::NewNote { .. })),
        "the window stays"
    );
    assert!(app.message.contains("Templater"), "{}", app.message);
}

const EXTRACT: &[(&str, &str)] = &[
    ("Source.md", "one\ntwo and\nmore two\nthree"),
    (
        "Templates/Card.md",
        "# <% tp.file.title %>\n\n<% tp.file.cursor() %>\n\n#card",
    ),
    ("Templates/Plain.md", "# <% tp.file.title %>"),
    (
        ".blackglass/plugins.toml",
        "installed = [\"templater\"]\nenabled = [\"templater\"]\n",
    ),
];

/// Opens Source.md with lines 2 and 3 selected.
fn source_with_selection(name: &str) -> App {
    let dir = vault(name, EXTRACT);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let source = note(&app, "Source.md");
    app.open(&source);
    key(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down, KeyModifiers::SHIFT);
    press(&mut app, KeyCode::End, KeyModifiers::SHIFT);
    app
}

#[test]
fn ctrl_x_extracts_the_selection_to_a_new_note() {
    let mut app = source_with_selection("extract-plain");
    ctrl(&mut app, 'x');
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Extract to a new note").is_some(), "{rows:#?}");
    typing(&mut app, "Twos");
    key(&mut app, KeyCode::Enter);
    let twos = note(&app, "Twos.md");
    assert_eq!(fs::read_to_string(&twos).unwrap(), "two and\nmore two");
    assert_eq!(active_path(&app), Some(twos), "the new note's tab");
    let source = app.tabs.iter().find(|t| t.title() == "Source.md").unwrap();
    assert_eq!(source.editor.lines, ["one", "[[Twos]]", "three"]);
    assert!(
        source.is_dirty(),
        "the original isn't saved behind your back"
    );
}

#[test]
fn an_extract_can_start_from_a_template() {
    let mut app = source_with_selection("extract-template");
    ctrl(&mut app, 'x');
    typing(&mut app, "Card one");
    key(&mut app, KeyCode::Tab);
    typing(&mut app, "card");
    key(&mut app, KeyCode::Enter);
    let card = note(&app, "Card one.md");
    assert_eq!(active_path(&app), Some(card.clone()), "{}", app.message);
    let view = app.view().unwrap();
    assert_eq!(
        view.editor.lines,
        ["# Card one", "", "two and", "more two", "", "#card"],
        "at the template's cursor"
    );
    // A template without a cursor: the text goes at the end.
    let mut app = source_with_selection("extract-template-end");
    ctrl(&mut app, 'x');
    typing(&mut app, "Plain one");
    key(&mut app, KeyCode::Tab);
    typing(&mut app, "plain");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["# Plain one", "", "two and", "more two"]
    );
    let source = app.tabs.iter().find(|t| t.title() == "Source.md").unwrap();
    assert_eq!(source.editor.lines, ["one", "[[Plain one]]", "three"]);
}

#[test]
fn ctrl_x_without_a_selection_closes_and_esc_cancels_an_extract() {
    let mut app = source_with_selection("extract-cancel");
    ctrl(&mut app, 'x');
    key(&mut app, KeyCode::Esc);
    let source = app.view().unwrap();
    assert_eq!(
        source.editor.lines,
        ["one", "two and", "more two", "three"],
        "unchanged"
    );
    assert!(!source.is_dirty());
    // No selection: Ctrl+X closes the tab, as before.
    key(&mut app, KeyCode::Left);
    ctrl(&mut app, 'x');
    assert!(app.tabs.is_empty());
}

/// A workspace whose config folder (the user's dotfiles) is a temp dir.
fn app_with_config(name: &str) -> (App, PathBuf) {
    let mut app = app(name);
    let config = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("config")
        .join(name);
    let _ = fs::remove_dir_all(&config);
    app.config_dir = Some(config.clone());
    (app, config)
}

#[test]
fn help_is_an_embedded_read_only_page_with_the_current_shortcuts() {
    let (mut app, _) = app_with_config("help");
    ctrl(&mut app, 'h');
    let view = app.view().expect("the help tab");
    assert_eq!(view.path, None, "not a note in the vault");
    assert!(view.reading, "read-only view mode");
    let text = view.editor.to_text();
    assert!(text.contains("# blackglass help"), "{text}");
    assert!(
        text.contains("| New note | Ctrl+N |"),
        "the shortcut list: {text}"
    );
    assert!(
        text.contains("**Ctrl+O** goes to a note"),
        "keys in the text: {text}"
    );
    assert!(
        !text.contains('{'),
        "every key placeholder is filled: {text}"
    );
    let rows = screen(&mut app, 100, 24);
    assert!(rows[0].contains("Help ×"), "{:?}", rows[0]);
    // Nothing edits it, Esc and Alt+V don't leave view mode.
    typing(&mut app, "x");
    key(&mut app, KeyCode::Esc);
    alt(&mut app, KeyCode::Char('v'));
    let view = app.view().unwrap();
    assert!(view.reading && !view.is_dirty(), "{}", app.message);
    // F1 shows the same tab.
    key(&mut app, KeyCode::F(1));
    assert_eq!(app.tabs.len(), 1);
}

#[test]
fn shortcuts_are_changed_in_the_settings_and_saved_to_the_dotfiles() {
    let (mut app, config) = app_with_config("shortcuts");
    alt(&mut app, KeyCode::Char(','));
    key(&mut app, KeyCode::Down); // Notes
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(settings_page(&app), Some((Page::Shortcuts, true)));
    // Typing searches (the focus goes to the search box).
    typing(&mut app, "new note");
    let rows = screen(&mut app, 100, 24);
    let (y, _) = find(&rows, "New note").expect("filtered");
    assert!(rows[y].contains("Ctrl+N"), "{:?}", rows[y]);
    key(&mut app, KeyCode::Enter); // to the page
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Press the new key").is_some(), "{rows:#?}");
    press(&mut app, KeyCode::Char('g'), KeyModifiers::CONTROL);
    assert!(
        app.message.contains("Search the vault"),
        "moved from it: {}",
        app.message
    );
    let saved = fs::read_to_string(config.join("keys.toml")).unwrap();
    assert!(saved.contains("new-note = \"Ctrl+G\""), "{saved}");
    assert!(
        saved.contains("search-the-vault = \"Ctrl+Shift+F\""),
        "{saved}"
    );
    // A key without Ctrl or Alt is refused.
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('q'));
    assert!(app.message.contains("Ctrl or Alt"), "{}", app.message);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    assert!(app.prompt.is_none(), "{:?}", app.prompt);
    // In use at once: Ctrl+G makes a note; Ctrl+N does nothing now.
    ctrl(&mut app, 'g');
    assert!(matches!(app.prompt, Some(Prompt::NewNote { .. })));
    key(&mut app, KeyCode::Esc);
    ctrl(&mut app, 'n');
    assert!(app.prompt.is_none());
    assert!(app.message.contains("Ctrl+N"), "{}", app.message);
    // And read back when blackglass starts.
    let mut again = app_with_config("shortcuts-again").0;
    let dir = again.config_dir.clone().unwrap();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("keys.toml"), "new-note = \"Alt+N\"\n").unwrap();
    again.load_user_config();
    alt(&mut again, KeyCode::Char('n'));
    assert!(matches!(again.prompt, Some(Prompt::NewNote { .. })));
}

#[test]
fn editor_settings_are_saved_to_the_dotfiles_and_used() {
    let (mut app, config) = app_with_config("editor-settings");
    assert!(app.shared.config.auto_pair);
    alt(&mut app, KeyCode::Char(','));
    key(&mut app, KeyCode::Enter);
    assert_eq!(settings_page(&app), Some((Page::Editor, true)));
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Auto-pair").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter); // auto-pair: on → off
    assert!(!app.shared.config.auto_pair, "applied at once");
    let saved = fs::read_to_string(config.join("config.toml")).unwrap();
    assert!(saved.contains("auto_pair = \"off\""), "{saved}");
    assert!(
        !saved.contains("heading1_color"),
        "empty settings aren't written: {saved}"
    );
    key(&mut app, KeyCode::Esc);
    assert_eq!(
        settings_page(&app),
        Some((Page::Editor, false)),
        "back to the pages"
    );
}

#[test]
fn a_note_can_be_moved_to_a_folder_and_deleted_to_the_trash() {
    let mut app = app("move-delete");
    let home = note(&app, "Welcome.md");
    app.open(&home);
    run_palette(&mut app, "move note");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Move Welcome.md to").is_some(), "{rows:#?}");
    typing(&mut app, "books");
    key(&mut app, KeyCode::Enter);
    let moved = note(&app, "Books/Welcome.md");
    assert!(moved.is_file() && !home.exists(), "{}", app.message);
    assert_eq!(active_path(&app), Some(moved.clone()), "the tab follows");
    assert!(app.vault.note(&moved).is_some(), "indexed");
    run_palette(&mut app, "delete note");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Delete Welcome.md?").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    assert!(!moved.exists());
    assert!(
        app.vault.root.join(".trash/Welcome.md").is_file(),
        "in the vault's trash"
    );
    assert!(app.tabs.is_empty(), "its tab closed");
    assert!(app.message.contains(".trash"), "{}", app.message);
}

#[test]
fn an_editor_command_on_another_key_still_works() {
    let (mut app, _) = app_with_config("editor-key");
    app.keymap.apply("save = \"Alt+S\"\n");
    let home = note(&app, "Welcome.md");
    app.open(&home);
    typing(&mut app, "Hi ");
    alt(&mut app, KeyCode::Char('s'));
    assert!(!app.view().unwrap().is_dirty(), "saved by its new key");
    assert!(fs::read_to_string(&home).unwrap().starts_with("Hi "));
    ctrl(&mut app, 'p');
    typing(&mut app, "save");
    let rows = screen(&mut app, 100, 30);
    let (y, _) = find(&rows, "Save").unwrap();
    assert!(
        rows[y].contains("Alt+S"),
        "the palette shows the keys now: {:?}",
        rows[y]
    );
    key(&mut app, KeyCode::Esc);
    ctrl(&mut app, 'p');
    typing(&mut app, "find and replace");
    let rows = screen(&mut app, 100, 30);
    let (y, _) = find(&rows, "Find and replace").unwrap();
    assert!(rows[y].contains("Ctrl+R"), "{:?}", rows[y]);
}

#[test]
fn todays_note_has_a_key() {
    let dir = vault("periodic-today", PERIODIC);
    let mut bg = App::new(Vault::open(&dir).unwrap());
    let path = note(&bg, &format!("Journal/{}.md", today("%Y-%m-%d")));
    // Before a note is open, the empty side says how; no button anywhere.
    let rows = screen(&mut bg, 100, 24);
    let (y, _) = find(&rows, "Open daily note").expect("on the empty side");
    assert!(rows[y].contains("Alt+D"), "{:?}", rows[y]);
    assert!(find(&rows, "Today").is_none(), "no Today button: {rows:#?}");
    // Alt+D opens (creates) today's note.
    alt(&mut bg, KeyCode::Char('d'));
    assert_eq!(active_path(&bg), Some(path.clone()), "{}", bg.message);
    // Its key can be changed like any other.
    ctrl(&mut bg, 'p');
    typing(&mut bg, "open daily note");
    let rows = screen(&mut bg, 100, 30);
    let (y, _) = find(&rows, "Periodic Notes: Open daily note").unwrap();
    assert!(rows[y].contains("Alt+D"), "{:?}", rows[y]);
    key(&mut bg, KeyCode::Esc);

    // Without the plugin, Alt+D is the editor's.
    let mut plain = app("no-periodic");
    alt(&mut plain, KeyCode::Char('d'));
    assert!(plain.tabs.is_empty());
}

/// Templater with a folder template: new notes in `Books/` get `Book`.
fn folder_templates_vault(name: &str) -> App {
    let mut files = TEMPLATER.to_vec();
    files.extend([
        (
            "Templates/Book.md",
            "---\nstatus: to read\n---\n# <% tp.file.title %>\nauthor:: <% tp.file.cursor() %>\n",
        ),
        ("Books/Dune.md", "# Dune"),
        (
            ".blackglass/plugins/templater/settings.toml",
            "templates_folder = \"Templates\"\n\n[folder_templates]\nBooks = \"Book\"\n",
        ),
    ]);
    App::new(Vault::open(&vault(name, &files)).unwrap())
}

#[test]
fn a_new_note_in_a_folder_gets_the_folders_template() {
    let mut app = folder_templates_vault("folder-templates");
    ctrl(&mut app, 'n');
    typing(&mut app, "Books/Sci-fi/Hyperion");
    key(&mut app, KeyCode::Enter);
    let view = app.view().unwrap();
    assert_eq!(
        view.editor.lines,
        ["---", "status: to read", "---", "# Hyperion", "author:: "],
        "the template of the nearest folder that has one: {}",
        app.message
    );
    assert_eq!((view.editor.row, view.editor.col), (4, 9), "at its cursor");
    // Elsewhere (the quick switcher makes it at the top), it stays empty.
    ctrl(&mut app, 'o');
    typing(&mut app, "Loose");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.view().unwrap().editor.lines, [""]);
    assert_eq!(active_path(&app), Some(note(&app, "Loose.md")));
}

#[test]
fn folder_templates_are_listed_and_added_in_the_settings() {
    let mut app = folder_templates_vault("folder-templates-settings");
    run_palette(&mut app, "templater settings");
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Folder templates").is_some(), "{rows:#?}");
    let (y, _) = find(&rows, "Books/").expect("the list");
    assert!(rows[y].contains("Book"), "{:?}", rows[y]);
    // The last row adds one: a folder, then a template.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Add a folder template").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Folder").is_some(), "{rows:#?}");
    typing(&mut app, "journal");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "daily");
    key(&mut app, KeyCode::Enter);
    let settings = fs::read_to_string(
        app.vault
            .root
            .join(".blackglass/plugins/templater/settings.toml"),
    )
    .unwrap();
    assert!(settings.contains("Journal = \"Daily\""), "{settings}");
    assert!(app.message.contains("Journal/"), "{}", app.message);
    // And used at once.
    ctrl(&mut app, 'n');
    typing(&mut app, "Journal/Trip");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.view().unwrap().editor.lines[0], "# Trip");
}

#[test]
fn a_template_is_applied_to_the_open_note() {
    let mut app = folder_templates_vault("apply-template");
    let path = note(&app, "Books/Dune.md");
    fs::write(&path, "---\ntags: [scifi]\nstatus: read\n---\nSpice.\n").unwrap();
    app.rescan();
    app.open(&path);
    run_palette(&mut app, "apply template");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Apply template").is_some(), "{rows:#?}");
    typing(&mut app, "book");
    key(&mut app, KeyCode::Enter);
    let view = app.view().unwrap();
    assert_eq!(
        view.editor.to_text(),
        "---\ntags: [scifi]\nstatus: read\n---\n# Dune\nauthor:: \nSpice.\n",
        "the template on top, the note's properties kept and joined"
    );
    assert_eq!((view.editor.row, view.editor.col), (5, 9));
    assert!(view.is_dirty());
}

#[test]
fn a_folder_templates_questions_are_asked() {
    let mut files = TEMPLATER.to_vec();
    files.extend([
        (
            "Templates/Book.md",
            "---\nauthor: <% tp.system.prompt(\"Author\") %>\n---\n# <% tp.file.title %>\n",
        ),
        ("Books/Dune.md", "# Dune"),
        (
            ".blackglass/plugins/templater/settings.toml",
            "[folder_templates]\nBooks = \"Book\"\n",
        ),
    ]);
    let mut app = App::new(Vault::open(&vault("folder-template-questions", &files)).unwrap());
    ctrl(&mut app, 'n');
    typing(&mut app, "Books/Hyperion");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Author").is_some(), "{rows:#?}");
    typing(&mut app, "Simmons");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["---", "author: Simmons", "---", "# Hyperion"]
    );
    assert_eq!(active_path(&app), Some(note(&app, "Books/Hyperion.md")));
}

#[test]
fn the_indentation_width_is_an_editor_setting() {
    let (mut app, config) = app_with_config("indent-width");
    let path = note(&app, "Welcome.md");
    fs::write(&path, "- a\n- b\n").unwrap();
    app.open(&path);
    alt(&mut app, KeyCode::Char(','));
    key(&mut app, KeyCode::Enter); // Editor
    let rows = screen(&mut app, 100, 30);
    let (y, _) = find(&rows, "Indentation").expect("the setting");
    assert!(rows[y].contains('2'), "the default: {:?}", rows[y]);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter); // edit
    key(&mut app, KeyCode::Backspace);
    typing(&mut app, "4");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.shared.config.indent_width, 4, "{}", app.message);
    let saved = fs::read_to_string(config.join("config.toml")).unwrap();
    assert!(saved.contains("indent_width = \"4\""), "{saved}");
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.view().unwrap().editor.lines, ["- a", "    - b"]);
}

#[test]
fn the_settings_window_has_grouped_pages_and_a_search() {
    let dir = vault(
        "settings-window",
        &[("Home.md", ""), (".blackglass/plugins.toml", ALL_PLUGINS)],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    alt(&mut app, KeyCode::Char(','));
    let rows = screen(&mut app, 110, 32);
    for text in [
        "Search settings",
        "Options",
        "Editor",
        "Notes",
        "Keyboard shortcuts",
        "Plugins",
        "Plugin options",
        "Dataview",
        "Templater",
        "Periodic Notes",
    ] {
        assert!(find(&rows, text).is_some(), "{text}: {rows:#?}");
    }
    // The chosen page is shown beside the list: each setting with what it
    // does below it, and its value.
    let (y, _) = find(&rows, "Auto-pair brackets").expect("the Editor page");
    assert!(rows[y].contains("on"), "{:?}", rows[y]);
    assert!(rows[y + 1].contains("Typing ( [ {"), "{:?}", rows[y + 1]);
    // ↓ shows the next page at once.
    key(&mut app, KeyCode::Down);
    let rows = screen(&mut app, 110, 32);
    assert!(find(&rows, "Note IDs").is_some(), "notes: {rows:#?}");
    key(&mut app, KeyCode::Down);
    let rows = screen(&mut app, 110, 32);
    assert!(find(&rows, "Go to note").is_some(), "shortcuts: {rows:#?}");
    // A search keeps only the pages and settings that match.
    typing(&mut app, "folder");
    let rows = screen(&mut app, 110, 32);
    assert!(find(&rows, "Move note to a folder").is_some(), "{rows:#?}");
    assert!(find(&rows, "Go to note").is_none(), "{rows:#?}");
    assert!(find(&rows, "Editor").is_none(), "no match there: {rows:#?}");
    assert!(find(&rows, "Templater").is_some(), "{rows:#?}");
    // A click chooses a page.
    let (y, x) = find(&rows, "Periodic Notes").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(settings_page(&app).map(|p| p.0), Some(Page::Plugin(2)));
    key(&mut app, KeyCode::Esc);
    assert!(app.prompt.is_none());
}

#[test]
fn backlinks_show_and_hide_under_the_note() {
    let mut app = app("backlinks");
    let dune = note(&app, "Books/Dune.md");
    app.open(&dune);
    let rows = screen(&mut app, 100, 30);
    assert!(
        find(&rows, "Backlinks").is_none(),
        "hidden at first: {rows:#?}"
    );
    // Alt+L shows them under the note, with the focus.
    alt(&mut app, KeyCode::Char('l'));
    assert_eq!(app.focus, Focus::Backlinks);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Backlinks (2)").is_some(), "{rows:#?}");
    assert!(find(&rows, "Welcome").is_some(), "{rows:#?}");
    assert!(
        find(&rows, "See [[Dune]].").is_some(),
        "the line: {rows:#?}"
    );
    assert!(
        find(&rows, "2026-08-09").is_some(),
        "an alias link too: {rows:#?}"
    );
    // ↓ and Enter open the linking note at its line.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Welcome.md")));
    assert_eq!(app.view().unwrap().editor.row, 1);
    assert_eq!(app.focus, Focus::Editor);
    // The pane follows the note.
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "No backlinks").is_some(), "{rows:#?}");
    // A click opens a backlink.
    app.open(&dune);
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "See [[Dune]].").unwrap();
    click(&mut app, x as u16, y as u16);
    assert_eq!(active_path(&app), Some(note(&app, "Welcome.md")));
    // A link saved in another note shows up.
    let other = note(&app, "Journal/2026-08-10.md");
    app.open(&other);
    key(&mut app, KeyCode::End);
    typing(&mut app, " [[Dune]]");
    ctrl(&mut app, 's');
    app.open(&dune);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Backlinks (3)").is_some(), "{rows:#?}");
    // Alt+L hides them.
    alt(&mut app, KeyCode::Char('l'));
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Backlinks").is_none(), "{rows:#?}");
    assert_eq!(app.focus, Focus::Editor);
}

#[test]
fn another_vault_is_opened_or_created_from_the_palette() {
    let (mut app, config) = app_with_config("open-vault");
    let other = vault("open-vault-other", &[("Other note.md", "hi")]);
    run_palette(&mut app, "open vault");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Open vault").is_some(), "{rows:#?}");
    ctrl(&mut app, 'u');
    typing(&mut app, other.to_str().unwrap());
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.vault.root, other, "{}", app.message);
    assert!(app.tabs.is_empty());
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Other note").is_some(), "{rows:#?}");
    let recent = blackglass::config::recent_vaults(&config);
    assert!(recent.contains(&other), "remembered: {recent:?}");
    // A folder that isn't there yet is made after asking.
    let new = Path::new(env!("CARGO_TARGET_TMPDIR")).join("open-vault-new");
    let _ = fs::remove_dir_all(&new);
    run_palette(&mut app, "open vault");
    ctrl(&mut app, 'u');
    typing(&mut app, new.join("Notes").to_str().unwrap());
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Enter again creates it").is_some(), "{rows:#?}");
    assert!(!new.exists());
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.vault.root,
        mdedit::platform::canonical(&new.join("Notes")).unwrap()
    );
    // Unsaved changes keep the vault.
    fs::write(app.vault.root.join("A.md"), "").unwrap();
    app.rescan();
    let a = note(&app, "A.md");
    app.open(&a);
    typing(&mut app, "x");
    run_palette(&mut app, "open vault");
    ctrl(&mut app, 'u');
    typing(&mut app, other.to_str().unwrap());
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.vault.root,
        mdedit::platform::canonical(&new.join("Notes")).unwrap()
    );
    assert!(app.message.contains("unsaved"), "{}", app.message);
}

#[test]
fn ctrl_b_cycles_through_every_open_pane() {
    let dir = vault(
        "focus-cycle",
        &[
            ("A.md", "[[B]]"),
            ("B.md", ""),
            (
                ".blackglass/plugins.toml",
                "installed = [\"periodic-notes\"]\nenabled = [\"periodic-notes\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let b = note(&app, "B.md");
    app.open(&b);
    alt(&mut app, KeyCode::Char('c')); // the calendar
    alt(&mut app, KeyCode::Char('l')); // the backlinks
    assert_eq!(app.focus, Focus::Backlinks);
    let mut seen = Vec::new();
    for _ in 0..8 {
        ctrl(&mut app, 'b');
        seen.push(app.focus);
    }
    use Focus::{Backlinks, Editor, Panel, Sidebar};
    assert_eq!(
        seen,
        [
            Sidebar, Panel, Editor, Backlinks, Sidebar, Panel, Editor, Backlinks
        ],
        "every open pane, in screen order, again and again"
    );
    // Hidden panes drop out of the cycle.
    alt(&mut app, KeyCode::Char('l'));
    alt(&mut app, KeyCode::Char('c'));
    let mut seen = Vec::new();
    for _ in 0..4 {
        ctrl(&mut app, 'b');
        seen.push(app.focus);
    }
    assert_eq!(seen, [Sidebar, Editor, Sidebar, Editor]);
}

/// The sidebar's list rows (below its tabs and header, above the status
/// bar), left of its edge.
fn sidebar(rows: &[String]) -> Vec<String> {
    rows[2..rows.len() - 1]
        .iter()
        .map(|r| r.chars().take_while(|&c| c != '│').collect())
        .collect()
}

#[test]
fn recent_files_is_a_plugin_with_a_sidebar_tab() {
    let mut app = app("recent-files");
    let rows = screen(&mut app, 100, 24);
    assert!(
        find(&rows, "Recent").is_none(),
        "not installed by default: {rows:#?}"
    );
    // Install it in the settings' Plugins page.
    run_palette(&mut app, "open plugins");
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    let rows = screen(&mut app, 100, 30);
    let (y, _) = find(&rows, "Recent Files").expect("in the list");
    assert!(rows[y].contains("not installed"), "{:?}", rows[y]);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.message, "Recent Files installed and enabled");
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    let rows = screen(&mut app, 100, 24);
    assert!(
        rows[0].contains("Files  Search  Tags  Recent"),
        "{:?}",
        rows[0]
    );
    // Every note visited, newest first, once.
    for rel in ["Welcome.md", "Books/Dune.md", "Journal/2026-08-09.md"] {
        let path = note(&app, rel);
        app.open(&path);
    }
    alt(&mut app, KeyCode::Char('1')); // back to Welcome's tab
    let rows = screen(&mut app, 100, 24);
    let (y, x) = find(&rows, "Recent").unwrap();
    click(&mut app, x as u16, y as u16);
    let rows = screen(&mut app, 100, 24);
    let order: Vec<usize> = ["Welcome", "2026-08-09", "Dune"]
        .iter()
        .map(|name| find(&sidebar(&rows), name).expect(name).0)
        .collect();
    assert!(order[0] < order[1] && order[1] < order[2], "{rows:#?}");
    // Enter opens one; Delete takes one off the list.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Books/Dune.md")));
    ctrl(&mut app, 'b');
    let rows = screen(&mut app, 100, 24);
    assert_eq!(
        find(&sidebar(&rows), "Dune").map(|p| p.0),
        Some(0),
        "Dune on top: {rows:#?}"
    );
    key(&mut app, KeyCode::Delete);
    let rows = screen(&mut app, 100, 24);
    assert!(
        find(&sidebar(&rows), "Dune").is_none(),
        "removed: {rows:#?}"
    );
    // Kept in the vault, for the next start.
    let saved = fs::read_to_string(
        app.vault
            .root
            .join(".blackglass/plugins/recent-files/recent.toml"),
    )
    .unwrap();
    assert!(
        saved.contains("Welcome.md") && !saved.contains("Dune"),
        "{saved}"
    );
    let mut again = App::new(Vault::open(&app.vault.root).unwrap());
    again.sidebar.show(Panel::Plugin("recent-files"));
    let rows = screen(&mut again, 100, 24);
    assert!(find(&sidebar(&rows), "Welcome").is_some(), "{rows:#?}");
}

#[test]
fn recent_files_commands_and_tab_only_while_it_is_enabled() {
    let mut app = app("recent-files-palette");
    ctrl(&mut app, 'p');
    typing(&mut app, "recent files");
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Recent Files: Open").is_none(), "{rows:#?}");
    key(&mut app, KeyCode::Esc);
    let i = app.plugins.borrow().index("recent-files").unwrap();
    let result = app.plugins.borrow_mut().toggle_installed(i, &app.vault);
    result.unwrap();
    run_palette(&mut app, "recent files open");
    assert_eq!(app.sidebar.panel, Panel::Plugin("recent-files"));
    assert_eq!(app.focus, Focus::Sidebar);
    // "Clear list" empties it.
    let welcome = note(&app, "Welcome.md");
    app.open(&welcome);
    run_palette(&mut app, "recent files clear list");
    assert!(app.message.contains("cleared"), "{}", app.message);
    app.sidebar.show(Panel::Plugin("recent-files"));
    let rows = screen(&mut app, 100, 24);
    assert!(find(&sidebar(&rows), "Nothing yet").is_some(), "{rows:#?}");
    // Disabled: the command and the tab go.
    let result = app.plugins.borrow_mut().toggle_enabled(i, &app.vault);
    result.unwrap();
    let rows = screen(&mut app, 100, 24);
    assert!(!rows[0].contains("Recent"), "{:?}", rows[0]);
    assert_eq!(app.sidebar.panel, Panel::Files, "back to the files");
    ctrl(&mut app, 'p');
    typing(&mut app, "recent files");
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Recent Files: Open").is_none(), "{rows:#?}");
}

#[test]
fn web_links_open_in_the_browser() {
    let dir = vault(
        "web-links",
        &[("A.md", "See [the site](https://example.com/x) now.")],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = std::rc::Rc::clone(&opened);
    app.opener = Box::new(move |path| {
        seen.borrow_mut().push(path.to_path_buf());
        Ok(())
    });
    let a = note(&app, "A.md");
    app.open(&a);
    app.tabs[0].editor.col = 6;
    press(&mut app, KeyCode::Enter, KeyModifiers::CONTROL);
    assert_eq!(*opened.borrow(), [PathBuf::from("https://example.com/x")]);
    assert!(app.message.contains("browser"), "{}", app.message);
}

#[test]
fn a_link_to_a_missing_note_offers_to_create_it() {
    let dir = vault(
        "missing-link",
        &[("A.md", "Idea: [[New idea]] and [[Ideas/Later]]")],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a);
    app.tabs[0].editor.col = 9;
    press(&mut app, KeyCode::Enter, KeyModifiers::CONTROL);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Create New idea?").is_some(), "{rows:#?}");
    // Esc: nothing is made.
    key(&mut app, KeyCode::Esc);
    assert!(!dir.join("New idea.md").exists());
    press(&mut app, KeyCode::Enter, KeyModifiers::CONTROL);
    key(&mut app, KeyCode::Enter);
    assert!(dir.join("New idea.md").is_file());
    assert_eq!(active_path(&app), Some(dir.join("New idea.md")));
    // A path in the link makes its folders.
    app.open(&a);
    app.tabs[app.active].editor.col = 25;
    press(&mut app, KeyCode::Enter, KeyModifiers::CONTROL);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(dir.join("Ideas/Later.md")));
}

#[test]
fn the_status_bar_counts_words() {
    let dir = vault(
        "word-count",
        &[("A.md", "---\ntags: [x]\n---\nOne two three.\nFour five")],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a);
    let rows = screen(&mut app, 120, 20);
    let status = rows.last().unwrap();
    assert!(status.contains("5 words · 23 chars"), "{status:?}");
    // With a selection: its words of all.
    (app.tabs[0].editor.row, app.tabs[0].editor.col) = (3, 0);
    for _ in 0..4 {
        press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
    }
    let rows = screen(&mut app, 120, 20);
    let status = rows.last().unwrap();
    assert!(status.contains("1 of 5 words"), "{status:?}");
}

#[test]
fn a_unique_note_is_named_by_the_time() {
    let mut app = app("unique-note");
    run_palette(&mut app, "new unique note");
    let path = active_path(&app).expect("made and opened");
    let name = path.file_stem().unwrap().to_string_lossy().into_owned();
    assert_eq!(name.len(), 12, "{name}");
    assert!(name.chars().all(|c| c.is_ascii_digit()), "{name}");
    assert!(name.starts_with(&chrono::Local::now().format("%Y%m%d").to_string()));
    // Two in the same minute don't clash.
    run_palette(&mut app, "new unique note");
    let second = active_path(&app).unwrap();
    assert_ne!(second, path);
    assert!(second.is_file());
}

#[test]
fn renaming_a_note_updates_the_links_to_it() {
    let files = [
        ("Books/Dune.md", "# Dune"),
        (
            "A.md",
            "See [[Dune]], [[Dune|the book]], [[Books/Dune#Plot]] and [md](Books/Dune.md).",
        ),
        ("B.md", "![[Dune]] and [[Dunes]]"),
        ("C.md", "```\n[[Dune]]\n```"),
    ];
    let dir = vault("rename-note", &files);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a); // open (unchanged): it shows the new links
    let dune = note(&app, "Books/Dune.md");
    app.open(&dune);
    key(&mut app, KeyCode::F(2));
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Rename Dune").is_some(), "{rows:#?}");
    ctrl(&mut app, 'u');
    typing(&mut app, "Arrakis");
    key(&mut app, KeyCode::Enter);
    let arrakis = dir.join("Books/Arrakis.md");
    assert!(arrakis.is_file() && !dune.exists(), "{}", app.message);
    assert_eq!(active_path(&app), Some(arrakis));
    assert_eq!(
        fs::read_to_string(&a).unwrap(),
        "See [[Arrakis]], [[Arrakis|the book]], [[Books/Arrakis#Plot]] and [md](Books/Arrakis.md)."
    );
    assert_eq!(
        fs::read_to_string(dir.join("B.md")).unwrap(),
        "![[Arrakis]] and [[Dunes]]"
    );
    assert_eq!(
        fs::read_to_string(dir.join("C.md")).unwrap(),
        "```\n[[Dune]]\n```",
        "not in code"
    );
    assert!(
        app.message.contains("5 links in 2 notes"),
        "{}",
        app.message
    );
    let a_tab = app
        .tabs
        .iter()
        .find(|t| t.path.as_deref() == Some(a.as_path()))
        .unwrap();
    assert!(
        a_tab.editor.lines[0].starts_with("See [[Arrakis]]"),
        "the open tab too"
    );
    // A name that's taken is refused.
    key(&mut app, KeyCode::F(2));
    ctrl(&mut app, 'u');
    typing(&mut app, "../A");
    key(&mut app, KeyCode::Enter);
    assert!(app.message.contains("can't leave"), "{}", app.message);
}

#[test]
fn folders_are_made_and_renamed_with_their_links() {
    let files = [("Books/Dune.md", ""), ("A.md", "[[Books/Dune]] [[Dune]]")];
    let dir = vault("rename-folder", &files);
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "new folder");
    typing(&mut app, "Reading/Now");
    key(&mut app, KeyCode::Enter);
    assert!(dir.join("Reading/Now").is_dir(), "{}", app.message);
    let rows = screen(&mut app, 100, 24);
    assert!(
        find(&rows, "Reading").is_some(),
        "in the explorer: {rows:#?}"
    );
    // Rename the folder chosen in the explorer.
    app.sidebar.selected = Some(dir.join("Books"));
    run_palette(&mut app, "rename folder");
    ctrl(&mut app, 'u');
    typing(&mut app, "Library");
    key(&mut app, KeyCode::Enter);
    assert!(dir.join("Library/Dune.md").is_file(), "{}", app.message);
    assert!(!dir.join("Books").exists());
    assert_eq!(
        fs::read_to_string(dir.join("A.md")).unwrap(),
        "[[Library/Dune]] [[Dune]]"
    );
}

const PROPERTIES: &[(&str, &str)] = &[
    (
        "Books/Dune.md",
        "---\naliases: [Arrakis book, Spice]\nstatus: read\n---\n# Dune",
    ),
    (
        "Emma.md",
        "---\naliases:\n  - Miss Woodhouse\nstatus: reading\n---\n# Emma",
    ),
    ("A.md", ""),
];

#[test]
fn aliases_find_notes_in_the_switcher_and_in_links() {
    let dir = vault("aliases", PROPERTIES);
    let mut app = App::new(Vault::open(&dir).unwrap());
    ctrl(&mut app, 'o');
    typing(&mut app, "woodhouse");
    let rows = screen(&mut app, 100, 24);
    let (y, _) = find(&rows, "Miss Woodhouse").expect("the alias is shown");
    assert!(rows[y].contains("Emma"), "{:?}", rows[y]);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Emma.md")));
    // `[[` suggestions: an alias makes `[[Note|Alias]]`.
    let a = note(&app, "A.md");
    app.open(&a);
    typing(&mut app, "[[spice");
    key(&mut app, KeyCode::Enter);
    assert_eq!(line(&app), "[[Dune|Spice]]");
}

#[test]
fn properties_are_listed_searched_and_renamed() {
    let dir = vault("properties-view", PROPERTIES);
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "show properties");
    let rows = screen(&mut app, 100, 24);
    let (y, _) = find(&rows, "status").expect("listed");
    assert!(rows[y].contains("2 notes"), "{:?}", rows[y]);
    typing(&mut app, "status");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.sidebar.query, "[status]");
    assert_eq!(app.focus, Focus::Sidebar);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&sidebar(&rows), "Dune").is_some() && find(&sidebar(&rows), "Emma").is_some());
    // Rename it in every note.
    run_palette(&mut app, "rename property");
    typing(&mut app, "status");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "state");
    key(&mut app, KeyCode::Enter);
    let dune = fs::read_to_string(dir.join("Books/Dune.md")).unwrap();
    assert!(
        dune.contains("state: read") && !dune.contains("status"),
        "{dune}"
    );
    assert!(
        fs::read_to_string(dir.join("Emma.md"))
            .unwrap()
            .contains("state: reading")
    );
    assert!(app.message.contains("2 notes"), "{}", app.message);
}

#[test]
fn a_linked_note_is_previewed_in_a_popup() {
    let files = [
        ("A.md", "Read [[Dune#Plot]] later."),
        (
            "Books/Dune.md",
            "# Dune\n## Setting\nArrakis\n## Plot\nThe spice must flow\n",
        ),
    ];
    let dir = vault("link-preview", &files);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a);
    app.tabs[0].editor.col = 8;
    alt(&mut app, KeyCode::Char('p'));
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Dune").is_some(), "{rows:#?}");
    assert!(
        find(&rows, "The spice must flow").is_some(),
        "at the heading: {rows:#?}"
    );
    assert_eq!(app.tabs.len(), 1, "nothing opened");
    // Keys scroll and don't reach the note; Esc closes.
    typing(&mut app, "x");
    key(&mut app, KeyCode::Esc);
    assert!(app.preview.is_none());
    assert_eq!(
        app.tabs[0].editor.to_text(),
        "Read [[Dune#Plot]] later.",
        "not typed into"
    );
    // Enter opens the note there.
    alt(&mut app, KeyCode::Char('p'));
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Books/Dune.md")));
    // Not on a link: says so.
    app.open(&a);
    app.tabs[app.active].editor.col = 0;
    alt(&mut app, KeyCode::Char('p'));
    assert!(app.preview.is_none());
    assert!(app.message.contains("link"), "{}", app.message);
}

#[test]
fn mermaid_diagrams_are_drawn_by_their_plugin() {
    let files = [
        (
            "D.md",
            "top\n```mermaid\nflowchart TD\n  A[Start] --> B{Ready?}\n  B -->|yes| C[Go]\n```\nafter",
        ),
        (
            ".blackglass/plugins.toml",
            "installed = [\"mermaid\"]\nenabled = [\"mermaid\"]\n",
        ),
    ];
    let dir = vault("mermaid", &files);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let d = note(&app, "D.md");
    app.open(&d);
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "└─▶ {Ready?}").is_some(), "{rows:#?}");
    assert!(find(&rows, "└─yes─▶ [Go]").is_some(), "{rows:#?}");
    assert!(
        find(&rows, "flowchart TD").is_none(),
        "drawn, not the source: {rows:#?}"
    );
}

#[test]
fn a_pdfs_text_becomes_a_note() {
    let dir = vault("pdf-note", &[("A.md", "")]);
    fs::create_dir_all(dir.join("Papers")).unwrap();
    fs::write(
        dir.join("Papers/Study.pdf"),
        blackglass::pdf::sample(&["Results are good"]),
    )
    .unwrap();
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "pdf to note");
    let rows = screen(&mut app, 100, 24);
    assert!(
        find(&rows, "Papers/Study.pdf").is_some(),
        "the vault's PDFs: {rows:#?}"
    );
    key(&mut app, KeyCode::Enter);
    let made = dir.join("Papers/Study.md");
    let text = fs::read_to_string(&made).unwrap();
    assert!(text.starts_with("# Study\n"), "{text}");
    assert!(text.contains("![[Papers/Study.pdf]]"), "links back: {text}");
    assert!(text.contains("Results are good"), "{text}");
    assert_eq!(active_path(&app), Some(made.clone()));
    // Again: a new name, not over the first.
    run_palette(&mut app, "pdf to note");
    key(&mut app, KeyCode::Enter);
    assert!(
        dir.join("Papers/Study (PDF text).md").is_file(),
        "{}",
        app.message
    );
}

#[test]
fn a_random_note_opens() {
    let dir = vault("random-note", &[("A.md", ""), ("B.md", "")]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a);
    run_palette(&mut app, "open random note");
    assert_eq!(active_path(&app), Some(note(&app, "B.md")), "another note");
}

#[test]
fn the_outline_jumps_to_a_heading() {
    let dir = vault(
        "outline",
        &[("A.md", "# Top\ntext\n## Middle\nmore\n### Deep\nend")],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a);
    alt(&mut app, KeyCode::Char('o'));
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Outline").is_some(), "{rows:#?}");
    let (y1, x1) = find(&rows, "Middle").unwrap();
    let (_, x2) = find(&rows, "Deep").unwrap();
    assert!(
        x2 > x1 && find(&rows, "Top").is_some_and(|(_, x)| x < x1),
        "indented by level: {:?}",
        rows[y1]
    );
    typing(&mut app, "deep");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.view().unwrap().editor.row, 4);
}

#[test]
fn outgoing_links_and_unlinked_mentions_join_the_backlinks() {
    let files = [
        ("Apple.md", "[[Berry]] tastes like crane."),
        ("Berry.md", "See [[Crane]] and [[Nowhere]]."),
        ("Crane.md", ""),
    ];
    let dir = vault("links-pane", &files);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let berry = note(&app, "Berry.md");
    app.open(&berry);
    alt(&mut app, KeyCode::Char('l'));
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Backlinks (1)").is_some(), "{rows:#?}");
    assert!(find(&rows, "Outgoing links (2)").is_some(), "{rows:#?}");
    assert!(find(&rows, "Nowhere (missing)").is_some(), "{rows:#?}");
    // Enter on an outgoing link opens its note.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Crane.md")));
    // Crane: a backlink from Berry, a mention in Apple; `l` links it.
    alt(&mut app, KeyCode::Char('l'));
    alt(&mut app, KeyCode::Char('l'));
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Unlinked mentions (1)").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Char('l'));
    assert_eq!(
        fs::read_to_string(dir.join("Apple.md")).unwrap(),
        "[[Berry]] tastes like [[crane]]."
    );
    let rows = screen(&mut app, 100, 30);
    assert!(
        find(&rows, "Backlinks (2)").is_some(),
        "now a backlink: {rows:#?}"
    );
}

/// Runs `git` in `dir` (a test's own repositories).
/// Whether `program` isn't there to run (said, for a test that's then
/// skipped: Git or curl under Wine, say).
fn missing(program: &str) -> bool {
    let there = std::process::Command::new(program)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok();
    if !there {
        eprintln!("skipped: no {program} here");
    }
    !there
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Calls the plugins' tick until the message says `done` (Git runs in the
/// background).
fn wait_for(app: &mut App, done: &str) {
    for _ in 0..200 {
        app.tick();
        if app.message.contains(done) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("no {done:?}: {}", app.message);
}

/// Ticks until a question is asked.
fn wait_prompt(app: &mut App) {
    for _ in 0..200 {
        app.tick();
        if matches!(app.prompt, Some(Prompt::Ask(_))) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("no question: {}", app.message);
}

/// Ticks until the active tab's title starts with `title`.
fn wait_title(app: &mut App, title: &str) {
    for _ in 0..200 {
        app.tick();
        if app.view().is_some_and(|v| v.title().starts_with(title)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("no tab {title:?}: {}", app.message);
}

/// Ticks until a plugin's status says `text`.
fn wait_status(app: &mut App, text: &str) {
    for _ in 0..200 {
        app.tick();
        if app.plugin_status().contains(text) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("no {text:?} in the status: {}", app.plugin_status());
}

#[test]
fn git_backs_up_and_syncs_the_vault() {
    if missing("git") {
        return;
    }
    let dir = vault(
        "git-sync",
        &[
            ("Note.md", "first"),
            (
                ".blackglass/plugins.toml",
                "installed = [\"git\"]\nenabled = [\"git\"]\n",
            ),
        ],
    );
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("git-sync-remote");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    git(&base, &["init", "--bare", "remote.git"]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "git initialize");
    key(&mut app, KeyCode::Enter); // commit everything
    wait_for(&mut app, "repository");
    assert!(dir.join(".git").is_dir());
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(
        &dir,
        &[
            "remote",
            "add",
            "origin",
            base.join("remote.git").to_str().unwrap(),
        ],
    );
    run_palette(&mut app, "git commit-and-sync");
    wait_for(&mut app, "pushed");
    let log = git(&base.join("remote.git"), &["log", "--oneline", "--all"]);
    assert!(log.contains("vault backup"), "{log}");
    // The status bar says the branch, and what's waiting.
    let branch = git(&dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    fs::write(dir.join("Note.md"), "edited").unwrap();
    run_palette(&mut app, "git commit all changes");
    wait_for(&mut app, "committed");
    wait_status(&mut app, "1 to push");
    let opened = note(&app, "Note.md");
    app.open(&opened);
    app.message.clear();
    let rows = screen(&mut app, 120, 20);
    let status = rows.last().unwrap();
    assert!(
        status.contains(&format!("{branch} · 1 to push")),
        "{status:?}"
    );
    run_palette(&mut app, "git push");
    wait_for(&mut app, "pushed");
    // A change made elsewhere comes in with a pull, open notes follow.
    let note_path = note(&app, "Note.md");
    app.open(&note_path);
    git(&base, &["clone", "remote.git", "other"]);
    let other = base.join("other");
    git(&other, &["config", "user.email", "o@example.com"]);
    git(&other, &["config", "user.name", "Other"]);
    fs::write(other.join("Note.md"), "changed elsewhere").unwrap();
    fs::write(other.join("New.md"), "new note").unwrap();
    git(&other, &["add", "-A"]);
    git(&other, &["commit", "-m", "elsewhere"]);
    git(&other, &["push"]);
    run_palette(&mut app, "git pull");
    wait_for(&mut app, "pulled");
    assert!(
        app.vault.note(&dir.join("New.md")).is_some(),
        "scanned again"
    );
    assert_eq!(
        app.view().unwrap().editor.lines[0],
        "changed elsewhere",
        "the open note too"
    );
}

#[test]
fn git_shows_changes_diffs_and_history() {
    if missing("git") {
        return;
    }
    let dir = vault(
        "git-changes",
        &[
            ("Note.md", "first\n"),
            (
                ".blackglass/plugins.toml",
                "installed = [\"git\"]\nenabled = [\"git\"]\n",
            ),
        ],
    );
    git(&dir, &["init"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "start"]);
    fs::write(dir.join("Note.md"), "first\nedited\n").unwrap();
    fs::write(dir.join("New.md"), "new").unwrap();
    let mut app = App::new(Vault::open(&dir).unwrap());
    wait_status(&mut app, "2 changed");
    // The Git tab lists the changes.
    app.sidebar.show(Panel::Plugin("git"));
    app.focus = Focus::Sidebar;
    let rows = screen(&mut app, 100, 24);
    let side = sidebar(&rows);
    let (y, _) = find(&side, "New").expect("the new file");
    assert!(side[y].contains("new"), "{:?}", side[y]);
    let (y, _) = find(&side, "Note").expect("the edited one");
    assert!(side[y].contains("modified"), "{:?}", side[y]);
    // s stages New, c commits what's staged.
    key(&mut app, KeyCode::Char('s'));
    wait_for(&mut app, "staged");
    key(&mut app, KeyCode::Char('c'));
    typing(&mut app, "add new");
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "committed");
    let log = git(&dir, &["log", "--oneline"]);
    assert!(log.contains("add new"), "{log}");
    assert!(
        git(&dir, &["status", "--porcelain"]).contains("Note.md"),
        "not committed"
    );
    // The open note's diff, in a read-only tab.
    let n = note(&app, "Note.md");
    app.open(&n);
    run_palette(&mut app, "git show the diff");
    wait_title(&mut app, "Diff: Note.md");
    assert_eq!(app.view().unwrap().title(), "Diff: Note.md");
    assert!(app.view().unwrap().editor.to_text().contains("+edited"));
    typing(&mut app, "x");
    assert!(!app.view().unwrap().is_dirty(), "read-only");
    // History: choose a commit, see it.
    run_palette(&mut app, "git history");
    for _ in 0..200 {
        app.tick();
        if matches!(app.prompt, Some(Prompt::Ask(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "add new").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    wait_title(&mut app, "Commit ");
    assert!(app.view().unwrap().editor.to_text().contains("New.md"));
    // d discards a file's changes, after asking.
    app.sidebar.show(Panel::Plugin("git"));
    app.focus = Focus::Sidebar;
    wait_status(&mut app, "1 changed");
    key(&mut app, KeyCode::Char('d'));
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "discarded");
    assert_eq!(fs::read_to_string(dir.join("Note.md")).unwrap(), "first\n");
}

#[test]
fn git_branches_remotes_ignores_and_raw_commands() {
    if missing("git") {
        return;
    }
    let dir = vault(
        "git-chores",
        &[
            ("Note.md", "first\n"),
            ("Draft.md", "private\n"),
            (
                ".blackglass/plugins.toml",
                "installed = [\"git\"]\nenabled = [\"git\"]\n",
            ),
        ],
    );
    git(&dir, &["init"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(&dir, &["add", "Note.md"]);
    git(&dir, &["commit", "-m", "start"]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    // A new branch, then back.
    let first = git(&dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    run_palette(&mut app, "git create new branch");
    typing(&mut app, "ideas");
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "on ideas");
    assert_eq!(
        git(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "ideas"
    );
    run_palette(&mut app, "git switch branch");
    wait_prompt(&mut app);
    typing(&mut app, &first);
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, &format!("on {first}"));
    run_palette(&mut app, "git delete branch");
    wait_prompt(&mut app);
    typing(&mut app, "ideas");
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "deleted the branch");
    assert!(!git(&dir, &["branch"]).contains("ideas"));
    // A remote, added then removed.
    run_palette(&mut app, "git edit remotes");
    typing(&mut app, "backup");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "/tmp/nowhere.git");
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "remote backup");
    assert!(git(&dir, &["remote", "-v"]).contains("/tmp/nowhere.git"));
    run_palette(&mut app, "git remove remote");
    wait_prompt(&mut app);
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "removed the remote");
    assert_eq!(git(&dir, &["remote"]).trim(), "");
    // The open note into .gitignore.
    let draft = note(&app, "Draft.md");
    app.open(&draft);
    run_palette(&mut app, "git add file to gitignore");
    assert!(
        fs::read_to_string(dir.join(".gitignore"))
            .unwrap()
            .contains("Draft.md")
    );
    // A raw command's output in a page.
    run_palette(&mut app, "git raw command");
    typing(&mut app, "log --oneline");
    key(&mut app, KeyCode::Enter);
    wait_title(&mut app, "git log --oneline");
    assert!(app.view().unwrap().editor.to_text().contains("start"));
}

#[test]
fn git_marks_changes_and_authors_in_the_margin() {
    if missing("git") {
        return;
    }
    let dir = vault(
        "git-margin",
        &[
            ("Note.md", "a\nb\nc\n"),
            (
                ".blackglass/plugins.toml",
                "installed = [\"git\"]\nenabled = [\"git\"]\n",
            ),
        ],
    );
    git(&dir, &["init"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Tester"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "start"]);
    fs::write(dir.join("Note.md"), "a\nB\nc\nd\n").unwrap();
    let mut app = App::new(Vault::open(&dir).unwrap());
    let n = note(&app, "Note.md");
    app.open(&n);
    for _ in 0..200 {
        app.tick();
        if !app.view().unwrap().margin.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let marks: Vec<(usize, String)> = app
        .view()
        .unwrap()
        .margin
        .iter()
        .map(|(l, m)| (*l, m.to_string()))
        .collect();
    assert_eq!(marks, [(1, "~".to_string()), (3, "+".to_string())]);
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "~ B").is_some(), "{rows:#?}");
    // Jump from change to change.
    run_palette(&mut app, "git go to next change");
    assert_eq!(app.view().unwrap().editor.row, 1);
    run_palette(&mut app, "git go to next change");
    assert_eq!(app.view().unwrap().editor.row, 3);
    run_palette(&mut app, "git go to previous change");
    assert_eq!(app.view().unwrap().editor.row, 1);
    // The line authors instead.
    run_palette(&mut app, "git toggle line author");
    for _ in 0..200 {
        app.tick();
        if app
            .view()
            .unwrap()
            .margin
            .get(&0)
            .is_some_and(|m| m.to_string().contains("Tester"))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let first = app
        .view()
        .unwrap()
        .margin
        .get(&0)
        .map(|m| m.to_string())
        .unwrap_or_default();
    assert!(first.contains("Tester"), "{first:?}");
    let second = app
        .view()
        .unwrap()
        .margin
        .get(&1)
        .map(|m| m.to_string())
        .unwrap_or_default();
    assert!(second.contains("not committed"), "{second:?}");
}

#[test]
fn footnotes_and_comments_in_a_note() {
    let text = "A claim[^1] here. %%secret%%\n%%\nblock comment\n%%\n\n[^1]: The source.\n[^two]: Another.";
    let dir = vault("footnotes", &[("A.md", text)]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a);
    app.tabs[0].editor.row = 4;
    let rows = screen(&mut app, 100, 20);
    assert!(
        find(&rows, "A claim[1] here. secret").is_some(),
        "{rows:#?}"
    );
    assert!(find(&rows, "%%secret").is_none(), "the markers are hidden");
    assert!(find(&rows, "[1] The source.").is_some(), "{rows:#?}");
    // The footnotes list: Enter goes to the definition.
    run_palette(&mut app, "footnotes");
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "[two] Another.").is_some(), "{rows:#?}");
    typing(&mut app, "two");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.view().unwrap().editor.row, 6);
}

#[test]
fn properties_are_edited_in_a_window() {
    let dir = vault(
        "property-editor",
        &[(
            "A.md",
            "---\ntitle: Dune\ndone: false\ntags: [a, b]\n---\nBody",
        )],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a);
    (app.tabs[0].editor.row, app.tabs[0].editor.col) = (5, 2);
    alt(&mut app, KeyCode::Char(';'));
    let rows = screen(&mut app, 100, 24);
    let (y, _) = find(&rows, "title").expect("listed");
    assert!(
        rows[y].contains("Dune") && rows[y].contains("text"),
        "{:?}",
        rows[y]
    );
    let (y, _) = find(&rows, "done").unwrap();
    assert!(rows[y].contains("checkbox"), "{:?}", rows[y]);
    // A checkbox toggles.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert!(app.view().unwrap().editor.to_text().contains("done: true"));
    // A list is edited as a, b, c and keeps its style.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    ctrl(&mut app, 'u');
    typing(&mut app, "x, y, z");
    key(&mut app, KeyCode::Enter);
    // a adds one (its type from its value); Delete removes one.
    key(&mut app, KeyCode::Char('a'));
    typing(&mut app, "rating: 5");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Delete);
    let rows = screen(&mut app, 100, 24);
    let (y, _) = find(&rows, "rating").unwrap();
    assert!(rows[y].contains("number"), "{:?}", rows[y]);
    key(&mut app, KeyCode::Esc);
    let view = app.view().unwrap();
    assert_eq!(
        view.editor.to_text(),
        "---\ndone: true\ntags: [x, y, z]\nrating: 5\n---\nBody"
    );
    assert_eq!(
        (view.editor.row, view.editor.col),
        (5, 2),
        "the cursor stays"
    );
    // One undo step per change.
    ctrl(&mut app, 'z');
    assert!(app.view().unwrap().editor.to_text().contains("title: Dune"));
}

#[test]
fn a_note_without_properties_gets_them() {
    let dir = vault("property-add", &[("A.md", "Body")]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = note(&app, "A.md");
    app.open(&a);
    alt(&mut app, KeyCode::Char(';'));
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "No properties").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Char('a'));
    typing(&mut app, "status: draft");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('r'));
    ctrl(&mut app, 'u');
    typing(&mut app, "state");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Esc);
    assert_eq!(
        app.view().unwrap().editor.to_text(),
        "---\nstate: draft\n---\nBody"
    );
}

#[test]
fn git_stages_previews_and_resets_one_change() {
    if missing("git") {
        return;
    }
    let dir = vault(
        "git-hunks",
        &[
            ("Note.md", "a\nb\nc\nd\n"),
            (
                ".blackglass/plugins.toml",
                "installed = [\"git\"]\nenabled = [\"git\"]\n",
            ),
        ],
    );
    git(&dir, &["init"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "start"]);
    fs::write(dir.join("Note.md"), "a\nB\nc\nD\n").unwrap();
    let mut app = App::new(Vault::open(&dir).unwrap());
    let n = note(&app, "Note.md");
    app.open(&n);
    // Preview the change on line 4.
    app.tabs[0].editor.row = 3;
    run_palette(&mut app, "git preview change");
    wait_title(&mut app, "Change at line 4");
    let page = app.view().unwrap().editor.to_text();
    assert!(page.contains("+D") && !page.contains("+B"), "{page}");
    ctrl(&mut app, 'w');
    // Stage only the change on line 2.
    app.tabs[0].editor.row = 1;
    run_palette(&mut app, "git stage change");
    wait_for(&mut app, "staged the change");
    let staged = git(&dir, &["diff", "--cached"]);
    assert!(staged.contains("+B") && !staged.contains("+D"), "{staged}");
    // Reset the change on line 4: the file and the tab.
    app.tabs[0].editor.row = 3;
    run_palette(&mut app, "git reset change");
    wait_for(&mut app, "reset the change");
    assert_eq!(
        fs::read_to_string(dir.join("Note.md")).unwrap(),
        "a\nB\nc\nd\n"
    );
    assert_eq!(app.view().unwrap().editor.lines[3], "d");
}

/// A vault with the Git plugin, as a repository with one commit.
fn git_vault(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let mut all = files.to_vec();
    all.push((
        ".blackglass/plugins.toml",
        "installed = [\"git\"]\nenabled = [\"git\"]\n",
    ));
    let dir = vault(name, &all);
    git(&dir, &["init"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-m", "start"]);
    dir
}

#[test]
fn git_clones_a_repository_as_the_vault() {
    if missing("git") {
        return;
    }
    let source = git_vault("git-clone-source", &[("Shared.md", "from the source")]);
    let dir = git_vault("git-clone-here", &[("A.md", "")]);
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("git-clone-target");
    let _ = fs::remove_dir_all(&target);
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "git clone");
    typing(&mut app, source.to_str().unwrap());
    key(&mut app, KeyCode::Enter);
    typing(&mut app, target.to_str().unwrap());
    key(&mut app, KeyCode::Enter);
    for _ in 0..200 {
        app.tick();
        if app.vault.root.ends_with("git-clone-target") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(
        app.vault.root,
        mdedit::platform::canonical(&target).unwrap(),
        "{}",
        app.message
    );
    assert!(app.vault.note(&app.vault.root.join("Shared.md")).is_some());
}

#[test]
fn git_conflicts_are_named_aborted_or_resolved() {
    if missing("git") {
        return;
    }
    let dir = git_vault("git-conflict", &[("Note.md", "base\n")]);
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("git-conflict-remote");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    git(&base, &["init", "--bare", "remote.git"]);
    git(
        &dir,
        &[
            "remote",
            "add",
            "origin",
            base.join("remote.git").to_str().unwrap(),
        ],
    );
    git(&dir, &["push", "-u", "origin", "HEAD"]);
    git(&base, &["clone", "remote.git", "other"]);
    let other = base.join("other");
    git(&other, &["config", "user.email", "o@example.com"]);
    git(&other, &["config", "user.name", "Other"]);
    fs::write(other.join("Note.md"), "theirs\n").unwrap();
    git(&other, &["commit", "-am", "theirs"]);
    git(&other, &["push"]);
    fs::write(dir.join("Note.md"), "mine\n").unwrap();
    let mut app = App::new(Vault::open(&dir).unwrap());
    let n = note(&app, "Note.md");
    app.open(&n);
    run_palette(&mut app, "git commit-and-sync");
    wait_for(&mut app, "conflict");
    assert!(app.message.contains("Note.md"), "{}", app.message);
    assert!(
        app.view().unwrap().editor.to_text().contains("<<<<<<<"),
        "the markers shown"
    );
    // Abort: back to before the pull.
    run_palette(&mut app, "git abort merge");
    wait_for(&mut app, "aborted");
    assert_eq!(fs::read_to_string(dir.join("Note.md")).unwrap(), "mine\n");
    // Again, then resolved: commit-and-sync finishes the merge and pushes.
    run_palette(&mut app, "git commit-and-sync");
    wait_for(&mut app, "conflict");
    fs::write(dir.join("Note.md"), "mine and theirs\n").unwrap();
    run_palette(&mut app, "git commit-and-sync");
    wait_for(&mut app, "pushed");
    let log = git(&base.join("remote.git"), &["log", "--oneline", "--all"]);
    assert!(log.contains("theirs") && log.lines().count() >= 4, "{log}");
}

#[test]
fn git_updates_submodules() {
    if missing("git") {
        return;
    }
    let sub = git_vault("git-sub-source", &[("Inside.md", "in the submodule")]);
    let top = git_vault("git-sub-top", &[("A.md", "")]);
    let allow = ["-c", "protocol.file.allow=always"];
    git(
        &top,
        &[
            allow[0],
            allow[1],
            "submodule",
            "add",
            sub.to_str().unwrap(),
            "Sub",
        ],
    );
    git(&top, &["commit", "-m", "with a submodule"]);
    let clone = Path::new(env!("CARGO_TARGET_TMPDIR")).join("git-sub-clone");
    let _ = fs::remove_dir_all(&clone);
    git(
        Path::new(env!("CARGO_TARGET_TMPDIR")),
        &["clone", top.to_str().unwrap(), "git-sub-clone"],
    );
    // Local paths need the file transport allowed (a git safety default).
    git(
        &clone,
        &[allow[0], allow[1], "submodule", "update", "--init"],
    );
    git(
        &clone.join("Sub"),
        &["config", "protocol.file.allow", "always"],
    );
    // The submodule moves on; the top repository follows it.
    fs::write(sub.join("Later.md"), "added later").unwrap();
    git(&sub, &["add", "-A"]);
    git(&sub, &["commit", "-m", "later"]);
    git(&top.join("Sub"), &["pull"]);
    git(&top, &["commit", "-am", "the submodule moved"]);
    git(&clone, &["pull"]);
    assert!(
        !clone.join("Sub/Later.md").exists(),
        "the clone's submodule is behind"
    );
    fs::create_dir_all(clone.join(".blackglass")).unwrap();
    fs::write(
        clone.join(".blackglass/plugins.toml"),
        "installed = [\"git\"]\nenabled = [\"git\"]\n",
    )
    .unwrap();
    let mut app = App::new(Vault::open(&clone).unwrap());
    run_palette(&mut app, "git update submodules");
    wait_for(&mut app, "updated the submodules");
    assert!(clone.join("Sub/Later.md").is_file(), "{}", app.message);
}

/// The background color of the screen's cell (`x`, `y`).
fn bg_at(app: &mut App, width: u16, height: u16, x: u16, y: u16) -> ratatui::style::Color {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    terminal.backend().buffer()[(x, y)].bg
}

#[test]
fn a_theme_is_chosen_with_a_preview_and_saved() {
    use ratatui::style::Color;
    let (mut app, config) = app_with_config("theme");
    let default = bg_at(&mut app, 80, 20, 79, 19);
    assert_eq!(default, Color::Rgb(0x14, 0x14, 0x14), "the status bar");
    run_palette(&mut app, "choose theme");
    let rows = screen(&mut app, 80, 20);
    for name in [
        "Default",
        "Paper",
        "Ember",
        "Ocean",
        "Forest",
        "High contrast",
    ] {
        assert!(find(&rows, name).is_some(), "{name}: {rows:#?}");
    }
    // ↓ previews the next theme at once; Esc goes back.
    key(&mut app, KeyCode::Down);
    let paper = bg_at(&mut app, 80, 20, 79, 19);
    assert_ne!(paper, default, "previewed");
    key(&mut app, KeyCode::Esc);
    assert_eq!(bg_at(&mut app, 80, 20, 79, 19), default, "restored");
    assert!(!config.join("appearance.toml").exists());
    // Enter keeps it, and it's used at the next start.
    run_palette(&mut app, "choose theme");
    typing(&mut app, "ocean");
    key(&mut app, KeyCode::Enter);
    let ocean = bg_at(&mut app, 80, 20, 79, 19);
    assert_ne!(ocean, default);
    let saved = fs::read_to_string(config.join("appearance.toml")).unwrap();
    assert!(saved.contains("theme = \"ocean\""), "{saved}");
    assert_eq!(
        app.shared.config.heading_colors[0],
        Some(Color::Rgb(0x82, 0xaa, 0xff)),
        "the editor's headings too"
    );
    let (mut again, _) = app_with_config("theme-again");
    again.config_dir = Some(config.clone());
    again.load_user_config();
    assert_eq!(bg_at(&mut again, 80, 20, 79, 19), ocean);
}

#[test]
fn a_users_theme_file_is_listed_and_falls_back_to_the_default() {
    use ratatui::style::Color;
    let (mut app, config) = app_with_config("theme-user");
    fs::create_dir_all(config.join("themes")).unwrap();
    fs::write(
        config.join("themes/mine.toml"),
        "name = \"Mine\"\nchrome = \"#ff0000\"\naccent = \"nonsense\"\n",
    )
    .unwrap();
    run_palette(&mut app, "choose theme");
    typing(&mut app, "mine");
    key(&mut app, KeyCode::Enter);
    assert_eq!(bg_at(&mut app, 80, 20, 79, 19), Color::Rgb(0xff, 0, 0));
    assert!(app.message.contains("accent"), "a warning: {}", app.message);
    // Keys it doesn't set are the default theme's.
    let sidebar = bg_at(&mut app, 80, 20, 2, 5);
    assert_eq!(sidebar, Color::Rgb(0x1c, 0x1c, 0x1c), "{}", app.message);
}

#[test]
fn the_theme_is_chosen_from_the_settings_window_too() {
    let (mut app, _) = app_with_config("theme-settings");
    alt(&mut app, KeyCode::Char(','));
    typing(&mut app, "choose a theme");
    key(&mut app, KeyCode::Enter); // to the page
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 80, 20);
    assert!(find(&rows, "Choose theme").is_some(), "{rows:#?}");
    assert!(find(&rows, "High contrast").is_some(), "{rows:#?}");
}

/// A workspace with the Advanced Tables plugin enabled and `note` open.
fn tables_app(name: &str, note_text: &str) -> App {
    let dir = vault(
        name,
        &[
            ("T.md", note_text),
            (
                STATE_FILE,
                "installed = [\"tables\"]\nenabled = [\"tables\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let path = app.vault.root.join("T.md");
    app.open(&path);
    app
}

fn lines(app: &App) -> Vec<String> {
    app.view().unwrap().editor.lines.clone()
}

fn cursor(app: &App) -> (usize, usize) {
    let e = &app.view().unwrap().editor;
    (e.row, e.col)
}

fn put_cursor(app: &mut App, row: usize, col: usize) {
    let view = app.tabs.get_mut(app.active).unwrap();
    view.editor.row = row;
    view.editor.col = col;
}

const TABLE: &str = "Intro\n| Name | Qty |\n|---|--:|\n| apple | 3 |\n| kiwi | 12 |\n\nafter\n";

#[test]
fn tables_are_formatted_and_walked_with_tab_and_enter() {
    let mut app = tables_app("tables-move", TABLE);
    put_cursor(&mut app, 3, 3);
    key(&mut app, KeyCode::Tab);
    assert_eq!(
        lines(&app)[1..5],
        [
            "| Name  | Qty |",
            "| ----- | --: |",
            "| apple |   3 |",
            "| kiwi  |  12 |",
        ],
        "formatted, the right-aligned column too"
    );
    assert_eq!(cursor(&app), (3, 13), "at the end of the next cell's text");
    // One undo step.
    ctrl(&mut app, 'z');
    assert_eq!(lines(&app)[3], "| apple | 3 |");
    put_cursor(&mut app, 3, 3);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Tab);
    assert_eq!(cursor(&app), (4, 6), "the next row's first cell");
    press(&mut app, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(cursor(&app), (3, 13), "Shift+Tab: the previous cell");
    // Enter: the same column, next row.
    key(&mut app, KeyCode::Enter);
    assert_eq!(cursor(&app), (4, 13));
    // Tab at the table's end adds a row.
    key(&mut app, KeyCode::Tab);
    assert_eq!(lines(&app)[5], "|       |     |");
    assert_eq!(cursor(&app), (5, 2));
    assert_eq!(lines(&app)[6..], ["", "after"]);
    // Enter on an empty last row leaves the table.
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[5..], ["", "after"]);
    assert_eq!(cursor(&app), (5, 0));
    // Outside a table Tab and Enter are the editor's.
    put_cursor(&mut app, 0, 5);
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[..2], ["Intro", ""]);
}

#[test]
fn a_table_starts_from_its_header_line() {
    let mut app = tables_app("tables-header", "| a | b |\n");
    put_cursor(&mut app, 0, 9);
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        lines(&app)[..3],
        ["| a   | b   |", "| --- | --- |", "|     |     |"]
    );
    assert_eq!(cursor(&app), (2, 2));
}

#[test]
fn table_rows_and_columns_change_by_command() {
    let mut app = tables_app(
        "tables-commands",
        "| Name | Qty |\n|---|---|\n| kiwi | 12 |\n| apple | 3 |\n| fig | 100 |\n\nend\n",
    );
    put_cursor(&mut app, 2, 8);
    run_palette(&mut app, "advanced tables sort rows ascending");
    assert_eq!(
        lines(&app)[2..5],
        ["| apple | 3   |", "| kiwi  | 12  |", "| fig   | 100 |"],
        "numbers sort as numbers"
    );
    run_palette(&mut app, "advanced tables sort rows descending");
    assert_eq!(lines(&app)[2], "| fig   | 100 |");
    run_palette(&mut app, "advanced tables insert column");
    assert_eq!(lines(&app)[0], "| Name  |     | Qty |");
    run_palette(&mut app, "advanced tables delete column");
    assert_eq!(lines(&app)[0], "| Name  | Qty |");
    put_cursor(&mut app, 3, 2);
    run_palette(&mut app, "advanced tables insert row");
    assert_eq!(lines(&app)[3], "|       |     |");
    run_palette(&mut app, "advanced tables delete row");
    assert_eq!(lines(&app)[3], "| kiwi  | 12  |");
    run_palette(&mut app, "advanced tables move row up");
    assert_eq!(lines(&app)[2], "| kiwi  | 12  |");
    assert_eq!(cursor(&app).0, 2, "the cursor goes with it");
    run_palette(&mut app, "advanced tables move column right");
    assert_eq!(lines(&app)[0], "| Qty | Name  |");
    run_palette(&mut app, "advanced tables align column center");
    assert_eq!(lines(&app)[1], "| --- | :---: |");
    run_palette(&mut app, "advanced tables transpose");
    assert_eq!(lines(&app)[0], "| Qty  | 12   | 100 | 3     |");
    run_palette(&mut app, "advanced tables export csv");
    let csv = fs::read_to_string(app.vault.root.join("T.csv")).unwrap();
    assert!(csv.starts_with("Qty,12,100,3\n"), "{csv}");
    assert!(app.message.contains("T.csv"), "{}", app.message);
    assert!(
        app.vault.files.iter().any(|f| f.ends_with("T.csv")),
        "in the vault (and so not a change from outside)"
    );
    // Outside a table the commands say so.
    let n = lines(&app).len();
    put_cursor(&mut app, n - 1, 0);
    run_palette(&mut app, "advanced tables sort rows ascending");
    assert!(app.message.contains("not in a table"), "{}", app.message);
}

#[test]
fn table_formulas_are_evaluated() {
    let mut app = tables_app(
        "tables-formulas",
        "| Item | Qty | Price | Total |\n|---|---|---|---|\n| a | 2 | 1.5 | |\n| b | 3 | 2 | |\n| Sum | | | |\n<!-- TBLFM: $4=($2*$3);%.2f -->\n<!-- TBLFM: @>$4=sum(@I..@-1)::@>$2=mean(@I..@-1) -->\n",
    );
    put_cursor(&mut app, 2, 2);
    run_palette(&mut app, "advanced tables evaluate formulas");
    let l = lines(&app);
    assert_eq!(l[2], "| a    | 2   | 1.5   | 3.00  |", "{}", app.message);
    assert_eq!(l[3], "| b    | 3   | 2     | 6.00  |");
    assert_eq!(l[4], "| Sum  | 2.5 |       | 9     |");
    // The cursor on a formula line works too.
    let mut app = tables_app(
        "tables-formula-line",
        "| a | b |\n|---|---|\n| 3 | |\n<!-- TBLFM: $2=($1*1.2) -->\n",
    );
    put_cursor(&mut app, 3, 5);
    run_palette(&mut app, "advanced tables evaluate formulas");
    assert_eq!(lines(&app)[2], "| 3   | 3.6 |", "{}", app.message);
    // A bad formula leaves the table as it was and says why.
    let mut app = tables_app(
        "tables-formula-error",
        "| a | b |\n|---|---|\n| 1 | |\n<!-- TBLFM: $2=nope(@1) -->\n",
    );
    put_cursor(&mut app, 2, 2);
    run_palette(&mut app, "advanced tables evaluate formulas");
    assert_eq!(lines(&app)[2], "| 1 | |");
    assert!(app.message.contains("nope"), "{}", app.message);
}

#[test]
fn table_keys_are_settings() {
    let dir = vault(
        "tables-settings",
        &[
            ("T.md", "| a | b |\n|---|---|\n| 1 | 2 |\n"),
            (
                STATE_FILE,
                "installed = [\"tables\"]\nenabled = [\"tables\"]\n",
            ),
            (
                ".blackglass/plugins/tables/settings.toml",
                "bind_tab = \"false\"\npad = \"false\"\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let path = app.vault.root.join("T.md");
    app.open(&path);
    put_cursor(&mut app, 2, 2);
    key(&mut app, KeyCode::Tab);
    assert_ne!(cursor(&app), (2, 6), "Tab is the editor's");
    assert_eq!(lines(&app)[1], "|---|---|", "not formatted");
    run_palette(&mut app, "advanced tables format table");
    assert_eq!(lines(&app)[1], "| --- | --- |", "one space, no padding");
}

/// Today plus `days`, as `YYYY-MM-DD`.
fn day(days: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

/// A workspace with the Tasks plugin enabled.
fn tasks_app(name: &str, files: &[(&str, &str)]) -> App {
    let mut all = files.to_vec();
    all.push((
        STATE_FILE,
        "installed = [\"tasks\"]\nenabled = [\"tasks\"]\n",
    ));
    App::new(Vault::open(&vault(name, &all)).unwrap())
}

#[test]
fn tasks_blocks_query_group_and_toggle_from_results() {
    let work = format!(
        "# Work\n## Soon\n- [ ] Ship report ⏫ 📅 {}\n- [ ] Plan trip 📅 {}\n- [x] Old thing ✅ {}\n## Later\n- [ ] Someday idea\n",
        day(0),
        day(30),
        day(-1)
    );
    let home = format!("- [ ] Pay rent #home 🔼 📅 {}\n- [.] Log entry\n", day(1));
    let mut app = tasks_app(
        "tasks-query",
        &[
            ("Work.md", &work),
            ("Home.md", &home),
            (
                "Query.md",
                "top\n```tasks\nnot done\ndue before in 7 days\ngroup by filename\nsort by priority\n```\nend\n",
            ),
        ],
    );
    let q = note(&app, "Query.md");
    app.open(&q);
    let rows = screen(&mut app, 110, 30);
    let (home_y, _) = find(&rows, "│ Home").expect("a group heading");
    let (rent_y, _) = find(&rows, "☐ Pay rent #home 🔼 📅").unwrap_or_else(|| panic!("{rows:#?}"));
    let (work_y, _) = find(&rows, "│ Work").unwrap();
    let (ship_y, ship_x) = find(&rows, "☐ Ship report ⏫").unwrap();
    assert!(
        home_y < rent_y && rent_y < work_y && work_y < ship_y,
        "{rows:#?}"
    );
    assert!(
        rows[ship_y].contains("(Work > Soon)"),
        "the backlink: {}",
        rows[ship_y]
    );
    assert!(find(&rows, "Plan trip").is_none(), "due later");
    assert!(find(&rows, "Old thing").is_none(), "done");
    assert!(find(&rows, "2 tasks").is_some());
    // A click checks it off, in its note, with today's done date.
    click(&mut app, ship_x as u16, ship_y as u16);
    let text = fs::read_to_string(note(&app, "Work.md")).unwrap();
    assert!(
        text.contains(&format!("- [x] Ship report ⏫ 📅 {} ✅ {}", day(0), day(0))),
        "{text}\n{}",
        app.message
    );
    // It stays a moment, checked (it's gone after: `a_checked_task_stays_in_its_query_for_a_moment`).
    let rows = screen(&mut app, 110, 30);
    assert!(find(&rows, "☑ Ship report").is_some(), "{rows:#?}");
}

#[test]
fn tasks_toggle_with_done_dates_recurrence_and_statuses() {
    let mut app = tasks_app(
        "tasks-toggle",
        &[
            (
                "T.md",
                "- [ ] Water 🔁 every week 📅 2026-10-01\n- [ ] Once 🏁 delete\n- [/] Busy\n- [~] Waiting for Bo\n- [.] Logged call\n",
            ),
            (
                "Q.md",
                "top\n```tasks\nstatus.type is NON_TASK\n```\n```tasks\nstatus.name is Waiting\n```\n",
            ),
            (
                ".blackglass/plugins/tasks/settings.toml",
                "statuses = \"~|Waiting|x|ON_HOLD\"\n",
            ),
        ],
    );
    let t = note(&app, "T.md");
    app.open(&t);
    run_palette(&mut app, "tasks toggle task done");
    assert_eq!(
        lines(&app)[..2],
        [
            "- [ ] Water 🔁 every week 📅 2026-10-08".to_string(),
            format!("- [x] Water 🔁 every week 📅 2026-10-01 ✅ {}", day(0)),
        ],
        "{}",
        app.message
    );
    assert_eq!(cursor(&app).0, 1, "on the task that was toggled");
    put_cursor(&mut app, 2, 0);
    run_palette(&mut app, "tasks toggle task done");
    assert_eq!(lines(&app)[2], "- [/] Busy", "🏁 delete removed Once");
    run_palette(&mut app, "tasks toggle task done");
    assert_eq!(lines(&app)[2], format!("- [x] Busy ✅ {}", day(0)));
    ctrl(&mut app, 's');
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "[.] Logged call").is_some(), "logs: {rows:#?}");
    assert!(
        find(&rows, "[~] Waiting for Bo").is_some(),
        "custom: {rows:#?}"
    );
    assert!(find(&rows, "Water").is_none());
}

#[test]
fn tasks_are_created_edited_and_postponed() {
    let mut app = tasks_app(
        "tasks-edit",
        &[("E.md", "Intro\n\n- [ ] Draft 📅 2026-10-01\n")],
    );
    let e = note(&app, "E.md");
    app.open(&e);
    put_cursor(&mut app, 1, 0);
    // Alt+T: every field at once, each with its default.
    alt(&mut app, KeyCode::Char('t'));
    let rows = screen(&mut app, 110, 40).join("\n");
    for field in [
        "Description",
        "Status",
        "Priority",
        "Due",
        "Recurs",
        "Created",
        "On completion",
    ] {
        assert!(rows.contains(field), "{field}: {rows}");
    }
    assert!(rows.contains("‹ None ›"), "no priority by default: {rows}");
    typing(&mut app, "Buy milk"); // the description
    key(&mut app, KeyCode::Down); // status
    key(&mut app, KeyCode::Down); // priority
    key(&mut app, KeyCode::Left); // None → Medium
    key(&mut app, KeyCode::Down); // estimate
    key(&mut app, KeyCode::Down); // due
    typing(&mut app, "tomorrow");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        lines(&app)[1],
        format!("- [ ] Buy milk 🔼 📅 {}", day(1)),
        "{}",
        app.message
    );
    // The cursor's task: its fields filled in.
    put_cursor(&mut app, 2, 4);
    alt(&mut app, KeyCode::Char('t'));
    let rows = screen(&mut app, 110, 40).join("\n");
    assert!(
        rows.contains("Draft") && rows.contains("2026-10-01"),
        "{rows}"
    );
    for _ in 0..4 {
        key(&mut app, KeyCode::Down); // estimate, due
    }
    ctrl(&mut app, 'u');
    typing(&mut app, "2026-10-05");
    for _ in 0..3 {
        key(&mut app, KeyCode::Down); // scheduled, start, recurs
    }
    typing(&mut app, "every month");
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[2], "- [ ] Draft 🔁 every month 📅 2026-10-05");
    // The rest: status, created, an ID, what happens on completion.
    alt(&mut app, KeyCode::Char('t'));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right); // status: in progress
    for _ in 0..7 {
        key(&mut app, KeyCode::Down); // created
    }
    typing(&mut app, "2026-09-30");
    for _ in 0..3 {
        key(&mut app, KeyCode::Down); // ID
    }
    typing(&mut app, "draft1");
    key(&mut app, KeyCode::Down); // blocked by
    key(&mut app, KeyCode::Down); // blocks
    key(&mut app, KeyCode::Down); // on completion
    key(&mut app, KeyCode::Right); // keep
    key(&mut app, KeyCode::Enter);
    let line = lines(&app)[2].clone();
    assert!(line.starts_with("- [/] Draft"), "{line}");
    for part in [
        "🔁 every month",
        "📅 2026-10-05",
        "➕ 2026-09-30",
        "🆔 draft1",
        "🏁 keep",
    ] {
        assert!(line.contains(part), "{part}: {line}");
    }
    // Esc changes nothing.
    alt(&mut app, KeyCode::Char('t'));
    typing(&mut app, " more");
    key(&mut app, KeyCode::Esc);
    assert_eq!(lines(&app)[2], line);
    // A bad date says so and changes nothing.
    alt(&mut app, KeyCode::Char('t'));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Left); // status back: to do
    for _ in 0..3 {
        key(&mut app, KeyCode::Down); // estimate, due
    }
    ctrl(&mut app, 'u');
    typing(&mut app, "someday");
    key(&mut app, KeyCode::Enter);
    assert!(app.message.contains("someday"), "{}", app.message);
    assert_eq!(lines(&app)[2], line);
    // Back to a plain task for the postponing below.
    let plain = "- [ ] Draft 🔁 every month 📅 2026-10-05";
    put_cursor(&mut app, 2, 0);
    press(&mut app, KeyCode::End, KeyModifiers::SHIFT);
    typing(&mut app, plain);
    assert_eq!(lines(&app)[2], plain);
    run_palette(&mut app, "tasks postpone task");
    typing(&mut app, "1 week");
    key(&mut app, KeyCode::Enter);
    let today = chrono::Local::now().date_naive();
    let base = chrono::NaiveDate::from_ymd_opt(2026, 10, 5)
        .unwrap()
        .max(today);
    let expected = (base + chrono::Duration::days(7)).format("%Y-%m-%d");
    assert_eq!(
        lines(&app)[2],
        format!("- [ ] Draft 🔁 every month 📅 {expected}")
    );
}

#[test]
fn tasks_dependencies_urgency_explain_and_global_settings() {
    let mut app = tasks_app(
        "tasks-rest",
        &[
            (
                "D.md",
                "- [ ] Design #task 🆔 d1\n- [ ] Build #task ⛔ d1\n- [x] Done dep #task 🆔 d0\n- [ ] Test #task ⛔ d0\n- [ ] Not a task here\n",
            ),
            (
                "Dv.md",
                &format!(
                    "- [ ] Dataview style #task [due:: {}] [priority:: high]\n",
                    day(0)
                ),
            ),
            (
                "Q.md",
                "top\n```tasks\nis blocked\n```\n```tasks\nis blocking\nexplain\n```\n```tasks\nfilename includes Dv\nshow urgency\n```\n",
            ),
            (
                ".blackglass/plugins/tasks/settings.toml",
                "global_filter = \"#task\"\nremove_global_filter = \"true\"\nglobal_query = \"not done\"\n",
            ),
        ],
    );
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 110, 40);
    let text = rows.join("\n");
    assert!(find(&rows, "☐ Build").is_some(), "blocked: {text}");
    assert!(find(&rows, "☐ Design").is_some(), "blocking");
    assert!(find(&rows, "☐ Test").is_none(), "its dependency is done");
    assert!(
        find(&rows, "Not a task here").is_none(),
        "no #task: not a task"
    );
    assert!(
        !text.contains("#task"),
        "the global filter's tag is hidden: {text}"
    );
    assert!(text.contains("Explanation of this Tasks query:"), "{text}");
    assert!(
        text.contains("not done"),
        "the global query is explained: {text}"
    );
    let (y, _) = find(&rows, "☐ Dataview style ⏫ 📅").unwrap_or_else(|| panic!("{text}"));
    assert!(rows[y].contains("⚡"), "urgency shown: {}", rows[y]);
}

const BOOKS: &[(&str, &str)] = &[
    (
        "Books/Dune.md",
        "---\ntype: book\nrating: 5\npages: 412\nstatus: read\n---\n# Dune\n",
    ),
    (
        "Books/Emma.md",
        "---\ntype: book\nrating: 3\npages: 474\nstatus: reading\n---\n# Emma\n",
    ),
    (
        "Books/Hobbit.md",
        "---\ntype: book\nrating: 4\npages: 310\nstatus: read\n---\n# Hobbit\n",
    ),
    ("Notes/Idea.md", "---\ntype: idea\n---\n"),
];

/// A workspace with the Bases plugin, the books and `extra` files.
fn bases_app(name: &str, extra: &[(&str, &str)]) -> App {
    let mut all = BOOKS.to_vec();
    all.extend_from_slice(extra);
    all.push((
        STATE_FILE,
        "installed = [\"bases\"]\nenabled = [\"bases\"]\n",
    ));
    App::new(Vault::open(&vault(name, &all)).unwrap())
}

#[test]
fn bases_blocks_filter_compute_and_show_a_table() {
    let mut app = bases_app(
        "bases-table",
        &[(
            "B.md",
            "top\n```base\nfilters:\n  and:\n    - type == \"book\"\n    - file.inFolder(\"Books\")\nformulas:\n  per_star: (pages / rating).toFixed(1)\nproperties:\n  rating:\n    displayName: Stars\nviews:\n  - type: table\n    name: Best\n    order:\n      - file.name\n      - rating\n      - formula.per_star\n    sort:\n      - property: rating\n        direction: DESC\n    limit: 2\n```\nmiddle\n```base\nfilters: rating >\n```\nend\n",
        )],
    );
    let b = note(&app, "B.md");
    app.open(&b);
    let rows = screen(&mut app, 100, 30);
    let text = rows.join("\n");
    assert!(text.contains("Best   2 results"), "{text}");
    let (h, _) = find(&rows, "name   │ Stars │ per_star").unwrap_or_else(|| panic!("{text}"));
    let (dune, x) = find(&rows, "Dune   │ 5     │ 82.4").unwrap_or_else(|| panic!("{text}"));
    let (hobbit, _) = find(&rows, "Hobbit │ 4     │ 77.5").unwrap_or_else(|| panic!("{text}"));
    assert!(h < dune && dune < hobbit, "sorted by rating, descending");
    assert!(!text.contains("Emma"), "the limit");
    assert!(!text.contains("Idea"), "filtered out");
    assert!(
        text.contains("Bases: filter"),
        "an error, not a crash: {text}"
    );
    // A click opens the row's menu; its first choice opens the note.
    click(&mut app, x as u16, dune as u16);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Books/Dune.md")));
}

#[test]
fn bases_group_summarize_and_show_lists_cards_and_boards() {
    let views = [
        "views:\n  - type: table\n    order: [file.name, rating]\n    groupBy:\n      property: status\n    summaries:\n      rating: Average",
        "views:\n  - type: list\n    order: [file.name, rating, status]",
        "views:\n  - type: cards\n    cardSize: 14\n    order: [file.name, rating]",
        "views:\n  - type: kanban\n    groupBy: status\n    order: [file.name]",
    ];
    let mut text = String::from("top\n");
    for v in views {
        text.push_str(&format!("```base\nfilters: type == \"book\"\n{v}\n```\n\n"));
    }
    let mut app = bases_app("bases-views", &[("V.md", &text)]);
    let v = note(&app, "V.md");
    app.open(&v);
    let rows = screen(&mut app, 110, 70);
    let all = rows.join("\n");
    // Table: a heading per status, its summary under the column.
    assert!(find(&rows, "│ read (2)").is_some(), "{all}");
    assert!(find(&rows, "│ reading (1)").is_some(), "{all}");
    assert!(all.contains("Average 4.5"), "{all}");
    assert!(all.contains("Average 3"), "{all}");
    assert!(
        rows.iter().any(|r| r.ends_with("│ Average 4")),
        "the total: {all}"
    );
    // List: the title, then the values.
    assert!(find(&rows, "• Dune, 5, read").is_some(), "{all}");
    // Cards: boxes with their properties.
    assert!(find(&rows, "│ rating: 5").is_some(), "{all}");
    assert!(
        find(&rows, "╭────────────────╮ ╭").is_some(),
        "side by side: {all}"
    );
    // Kanban: a column per status.
    let (y, _) = find(&rows, "read (2)").unwrap();
    let board = rows
        .iter()
        .find(|r| r.contains("read (2)") && r.contains("reading (1)"));
    assert!(board.is_some(), "columns side by side: {all} {y}");
    assert!(find(&rows, "▪ Hobbit").is_some(), "{all}");
}

#[test]
fn base_files_open_rendered_and_bases_commands() {
    let mut app = bases_app(
        "bases-files",
        &[
            (
                "Library.base",
                "filters: type == \"book\"\nviews:\n  - type: table\n    name: All\n    order: [file.name, rating]\n  - type: list\n    name: Titles\n    order: [file.name]\n",
            ),
            ("N.md", "Some text\n"),
        ],
    );
    let library = note(&app, "Library.base");
    app.open(&library);
    let view = app.view().unwrap();
    assert_eq!(view.title(), "Library.base");
    let rows = screen(&mut app, 100, 30);
    let all = rows.join("\n");
    assert!(all.contains("All · Titles   3 results"), "{all}");
    assert!(find(&rows, "Dune   │ 5").is_some(), "{all}");
    // Switch the view.
    run_palette(&mut app, "bases switch view");
    typing(&mut app, "Titles");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    assert!(
        find(&rows, "• Dune").is_some(),
        "{}\n{}",
        rows.join("\n"),
        app.message
    );
    // Export it.
    run_palette(&mut app, "bases export view as csv");
    let csv = fs::read_to_string(app.vault.root.join("Library.csv")).unwrap();
    assert_eq!(csv, "name\nDune\nEmma\nHobbit\n");
    // Its YAML, as text.
    run_palette(&mut app, "bases edit base source");
    let view = app.view().unwrap();
    assert_eq!(view.path.as_deref(), Some(library.as_path()));
    assert!(view.editor.lines[0].starts_with("filters:"));
    // A new base, opened rendered.
    app.focus = Focus::Sidebar;
    run_palette(&mut app, "bases create new base");
    assert!(
        app.vault.root.join("Untitled.base").is_file(),
        "{}",
        app.message
    );
    assert_eq!(app.view().unwrap().title(), "Untitled.base");
    // A base block inserted in a note.
    let n = note(&app, "N.md");
    app.open(&n);
    put_cursor(&mut app, 0, 9);
    run_palette(&mut app, "bases insert new base");
    let text = app.view().unwrap().editor.to_text();
    assert!(text.contains("```base\n"), "{text}");
}

#[test]
fn base_rows_properties_are_edited() {
    let mut app = bases_app(
        "bases-edit",
        &[(
            "B.md",
            "top\n```base\nfilters: type == \"book\"\nviews:\n  - type: kanban\n    groupBy: status\n    order: [file.name, status, rating]\n```\n",
        )],
    );
    let b = note(&app, "B.md");
    app.open(&b);
    run_palette(&mut app, "bases edit a property");
    typing(&mut app, "Emma");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "status");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "read");
    key(&mut app, KeyCode::Enter);
    let emma = fs::read_to_string(note(&app, "Books/Emma.md")).unwrap();
    assert!(emma.contains("status: read\n"), "{emma}\n{}", app.message);
    assert!(emma.contains("rating: 3\n"), "the rest kept: {emma}");
    let rows = screen(&mut app, 100, 30);
    assert!(
        rows.iter().any(|r| r.contains("read (3)")),
        "moved to the other column: {}",
        rows.join("\n")
    );
}

#[test]
fn blocks_of_other_notes_are_embedded_and_linked() {
    let dir = vault(
        "block-embeds",
        &[
            (
                "Deep/Source.md",
                "# Source\nintro\n\nthe quoted idea\nin two lines ^idea\n\n- task list ^list\n  - sub\n",
            ),
            (
                "A.md",
                "top\n![[Source#^idea]]\n![[Source#^list]]\nsee [[Source#^idea]]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let a = app.vault.root.join("A.md");
    app.open(&a);
    let rows = screen(&mut app, 100, 20);
    let all = rows.join("\n");
    assert!(all.contains("Source › ^idea"), "{all}");
    assert!(all.contains("│ the quoted idea"), "{all}");
    assert!(all.contains("│ in two lines"), "the id isn't shown: {all}");
    assert!(!all.contains("intro"), "only the block: {all}");
    assert!(all.contains("◦ sub"), "the item's sub-items: {all}");
    // The link goes to the block's line.
    put_cursor(&mut app, 3, 7);
    press(&mut app, KeyCode::Enter, KeyModifiers::CONTROL);
    assert_eq!(
        active_path(&app),
        Some(app.vault.root.join("Deep/Source.md")),
        "{}",
        app.message
    );
    assert_eq!(cursor(&app).0, 4);
}

#[test]
fn query_blocks_show_a_vault_search() {
    let dir = vault(
        "query-embeds",
        &[
            ("Books/Dune.md", "# Dune\nspice must flow\nmore spice\n"),
            ("Notes/Cooking.md", "salt and spice\n"),
            ("Other.md", "nothing here\n"),
            ("Q.md", "top\n```query\nspice -path:Notes\n```\nend\n"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let q = app.vault.root.join("Q.md");
    app.open(&q);
    let rows = screen(&mut app, 100, 20);
    let all = rows.join("\n");
    assert!(all.contains("spice -path:Notes   1 note"), "{all}");
    let (y, x) = find(&rows, "│ Dune").unwrap_or_else(|| panic!("{all}"));
    assert!(all.contains("│   spice must flow"), "{all}");
    assert!(all.contains("│   more spice"), "{all}");
    assert!(!all.contains("Cooking"), "{all}");
    // A matching line opens the note at it.
    let (ly, lx) = find(&rows, "more spice").unwrap();
    click(&mut app, lx as u16, ly as u16);
    assert_eq!(
        active_path(&app),
        Some(app.vault.root.join("Books/Dune.md"))
    );
    assert_eq!(cursor(&app).0, 2);
    app.open(&q);
    click(&mut app, x as u16 + 2, y as u16);
    assert_eq!(
        active_path(&app),
        Some(app.vault.root.join("Books/Dune.md"))
    );
    // A saved change is searched at once (the vault isn't a stale copy).
    let other = app.vault.root.join("Other.md");
    app.open(&other);
    put_cursor(&mut app, 0, 0);
    typing(&mut app, "spice ");
    ctrl(&mut app, 's');
    app.open(&q);
    let rows = screen(&mut app, 100, 20);
    assert!(rows.join("\n").contains("   2 notes"), "{rows:#?}");
}

#[test]
fn a_recent_note_is_linked_at_the_cursor() {
    let dir = vault(
        "recent-link",
        &[
            ("A.md", "start "),
            ("Other.md", "x"),
            (
                STATE_FILE,
                "installed = [\"recent-files\"]\nenabled = [\"recent-files\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let (a, other) = (note(&app, "A.md"), note(&app, "Other.md"));
    app.open(&other);
    app.open(&a);
    put_cursor(&mut app, 0, 6);
    run_palette(&mut app, "recent files open");
    key(&mut app, KeyCode::Down); // Other, below A
    typing(&mut app, "l");
    assert_eq!(lines(&app)[0], "start [[Other]]", "{}", app.message);
}

/// Runs a Git palette command, again while the last one still runs.
fn run_git(app: &mut App, command: &str) {
    for _ in 0..200 {
        run_palette(app, command);
        if !app.message.contains("still working") {
            return;
        }
        app.tick();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{command} never started: {}", app.message);
}

/// The last commit's subject in `dir`.
fn last_subject(dir: &Path) -> String {
    git(dir, &["log", "-1", "--pretty=%s"]).trim().to_string()
}

#[test]
fn git_commits_staged_amended_with_messages_and_discards() {
    if missing("git") {
        return;
    }
    let dir = git_vault("git-commits", &[("A.md", "a\n"), ("B.md", "b\n")]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    fs::write(dir.join("A.md"), "a2\n").unwrap();
    fs::write(dir.join("B.md"), "b2\n").unwrap();
    let a = note(&app, "A.md");
    app.open(&a);
    run_git(&mut app, "git stage current file");
    wait_for(&mut app, "staged A.md");
    run_git(&mut app, "git commit staged with specific message");
    typing(&mut app, "only a");
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "committed");
    assert_eq!(last_subject(&dir), "only a");
    let status = git(&dir, &["status", "--porcelain"]);
    assert!(status.contains("B.md"), "B isn't committed: {status}");
    // "Commit": nothing staged, so everything.
    run_git(&mut app, "git commit staged or all");
    wait_for(&mut app, "committed");
    assert!(git(&dir, &["status", "--porcelain"]).trim().is_empty());
    let commits = git(&dir, &["rev-list", "--count", "HEAD"]);
    // Amend: a staged change joins the last commit.
    fs::write(dir.join("A.md"), "a3\n").unwrap();
    run_git(&mut app, "git stage current file");
    wait_for(&mut app, "staged A.md");
    run_git(&mut app, "git amend staged");
    wait_for(&mut app, "amended");
    assert_eq!(git(&dir, &["rev-list", "--count", "HEAD"]), commits);
    assert!(git(&dir, &["status", "--porcelain"]).trim().is_empty());
    // Unstage, then discard everything (asked first).
    fs::write(dir.join("A.md"), "a4\n").unwrap();
    run_git(&mut app, "git stage current file");
    wait_for(&mut app, "staged A.md");
    run_git(&mut app, "git unstage current file");
    wait_for(&mut app, "unstaged A.md");
    assert!(
        git(&dir, &["diff", "--cached", "--name-only"])
            .trim()
            .is_empty()
    );
    run_git(&mut app, "git discard all changes");
    key(&mut app, KeyCode::Enter); // "Discard them"
    wait_for(&mut app, "discarded");
    assert_eq!(fs::read_to_string(dir.join("A.md")).unwrap(), "a3\n");
    // A message script, and the files in the body.
    git_settings(
        &dir,
        "commit_message_script = \"echo scripted\"\nlist_changed_files = \"true\"\n",
    );
    app.rescan();
    fs::write(dir.join("B.md"), "b3\n").unwrap();
    run_git(&mut app, "git commit all changes");
    wait_for(&mut app, "committed");
    assert_eq!(last_subject(&dir), "scripted");
    let body = git(&dir, &["log", "-1", "--pretty=%b"]);
    assert!(body.contains("B.md"), "{body}");
    // Commit-and-sync, then quit.
    fs::write(dir.join("B.md"), "b4\n").unwrap();
    run_git(&mut app, "git commit-and-sync and then close");
    for _ in 0..200 {
        app.tick();
        if app.quit_requested {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(app.quit_requested, "{}", app.message);
    assert!(git(&dir, &["status", "--porcelain"]).trim().is_empty());
}

/// A vault repository with a bare remote it has pushed to, and a second
/// clone of it (`other`) to make changes "elsewhere".
fn git_with_remote(name: &str) -> (PathBuf, PathBuf) {
    let dir = git_vault(name, &[("Note.md", "base\n")]);
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-remote"));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    git(&base, &["init", "--bare", "remote.git"]);
    let remote = base.join("remote.git");
    git(&dir, &["remote", "add", "origin", remote.to_str().unwrap()]);
    git(&dir, &["push", "-u", "origin", "HEAD"]);
    git(&base, &["clone", "remote.git", "other"]);
    let other = base.join("other");
    git(&other, &["config", "user.email", "o@example.com"]);
    git(&other, &["config", "user.name", "Other"]);
    (dir, other)
}

fn git_settings(dir: &Path, text: &str) {
    fs::create_dir_all(dir.join(".blackglass/plugins/git")).unwrap();
    fs::write(dir.join(".blackglass/plugins/git/settings.toml"), text).unwrap();
}

#[test]
fn git_fetches_resets_merges_theirs_and_runs_on_timers() {
    if missing("git") {
        return;
    }
    let (dir, other) = git_with_remote("git-sync-options");
    let mut app = App::new(Vault::open(&dir).unwrap());
    fs::write(other.join("Note.md"), "theirs\n").unwrap();
    git(&other, &["commit", "-am", "theirs"]);
    git(&other, &["push"]);
    run_git(&mut app, "git fetch");
    wait_for(&mut app, "fetched");
    let theirs = git(&other, &["rev-parse", "HEAD"]);
    assert_eq!(git(&dir, &["rev-parse", "@{u}"]), theirs);
    // A conflicting change of ours, merged with "theirs".
    fs::write(dir.join("Note.md"), "ours\n").unwrap();
    git(&dir, &["commit", "-am", "ours"]);
    git_settings(&dir, "merge_strategy = \"theirs\"\n");
    app.rescan();
    run_git(&mut app, "git pull");
    wait_for(&mut app, "pulled");
    assert_eq!(fs::read_to_string(dir.join("Note.md")).unwrap(), "theirs\n");
    // Reset: the remote's state, whatever is here.
    fs::write(other.join("Note.md"), "reset to this\n").unwrap();
    git(&other, &["commit", "-am", "again"]);
    git(&other, &["push"]);
    git_settings(&dir, "sync_method = \"reset\"\n");
    app.rescan();
    run_git(&mut app, "git pull");
    wait_for(&mut app, "pulled");
    assert_eq!(
        git(&dir, &["rev-parse", "HEAD"]),
        git(&other, &["rev-parse", "HEAD"])
    );
    // Commit a moment after the last change; push on a timer of its own.
    git_settings(
        &dir,
        "interval = \"0.01\"\nauto_after_change = \"true\"\npush_interval = \"0.01\"\npush = \"false\"\n",
    );
    app.rescan();
    let n = note(&app, "Note.md");
    app.open(&n);
    typing(&mut app, "x");
    ctrl(&mut app, 's');
    wait_for(&mut app, "committed");
    wait_for(&mut app, "pushed");
    let remote_head = git(&other, &["ls-remote", "origin", "HEAD"]);
    assert!(
        remote_head.starts_with(git(&dir, &["rev-parse", "HEAD"]).trim()),
        "{remote_head}"
    );
    // Nothing to commit is said only if wanted.
    git_settings(&dir, "interval = \"0.01\"\nquiet = \"true\"\n");
    app.rescan();
    app.message.clear();
    for _ in 0..40 {
        app.tick();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !app.message.contains("nothing to commit"),
        "{}",
        app.message
    );
}

#[test]
fn git_repository_chores_status_and_links() {
    if missing("git") {
        return;
    }
    let (dir, other) = git_with_remote("git-repo-chores");
    let mut app = App::new(Vault::open(&dir).unwrap());
    let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = std::rc::Rc::clone(&opened);
    app.opener = Box::new(move |p| {
        seen.borrow_mut().push(p.to_string_lossy().into_owned());
        Ok(())
    });
    // A remote branch to switch to.
    git(&other, &["switch", "-c", "feature"]);
    fs::write(other.join("F.md"), "f\n").unwrap();
    git(&other, &["add", "-A"]);
    git(&other, &["commit", "-m", "feature"]);
    git(&other, &["push", "-u", "origin", "feature"]);
    run_git(&mut app, "git fetch");
    wait_for(&mut app, "fetched");
    run_git(&mut app, "git switch to remote branch");
    wait_prompt(&mut app);
    typing(&mut app, "feature");
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "on feature");
    assert_eq!(
        git(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "feature"
    );
    // The status bar says when the last commit was.
    wait_status(&mut app, "last commit");
    // The Git tab pushes and pulls.
    fs::write(dir.join("Note.md"), "changed\n").unwrap();
    run_git(&mut app, "git commit all changes");
    wait_for(&mut app, "committed");
    run_git(&mut app, "git open the git tab");
    typing(&mut app, "P");
    wait_for(&mut app, "pushed");
    typing(&mut app, "p");
    wait_for(&mut app, "pulled");
    // On GitHub: the remote's web address.
    git(
        &dir,
        &[
            "remote",
            "set-url",
            "origin",
            "git@github.com:someone/notes.git",
        ],
    );
    let n = note(&app, "Note.md");
    app.open(&n);
    run_git(&mut app, "git open file on github");
    wait_for(&mut app, "github.com");
    assert_eq!(
        opened.borrow().last().map(String::as_str),
        Some("https://github.com/someone/notes/blob/feature/Note.md")
    );
    // Deleting the repository asks twice.
    run_git(&mut app, "git delete repository");
    key(&mut app, KeyCode::Enter); // yes
    key(&mut app, KeyCode::Enter); // yes, really
    wait_for(&mut app, "deleted the repository");
    assert!(!dir.join(".git").exists());
    // A new one, with .blackglass/ left out.
    run_git(&mut app, "git initialize a new repo");
    typing(&mut app, "leave out");
    key(&mut app, KeyCode::Enter);
    wait_for(&mut app, "repository");
    let ignore = fs::read_to_string(dir.join(".gitignore")).unwrap();
    assert!(ignore.contains(".blackglass/"), "{ignore}");
}

#[test]
fn git_runs_in_a_base_path() {
    if missing("git") {
        return;
    }
    let dir = vault(
        "git-base-path",
        &[
            ("repo/In.md", "in\n"),
            ("Out.md", "out\n"),
            (STATE_FILE, "installed = [\"git\"]\nenabled = [\"git\"]\n"),
            (
                ".blackglass/plugins/git/settings.toml",
                "base_path = \"repo\"\n",
            ),
        ],
    );
    let repo = dir.join("repo");
    git(&repo, &["init"]);
    git(&repo, &["config", "user.email", "t@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_git(&mut app, "git commit all changes");
    wait_for(&mut app, "committed");
    assert_eq!(git(&repo, &["ls-files"]).trim(), "In.md");
}

#[test]
fn a_theme_colors_the_editor_and_plugin_results() {
    use ratatui::style::Color;
    let dir = vault(
        "theme-editor",
        &[
            (
                "A.md",
                "see [[B]] here\n\n```tasks\ngroup by filename\n```\n",
            ),
            ("B.md", "- [ ] a task\n"),
            (
                STATE_FILE,
                "installed = [\"tasks\"]\nenabled = [\"tasks\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let config = Path::new(env!("CARGO_TARGET_TMPDIR")).join("config/theme-editor");
    let _ = fs::remove_dir_all(&config);
    app.config_dir = Some(config);
    let a = note(&app, "A.md");
    app.open(&a);
    put_cursor(&mut app, 1, 0);
    let cell = |app: &mut App, text: &str| {
        let rows = screen(app, 100, 20);
        let (y, x) = find(&rows, text).unwrap_or_else(|| panic!("{text}: {rows:#?}"));
        let mut t = Terminal::new(TestBackend::new(100, 20)).unwrap();
        t.draw(|f| ui::draw(f, app)).unwrap();
        t.backend().buffer()[(x as u16, y as u16)].clone()
    };
    let link = cell(&mut app, "B here").fg;
    run_palette(&mut app, "choose theme");
    typing(&mut app, "paper");
    key(&mut app, KeyCode::Enter);
    let paper_link = cell(&mut app, "B here").fg;
    assert_ne!(
        paper_link, link,
        "the editor's link color follows the theme"
    );
    let empty = cell(&mut app, "here");
    assert_eq!(
        bg_at(&mut app, 100, 20, 90, 10),
        empty.bg,
        "the editor's background: {:?}",
        empty.bg
    );
    assert_eq!(empty.bg, Color::Rgb(0xfa, 0xf8, 0xf4), "Paper's page");
    // A plugin's heading takes the theme's accent (after the block's frame).
    let rows = screen(&mut app, 100, 20);
    let (y, x) = find(&rows, "│ B").unwrap_or_else(|| panic!("{rows:#?}"));
    let mut t = Terminal::new(TestBackend::new(100, 20)).unwrap();
    t.draw(|f| ui::draw(f, &mut app)).unwrap();
    let heading = &t.backend().buffer()[(x as u16 + 2, y as u16)];
    assert_eq!(heading.fg, Color::Rgb(0x6a, 0x4f, 0xd8));
}

#[test]
fn base_files_are_embedded_whole_or_one_view() {
    let mut app = bases_app(
        "bases-embeds",
        &[
            (
                "Library.base",
                "filters: type == \"book\"\nviews:\n  - type: table\n    name: All\n    order: [file.name, rating]\n  - type: list\n    name: Titles\n    order: [file.name]\n",
            ),
            (
                "E.md",
                "top\n![[Library.base#Titles]]\n\n![[Library.base]]\nend\n",
            ),
        ],
    );
    let e = note(&app, "E.md");
    app.open(&e);
    let rows = screen(&mut app, 100, 40);
    let all = rows.join("\n");
    assert!(all.contains("Library.base › Titles"), "{all}");
    assert!(find(&rows, "• Dune").is_some(), "the one view: {all}");
    assert!(
        find(&rows, "Dune   │ 5").is_some(),
        "the whole base, its first view: {all}"
    );
    assert!(all.contains("All · Titles"), "{all}");
}

#[test]
fn base_rows_open_or_edit_their_cells() {
    let mut app = bases_app(
        "bases-cells",
        &[
            (
                "Books/Extra.md",
                "---\ntype: book\nstatus: reading\ndone: false\n---\n",
            ),
            (
                "B.md",
                "top\n```base\nfilters: type == \"book\"\nviews:\n  - type: table\n    order: [file.name, status, done]\n```\n",
            ),
        ],
    );
    let b = note(&app, "B.md");
    app.open(&b);
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "Extra").unwrap_or_else(|| panic!("{rows:#?}"));
    click(&mut app, x as u16, y as u16);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "Open Extra").is_some(), "a menu: {rows:#?}");
    // A checkbox switches at once.
    typing(&mut app, "done");
    key(&mut app, KeyCode::Enter);
    let extra = fs::read_to_string(note(&app, "Books/Extra.md")).unwrap();
    assert!(extra.contains("done: true"), "{extra}\n{}", app.message);
    // Other values are asked, the current one offered.
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "Extra").unwrap();
    click(&mut app, x as u16, y as u16);
    typing(&mut app, "status");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "reading").is_some(), "the value now: {rows:#?}");
    ctrl(&mut app, 'u');
    typing(&mut app, "finished");
    key(&mut app, KeyCode::Enter);
    let extra = fs::read_to_string(note(&app, "Books/Extra.md")).unwrap();
    assert!(extra.contains("status: finished"), "{extra}");
    // "Open" opens the note.
    let rows = screen(&mut app, 100, 30);
    let (y, x) = find(&rows, "Dune").unwrap();
    click(&mut app, x as u16, y as u16);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Books/Dune.md")));
}

/// The text of the base block in the open note (between its fences).
fn base_block(app: &App) -> String {
    let lines = lines(app);
    let start = lines.iter().position(|l| l == "```base").unwrap();
    let end = start + 1 + lines[start + 1..].iter().position(|l| l == "```").unwrap();
    lines[start + 1..end].join("\n")
}

#[test]
fn a_base_view_is_changed_by_commands() {
    let mut app = bases_app(
        "bases-view-edit",
        &[(
            "B.md",
            "top\n```base\nfilters: type == \"book\"\nviews:\n  - type: table\n    name: Books\n    order: [file.name, rating]\n```\n",
        )],
    );
    let b = note(&app, "B.md");
    app.open(&b);
    run_palette(&mut app, "bases sort view");
    typing(&mut app, "rating");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "desc");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    let (dune, _) = find(&rows, "Dune").unwrap();
    let (emma, _) = find(&rows, "Emma").unwrap();
    assert!(dune < emma, "rating descending: {rows:#?}");
    assert!(base_block(&app).contains("DESC"), "{}", base_block(&app));
    run_palette(&mut app, "bases add formula");
    typing(&mut app, "double");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "rating *");
    key(&mut app, KeyCode::Enter);
    assert!(
        app.message.contains("rating *"),
        "checked first: {}",
        app.message
    );
    assert!(!base_block(&app).contains("double"));
    run_palette(&mut app, "bases add formula");
    typing(&mut app, "double");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "rating * 2");
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "bases choose view properties");
    ctrl(&mut app, 'u');
    typing(&mut app, "file.name, formula.double");
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "bases add view filter");
    typing(&mut app, "rating > 3");
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "bases set view limit");
    typing(&mut app, "1");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    let all = rows.join("\n");
    assert!(
        find(&rows, "Dune │ 10").is_some(),
        "{all}\n{}",
        base_block(&app)
    );
    assert!(!all.contains("Hobbit"), "limit 1: {all}");
    run_palette(&mut app, "bases group view");
    typing(&mut app, "status");
    key(&mut app, KeyCode::Enter);
    assert!(base_block(&app).contains("groupBy"), "{}", base_block(&app));
    run_palette(&mut app, "bases add view");
    typing(&mut app, "list");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "Titles");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    assert!(rows.join("\n").contains("Books · Titles"), "{rows:#?}");
    assert!(
        find(&rows, "• Dune").is_some(),
        "the new view is shown: {rows:#?}"
    );
}

#[test]
fn a_base_view_is_searched_copied_and_adds_notes() {
    let mut app = bases_app(
        "bases-search-copy",
        &[(
            "B.md",
            "top\n```base\nfilters:\n  and:\n    - type == \"book\"\n    - file.inFolder(\"Books\")\nviews:\n  - type: table\n    order: [file.name, rating]\n```\n",
        )],
    );
    let copied = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let seen = std::rc::Rc::clone(&copied);
    app.copier = Box::new(move |text| {
        *seen.borrow_mut() = text.to_string();
        Ok(())
    });
    let b = note(&app, "B.md");
    app.open(&b);
    run_palette(&mut app, "bases search view");
    typing(&mut app, "hob");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    let all = rows.join("\n");
    assert!(all.contains("Hobbit") && !all.contains("Dune"), "{all}");
    assert!(all.contains("search: hob"), "{all}");
    run_palette(&mut app, "bases copy view");
    assert_eq!(
        *copied.borrow(),
        "| name | rating |\n| --- | --- |\n| Hobbit | 4 |\n",
        "{}",
        app.message
    );
    run_palette(&mut app, "bases search view");
    key(&mut app, KeyCode::Enter); // nothing typed: every row again
    let rows = screen(&mut app, 100, 30);
    assert!(rows.join("\n").contains("Dune"));
    // A new note from the view: in its folder, with what its filters ask.
    run_palette(&mut app, "bases new note from view");
    typing(&mut app, "Ulysses");
    key(&mut app, KeyCode::Enter);
    let text = fs::read_to_string(note(&app, "Books/Ulysses.md")).unwrap();
    assert!(text.contains("type: book"), "{text}");
    assert_eq!(active_path(&app), Some(note(&app, "Books/Ulysses.md")));
}

#[test]
fn tasks_show_trees_columns_and_leave_out_sub_items() {
    let mut app = tasks_app(
        "tasks-tree",
        &[
            (
                "P.md",
                "- [ ] Plan trip\n  - [ ] Book flights\n  - [x] Ask for leave\n- [/] Write talk\n  - notes, not a task\n",
            ),
            (
                "Q.md",
                "top\n```tasks\npath includes P\nshow tree\nhide backlink\nhide edit button\nhide postpone button\n```\n```tasks\npath includes P\nexclude sub-items\nhide backlink\nhide edit button\nhide postpone button\n```\n```tasks\npath includes P\ncolumns by status.name\nhide backlink\n```\n",
            ),
        ],
    );
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 110, 40);
    let all = rows.join("\n");
    // The tree: sub-tasks under their task, not again on their own.
    assert!(all.contains("│ ☐ Plan trip\n"), "{all}");
    assert!(all.contains("│   ☐ Book flights"), "{all}");
    assert!(all.contains("│   ☑ Ask for leave"), "{all}");
    assert_eq!(
        all.matches("Book flights").count(),
        2,
        "tree + columns: {all}"
    );
    // Sub-items left out.
    let second = &all[all.find("4 tasks").unwrap()..];
    assert!(second.contains("2 tasks"), "only the top ones: {all}");
    // Columns: one per status, side by side.
    let board = rows
        .iter()
        .find(|r| r.contains("Todo (2)") && r.contains("In Progress (1)"));
    assert!(board.is_some(), "{all}");
}

#[test]
fn task_fields_are_suggested_while_typing() {
    let mut app = tasks_app("tasks-suggest", &[("S.md", "- [ ] Pay rent\nplain du\n")]);
    let s = note(&app, "S.md");
    app.open(&s);
    put_cursor(&mut app, 0, 14);
    typing(&mut app, " du");
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "📅 due date").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[0], "- [ ] Pay rent 📅 ");
    // Then dates.
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "today").is_some(), "{rows:#?}");
    typing(&mut app, "tom");
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[0], format!("- [ ] Pay rent 📅 {}", day(1)));
    // A rule after 🔁.
    typing(&mut app, " recur");
    key(&mut app, KeyCode::Tab);
    typing(&mut app, "every w");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        lines(&app)[0],
        format!("- [ ] Pay rent 📅 {} 🔁 every week", day(1))
    );
    // Not on other lines; Esc closes the list.
    put_cursor(&mut app, 1, 8);
    typing(&mut app, "e");
    assert!(app.view().unwrap().editor.lines[1] == "plain due");
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "due date").is_none(), "{rows:#?}");
    let end = lines(&app)[0].chars().count();
    put_cursor(&mut app, 0, end);
    typing(&mut app, " hi");
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        lines(&app)[1],
        "- [ ] ",
        "Enter went to the editor: a new task"
    );
}

#[test]
fn table_controls_are_a_sidebar_tab() {
    let mut app = tables_app(
        "tables-controls",
        "| Name | Qty |\n|---|---|\n| kiwi | 12 |\n| apple | 3 |\n",
    );
    put_cursor(&mut app, 2, 8);
    run_palette(&mut app, "advanced tables open table controls");
    let rows = screen(&mut app, 100, 30);
    assert!(rows[0].contains("Table"), "a tab: {:?}", rows[0]);
    let (y, _) = find(&rows, "Sort rows ascending").unwrap_or_else(|| panic!("{rows:#?}"));
    // Down to it, then Enter: on the table at the editor's cursor.
    let (first, _) = find(&rows, "Format table").unwrap();
    for _ in first..y {
        key(&mut app, KeyCode::Down);
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[2], "| apple | 3   |", "{}", app.message);
    assert_eq!(app.focus, Focus::Sidebar, "the controls keep the focus");
}

#[test]
fn git_tree_view_squash_and_side_by_side_diffs() {
    if missing("git") {
        return;
    }
    let (dir, other) = git_with_remote("git-tree-squash");
    git_settings(
        &dir,
        "tree_view = \"true\"\nsquash_before_push = \"true\"\ndiff_style = \"split\"\n",
    );
    fs::create_dir_all(dir.join("Sub")).unwrap();
    fs::write(dir.join("A.md"), "a\n").unwrap();
    fs::write(dir.join("Sub/B.md"), "b\n").unwrap();
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_git(&mut app, "git open the git tab");
    wait_status(&mut app, "changed");
    app.refresh_plugin_tabs();
    let rows = screen(&mut app, 100, 30);
    let side = sidebar(&rows).join("\n");
    assert!(side.contains("▾ Sub/"), "a folder row: {side}");
    let (folder, _) = find(&rows, "▾ Sub/").unwrap();
    let (b, _) = find(&rows, "  B").unwrap_or_else(|| panic!("{side}"));
    assert!(b > folder, "under its folder: {side}");
    // `s` on B stages B (not the folder's neighbour).
    // The sidebar's cursor starts on its first row (screen row 2).
    for _ in 2..b {
        key(&mut app, KeyCode::Down);
    }
    typing(&mut app, "s");
    wait_for(&mut app, "staged Sub/B.md");
    // Two commits waiting: pushed as one.
    run_git(&mut app, "git commit all changes");
    wait_for(&mut app, "committed");
    fs::write(dir.join("A.md"), "a again\n").unwrap();
    run_git(&mut app, "git commit all changes");
    wait_for(&mut app, "committed");
    let before = git(&other, &["ls-remote", "origin", "HEAD"]);
    run_git(&mut app, "git push");
    wait_for(&mut app, "pushed");
    let base = before.split_whitespace().next().unwrap().to_string();
    let count = git(&dir, &["rev-list", "--count", &format!("{base}..HEAD")]);
    assert_eq!(count.trim(), "1", "squashed into one");
    // A diff side by side.
    fs::write(dir.join("A.md"), "a changed\n").unwrap();
    let a = note(&app, "A.md");
    app.open(&a);
    run_git(&mut app, "git show the diff of this note");
    wait_title(&mut app, "Diff");
    let text = app.view().unwrap().editor.to_text();
    assert!(text.contains("| Before | After |"), "{text}");
    assert!(text.contains("| a again | a changed |"), "{text}");
}

fn wheel(app: &mut App, down: bool, x: u16, y: u16) {
    app.handle_mouse(MouseEvent {
        kind: if down {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        },
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    });
}

#[test]
fn the_wheel_scrolls_through_a_long_result_in_the_live_preview() {
    let tasks: String = (0..40).map(|i| format!("- [ ] task {i:02}\n")).collect();
    let mut app = tasks_app(
        "tasks-wheel",
        &[
            ("T.md", &tasks),
            (
                "Q.md",
                "top\n```tasks\nnot done\nsort by description\nhide backlink\n```\nafter\n",
            ),
        ],
    );
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "task 00").is_some(), "{rows:#?}");
    assert!(find(&rows, "task 30").is_none());
    for _ in 0..8 {
        wheel(&mut app, true, 60, 10);
    }
    let rows = screen(&mut app, 100, 20);
    let all = rows.join("\n");
    assert!(
        find(&rows, "task 30").is_some(),
        "further down the result: {all}"
    );
    assert!(!all.contains("sort by description"), "not the query: {all}");
    assert_eq!(cursor(&app).0, 0, "the cursor stayed");
    for _ in 0..20 {
        wheel(&mut app, true, 60, 10);
    }
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "after").is_some(), "{rows:#?}");
    for _ in 0..30 {
        wheel(&mut app, false, 60, 10);
    }
    let rows = screen(&mut app, 100, 20);
    assert!(find(&rows, "│  top").is_some(), "back up: {rows:#?}");
}

#[test]
fn a_tasks_query_in_a_callout_with_and_without_parentheses() {
    let mut app = tasks_app(
        "tasks-callout",
        &[
            (
                "2026-07-02.md",
                "## Tasks\n- [ ] This is a test open task for [[Project]] 📅 2026-07-02 ⏫\n- [ ] No priority 📅 2026-07-03\n- [ ] Later ⏫ 📅 2026-09-01\n",
            ),
            (
                "Q.md",
                "top\n> [!danger]+ Overdue\n> ```tasks\n> not done\n> due before 2026-08-04 AND priority is above none\n> hide recurrence rule\n> sort by due date\n> ```\nend\n",
            ),
        ],
    );
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 110, 20);
    let all = rows.join("\n");
    assert!(all.contains("☐ This is a test open task"), "{all}");
    assert!(!all.contains("No priority"), "AND: {all}");
    assert!(!all.contains("Later"), "{all}");
    assert!(all.contains("1 task"), "{all}");
    assert!(!all.contains("Tasks query:"), "no error: {all}");
}

#[test]
fn a_project_notes_tasks_and_dataview_queries() {
    let dir = vault(
        "project-queries",
        &[
            (
                STATE_FILE,
                "installed = [\"tasks\", \"dataview\"]\nenabled = [\"tasks\", \"dataview\"]\n",
            ),
            (
                "Roadmap.md",
                "top\n## Tasks\n```tasks\nstatus.type is not non_task\ndescription includes AI CTO Roadmap\ngroup by path\n```\n### Completed\n```tasks\nstatus.type is done\ndescription includes AI CTO Roadmap\ngroup by path\n```\n## Logs\n```tasks\nstatus.type is non_task\ndescription includes AI CTO Roadmap\ngroup by path\n```\n## Sub\n```dataview\nTABLE status AS \"Status\", summary AS \"Summary\"\nWHERE contains(up, this.file.link)\nAND contains(tags, \"project\")\n```\n## Meetings\n```dataview\nTABLE date, participants, summary\nFROM \"\"\nWHERE contains(up, this.file.link)\nAND contains(tags, \"meeting\")\nSORT date DESC\n```\n## Related\n```dataview\nTABLE summary AS \"Summary\", tags AS \"Type\"\nWHERE contains(related, this.file.link) OR contains(this.file.link, related)\n```\nend\n",
            ),
            (
                "Log.md",
                "- [ ] Draft plan for [[AI CTO Roadmap]]\n- [x] Finished AI CTO Roadmap item\n- [.] Logged call about AI CTO Roadmap\n- [ ] unrelated\n",
            ),
            (
                "Sub1.md",
                "---\nup: \"[[Roadmap]]\"\ntags: [project]\nstatus: active\nsummary: First sub\n---\n",
            ),
            (
                "Sub2.md",
                "---\nup:\n  - \"[[Roadmap]]\"\n  - \"[[Other]]\"\ntags:\n  - project\nstatus: done\nsummary: Second sub\n---\n",
            ),
            (
                "Meet1.md",
                "---\nup: \"[[Roadmap]]\"\ntags: [meeting]\ndate: 2026-09-01\nparticipants: [Ann, Bob]\nsummary: Kickoff\n---\n",
            ),
            (
                "Meet2.md",
                "---\nup: \"[[Roadmap]]\"\ntags: [meeting]\ndate: 2026-09-20\nparticipants: [Ann]\nsummary: Review\n---\n",
            ),
            (
                "Rel.md",
                "---\nrelated: \"[[Roadmap]]\"\ntags: [context]\nsummary: Related one\n---\n",
            ),
            (
                "Loose.md",
                "---\ntags: [project]\nsummary: not under it\n---\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let r = note(&app, "Roadmap.md");
    app.open(&r);
    let rows = screen(&mut app, 120, 70);
    let all = rows.join("\n");
    let section = |from: &str, to: &str| {
        let a = all.find(from).unwrap_or_else(|| panic!("{from}: {all}"));
        let b = all[a..].find(to).map_or(all.len(), |b| a + b);
        all[a..b].to_string()
    };
    let open = section("▌ Tasks", "Completed");
    assert!(
        open.contains("Draft plan") && open.contains("Finished"),
        "{open}"
    );
    assert!(
        !open.contains("Logged call") && !open.contains("unrelated"),
        "{open}"
    );
    let done = section("Completed", "▌ Logs");
    assert!(
        done.contains("Finished") && !done.contains("Draft"),
        "{done}"
    );
    let logs = section("▌ Logs", "▌ Sub");
    assert!(
        logs.contains("[.] Logged call") && logs.contains("1 task"),
        "{logs}"
    );
    let sub = section("▌ Sub", "▌ Meetings");
    assert!(
        sub.contains("First sub") && sub.contains("Second sub"),
        "a YAML list of links too: {sub}"
    );
    assert!(!sub.contains("not under it"), "{sub}");
    let meetings = section("▌ Meetings", "▌ Related");
    let (review, kickoff) = (
        meetings.find("Review").unwrap(),
        meetings.find("Kickoff").unwrap(),
    );
    assert!(review < kickoff, "SORT date DESC: {meetings}");
    assert!(meetings.contains("Ann, Bob"), "{meetings}");
    let related = section("▌ Related", "end");
    assert!(
        related.contains("Related one") && related.contains("context"),
        "{related}"
    );
}

#[test]
fn a_dataview_list_of_projects_outside_some_folders_newest_first() {
    let query = "```dataview\nlist\nfrom \"\"\nwhere \ncontains(tags, \"project\")\nand !contains(file.folder, \"Examples\") \nand !contains(file.folder, \"Templates\")\nsort file.mtime desc\nlimit 15\n```";
    let mut files: Vec<(String, String)> = vec![
        (
            STATE_FILE.into(),
            "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n".into(),
        ),
        ("Q.md".into(), format!("top\n{query}\nend\n")),
        (
            "Examples/Demo.md".into(),
            "---\ntags: [project]\n---\n".into(),
        ),
        ("Examples/Sub/Deep.md".into(), "#project".into()),
        (
            "Templates/Project.md".into(),
            "---\ntags: project\n---\n".into(),
        ),
        (
            "Work/Not tagged.md".into(),
            "---\ntags: [other]\n---\n".into(),
        ),
        ("Work/Inline.md".into(), "a #project in the text".into()),
        (
            "Work/Block list.md".into(),
            "---\ntags:\n  - area\n  - project\n---\n".into(),
        ),
    ];
    for i in 0..15 {
        files.push((
            format!("Projects/P{i:02}.md"),
            "---\ntags: [project]\n---\n".into(),
        ));
    }
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let dir = vault("dataview-projects", &refs);
    // When each note changed: P00 oldest … P14 newest; the two in Work
    // newest of all.
    let at = |rel: &str, minutes: u64| {
        let time = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(1_800_000_000 + minutes * 60);
        let file = fs::File::options().write(true).open(dir.join(rel)).unwrap();
        file.set_modified(time).unwrap();
    };
    for i in 0..15 {
        at(&format!("Projects/P{i:02}.md"), i);
    }
    at("Work/Inline.md", 100);
    at("Work/Block list.md", 101);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 100, 40);
    let items: Vec<String> = rows
        .iter()
        .filter_map(|r| r.split("│ • ").nth(1).map(|i| i.trim().to_string()))
        .collect();
    let all = rows.join("\n");
    // Newest first; 15 of the 16 projects outside Examples and Templates.
    // `tags` is the frontmatter's (as in Dataview): a `#project` only in
    // the text is in `file.tags`, not `tags`.
    assert_eq!(items.len(), 15, "limit 15: {all}");
    assert_eq!(items[0], "Block list", "a YAML list of tags: {all}");
    assert_eq!(items[1], "P14", "{all}");
    assert_eq!(items[14], "P01", "the oldest left out: {all}");
    for gone in ["Demo", "Deep", "Project", "Not tagged", "Inline", "P00"] {
        assert!(!items.iter().any(|i| i == gone), "{gone}: {all}");
    }
    // With `file.tags`, the tag in the text counts.
    let q = query
        .replace("contains(tags,", "contains(file.tags,")
        .replace("\"project\")", "\"#project\")");
    fs::write(dir.join("Q.md"), format!("top\n{q}\nend\n")).unwrap();
    app.rescan();
    app.close(app.active);
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 100, 40);
    assert!(
        rows.iter().any(|r| r.contains("│ • Inline")),
        "{}",
        rows.join("\n")
    );
}

#[test]
fn a_dataview_list_from_a_folder_newest_first_and_saves_keep_the_creation_time() {
    let query = "```dataview\nlist\nfrom \"Projects\"\nsort file.mtime desc\nlimit 15\n```";
    let mut files: Vec<(String, String)> = vec![
        (
            STATE_FILE.into(),
            "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n".into(),
        ),
        ("Q.md".into(), format!("top\n{query}\nend\n")),
        ("Projects/Sub/Nested.md".into(), "nested".into()),
        ("Projects Old/Elsewhere.md".into(), "not in Projects".into()),
        ("Other/Outside.md".into(), "outside".into()),
    ];
    for i in 0..16 {
        files.push((format!("Projects/P{i:02}.md"), format!("project {i}")));
    }
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let dir = vault("dataview-from-folder", &refs);
    let at = |rel: &str, minutes: u64| {
        let time = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(1_700_000_000 + minutes * 60);
        let file = fs::File::options().write(true).open(dir.join(rel)).unwrap();
        file.set_modified(time).unwrap();
    };
    for i in 0..16 {
        at(&format!("Projects/P{i:02}.md"), 10 + i);
    }
    at("Projects/Sub/Nested.md", 5);
    at("Other/Outside.md", 999);
    at("Projects Old/Elsewhere.md", 998);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let items = |app: &mut App| -> Vec<String> {
        let rows = screen(app, 100, 40);
        rows.iter()
            .filter_map(|r| r.split("│ • ").nth(1).map(|i| i.trim().to_string()))
            .collect()
    };
    let q = note(&app, "Q.md");
    app.open(&q);
    let list = items(&mut app);
    // 17 notes in Projects (its sub-folder too); the newest 15.
    assert_eq!(list.len(), 15, "{list:?}");
    assert_eq!(list[0], "P15");
    assert_eq!(list[14], "P01", "{list:?}");
    for gone in ["P00", "Nested", "Outside", "Elsewhere"] {
        assert!(!list.iter().any(|i| i == gone), "{gone}: {list:?}");
    }
    // Saving a note makes it the newest, and keeps when it was created.
    let p00 = note(&app, "Projects/P00.md");
    // A creation time of its own (Wine's is the modification time, which
    // the test set: nothing to keep there).
    let meta = fs::metadata(&p00).unwrap();
    let created = meta
        .created()
        .ok()
        .filter(|c| meta.modified().ok() != Some(*c));
    app.open(&p00);
    typing(&mut app, "edited ");
    ctrl(&mut app, 's');
    app.open(&q);
    let list = items(&mut app);
    assert_eq!(list[0], "P00", "{list:?}");
    if let Some(created) = created {
        assert_eq!(
            fs::metadata(&p00).unwrap().created().unwrap(),
            created,
            "file.ctime kept"
        );
    }
}

/// The callouts of `example_vault`'s task dashboard (and one more).
const TASK_CALLOUTS: &str = ">[!danger] Tasks Within Two Weeks\n>```tasks\n>not done\n>due AFTER yesterday\n>due BEFORE in two weeks\n>sort by due\n>limit 10\n>```\n\n>[!warning] Next Month\n>```tasks\n>not done\n>due AFTER two weeks\n>due next month\n>no scheduled date\n>sort by due\n>limit 10\n>```\n\n>[!example] [[Tasks Backlog]]\n>```tasks\n>limit 10\nnot done\n";

#[test]
fn task_dashboards_in_callouts() {
    let today = chrono::Local::now().date_naive();
    let next_month = today
        .checked_add_months(chrono::Months::new(1))
        .unwrap()
        .with_day(15)
        .unwrap();
    use chrono::Datelike;
    let tasks = format!(
        "- [ ] Overdue 📅 {}\n- [ ] Soon 📅 {}\n- [ ] Ten days 📅 {}\n- [ ] Next month 📅 {next_month}\n- [ ] Planned ⏳ {} 📅 {next_month}\n- [x] Done soon 📅 {}\n",
        day(-3),
        day(2),
        day(10),
        day(1),
        day(1),
    );
    let mut app = tasks_app(
        "task-dashboards",
        &[
            ("T.md", &tasks),
            ("Q.md", &format!("top\n## Tasks\n{TASK_CALLOUTS}\nend\n")),
        ],
    );
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 110, 40);
    let all = rows.join("\n");
    let section = |from: &str, to: &str| {
        let a = all.find(from).unwrap_or_else(|| panic!("{from}: {all}"));
        let b = all[a..].find(to).map_or(all.len(), |b| a + b);
        all[a..b].to_string()
    };
    assert!(!all.contains("Tasks query:"), "no errors: {all}");
    // `in two weeks`, `AFTER` / `BEFORE` in capitals.
    let soon = section("Within Two Weeks", "Next Month");
    assert!(soon.contains("Soon") && soon.contains("Ten days"), "{soon}");
    assert!(
        !soon.contains("Overdue") && !soon.contains("Done soon"),
        "{soon}"
    );
    assert!(soon.contains("2 tasks"), "{soon}");
    // `two weeks` from today, `next month`, without a scheduled date.
    let later = section("Next Month\n", "Tasks Backlog");
    assert!(later.contains("☐ Next month"), "{later}");
    assert!(!later.contains("Planned"), "it's scheduled: {later}");
    // A block the callout ends: only `limit 10` is in it (the `not done`
    // line without `>` is after the callout), so every task, done too.
    let backlog = section("Tasks Backlog", "end");
    assert!(
        backlog.contains("☑ Done soon") && backlog.contains("6 tasks"),
        "{backlog}"
    );
    assert!(
        all.contains("│  not done") || all.contains("  not done"),
        "after the callout: {all}"
    );
}

/// Orphans: notes nothing links to and that link to nothing.
const ORPHANS: &str = "```dataview \nlist from \"\" where length(file.inlinks) =0 and length(file.outlinks) = 0\nsort file.mtime desc\nlimit 3\n```";

#[test]
fn a_dataview_list_of_orphan_notes() {
    let dir = vault(
        "dataview-orphans",
        &[
            (
                STATE_FILE,
                "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
            ),
            ("Q.md", &format!("top\n{ORPHANS}\nend\n")),
            ("Links out.md", "see [[Linked to]]"),
            ("Linked to.md", "something"),
            ("Missing link.md", "see [[Nowhere]]"),
            ("Alone 1.md", "a"),
            ("Alone 2.md", "b"),
            ("Alone 3.md", "c"),
            ("Alone 4.md", "d"),
        ],
    );
    for (i, rel) in [
        "Alone 4.md",
        "Alone 1.md",
        "Alone 2.md",
        "Alone 3.md",
        "Q.md",
    ]
    .iter()
    .enumerate()
    {
        let time = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(1_700_000_000 + i as u64 * 60);
        let file = fs::File::options().write(true).open(dir.join(rel)).unwrap();
        file.set_modified(time).unwrap();
    }
    let mut app = App::new(Vault::open(&dir).unwrap());
    let q = note(&app, "Q.md");
    app.open(&q);
    let rows = screen(&mut app, 100, 30);
    let items: Vec<String> = rows
        .iter()
        .filter_map(|r| r.split("│ • ").nth(1).map(|i| i.trim().to_string()))
        .collect();
    // Q links to nothing and nothing links to it: an orphan too, the
    // newest. The links (a missing note's too) leave the others out.
    assert_eq!(items, ["Q", "Alone 3", "Alone 2"], "{}", rows.join("\n"));
}

/// A vault whose notes get IDs (`notes.toml`: `ids`, `id_format`).
fn ids_app(name: &str, ids: &str, format: &str, files: &[(&str, &str)]) -> App {
    let mut all = files.to_vec();
    let settings = format!("ids = \"{ids}\"\nid_format = \"{format}\"\n");
    all.push((".blackglass/notes.toml", &settings));
    App::new(Vault::open(&vault(name, &all)).unwrap())
}

fn now_as(format: &str) -> String {
    chrono::Local::now().format(format).to_string()
}

#[test]
fn new_notes_get_an_id_in_their_name() {
    let mut app = ids_app("ids-in-name", "in the name", "YYYY-MM", &[("A.md", "a")]);
    let month = now_as("%Y-%m");
    ctrl(&mut app, 'n');
    typing(&mut app, "Meeting");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        active_path(&app),
        Some(note(&app, &format!("{month} Meeting.md")))
    );
    // The same month again: the next free ID.
    ctrl(&mut app, 'n');
    typing(&mut app, "Plan");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        active_path(&app),
        Some(note(&app, &format!("{month}-2 Plan.md")))
    );
}

#[test]
fn new_notes_get_an_id_in_their_properties_or_as_their_name() {
    let mut app = ids_app(
        "ids-properties",
        "in the properties",
        "YYYYMMDDHHmm",
        &[("A.md", "a")],
    );
    let stamp = now_as("%Y%m%d%H%M");
    ctrl(&mut app, 'n');
    typing(&mut app, "Meeting");
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Meeting.md")));
    let text = app.view().unwrap().editor.to_text();
    assert!(
        text.starts_with(&format!("---\nid: {stamp}\n---")),
        "{text}"
    );
    // As the name: the title in a property; a link to a new note follows it.
    let mut app = ids_app(
        "ids-as-name",
        "as the name",
        "YYYYMMDDHHmm",
        &[("A.md", "see [[Idea]]\n")],
    );
    let a = note(&app, "A.md");
    app.open(&a);
    put_cursor(&mut app, 0, 7);
    press(&mut app, KeyCode::Enter, KeyModifiers::CONTROL);
    key(&mut app, KeyCode::Enter); // create it
    assert_eq!(active_path(&app), Some(note(&app, &format!("{stamp}.md"))));
    let text = app.view().unwrap().editor.to_text();
    assert!(text.contains("title: Idea"), "{text}");
    let source = fs::read_to_string(&a).unwrap_or_default();
    let shown = app
        .tabs
        .iter()
        .find(|v| v.path.as_deref() == Some(a.as_path()))
        .map(|v| v.editor.to_text())
        .unwrap();
    assert!(
        shown.contains(&format!("[[{stamp}|Idea]]")),
        "the link follows: {shown} / {source}"
    );
}

#[test]
fn note_ids_are_set_in_the_settings_window() {
    let (mut app, _) = app_with_config("ids-settings");
    alt(&mut app, KeyCode::Char(','));
    typing(&mut app, "unique id");
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Notes").is_some(), "the Notes page: {rows:#?}");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24);
    assert!(find(&rows, "Note IDs").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Right); // off → in the name
    let saved = fs::read_to_string(app.vault.root.join(".blackglass/notes.toml")).unwrap();
    assert!(saved.contains("ids = \"in the name\""), "{saved}");
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    ctrl(&mut app, 'n');
    typing(&mut app, "Thought");
    key(&mut app, KeyCode::Enter);
    let name = active_path(&app).unwrap();
    let stamp = now_as("%Y%m%d%H%M");
    assert_eq!(
        name.file_name().unwrap().to_string_lossy(),
        format!("{stamp} Thought.md")
    );
}

#[test]
fn a_folder_template_keeps_its_cursor_with_an_id_in_the_properties() {
    let mut app = folder_templates_vault("ids-folder-template");
    fs::write(
        app.vault.root.join(".blackglass/notes.toml"),
        "ids = \"in the properties\"\nid_format = \"YYYYMMDDHHmm\"\n",
    )
    .unwrap();
    let stamp = now_as("%Y%m%d%H%M");
    ctrl(&mut app, 'n');
    typing(&mut app, "Books/Sci-fi/Hyperion");
    key(&mut app, KeyCode::Enter);
    let view = app.view().unwrap();
    // The minute may have turned while the note was made.
    let later = now_as("%Y%m%d%H%M");
    let stamp = if view.editor.lines[2].ends_with(&stamp) {
        stamp
    } else {
        later
    };
    let id = format!("id: {stamp}");
    assert_eq!(
        view.editor.lines,
        [
            "---",
            "status: to read",
            &id,
            "---",
            "# Hyperion",
            "author:: "
        ],
        "the template, with the ID among its properties: {}",
        app.message
    );
    assert_eq!((view.editor.row, view.editor.col), (5, 9), "at its cursor");
}

/// Ticks (the main loop when idle) until `done` holds (a background job
/// finished).
fn ticks_until(app: &mut App, done: impl Fn(&App) -> bool) {
    for _ in 0..500 {
        if done(app) {
            return;
        }
        app.tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("never done: {}", app.message);
}

/// A vault with Templater's settings `settings` and these notes; the
/// quotes database is a file in it.
fn templater_app(name: &str, settings: &str, files: &[(&str, &str)]) -> App {
    let mut all = files.to_vec();
    all.push((
        "quotes.json",
        r#"[{"quote":"Stay hungry.","author":"Someone"}]"#,
    ));
    if !all.iter().any(|(p, _)| *p == ".blackglass/plugins.toml") {
        all.push((
            ".blackglass/plugins.toml",
            "installed = [\"templater\"]\nenabled = [\"templater\"]\n",
        ));
    }
    let dir = vault(name, &all);
    let quotes = format!(
        "[web]\nquotes_url = \"file://{}\"\n",
        dir.join("quotes.json").display()
    );
    let file = dir.join(".blackglass/plugins/templater/settings.toml");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, format!("{settings}\n{quotes}")).unwrap();
    App::new(Vault::open(&dir).unwrap())
}

#[test]
fn a_daily_template_gets_a_quote_from_the_web() {
    if missing("curl") {
        return;
    }
    // The line from a daily template, as Templater writes it.
    let daily = "# <% tp.file.title %>\n\n<% tp.web.daily_quote() %>\n";
    let mut app = templater_app(
        "templater-quote",
        "",
        &[("Templates/Daily.md", daily), ("Note.md", "")],
    );
    app.open(&note(&app, "Note.md"));
    // Alt+E: "Insert template".
    alt(&mut app, KeyCode::Char('e'));
    typing(&mut app, "Daily");
    key(&mut app, KeyCode::Enter);
    assert!(
        app.message.contains("getting"),
        "fetched in the background: {}",
        app.message
    );
    ticks_until(&mut app, |app| app.view().unwrap().editor.lines.len() > 1);
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["# Note", "", "> [!quote] Stay hungry.", "> — Someone"]
    );
    // Periodic Notes' daily note, from the same template.
    let journal = "[daily]\nfolder = \"Journal\"\ntemplate = \"Templates/Daily\"\n";
    let mut app = templater_app(
        "periodic-quote",
        "",
        &[
            ("Templates/Daily.md", daily),
            (".blackglass/plugins/periodic-notes/settings.toml", journal),
            (
                ".blackglass/plugins.toml",
                "installed = [\"templater\", \"periodic-notes\"]\nenabled = [\"templater\", \"periodic-notes\"]\n",
            ),
        ],
    );
    run_palette(&mut app, "open daily note");
    let path = note(&app, &format!("Journal/{}.md", today("%Y-%m-%d")));
    ticks_until(&mut app, |_| path.exists());
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        format!(
            "# {}\n\n> [!quote] Stay hungry.\n> — Someone\n",
            today("%Y-%m-%d")
        )
    );
}

#[test]
fn templates_rename_and_move_their_note_with_every_link() {
    let mut app = templater_app(
        "templater-rename",
        "",
        &[
            (
                "Templates/Rename.md",
                "<%* await tp.file.rename('Renamed') %>Done",
            ),
            (
                "Templates/Archive.md",
                "<%* await tp.file.move('Archive/' + tp.file.title) %>",
            ),
            ("Note.md", ""),
            ("Other.md", "see [[Note]] and [[Note#Part|part]]"),
        ],
    );
    app.open(&note(&app, "Note.md"));
    run_palette(&mut app, "templater insert template");
    typing(&mut app, "Rename");
    key(&mut app, KeyCode::Enter);
    let renamed = note(&app, "Renamed.md");
    assert_eq!(active_path(&app), Some(renamed.clone()), "{}", app.message);
    assert!(!app.vault.root.join("Note.md").exists());
    assert_eq!(app.view().unwrap().editor.lines, ["Done"]);
    assert_eq!(
        fs::read_to_string(note(&app, "Other.md")).unwrap(),
        "see [[Renamed]] and [[Renamed#Part|part]]",
        "every link follows"
    );
    ctrl(&mut app, 's');
    run_palette(&mut app, "templater insert template");
    typing(&mut app, "Archive");
    key(&mut app, KeyCode::Enter);
    let moved = app.vault.root.join("Archive/Renamed.md");
    assert_eq!(active_path(&app), Some(moved.clone()), "{}", app.message);
    assert_eq!(fs::read_to_string(moved).unwrap(), "Done\n");
}

#[test]
fn templates_create_notes_append_at_the_cursor_and_ask_more_kinds_of_questions() {
    let mut app = templater_app(
        "templater-create-new",
        "",
        &[
            ("Templates/Child.md", "# <% tp.file.title %>"),
            (
                "Templates/Make.md",
                "<%* await tp.file.create_new(tp.file.find_tfile('Child'), 'Kid', false, 'Kids'); tp.file.cursor_append('!') %>made",
            ),
            (
                "Templates/Ask.md",
                "<%* const a = await tp.system.multi_suggester(['A', 'B', 'C'], ['a', 'b', 'c'], false, 'Which?'); const l = await tp.system.prompt('Lines', '', false, true) %><% a.join('+') %>|<% l %>",
            ),
            ("Note.md", ""),
        ],
    );
    app.open(&note(&app, "Note.md"));
    run_palette(&mut app, "templater insert template");
    typing(&mut app, "Make");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        fs::read_to_string(app.vault.root.join("Kids/Kid.md")).unwrap(),
        "# Kid",
        "its own template ran: {}",
        app.message
    );
    assert_eq!(active_path(&app), Some(note(&app, "Note.md")), "not opened");
    assert_eq!(app.view().unwrap().editor.lines, ["made!"]);
    // A choice of many (Tab marks) and several lines (Alt+Enter).
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "templater insert template");
    typing(&mut app, "Ask");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "[ ] A").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Tab);
    let rows = screen(&mut app, 100, 30);
    assert!(find(&rows, "[x] C").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "one");
    alt(&mut app, KeyCode::Enter);
    typing(&mut app, "two");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["made!", "a+c|one", "two"],
        "{}",
        app.message
    );
}

#[test]
fn user_scripts_and_system_commands_are_tp_user() {
    // The argument is an environment variable: as the system's shell reads it.
    let who = if cfg!(windows) { "%who%" } else { "$who" };
    let settings = format!(
        "[scripts]\nscripts_folder = \"Scripts\"\nsystem_commands = true\n\n[user_functions]\ngreet = \"echo hi {who}\"\n"
    );
    let mut app = templater_app(
        "templater-user",
        &settings,
        &[
            (
                "Scripts/shout.js",
                "module.exports = (s) => s.toUpperCase() + '!';",
            ),
            (
                "Templates/User.md",
                "<% tp.user.shout('hey') %> <% tp.user.greet({who: 'Ann'}) %>",
            ),
            ("Note.md", ""),
        ],
    );
    app.open(&note(&app, "Note.md"));
    run_palette(&mut app, "templater insert template");
    typing(&mut app, "User");
    key(&mut app, KeyCode::Enter);
    ticks_until(&mut app, |app| app.view().unwrap().editor.lines != [""]);
    assert_eq!(app.view().unwrap().editor.lines, ["HEY! hi Ann"]);
    // Off, no command runs.
    let mut app = templater_app(
        "templater-user-off",
        "[user_functions]\ngreet = \"echo hi\"\n",
        &[
            ("Templates/User.md", "<% tp.user.greet() %>"),
            ("Note.md", ""),
        ],
    );
    app.open(&note(&app, "Note.md"));
    run_palette(&mut app, "templater insert template");
    typing(&mut app, "User");
    key(&mut app, KeyCode::Enter);
    assert!(app.message.contains("System commands"), "{}", app.message);
    assert_eq!(app.view().unwrap().editor.lines, [""]);
}

#[test]
fn cursors_are_jumped_to_one_after_the_other() {
    let mut app = templater_app(
        "templater-cursors",
        "[commands]\nauto_jump_to_cursor = false\ntemplate_hotkeys = \"Form\"\n",
        &[
            (
                "Templates/Form.md",
                "a<% tp.file.cursor(1) %>b<% tp.file.cursor(2) %>c",
            ),
            ("Note.md", ""),
        ],
    );
    app.open(&note(&app, "Note.md"));
    // The template's own command (a hotkey can be given to it).
    run_palette(&mut app, "templater insert form");
    let view = app.view().unwrap();
    assert_eq!(
        view.editor.lines,
        ["a<% tp.file.cursor(1) %>b<% tp.file.cursor(2) %>c"],
        "{}",
        app.message
    );
    run_palette(&mut app, "jump to next cursor location");
    let view = app.view().unwrap();
    assert_eq!(view.editor.lines, ["ab<% tp.file.cursor(2) %>c"]);
    assert_eq!((view.editor.row, view.editor.col), (0, 1));
    run_palette(&mut app, "jump to next cursor location");
    let view = app.view().unwrap();
    assert_eq!(view.editor.lines, ["abc"]);
    assert_eq!((view.editor.row, view.editor.col), (0, 2));
    run_palette(&mut app, "jump to next cursor location");
    assert!(
        app.message.contains("no cursor location left"),
        "{}",
        app.message
    );
}

#[test]
fn new_notes_get_regex_templates_and_have_their_commands_run_and_startup_templates_run() {
    let mut app = templater_app(
        "templater-triggers",
        "[file_templates]\n\"^Journal/.*\" = \"Day\"\n\n[new_notes]\nignore_folders = \"Inbox\"\n\n[commands]\nstartup_templates = \"Boot\"\n",
        &[
            ("Templates/Day.md", "# Day <% tp.file.title %>"),
            (
                "Templates/Boot.md",
                "<%* await tp.file.create_new('booted', 'Booted', false, '') %>",
            ),
            ("Note.md", "Title: <% tp.file.title %>"),
        ],
    );
    // A startup template runs at the first tick.
    ticks_until(&mut app, |app| app.vault.root.join("Booted.md").exists());
    assert_eq!(
        fs::read_to_string(app.vault.root.join("Booted.md")).unwrap(),
        "booted"
    );
    // A new note with commands in it (an extract): they run.
    app.open(&note(&app, "Note.md"));
    let view = app.tabs.get_mut(app.active).unwrap();
    view.editor.anchor = Some((0, 0));
    view.editor.col = view.editor.lines[0].chars().count();
    ctrl(&mut app, 'x');
    typing(&mut app, "Pulled");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        active_path(&app),
        Some(note(&app, "Pulled.md")),
        "{}",
        app.message
    );
    assert_eq!(app.view().unwrap().editor.lines, ["Title: Pulled"]);
    // The quick switcher makes notes from the top of the vault.
    ctrl(&mut app, 'o');
    typing(&mut app, "Journal/Mon");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["# Day Mon"],
        "{}",
        app.message
    );
    ctrl(&mut app, 'o');
    typing(&mut app, "Inbox/Loose");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.view().unwrap().editor.lines, [""], "excluded");
}

#[test]
fn files_and_folders_chosen_in_the_explorer_are_renamed_and_moved_with_their_links() {
    let files = [
        ("Images/photo.png", "png"),
        ("Books/Dune.md", "# Dune"),
        ("Books/cover.jpg", "jpg"),
        (
            "Note.md",
            "![[photo.png]] ![pic](Images/photo.png) [[Books/Dune]] ![[Books/cover.jpg]]",
        ),
        ("Archive/Old.md", ""),
    ];
    let dir = vault("explorer-moves", &files);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let text = |app: &App| fs::read_to_string(app.vault.root.join("Note.md")).unwrap();
    // F2 on an image: its extension stays.
    app.focus = Focus::Sidebar;
    app.sidebar.selected = Some(dir.join("Images/photo.png"));
    key(&mut app, KeyCode::F(2));
    ctrl(&mut app, 'u');
    typing(&mut app, "cover photo");
    key(&mut app, KeyCode::Enter);
    assert!(
        dir.join("Images/cover photo.png").is_file(),
        "{}",
        app.message
    );
    assert_eq!(
        text(&app),
        "![[cover photo.png]] ![pic](Images/cover%20photo.png) [[Books/Dune]] ![[Books/cover.jpg]]"
    );
    // Move it (chosen in the explorer) to the top of the vault.
    app.focus = Focus::Sidebar;
    app.sidebar.selected = Some(dir.join("Images/cover photo.png"));
    run_palette(&mut app, "move note to a folder");
    typing(&mut app, "/");
    key(&mut app, KeyCode::Enter);
    assert!(dir.join("cover photo.png").is_file(), "{}", app.message);
    assert!(text(&app).starts_with("![[cover photo.png]] ![pic](cover%20photo.png) "));
    // A folder moves into another, with the links to everything in it.
    app.focus = Focus::Sidebar;
    app.sidebar.selected = Some(dir.join("Books"));
    run_palette(&mut app, "move note to a folder");
    typing(&mut app, "Archive");
    key(&mut app, KeyCode::Enter);
    assert!(
        dir.join("Archive/Books/Dune.md").is_file(),
        "{}",
        app.message
    );
    assert!(!dir.join("Books").exists());
    assert!(
        text(&app).ends_with("[[Archive/Books/Dune]] ![[Archive/Books/cover.jpg]]"),
        "{}",
        text(&app)
    );
    // Renaming a folder updates the links to its attachments too.
    app.sidebar.selected = Some(dir.join("Archive/Books"));
    run_palette(&mut app, "rename folder");
    ctrl(&mut app, 'u');
    typing(&mut app, "Library");
    key(&mut app, KeyCode::Enter);
    assert!(
        text(&app).ends_with("[[Archive/Library/Dune]] ![[Archive/Library/cover.jpg]]"),
        "{}",
        text(&app)
    );
}

#[test]
fn template_tags_are_shown_as_written() {
    let mut app = templater_app(
        "templater-tags",
        "",
        &[
            ("Templates/Tag.md", "Day <% \"*x*\" + tp.file.title %> *b*"),
            ("Other.md", ""),
        ],
    );
    app.open(&note(&app, "Templates/Tag.md"));
    // The cursor on another line: the live preview of the first.
    app.open(&note(&app, "Other.md"));
    app.open(&note(&app, "Templates/Tag.md"));
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 10);
    assert!(
        rows.iter()
            .any(|r| r.contains("Day <% \"*x*\" + tp.file.title %> b")),
        "the tag as written, the rest rendered: {rows:#?}"
    );
}

#[test]
fn a_new_template_replaces_one_still_waiting_for_the_web() {
    let mut app = templater_app(
        "templater-replace-wait",
        "",
        &[
            ("Templates/Quote.md", "<% tp.web.daily_quote() %>"),
            ("Templates/Plain.md", "plain"),
            ("Note.md", ""),
        ],
    );
    app.open(&note(&app, "Note.md"));
    alt(&mut app, KeyCode::Char('e'));
    typing(&mut app, "Quote");
    key(&mut app, KeyCode::Enter);
    alt(&mut app, KeyCode::Char('e'));
    typing(&mut app, "Plain");
    key(&mut app, KeyCode::Enter);
    for _ in 0..20 {
        app.tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["plain"],
        "{}",
        app.message
    );
}

#[test]
fn a_click_in_the_text_places_the_cursor() {
    let dir = vault(
        "click-cursor",
        &[("Note.md", "first line\nsome **bold** words here\nlast")],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    let rows = screen(&mut app, 100, 20);
    // A rendered line (markup hidden): the cursor lands in its source.
    let (y, x) = find(&rows, "words").expect("drawn");
    click(&mut app, x as u16, y as u16);
    let view = app.view().unwrap();
    assert_eq!(
        (view.editor.row, view.editor.col),
        (1, "some **bold** ".chars().count())
    );
    // The line being edited is raw.
    let rows = screen(&mut app, 100, 20);
    let (y, x) = find(&rows, "bold**").expect("raw");
    click(&mut app, x as u16, y as u16);
    assert_eq!(app.view().unwrap().editor.col, "some **".chars().count());
    // From the sidebar, a click in the text gives the editor the focus.
    app.focus = Focus::Sidebar;
    let (y, x) = find(&rows, "last").expect("drawn");
    click(&mut app, x as u16 + 2, y as u16);
    assert_eq!(app.focus, Focus::Editor);
    let view = app.view().unwrap();
    assert_eq!((view.editor.row, view.editor.col), (2, 2));
}

#[test]
fn links_to_missing_notes_are_dimmed() {
    let dir = vault(
        "unresolved-links",
        &[
            ("Books/Dune.md", "# Dune"),
            ("Note.md", "top\nread [[Dune]] and [[Nowhere]] next"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    let rows = screen(&mut app, 100, 20);
    let modifiers = |app: &mut App, text: &str| {
        let (y, x) = find(&rows, text).unwrap_or_else(|| panic!("{text}: {rows:#?}"));
        let mut t = Terminal::new(TestBackend::new(100, 20)).unwrap();
        t.draw(|f| ui::draw(f, app)).unwrap();
        t.backend().buffer()[(x as u16, y as u16)].modifier
    };
    assert!(
        modifiers(&mut app, "Nowhere").contains(ratatui::style::Modifier::DIM),
        "no such note in the vault"
    );
    assert!(
        !modifiers(&mut app, "Dune and").contains(ratatui::style::Modifier::DIM),
        "found anywhere in the vault by its name"
    );
    // Made, it's a link like the others.
    ctrl(&mut app, 'o');
    typing(&mut app, "Nowhere");
    key(&mut app, KeyCode::Enter);
    app.open(&note(&app, "Note.md"));
    assert!(!modifiers(&mut app, "Nowhere").contains(ratatui::style::Modifier::DIM));
}

#[test]
fn changes_made_outside_are_picked_up() {
    let dir = vault(
        "watch-outside",
        &[("Note.md", "one"), ("Other.md", "other")],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    // Another program changes a note and adds one.
    fs::write(dir.join("Note.md"), "changed elsewhere").unwrap();
    fs::write(dir.join("New.md"), "new").unwrap();
    ticks_until(&mut app, |app| {
        app.view().unwrap().editor.lines == ["changed elsewhere"]
    });
    assert!(app.vault.note(&dir.join("New.md")).is_some(), "scanned");
    assert!(app.message.contains("outside"), "{}", app.message);
    // Its own save isn't a change from outside.
    app.message.clear();
    key(&mut app, KeyCode::End);
    typing(&mut app, "!");
    ctrl(&mut app, 's');
    app.message.clear();
    for _ in 0..80 {
        app.tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!app.message.contains("outside"), "{}", app.message);
    assert_eq!(app.view().unwrap().editor.col, "changed elsewhere!".len());
    // Unsaved changes are never overwritten: it says so.
    typing(&mut app, "?");
    fs::write(dir.join("Note.md"), "third").unwrap();
    ticks_until(&mut app, |app| app.message.contains("unsaved"));
    assert_eq!(app.view().unwrap().editor.lines, ["changed elsewhere!?"]);
    // A note deleted elsewhere leaves the vault.
    fs::remove_file(dir.join("Other.md")).unwrap();
    ticks_until(&mut app, |app| {
        app.vault.note(&dir.join("Other.md")).is_none()
    });
}

#[test]
fn open_tabs_and_folders_come_back_next_time() {
    let dir = vault(
        "session",
        &[
            ("Books/Dune.md", "# Dune\nspice\nmore spice"),
            ("Journal/A.md", "a"),
            ("Top.md", "top"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&dir.join("Books/Dune.md"));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Right);
    app.open(&dir.join("Journal/A.md"));
    app.sidebar.expanded.insert(dir.join("Books"));
    app.sidebar.expanded.insert(dir.join("Journal"));
    app.tick();
    drop(app);
    // Next time: the same tabs, the same one active, cursors and folders.
    let app = App::new(Vault::open(&dir).unwrap());
    let paths: Vec<_> = app.tabs.iter().map(|t| t.path.clone().unwrap()).collect();
    assert_eq!(paths, [dir.join("Books/Dune.md"), dir.join("Journal/A.md")]);
    assert_eq!(app.active, 1);
    assert_eq!((app.tabs[0].editor.row, app.tabs[0].editor.col), (2, 2));
    assert!(app.sidebar.expanded.contains(&dir.join("Books")));
    assert!(app.sidebar.expanded.contains(&dir.join("Journal")));
    // A note gone since is left out; quitting saves too.
    let mut app = app;
    ctrl(&mut app, 'w');
    app.quit();
    fs::remove_file(dir.join("Books/Dune.md")).unwrap();
    let app = App::new(Vault::open(&dir).unwrap());
    assert!(
        app.tabs.is_empty(),
        "{:?}",
        app.tabs.iter().map(|t| &t.path).collect::<Vec<_>>()
    );
}

#[test]
fn nested_tags_are_a_tree() {
    let dir = vault(
        "nested-tags",
        &[
            ("A.md", "#project/alpha and #solo"),
            ("B.md", "#project/beta #Project"),
            ("C.md", "#project/alpha/deep"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.focus = Focus::Sidebar;
    app.sidebar.show(Panel::Tags);
    let rows = screen(&mut app, 60, 12);
    let (y, _) = find(&rows, "#project").expect("a parent");
    // The sidebar's part of a row.
    let count = |row: &str| {
        row.split('│')
            .next()
            .unwrap_or_default()
            .trim_end()
            .to_string()
    };
    assert!(count(&rows[y]).ends_with('3'), "three notes: {:?}", rows[y]);
    assert!(find(&rows, "alpha").is_none(), "folded: {rows:#?}");
    assert!(find(&rows, "#solo").is_some());
    // → opens it: its tags, by their last part.
    key(&mut app, KeyCode::Right);
    let rows = screen(&mut app, 60, 12);
    let (y, x) = find(&rows, "#alpha").expect("opened");
    assert!(count(&rows[y]).ends_with('2'), "{:?}", rows[y]);
    let (_, px) = find(&rows, "#project").unwrap();
    assert!(x > px, "indented");
    assert!(find(&rows, "#beta").is_some());
    assert!(find(&rows, "#deep").is_none(), "its own tags folded");
    // Enter on a nested tag searches it (with its own nested tags).
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.sidebar.panel, Panel::Search);
    assert_eq!(app.sidebar.query, "tag:#project/alpha");
    assert_eq!(app.sidebar.results.len(), 2);
    // ← goes up to the parent, then folds it.
    app.sidebar.show(Panel::Tags);
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Left);
    let rows = screen(&mut app, 60, 12);
    assert!(find(&rows, "#alpha").is_none(), "{rows:#?}");
}

#[test]
fn bookmarks_keep_notes_headings_folders_and_searches() {
    let dir = vault(
        "bookmarks",
        &[
            ("Books/Dune.md", "# Dune\ntext\n## Spice\nmelange"),
            ("Top.md", "top"),
            (
                ".blackglass/plugins.toml",
                "installed = [\"bookmarks\"]\nenabled = [\"bookmarks\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    let dune = dir.join("Books/Dune.md");
    app.open(&dune);
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    run_palette(&mut app, "bookmark this note");
    assert!(app.message.contains("Bookmarked Dune"), "{}", app.message);
    run_palette(&mut app, "bookmark this heading");
    assert!(
        app.message.contains("Spice"),
        "the heading above the cursor: {}",
        app.message
    );
    // The search in the sidebar.
    ctrl(&mut app, 'g');
    typing(&mut app, "melange");
    run_palette(&mut app, "bookmark the search");
    // The folder chosen in the explorer.
    app.sidebar.show(Panel::Files);
    app.sidebar.selected = Some(dir.join("Books"));
    run_palette(&mut app, "bookmark this folder");
    run_palette(&mut app, "bookmarks: open");
    let rows = screen(&mut app, 80, 16);
    for row in ["Dune", "Dune › Spice", "melange", "Books/"] {
        assert!(find(&rows, row).is_some(), "{row}: {rows:#?}");
    }
    // Enter on the heading: the note at it.
    app.open(&dir.join("Top.md"));
    run_palette(&mut app, "bookmarks: open");
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(dune.clone()));
    assert_eq!(app.view().unwrap().editor.row, 2);
    // The search: run again.
    run_palette(&mut app, "bookmarks: open");
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.sidebar.panel, Panel::Search);
    assert_eq!(app.sidebar.query, "melange");
    // The folder: shown in the explorer.
    run_palette(&mut app, "bookmarks: open");
    key(&mut app, KeyCode::Home);
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.sidebar.panel, Panel::Files);
    assert_eq!(app.sidebar.selected, Some(dir.join("Books")));
    // r renames a bookmark, Delete removes one.
    run_palette(&mut app, "bookmarks: open");
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Char('r'));
    ctrl(&mut app, 'u');
    typing(&mut app, "Desert planet");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Delete);
    let rows = screen(&mut app, 80, 16);
    assert!(find(&rows, "Desert planet").is_some(), "{rows:#?}");
    let in_sidebar = |text: &str| {
        rows.iter()
            .any(|r| r.split('│').next().unwrap_or_default().contains(text))
    };
    assert!(!in_sidebar("melange"), "{rows:#?}");
    // A renamed note keeps its bookmarks; they're kept in the vault.
    app.open(&dune);
    key(&mut app, KeyCode::F(2));
    ctrl(&mut app, 'u');
    typing(&mut app, "Arrakis");
    key(&mut app, KeyCode::Enter);
    let saved =
        fs::read_to_string(dir.join(".blackglass/plugins/bookmarks/bookmarks.json")).unwrap();
    assert!(
        saved.contains("Books/Arrakis.md") && !saved.contains("Dune.md"),
        "{saved}"
    );
    assert!(saved.contains("Desert planet"), "{saved}");
    // Again on a bookmarked note: off.
    run_palette(&mut app, "bookmark this note");
    assert!(app.message.contains("no longer"), "{}", app.message);
}

#[test]
fn tags_and_properties_are_suggested_while_typing() {
    let dir = vault(
        "tag-suggest",
        &[
            ("A.md", "---\nstatus: done\n---\n#project/alpha #reading"),
            ("B.md", "#project"),
            ("New.md", ""),
            ("Props.md", ""),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "New.md"));
    typing(&mut app, "see #pro");
    let rows = screen(&mut app, 80, 16);
    assert!(find(&rows, "#project/alpha").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.view().unwrap().editor.lines, ["see #project/alpha"]);
    // A property's value from the other notes.
    app.open(&note(&app, "Props.md"));
    typing(&mut app, "---");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "status: d");
    key(&mut app, KeyCode::Tab);
    assert_eq!(
        app.view().unwrap().editor.lines[..2],
        ["---".to_string(), "status: done".to_string()]
    );
}

#[test]
fn every_action_is_a_command_whose_keys_can_change() {
    let (mut app, config) = app_with_config("all-commands");
    let names: Vec<String> = app.all_commands().into_iter().map(|c| c.name).collect();
    for name in [
        "Go to tab 1",
        "Go to tab 8",
        "Go to last tab",
        "Indent list item",
        "Unindent list item",
    ] {
        assert!(names.iter().any(|n| n == name), "{name}: {names:?}");
    }
    for rel in ["Welcome.md", "Books/Dune.md", "Journal/2026-08-09.md"] {
        app.open(&note(&app, rel));
    }
    alt(&mut app, KeyCode::Char('1'));
    assert_eq!(app.active, 0);
    alt(&mut app, KeyCode::Char('9'));
    assert_eq!(app.active, 2, "the last tab");
    alt(&mut app, KeyCode::Char('2'));
    assert_eq!(app.active, 1);
    // Rebound in keys.toml: the new key works, the old one is free.
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("keys.toml"),
        "go-to-tab-1 = \"Ctrl+Alt+1\"\nindent-list-item = \"\"\n",
    )
    .unwrap();
    app.load_user_config();
    alt(&mut app, KeyCode::Char('1'));
    assert_eq!(app.active, 1, "Alt+1 is free now");
    press(
        &mut app,
        KeyCode::Char('1'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    );
    assert_eq!(app.active, 0);
    // Tab taken off "Indent list item": the editor doesn't get it …
    typing(&mut app, "- item");
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.view().unwrap().editor.lines[0], "- item# Welcome");
    assert!(app.message.contains("no command"), "{}", app.message);
    // … but Tab still does what it does elsewhere (the sidebar's tabs).
    app.focus = Focus::Sidebar;
    let panel = app.sidebar.panel;
    key(&mut app, KeyCode::Tab);
    assert_ne!(app.sidebar.panel, panel);
    // On another key, the command still indents.
    fs::write(config.join("keys.toml"), "indent-list-item = \"Alt+I\"\n").unwrap();
    app.load_user_config();
    app.focus = Focus::Editor;
    alt(&mut app, KeyCode::Char('i'));
    let line = &app.view().unwrap().editor.lines[0];
    assert!(line.starts_with([' ', '\t']), "indented: {line:?}");
}

const ZETTELKASTEN: &str = "installed = [\"zettelkasten\"]\nenabled = [\"zettelkasten\"]\n";

/// A vault with the Zettelkasten plugin, these notes and its settings.
fn zk_app(name: &str, settings: &str, files: &[(&str, &str)]) -> App {
    let mut all = files.to_vec();
    all.push((".blackglass/plugins.toml", ZETTELKASTEN));
    all.push((".blackglass/plugins/zettelkasten/settings.toml", settings));
    App::new(Vault::open(&vault(name, &all)).unwrap())
}

/// The line the cursor is on.
fn cursor_line(app: &App) -> String {
    let view = app.view().unwrap();
    view.editor.lines[view.editor.row].clone()
}

#[test]
fn zettelkasten_links_to_headings_and_blocks() {
    let mut app = zk_app(
        "zk-links",
        "",
        &[
            (
                "Dune.md",
                "# Dune\n## Spice\nmelange is the spice\n- a list item ^old",
            ),
            ("Arrakis.md", "# Arrakis\n## Desert\nsand everywhere"),
            ("New.md", ""),
        ],
    );
    app.open(&note(&app, "New.md"));
    // [[Note# : the note's headings.
    typing(&mut app, "[[Dune#Sp");
    let rows = screen(&mut app, 80, 16);
    assert!(find(&rows, "Spice").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter);
    assert!(
        cursor_line(&app).starts_with("[[Dune#Spice"),
        "{}",
        cursor_line(&app)
    );
    // [[Note#^ : its blocks; one without an id gets one.
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "[[Dune#^melan");
    key(&mut app, KeyCode::Enter);
    let dune = fs::read_to_string(note(&app, "Dune.md")).unwrap();
    let id = dune
        .lines()
        .find_map(|l| l.strip_prefix("melange is the spice ^"))
        .unwrap_or_else(|| panic!("an id added: {dune}"))
        .to_string();
    assert_eq!(id.len(), 6);
    assert!(
        cursor_line(&app).starts_with(&format!("[[Dune#^{id}")),
        "{}",
        cursor_line(&app)
    );
    // One with an id keeps it.
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "[[Dune#^list");
    key(&mut app, KeyCode::Enter);
    assert!(
        cursor_line(&app).starts_with("[[Dune#^old"),
        "{}",
        cursor_line(&app)
    );
    // [[## and [[^^ : headings and blocks anywhere in the vault.
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "[[##Des");
    key(&mut app, KeyCode::Enter);
    assert!(
        cursor_line(&app).starts_with("[[Arrakis#Desert"),
        "{}",
        cursor_line(&app)
    );
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "[[^^sand");
    key(&mut app, KeyCode::Enter);
    let arrakis = fs::read_to_string(note(&app, "Arrakis.md")).unwrap();
    let id = arrakis
        .lines()
        .find_map(|l| l.strip_prefix("sand everywhere ^"))
        .unwrap_or_else(|| panic!("{arrakis}"))
        .to_string();
    assert!(
        cursor_line(&app).starts_with(&format!("[[Arrakis#^{id}")),
        "{}",
        cursor_line(&app)
    );
}

#[test]
fn zettelkasten_copies_links_and_makes_linked_unique_notes() {
    let mut app = zk_app(
        "zk-copy",
        "unique_folder = \"Inbox\"\nunique_template = \"Templates/Zettel.md\"\n",
        &[
            ("Templates/Zettel.md", "# New thought\n"),
            ("Dune.md", "# Dune\n## Spice\nmelange\nsand ^here"),
        ],
    );
    let copied = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let seen = std::rc::Rc::clone(&copied);
    app.copier = Box::new(move |text| {
        *seen.borrow_mut() = text.to_string();
        Ok(())
    });
    app.open(&note(&app, "Dune.md"));
    run_palette(&mut app, "copy link to this note");
    assert_eq!(*copied.borrow(), "[[Dune]]");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    run_palette(&mut app, "copy link to this heading");
    assert_eq!(*copied.borrow(), "[[Dune#Spice]]");
    // A block: an id added to the line when it has none.
    run_palette(&mut app, "copy link to this block");
    let line = cursor_line(&app);
    let id = line.strip_prefix("melange ^").expect("an id").to_string();
    assert_eq!(*copied.borrow(), format!("[[Dune#^{id}]]"));
    key(&mut app, KeyCode::Down);
    run_palette(&mut app, "copy link to this block");
    assert_eq!(*copied.borrow(), "[[Dune#^here]]", "its own id");
    // A new unique note, linked from here (the selection its alias).
    key(&mut app, KeyCode::Home);
    press(&mut app, KeyCode::End, KeyModifiers::SHIFT);
    let stamp = now_as("%Y%m%d%H%M");
    run_palette(&mut app, "new unique note linked from here");
    let new = active_path(&app).unwrap();
    assert_eq!(
        new.parent().unwrap(),
        app.vault.root.join("Inbox"),
        "its folder"
    );
    let name = new.file_stem().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with(&stamp[..10]), "{name}");
    assert_eq!(
        app.view().unwrap().editor.lines[0],
        "# New thought",
        "its template"
    );
    let dune_tab = app
        .tabs
        .iter()
        .find(|t| t.path.as_deref() == Some(note(&app, "Dune.md").as_path()))
        .unwrap()
        .editor
        .to_text();
    assert!(
        dune_tab.contains(&format!("[[{name}|sand ^here]]")),
        "the link in place of the selection: {dune_tab}"
    );
}

#[test]
fn a_unique_notes_template_runs_its_commands() {
    let both = "installed = [\"templater\", \"zettelkasten\"]\nenabled = [\"templater\", \"zettelkasten\"]\n";
    let dir = vault(
        "zk-unique-templater",
        &[
            ("Templates/Zettel.md", "# <% tp.file.title %>"),
            ("Here.md", ""),
            (".blackglass/plugins.toml", both),
            (
                ".blackglass/plugins/zettelkasten/settings.toml",
                "unique_template = \"Templates/Zettel\"\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Here.md"));
    run_palette(&mut app, "new unique note linked from here");
    let name = active_path(&app)
        .unwrap()
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(app.view().unwrap().editor.lines[0], format!("# {name}"));
}

#[test]
fn zettelkasten_shows_titles_instead_of_ids() {
    let mut app = zk_app(
        "zk-titles",
        "show_titles = true\n",
        &[
            ("202610011445.md", "---\ntitle: Spice must flow\n---\ntext"),
            ("Plain.md", "no title"),
        ],
    );
    let id = note(&app, "202610011445.md");
    app.open(&id);
    let rows = screen(&mut app, 100, 12);
    // Not the status bar (the last row: the file's own name).
    let explorer = |rows: &[String], text: &str| {
        rows[..rows.len() - 1]
            .iter()
            .any(|r| r.split('│').next().unwrap_or_default().contains(text))
    };
    assert!(
        explorer(&rows, "Spice must flow"),
        "the explorer: {rows:#?}"
    );
    assert!(!explorer(&rows, "202610011445"), "{rows:#?}");
    assert!(explorer(&rows, "Plain"), "no title: its name");
    assert!(
        rows[0].contains("Spice must flow"),
        "the tab: {:?}",
        rows[0]
    );
    assert!(
        rows[1].contains("Spice must flow"),
        "above the note: {:?}",
        rows[1]
    );
    ctrl(&mut app, 'o');
    typing(&mut app, "2026");
    let rows = screen(&mut app, 100, 12);
    assert!(
        find(&rows, "Spice must flow").is_some(),
        "the switcher: {rows:#?}"
    );
    key(&mut app, KeyCode::Esc);
    // Off: names again.
    fs::write(
        app.vault
            .root
            .join(".blackglass/plugins/zettelkasten/settings.toml"),
        "show_titles = false\n",
    )
    .unwrap();
    app.rescan();
    let rows = screen(&mut app, 100, 12);
    assert!(explorer(&rows, "202610011445"), "{rows:#?}");
}

#[test]
fn zettelkasten_folgezettel_notes_in_sequence() {
    let mut app = zk_app(
        "zk-folgezettel",
        "",
        &[
            ("1 Start.md", "start"),
            ("1a Branch.md", "branch"),
            ("2 Other.md", "other"),
            ("10 Ten.md", "ten"),
            ("Loose.md", "not in the sequence"),
        ],
    );
    app.open(&note(&app, "1a Branch.md"));
    run_palette(&mut app, "new sequel note");
    typing(&mut app, "Next");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        active_path(&app),
        Some(note(&app, "1b Next.md")),
        "{}",
        app.message
    );
    let branch = |app: &App| {
        app.tabs
            .iter()
            .find(|t| t.path.as_deref() == Some(note(app, "1a Branch.md").as_path()))
            .unwrap()
            .editor
            .to_text()
    };
    assert!(
        branch(&app).contains("[[1b Next]]"),
        "linked: {}",
        branch(&app)
    );
    app.open(&note(&app, "1a Branch.md"));
    run_palette(&mut app, "new branch note");
    typing(&mut app, "Deep");
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "1a1 Deep.md")));
    // Up, along, back.
    run_palette(&mut app, "go to parent in sequence");
    assert_eq!(active_path(&app), Some(note(&app, "1a Branch.md")));
    run_palette(&mut app, "go to next in sequence");
    assert_eq!(
        active_path(&app),
        Some(note(&app, "1a1 Deep.md")),
        "depth first"
    );
    run_palette(&mut app, "go to next in sequence");
    assert_eq!(active_path(&app), Some(note(&app, "1b Next.md")));
    run_palette(&mut app, "go to previous in sequence");
    assert_eq!(active_path(&app), Some(note(&app, "1a1 Deep.md")));
    // The Sequence tab: in order, indented by depth.
    run_palette(&mut app, "zettelkasten: show sequence");
    let rows = screen(&mut app, 100, 16);
    let side: Vec<String> = rows
        .iter()
        .map(|r| {
            r.split('│')
                .next()
                .unwrap_or_default()
                .trim_end()
                .to_string()
        })
        .collect();
    let order: Vec<usize> = [
        "1 Start",
        "1a Branch",
        "1a1 Deep",
        "1b Next",
        "2 Other",
        "10 Ten",
    ]
    .iter()
    .map(|n| {
        side.iter()
            .position(|r| r.contains(n))
            .unwrap_or_else(|| panic!("{n}: {side:#?}"))
    })
    .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{side:#?}");
    let indent = |n: &str| {
        side.iter()
            .find(|r| r.contains(n))
            .unwrap()
            .find(n)
            .unwrap()
    };
    assert!(indent("1a1 Deep") > indent("1a Branch") && indent("1a Branch") > indent("1 Start"));
    assert!(!side.iter().any(|r| r.contains("Loose")));
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "1 Start.md")));
}

#[test]
fn zettelkasten_breadcrumbs_and_hierarchy_moves() {
    let mut app = zk_app(
        "zk-breadcrumbs",
        "",
        &[
            (
                "Home.md",
                "---\ndown: [\"[[Project]]\", \"[[Other]]\"]\n---\n",
            ),
            (
                "Project.md",
                "---\nup: \"[[Home]]\"\nnext: \"[[Project 2]]\"\n---\n",
            ),
            (
                "Project 2.md",
                "---\nup: \"[[Home]]\"\nprev: \"[[Project]]\"\n---\n",
            ),
            ("Other.md", ""),
            ("Note.md", "---\nup: \"[[Project]]\"\n---\nbody"),
        ],
    );
    app.open(&note(&app, "Note.md"));
    assert!(
        app.plugin_status().contains("Home › Project › Note"),
        "{}",
        app.plugin_status()
    );
    run_palette(&mut app, "go up");
    assert_eq!(active_path(&app), Some(note(&app, "Project.md")));
    run_palette(&mut app, "go to next note");
    assert_eq!(active_path(&app), Some(note(&app, "Project 2.md")));
    run_palette(&mut app, "go to previous note");
    assert_eq!(active_path(&app), Some(note(&app, "Project.md")));
    run_palette(&mut app, "go up");
    run_palette(&mut app, "go down");
    // Two notes down: asked which.
    typing(&mut app, "Other");
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Other.md")));
    run_palette(&mut app, "go up");
    assert!(app.message.contains("no up"), "{}", app.message);
}

/// The text of `rel`'s tab if it's open, else its file.
fn note_text(app: &App, rel: &str) -> String {
    let path = note(app, rel);
    app.tabs
        .iter()
        .find(|t| t.path.as_deref() == Some(path.as_path()))
        .map(|t| t.editor.to_text())
        .unwrap_or_else(|| fs::read_to_string(&path).unwrap_or_default())
}

#[test]
fn zettelkasten_extracts_selections_and_headings() {
    let mut app = zk_app(
        "zk-extract",
        "extract_template = \"From [[{{fromTitle}}]]: {{content}}\"\n",
        &[
            (
                "Source.md",
                "intro\nan idea worth keeping\n## Part\npart body\nmore\n## Next\nnext",
            ),
            ("Ideas.md", "# Ideas"),
        ],
    );
    app.open(&note(&app, "Source.md"));
    // The selection to an existing note (appended), a link in its place.
    key(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::End, KeyModifiers::SHIFT);
    run_palette(&mut app, "zettelkasten: extract selection to another note");
    typing(&mut app, "Ideas");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        note_text(&app, "Ideas.md"),
        "# Ideas\nFrom [[Source]]: an idea worth keeping\n"
    );
    assert_eq!(app.view().unwrap().editor.lines[1], "[[Ideas]]");
    // … or to a new one.
    key(&mut app, KeyCode::Home);
    press(&mut app, KeyCode::End, KeyModifiers::SHIFT);
    run_palette(&mut app, "zettelkasten: extract selection to another note");
    key(&mut app, KeyCode::Enter); // "New note…"
    typing(&mut app, "Linking");
    key(&mut app, KeyCode::Enter);
    assert_eq!(note_text(&app, "Linking.md"), "From [[Source]]: [[Ideas]]");
    assert_eq!(app.view().unwrap().editor.lines[1], "[[Linking]]");
    // A heading's section to a note named by it.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    run_palette(&mut app, "extract this heading");
    assert_eq!(note_text(&app, "Part.md"), "part body\nmore");
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["intro", "[[Linking]]", "[[Part]]", "## Next", "next"]
    );
}

#[test]
fn zettelkasten_splits_and_merges_notes() {
    let mut app = zk_app(
        "zk-split-merge",
        "",
        &[
            ("Long.md", "# Long\nabout\n## One\nfirst\n## Two\nsecond"),
            ("Keep.md", "# Keep\nkept"),
            ("Old.md", "old text"),
            ("Linker.md", "see [[Old]] and [[Old|the old one]]"),
        ],
    );
    app.open(&note(&app, "Long.md"));
    run_palette(&mut app, "split note at headings");
    typing(&mut app, "2");
    key(&mut app, KeyCode::Enter);
    assert_eq!(note_text(&app, "One.md"), "first");
    assert_eq!(note_text(&app, "Two.md"), "second");
    assert_eq!(
        app.view().unwrap().editor.lines,
        ["# Long", "about", "- [[One]]", "- [[Two]]"]
    );
    // Old merged into Keep: its text at the end, links follow, it's gone.
    app.open(&note(&app, "Old.md"));
    run_palette(&mut app, "merge this note into another");
    typing(&mut app, "Keep");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter); // "Merge and delete Old"
    assert_eq!(note_text(&app, "Keep.md"), "# Keep\nkept\nold text\n");
    assert!(!app.vault.root.join("Old.md").exists());
    assert!(
        app.vault.root.join(".trash/Old.md").exists(),
        "in the trash"
    );
    assert_eq!(
        fs::read_to_string(note(&app, "Linker.md")).unwrap(),
        "see [[Keep]] and [[Keep|the old one]]"
    );
    assert_eq!(active_path(&app), Some(note(&app, "Keep.md")));
}

#[test]
fn zettelkasten_lists_orphans_dead_ends_and_unresolved_links() {
    let mut app = zk_app(
        "zk-retrieval",
        "",
        &[
            ("Hub.md", "[[A]] [[B]] [[Missing]]"),
            ("A.md", "[[Hub]]"),
            ("B.md", "no links"),
            ("Lonely.md", "alone"),
        ],
    );
    let page = |app: &mut App, command: &str| {
        run_palette(app, command);
        app.view().unwrap().editor.to_text()
    };
    let orphans = page(&mut app, "show orphan notes");
    assert!(
        orphans.contains("[[Lonely]]") && !orphans.contains("[[B]]"),
        "{orphans}"
    );
    let dead = page(&mut app, "show dead-end notes");
    assert!(
        dead.contains("[[B]]") && dead.contains("[[Lonely]]") && !dead.contains("[[Hub]]"),
        "{dead}"
    );
    let unresolved = page(&mut app, "show unresolved links");
    assert!(
        unresolved.contains("Missing") && unresolved.contains("[[Hub]]"),
        "{unresolved}"
    );
}

#[test]
fn zettelkasten_local_links_link_counts_and_quick_capture() {
    let mut app = zk_app(
        "zk-local",
        "link_counts = true\ninbox = \"Inbox\"\n",
        &[
            ("Hub.md", "top\n[[A]] and [[B]]"),
            ("A.md", "[[Deep]]"),
            ("B.md", ""),
            ("Deep.md", ""),
            ("C.md", "[[Hub]]"),
        ],
    );
    app.open(&note(&app, "Hub.md"));
    // How many notes link to each link's note, beside it.
    let rows = screen(&mut app, 80, 10);
    assert!(find(&rows, "A 1 and B 1").is_some(), "{rows:#?}");
    // The note's links, out and in, as a tree.
    run_palette(&mut app, "show local links");
    let rows = screen(&mut app, 80, 12);
    let side: Vec<String> = rows
        .iter()
        .map(|r| {
            r.split('│')
                .next()
                .unwrap_or_default()
                .trim_end()
                .to_string()
        })
        .collect();
    let at = |text: &str| {
        side.iter()
            .position(|r| r.contains(text))
            .unwrap_or_else(|| panic!("{text}: {side:#?}"))
    };
    assert!(at("→ A") < at("→ Deep") && at("→ Deep") < at("→ B") && at("→ B") < at("← C"));
    assert!(
        side[at("→ Deep")].find('→') > side[at("→ A")].find('→'),
        "indented"
    );
    key(&mut app, KeyCode::Home);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "A.md")));
    // Quick capture to the inbox, staying here.
    run_palette(&mut app, "quick capture");
    typing(&mut app, "an idea");
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "quick capture");
    typing(&mut app, "another");
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "A.md")));
    assert_eq!(
        fs::read_to_string(note(&app, "Inbox.md"))
            .unwrap()
            .trim_end(),
        "- an idea\n- another"
    );
}

const BIB: &str = r#"@book{herbert1965,
  title = {Dune},
  author = {Herbert, Frank},
  year = {1965},
  url = {https://example.org/dune},
  abstract = {A desert planet.}
}
@article{doe2020,
  title = "On {Spice}",
  author = {Doe, Jane and Smith, Ann and Roe, Rick},
  year = 2020
}
"#;

#[test]
fn citations_insert_cite_and_open_literature_notes() {
    let plugins = "installed = [\"citations\"]\nenabled = [\"citations\"]\n";
    let dir = vault(
        "citations",
        &[
            ("refs.bib", BIB),
            ("Note.md", "top\n"),
            (".blackglass/plugins.toml", plugins),
            (
                ".blackglass/plugins/citations/settings.toml",
                "bibliography = \"refs.bib\"\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "insert citation");
    let rows = screen(&mut app, 100, 16);
    assert!(find(&rows, "Doe et al. 2020").is_some(), "{rows:#?}");
    typing(&mut app, "dune");
    key(&mut app, KeyCode::Enter);
    assert_eq!(cursor_line(&app), "[@herbert1965]");
    // Shown as its author and year (on another line than the cursor's).
    key(&mut app, KeyCode::Up);
    let rows = screen(&mut app, 100, 8);
    assert!(find(&rows, "Herbert 1965").is_some(), "{rows:#?}");
    // Its literature note, made from the template, at the citation.
    key(&mut app, KeyCode::Down);
    run_palette(&mut app, "open literature note");
    let lit = app.vault.root.join("Reading notes/@herbert1965.md");
    assert_eq!(active_path(&app), Some(lit.clone()), "{}", app.message);
    let text = fs::read_to_string(&lit).unwrap();
    assert!(
        text.starts_with("# Dune\n")
            && text.contains("Frank Herbert (1965)")
            && text.contains("A desert planet."),
        "{text}"
    );
    // Again: the same note, not a new one; a link to it.
    app.open(&note(&app, "Note.md"));
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "insert link to literature note");
    typing(&mut app, "dune");
    key(&mut app, KeyCode::Enter);
    assert_eq!(cursor_line(&app), "[[@herbert1965]]");
}

/// A vault with Dataview and these notes (and its settings).
fn dv_app(name: &str, settings: &str, files: &[(&str, &str)]) -> App {
    let mut all = files.to_vec();
    all.push((
        ".blackglass/plugins.toml",
        "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
    ));
    all.push((".blackglass/plugins/dataview/settings.toml", settings));
    App::new(Vault::open(&vault(name, &all)).unwrap())
}

#[test]
fn dataview_inline_queries_show_their_values() {
    let mut app = dv_app(
        "dv-inline",
        "",
        &[
            (
                "Note.md",
                "top\nName: `= this.file.name`, double: `= [[Other]].rating * 2`, js: `$= dv.current().file.name.length`\nend\nop: `==` and `=`",
            ),
            ("Other.md", "rating:: 4"),
        ],
    );
    app.open(&note(&app, "Note.md"));
    let rows = screen(&mut app, 100, 10);
    assert!(
        find(&rows, "Name: Note, double: 8, js: 4").is_some(),
        "{rows:#?}"
    );
    assert!(
        find(&rows, "op: == and =").is_some() && !rows.iter().any(|r| r.contains('⚠')),
        "`==` and `=` are code, not queries: {rows:#?}"
    );
    // On the line being edited, as written.
    key(&mut app, KeyCode::Down);
    let rows = screen(&mut app, 100, 10);
    assert!(find(&rows, "`= this.file.name`").is_some(), "{rows:#?}");
}

#[test]
fn dataview_tasks_are_checked_off_from_results() {
    let mut app = dv_app(
        "dv-check",
        "",
        &[
            ("Q.md", "top\n```dataview\nTASK FROM \"Todo\"\n```"),
            ("Todo.md", "- [ ] one\n- [x] two"),
        ],
    );
    app.open(&note(&app, "Q.md"));
    view_mode(&mut app);
    screen(&mut app, 100, 20);
    // The page, then its tasks: Tab, Tab to the first task.
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Enter);
    let todo = note(&app, "Todo.md");
    assert_eq!(
        fs::read_to_string(&todo).unwrap(),
        "- [x] one\n- [x] two\n",
        "{}",
        app.message
    );
    assert_eq!(active_path(&app), Some(note(&app, "Q.md")), "it stays here");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        fs::read_to_string(&todo).unwrap(),
        "- [ ] one\n- [x] two\n",
        "and back"
    );
    // With completion tracking, the date it was done.
    let mut app = dv_app(
        "dv-check-dates",
        "completion_tracking = true\n",
        &[
            ("Q.md", "top\n```dataview\nTASK FROM \"Todo\"\n```"),
            ("Todo.md", "- [ ] one"),
        ],
    );
    app.open(&note(&app, "Q.md"));
    view_mode(&mut app);
    screen(&mut app, 100, 20);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Enter);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(
        fs::read_to_string(note(&app, "Todo.md")).unwrap(),
        format!("- [x] one ✅ {today}\n")
    );
}

#[test]
fn dataview_results_follow_unsaved_edits() {
    let mut app = dv_app(
        "dv-live",
        "",
        &[(
            "Note.md",
            "rating:: 1\n\n```dataview\nLIST rating WHERE rating\n```",
        )],
    );
    let path = note(&app, "Note.md");
    app.open(&path);
    let rows = screen(&mut app, 80, 10);
    assert!(find(&rows, "Note: 1").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Backspace);
    typing(&mut app, "5");
    app.tick();
    let rows = screen(&mut app, 80, 10);
    assert!(find(&rows, "Note: 5").is_some(), "before saving: {rows:#?}");
    assert!(
        fs::read_to_string(&path).unwrap().starts_with("rating:: 1"),
        "not saved"
    );
    // Closed without saving: the results go back.
    ctrl(&mut app, 'w');
    key(&mut app, KeyCode::Char('n'));
    app.open(&path);
    let rows = screen(&mut app, 80, 10);
    assert!(find(&rows, "Note: 1").is_some(), "{rows:#?}");
}

/// A vault with the Task Archiver enabled, its settings, and these notes.
fn archiver_app(name: &str, settings: &str, notes: &[(&str, &str)]) -> App {
    let mut files = vec![
        (
            ".blackglass/plugins.toml",
            "installed = [\"archiver\"]\nenabled = [\"archiver\"]\n",
        ),
        (".blackglass/plugins/archiver/settings.toml", settings),
    ];
    files.extend_from_slice(notes);
    App::new(Vault::open(&vault(name, &files)).unwrap())
}

fn editor_text(app: &App) -> String {
    app.view().unwrap().editor.lines.join("\n")
}

#[test]
fn the_task_archiver_archives_deletes_sorts_and_checks_off() {
    let mut app = archiver_app(
        "archiver",
        "add_metadata = \"false\"\n\n[rule.1]\nstatuses = \">\"\nfile = \"Later\"\n",
        &[
            (
                "Plan.md",
                "# Work\n- [x] done\n    - its notes\n- [ ] open\n- [>] later\n- [-] cancelled",
            ),
            ("List.md", "- [x] a\n- b\n- [ ] c\n\n# Keep\nx\n# Gone\ny"),
        ],
    );
    app.open(&note(&app, "Plan.md"));
    run_palette(&mut app, "archive tasks in this file");
    assert_eq!(app.message, "Task Archiver: archived 2 tasks");
    assert_eq!(
        editor_text(&app),
        "# Work\n- [ ] open\n- [-] cancelled\n\n# Archived\n\n- [x] done\n    - its notes\n"
    );
    assert_eq!(
        fs::read_to_string(note(&app, "Later.md")).unwrap(),
        "# Archived\n\n- [>] later\n",
        "a rule's task in its own note"
    );
    // Check one off and archive it, from the cursor.
    key(&mut app, KeyCode::Down);
    run_palette(&mut app, "toggle task done and archive it");
    assert_eq!(
        editor_text(&app),
        "# Work\n- [-] cancelled\n\n# Archived\n\n- [x] done\n    - its notes\n- [x] open\n"
    );
    assert_eq!(app.view().unwrap().editor.row, 1, "where the task was");
    // Sort a list, delete the done tasks, archive a heading.
    app.open(&note(&app, "List.md"));
    run_palette(&mut app, "sort tasks in list under cursor");
    assert!(
        editor_text(&app).starts_with("- b\n- [ ] c\n- [x] a\n"),
        "{}",
        editor_text(&app)
    );
    run_palette(&mut app, "delete tasks in this file");
    assert_eq!(app.message, "Task Archiver: deleted 1 task");
    for _ in 0..6 {
        key(&mut app, KeyCode::Down);
    }
    run_palette(&mut app, "archive heading under cursor");
    assert_eq!(
        editor_text(&app),
        "- b\n- [ ] c\n\n# Keep\nx\n\n# Archived\n\n## Gone\ny"
    );
}

/// A day from today, as `YYYY-MM-DD`.
fn day_from_today(days: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn dates_in_words_become_links_to_their_days() {
    let dir = vault("dates", &[("Note.md", "")]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    typing(&mut app, "Meet @tom");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("Tomorrow"), "suggested: {rows}");
    key(&mut app, KeyCode::Enter);
    let tomorrow = day_from_today(1);
    assert_eq!(cursor_line(&app), format!("Meet [[{tomorrow}]]"));
    // Shift+Enter keeps the words as the link's alias.
    typing(&mut app, " and @in 3 da");
    press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(
        cursor_line(&app),
        format!(
            "Meet [[{tomorrow}]] and [[{}|in 3 days]]",
            day_from_today(3)
        )
    );
    // An @ in a word is not a date.
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "me@today");
    assert!(app.plugin_suggest.is_none(), "an e-mail address");
    // The selection to a date.
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "the day after tomorrow");
    press(&mut app, KeyCode::Home, KeyModifiers::SHIFT);
    run_palette(&mut app, "parse natural language date");
    assert_eq!(cursor_line(&app), format!("[[{}]]", day_from_today(2)));
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "yesterday");
    press(&mut app, KeyCode::Home, KeyModifiers::SHIFT);
    run_palette(&mut app, "parse natural language date as plain text");
    assert_eq!(cursor_line(&app), day_from_today(-1));
    // The date picker: a date in words, shown as it's read.
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "date picker");
    typing(&mut app, "in a week");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(
        rows.contains(&day_from_today(7)),
        "the date it reads: {rows}"
    );
    key(&mut app, KeyCode::Enter);
    assert_eq!(cursor_line(&app), format!("[[{}]]", day_from_today(7)));
}

#[test]
fn dates_follow_their_settings() {
    let dir = vault(
        "dates-settings",
        &[
            ("Note.md", ""),
            (
                ".blackglass/dates.toml",
                "link = \"false\"\nformat = \"DD.MM.YYYY\"\ntrigger = \"//\"\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    typing(&mut app, "@today ");
    assert!(app.plugin_suggest.is_none(), "another trigger");
    typing(&mut app, "//tod");
    key(&mut app, KeyCode::Enter);
    let today = chrono::Local::now().format("%d.%m.%Y").to_string();
    assert_eq!(cursor_line(&app), format!("@today {today}"));
    run_palette(&mut app, "insert today's date");
    assert!(cursor_line(&app).ends_with(&format!("{today}{today}")));
    // The settings window has them.
    app.open_settings_window(blackglass::settings_window::Page::Dates, true);
    let rows = screen(&mut app, 120, 30).join("\n");
    assert!(rows.contains("Insert dates as links"), "{rows}");
}

/// A vault with Emoji Shortcodes enabled and its settings.
fn emoji_app(name: &str, settings: &str) -> App {
    let dir = vault(
        name,
        &[
            ("Note.md", ""),
            (
                ".blackglass/plugins.toml",
                "installed = [\"emoji-shortcodes\"]\nenabled = [\"emoji-shortcodes\"]\n",
            ),
            (
                ".blackglass/plugins/emoji-shortcodes/settings.toml",
                settings,
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    app
}

#[test]
fn emoji_shortcodes_are_suggested_and_replaced() {
    let mut app = emoji_app("emoji", "");
    typing(&mut app, "So funny :jo");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(
        rows.contains("😂") && rows.contains("joy"),
        "suggested: {rows}"
    );
    let first = &app.plugin_suggest.as_ref().unwrap().items[0].1;
    assert_eq!(first, "😂", "the exact name's start first");
    key(&mut app, KeyCode::Enter);
    assert_eq!(cursor_line(&app), "So funny 😂");
    // A time isn't a shortcode.
    typing(&mut app, " at 10:30");
    assert!(app.plugin_suggest.is_none(), "not after a digit");
    // What was used comes first next time.
    typing(&mut app, " :o");
    let items = &app.plugin_suggest.as_ref().unwrap().items;
    assert_eq!(
        items[0].1, "😂",
        "recently used, though joy doesn't start with o"
    );
    key(&mut app, KeyCode::Esc);
    // Shortcodes typed out show as their emoji where the cursor isn't.
    typing(&mut app, " :heart: ");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("❤"), "the emoji in place of :heart:: {rows}");
}

#[test]
fn emoji_shortcodes_can_stay_shortcodes() {
    let mut app = emoji_app("emoji-keep", "immediate_replace = \"false\"\n");
    typing(&mut app, ":thumbsu");
    key(&mut app, KeyCode::Enter);
    assert_eq!(cursor_line(&app), ":thumbsup: ");
}

#[test]
fn highlights_have_colors() {
    let dir = vault("highlights", &[("Note.md", "")]);
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    // Typing == suggests a color.
    typing(&mut app, "a ==");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("🔴") && rows.contains("purple"), "{rows}");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "hot");
    assert!(
        cursor_line(&app).starts_with("a ==🟠hot"),
        "{}",
        cursor_line(&app)
    );
    // The color of the highlight at the cursor changes; it can go.
    let line = cursor_line(&app);
    if !line.ends_with("==") {
        typing(&mut app, "==");
    }
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Left);
    run_palette(&mut app, "highlight in green");
    assert_eq!(cursor_line(&app), "a ==🟢hot==");
    run_palette(&mut app, "remove highlight color");
    assert_eq!(cursor_line(&app), "a ==hot==");
    // A selection is highlighted in a color.
    key(&mut app, KeyCode::End);
    typing(&mut app, " cold");
    for _ in 0..4 {
        press(&mut app, KeyCode::Left, KeyModifiers::SHIFT);
    }
    run_palette(&mut app, "highlight in blue");
    assert_eq!(cursor_line(&app), "a ==hot== ==🔵cold==");
    // Shown in its color, the emoji hidden, off the cursor's line.
    key(&mut app, KeyCode::End);
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(
        rows.contains("a hot cold") && !rows.contains("🔵"),
        "{rows}"
    );
}

#[test]
fn periodic_notes_sets_jumps_startup_and_deleting() {
    let dir = vault(
        "periodic-more",
        &[
            ("Journal/2026-08-08.md", "eight"),
            ("Journal/2026-08-12.md", "twelve"),
            ("Work/note.md", ""),
            (
                ".blackglass/plugins/periodic-notes/settings.toml",
                "[general]\nopen_at_startup = \"true\"\n\n[daily]\nfolder = \"Journal\"\n\n[work/daily]\nfolder = \"Work\"\nformat = \"[day] YYYYMMDD\"\n",
            ),
            (
                ".blackglass/plugins.toml",
                "installed = [\"periodic-notes\"]\nenabled = [\"periodic-notes\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    // Today's note opens at startup.
    app.tick();
    let todays = note(&app, &format!("Journal/{}.md", today("%Y-%m-%d")));
    assert_eq!(active_path(&app), Some(todays.clone()), "{}", app.message);
    // Jumps for a granularity: from a daily note to the closest one.
    app.open(&note(&app, "Journal/2026-08-08.md"));
    run_palette(&mut app, "jump forwards to closest daily note");
    assert_eq!(active_path(&app), Some(note(&app, "Journal/2026-08-12.md")));
    // Another calendar set: its own folder and names.
    run_palette(&mut app, "switch calendar set");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("work"), "{rows}");
    typing(&mut app, "work");
    key(&mut app, KeyCode::Enter);
    run_palette(&mut app, "open daily note");
    let work = note(&app, &format!("Work/day {}.md", today("%Y%m%d")));
    assert_eq!(active_path(&app), Some(work.clone()), "{}", app.message);
    // Delete from the calendar (asked first).
    alt(&mut app, KeyCode::Char('c'));
    key(&mut app, KeyCode::Delete);
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("Delete"), "{rows}");
    key(&mut app, KeyCode::Enter);
    assert!(!work.exists(), "{}", app.message);
    assert!(todays.exists(), "the other set's note stays");
}

/// A kanban base and its projects.
const BOARD: &[(&str, &str)] = &[
    (
        "Projects.base",
        "filters: file.inFolder(\"Projects\")\nviews:\n  - type: kanban\n    name: Board\n    groupBy:\n      property: status\n      direction: ASC\n    groupOrder:\n      - Planned\n      - Doing\n      - Done\n    groupColors:\n      Doing: yellow\n      Done: green\n    order:\n      - file.name\n      - priority\n",
    ),
    (
        "Projects/Garden.md",
        "---\nstatus: Planned\npriority: high\n---\n",
    ),
    (
        "Projects/Pond.md",
        "---\nstatus: Doing\ntags: [water, Doing]\n---\n",
    ),
    ("Projects/Shed.md", "---\nstatus: Doing\n---\n"),
    ("Projects/Fence.md", "---\nstatus: Done\n---\n"),
    ("Projects/Idea.md", "no status\n"),
    (
        ".blackglass/plugins.toml",
        "installed = [\"bases\"]\nenabled = [\"bases\"]\n",
    ),
];

#[test]
fn a_kanban_board_is_moved_around_by_key() {
    let dir = vault("kanban", BOARD);
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Projects.base"));
    let rows = screen(&mut app, 120, 30).join("\n");
    assert!(
        rows.contains("Planned (1)") && rows.contains("Doing (2)") && rows.contains("Done (1)"),
        "the columns in their groupOrder: {rows}"
    );
    assert!(!rows.contains("Idea"), "None isn't in groupOrder: hidden");
    // → to Doing (Pond first), Shift+→ moves Pond to Done.
    key(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
    let pond = fs::read_to_string(dir.join("Projects/Pond.md")).unwrap();
    assert!(pond.contains("status: Done"), "{pond}\n{}", app.message);
    let rows = screen(&mut app, 120, 30).join("\n");
    assert!(
        rows.contains("Doing (1)") && rows.contains("Done (2)"),
        "{rows}"
    );
    // Alt+Shift+← moves the Done column before Doing (groupOrder in the
    // file); Alt+← alone is still the previous tab.
    press(
        &mut app,
        KeyCode::Left,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    );
    let base = fs::read_to_string(dir.join("Projects.base")).unwrap();
    assert!(
        base.contains("groupOrder:\n      - Planned\n      - Done\n      - Doing"),
        "{base}\n{}",
        app.message
    );
    let rows = screen(&mut app, 120, 30);
    let heads = rows
        .iter()
        .find(|r| r.contains("Planned ("))
        .expect("the column headings");
    let (done, doing) = (
        heads.find("Done (").unwrap(),
        heads.find("Doing (").unwrap(),
    );
    assert!(done < doing, "shown in the new order: {heads}");
    // Enter opens the selected card: Pond (it followed its move).
    key(&mut app, KeyCode::Enter);
    assert_eq!(active_path(&app), Some(note(&app, "Projects/Pond.md")));
    // n: a new note in the selected column, with its value.
    app.open(&note(&app, "Projects.base"));
    key(&mut app, KeyCode::Left);
    typing(&mut app, "n");
    typing(&mut app, "Wall");
    key(&mut app, KeyCode::Enter);
    let wall = fs::read_to_string(dir.join("Projects/Wall.md")).unwrap();
    assert!(wall.contains("status: Planned"), "{wall}\n{}", app.message);
}

/// The text of the screen rows from the one containing `from` to the one
/// containing `to`.
fn rows_between(rows: &[String], from: &str, to: &str) -> String {
    let a = rows.iter().position(|r| r.contains(from)).unwrap_or(0);
    let b = rows
        .iter()
        .rposition(|r| r.contains(to))
        .unwrap_or(rows.len() - 1);
    rows[a..=b.max(a)].join("\n")
}

#[test]
fn a_bases_filters_are_changed_in_a_pane_with_live_results() {
    let dir = vault("bases-filter-pane", BOARD);
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Projects.base"));
    // Alt+F: the filters above the results, with the focus.
    alt(&mut app, KeyCode::Char('f'));
    assert_eq!(app.focus, Focus::Pane);
    let rows = screen(&mut app, 120, 40);
    let pane = rows_between(&rows, "Filters", "add filter");
    assert!(
        pane.contains("All views") && pane.contains("file.folder") && pane.contains("is in folder"),
        "{pane}"
    );
    // a: a new filter; its property, then its value, typed: the results
    // follow at each key.
    key(&mut app, KeyCode::Char('a'));
    typing(&mut app, "status");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "Doing");
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(rows.contains("2 results"), "Pond and Shed: {rows}");
    key(&mut app, KeyCode::Enter);
    let base = fs::read_to_string(dir.join("Projects.base")).unwrap();
    assert!(
        base.contains(
            "filters:\n  and:\n    - file.inFolder(\"Projects\")\n    - status == \"Doing\""
        ),
        "{base}"
    );
    // o: another operator; d: deleted.
    key(&mut app, KeyCode::Char('o'));
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(
        rows.contains("is not") && rows.contains("3 results"),
        "Garden, Fence and Idea (no status): {rows}"
    );
    key(&mut app, KeyCode::Char('d'));
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(rows.contains("5 results"), "{rows}");
    // Tab: this view's own filters (none yet).
    key(&mut app, KeyCode::Tab);
    let rows = screen(&mut app, 120, 40);
    let pane = rows_between(&rows, "Filters", "add filter");
    assert!(
        pane.contains("This view") && !pane.contains("is in folder"),
        "{pane}"
    );
    key(&mut app, KeyCode::Tab);
    // Alt+M: maximized, then minimized to a summary line.
    alt(&mut app, KeyCode::Char('m'));
    alt(&mut app, KeyCode::Char('m'));
    let rows = screen(&mut app, 120, 40);
    assert!(
        rows.iter()
            .any(|r| r.contains("filters:") && r.contains("file.folder is in folder Projects")),
        "{}",
        rows.join("\n")
    );
    // Esc: back to the results; Alt+F hides the pane.
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.focus, Focus::Editor);
    alt(&mut app, KeyCode::Char('f'));
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(
        !rows.contains("add filter") && !rows.contains("filters:"),
        "{rows}"
    );
}

#[test]
fn a_new_base_opens_with_its_filters() {
    let dir = vault("bases-new-pane", BOARD);
    let mut app = App::new(Vault::open(&dir).unwrap());
    run_palette(&mut app, "bases create new base");
    assert_eq!(app.focus, Focus::Pane, "{}", app.message);
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(
        rows.contains("Filters") && rows.contains("add filter"),
        "{rows}"
    );
}

#[test]
fn editing_a_task_keeps_its_tags_after_the_fields() {
    let mut app = tasks_app(
        "tasks-edit-tags",
        &[("T.md", "- [ ] Paint the fence ⏫ 📅 2026-10-12 #garden\n")],
    );
    app.open(&note(&app, "T.md"));
    alt(&mut app, KeyCode::Char('t'));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right); // high → medium
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        lines(&app)[0],
        "- [ ] Paint the fence #garden 🔼 📅 2026-10-12",
        "{}",
        app.message
    );
}

#[test]
fn a_tasks_result_is_postponed_and_edited_where_it_is() {
    let work = format!("# Work\n- [ ] Ship report 📅 {}\n", day(0));
    let mut app = tasks_app(
        "tasks-result-postpone",
        &[
            ("Work.md", &work),
            ("Query.md", "top\n```tasks\nnot done\n```\nend\n"),
        ],
    );
    app.open(&note(&app, "Query.md"));
    run_palette(&mut app, "view mode");
    // Down: the block's frame, its toolbar, then its first result.
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    run_palette(&mut app, "tasks postpone task");
    typing(&mut app, "1 day");
    key(&mut app, KeyCode::Enter);
    let text = fs::read_to_string(note(&app, "Work.md")).unwrap();
    assert!(
        text.contains(&format!("- [ ] Ship report 📅 {}", day(1))),
        "{text}\n{}",
        app.message
    );
    // The task window works on the result's task too.
    screen(&mut app, 110, 30);
    alt(&mut app, KeyCode::Char('t'));
    let rows = screen(&mut app, 110, 30).join("\n");
    assert!(rows.contains("Ship report"), "{rows}");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Left); // none → medium
    key(&mut app, KeyCode::Enter);
    let text = fs::read_to_string(note(&app, "Work.md")).unwrap();
    assert!(
        text.contains(&format!("- [ ] Ship report 🔼 📅 {}", day(1))),
        "{text}\n{}",
        app.message
    );
    assert_eq!(lines(&app)[0], "top", "the query note is as it was");
}

#[test]
fn the_filter_pane_suggests_properties() {
    let dir = vault("bases-filter-suggest", BOARD);
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Projects.base"));
    alt(&mut app, KeyCode::Char('f'));
    key(&mut app, KeyCode::Char('a'));
    typing(&mut app, "prio");
    let rows = screen(&mut app, 120, 40);
    let pane = rows_between(&rows, "Filters", "add filter");
    assert!(pane.contains("priority"), "suggested: {pane}");
    // Tab takes it; then the value.
    key(&mut app, KeyCode::Tab);
    typing(&mut app, "high");
    key(&mut app, KeyCode::Enter);
    let base = fs::read_to_string(dir.join("Projects.base")).unwrap();
    assert!(base.contains("- priority == \"high\""), "{base}");
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(rows.contains("1 result"), "Garden: {rows}");
}

#[test]
fn a_tables_formula_line_hides_until_the_cursor_is_on_it() {
    let dir = vault(
        "tables-formula-hidden",
        &[
            (
                "T.md",
                "top\n| a | b |\n|---|---|\n| 1 | 2 |\n<!-- TBLFM: $2=$1*2 -->\nend",
            ),
            (
                ".blackglass/plugins.toml",
                "installed = [\"tables\"]\nenabled = [\"tables\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "T.md"));
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(
        !rows.contains("TBLFM") && rows.contains("end"),
        "hidden: {rows}"
    );
    put_cursor(&mut app, 4, 0);
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(
        rows.contains("<!-- TBLFM: $2=$1*2 -->"),
        "at the cursor: {rows}"
    );
}

#[test]
fn task_dates_are_chosen_on_a_calendar() {
    let mut app = tasks_app(
        "tasks-date-picker",
        &[("T.md", "- [ ] Paint the fence 📅 2026-10-01\n")],
    );
    app.open(&note(&app, "T.md"));
    alt(&mut app, KeyCode::Char('t'));
    for _ in 0..4 {
        key(&mut app, KeyCode::Down); // estimate, due
    }
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(
        rows.contains("October 2026") && rows.contains("Mo Tu We"),
        "{rows}"
    );
    // Shift+→ a day, Shift+↓ a week, PgDn a month.
    press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
    press(&mut app, KeyCode::Down, KeyModifiers::SHIFT);
    key(&mut app, KeyCode::PageDown);
    let rows = screen(&mut app, 120, 40);
    let all = rows.join("\n");
    assert!(
        all.contains("2026-11-09") && all.contains("November 2026"),
        "{all}"
    );
    // A click on a day chooses it.
    let (y, x) = rows
        .iter()
        .enumerate()
        .find_map(|(y, r)| r.find(" 20 ").map(|x| (y, r[..x].chars().count() + 1)))
        .expect("the 20th on the calendar");
    click(&mut app, x as u16, y as u16);
    // An empty date: the first key gives today.
    key(&mut app, KeyCode::Down); // scheduled
    press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        lines(&app)[0],
        format!("- [ ] Paint the fence ⏳ {} 📅 2026-11-20", day(0)),
        "{}",
        app.message
    );
}

/// The cells of a table line, trimmed.
fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn formula_vault(name: &str, settings: &str) -> App {
    let dir = vault(
        name,
        &[
            (
                "T.md",
                "| item | qty | price | total |\n|---|---|---|---|\n| a | 2 | 3 | |\n| b | 1 | 5 | |\n<!-- TBLFM: $4=$2*$3 -->\nend",
            ),
            (
                ".blackglass/plugins.toml",
                "installed = [\"tables\"]\nenabled = [\"tables\"]\n",
            ),
            (".blackglass/plugins/tables/settings.toml", settings),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "T.md"));
    app
}

#[test]
fn table_formulas_recalculate_by_themselves() {
    let mut app = formula_vault("tables-auto-formulas", "");
    // Tab in the table: lined up, and the formulas worked out.
    put_cursor(&mut app, 2, 3);
    key(&mut app, KeyCode::Tab);
    assert_eq!(cells(&lines(&app)[2])[3], "6", "{:?}", lines(&app));
    assert_eq!(cells(&lines(&app)[3])[3], "5");
    // A value typed, then the cursor leaves the table: worked out again.
    put_cursor(&mut app, 3, 0);
    key(&mut app, KeyCode::End);
    let qty = lines(&app)[3].find("| 1").unwrap() + 3;
    put_cursor(&mut app, 3, qty);
    typing(&mut app, "0");
    assert_eq!(cells(&lines(&app)[3])[1], "10");
    app.tick();
    put_cursor(&mut app, 5, 0);
    app.tick();
    assert_eq!(cells(&lines(&app)[3])[3], "50", "{:?}", lines(&app));
    assert_eq!(
        (
            app.view().unwrap().editor.row,
            app.view().unwrap().editor.col
        ),
        (5, 0),
        "the cursor stays"
    );
}

#[test]
fn table_formulas_can_wait_for_their_command() {
    let mut app = formula_vault("tables-command-formulas", "formulas = \"on command\"\n");
    put_cursor(&mut app, 2, 3);
    key(&mut app, KeyCode::Tab);
    assert_eq!(cells(&lines(&app)[2])[3], "");
    run_palette(&mut app, "evaluate table formulas");
    assert_eq!(cells(&lines(&app)[2])[3], "6");
}

#[test]
fn table_cells_starting_with_equals_show_their_results() {
    let dir = vault(
        "tables-cell-formulas",
        &[
            (
                "T.md",
                "top\n| item | qty | price | total |\n|---|---|---|---|\n| a | 2 | 1.5 | =B2*C2 |\n| b | 3 | 2 | =B3*C3 |\n| all | =SUM(B2:B3) | | =SUM(D2:D3) |\nend",
            ),
            (
                ".blackglass/plugins.toml",
                "installed = [\"tables\"]\nenabled = [\"tables\"]\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "T.md"));
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(
        rows.contains("│ 3 ") && rows.contains("│ 9 ") && !rows.contains("=SUM"),
        "results: {rows}"
    );
    // In the table: the formulas, to edit.
    put_cursor(&mut app, 3, 0);
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(rows.contains("=B2*C2"), "{rows}");
    // Turned off in the settings: as written.
    let settings = dir.join(".blackglass/plugins/tables/settings.toml");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(&settings, "cell_formulas = \"false\"\n").unwrap();
    app.rescan();
    put_cursor(&mut app, 0, 0);
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(rows.contains("=SUM"), "{rows}");
}

#[test]
fn changes_to_other_notes_are_undone_and_redone() {
    let dir = vault("undo-other-notes", BOARD);
    let mut app = App::new(Vault::open(&dir).unwrap());
    let pond = dir.join("Projects/Pond.md");
    // A card moved on a board (the page has nothing of its own to undo).
    app.open(&note(&app, "Projects.base"));
    key(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
    assert!(fs::read_to_string(&pond).unwrap().contains("status: Done"));
    ctrl(&mut app, 'z');
    let text = fs::read_to_string(&pond).unwrap();
    assert!(text.contains("status: Doing"), "{text}\n{}", app.message);
    assert!(app.message.contains("Undid"), "{}", app.message);
    ctrl(&mut app, 'y');
    assert!(fs::read_to_string(&pond).unwrap().contains("status: Done"));
    // A rename, with the links to it: undone from the palette.
    app.open(&note(&app, "Projects/Shed.md"));
    key(&mut app, KeyCode::F(2));
    ctrl(&mut app, 'u');
    typing(&mut app, "Barn");
    key(&mut app, KeyCode::Enter);
    assert!(dir.join("Projects/Barn.md").is_file(), "{}", app.message);
    run_palette(&mut app, "undo last change to other notes");
    assert!(
        dir.join("Projects/Shed.md").is_file() && !dir.join("Projects/Barn.md").exists(),
        "{}",
        app.message
    );
    // A file changed again since: not undone over it.
    run_palette(&mut app, "redo last change to other notes");
    assert!(dir.join("Projects/Barn.md").is_file());
    fs::write(
        dir.join("Projects/Barn.md"),
        "---\nstatus: Doing\n---\nchanged since\n",
    )
    .unwrap();
    run_palette(&mut app, "undo last change to other notes");
    assert!(app.message.contains("changed since"), "{}", app.message);
    assert!(dir.join("Projects/Barn.md").is_file());
}

fn encrypt_app(name: &str, settings: &str, text: &str) -> App {
    let dir = vault(
        name,
        &[
            ("Note.md", text),
            (
                ".blackglass/plugins.toml",
                "installed = [\"encrypt\"]\nenabled = [\"encrypt\"]\n",
            ),
            (".blackglass/plugins/encrypt/settings.toml", settings),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    app
}

/// Runs the palette's best match for `name`.
fn palette(app: &mut App, name: &str) {
    ctrl(app, 'p');
    typing(app, name);
    key(app, KeyCode::Enter);
}

fn note_line(app: &App, row: usize) -> String {
    app.view().unwrap().editor.lines[row].clone()
}

#[test]
fn a_selection_is_encrypted_and_decrypted_with_a_password() {
    let mut app = encrypt_app("encrypt", "", "My PIN is 1234 ok\n\nend");
    for _ in 0.."My PIN is ".len() {
        key(&mut app, KeyCode::Right);
    }
    for _ in 0..4 {
        press(&mut app, KeyCode::Right, KeyModifiers::SHIFT);
    }
    palette(&mut app, "encrypt selection");
    let rows = screen(&mut app, 100, 24).join("\n");
    for field in ["Password", "Confirm", "Hint", "When reading"] {
        assert!(rows.contains(field), "{field}: {rows}");
    }
    typing(&mut app, "s3cret");
    key(&mut app, KeyCode::Tab);
    typing(&mut app, "s3cret");
    key(&mut app, KeyCode::Tab);
    typing(&mut app, "four digits");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(!rows.contains("s3cret"), "passwords aren't shown: {rows}");
    assert!(rows.contains("••••••"), "{rows}");
    key(&mut app, KeyCode::Enter);
    let encrypted = note_line(&app, 0);
    assert!(
        encrypted.starts_with("My PIN is 🔐β 💡four digits💡") && encrypted.ends_with(" 🔐 ok"),
        "{encrypted}"
    );
    assert!(!encrypted.contains("1234"));
    // Shown as its marker and hint where the cursor isn't.
    key(&mut app, KeyCode::Down);
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("My PIN is 🔐 four digits ok"), "{rows}");
    // Decrypt at the cursor: the password is remembered.
    key(&mut app, KeyCode::Up);
    palette(&mut app, "decrypt");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(
        rows.contains("1234") && rows.contains("Decrypt in place"),
        "{rows}"
    );
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Enter);
    assert_eq!(note_line(&app, 0), "My PIN is 1234 ok");
    // Passwords that differ aren't taken.
    for _ in 0..4 {
        press(&mut app, KeyCode::Left, KeyModifiers::SHIFT);
    }
    palette(&mut app, "encrypt selection");
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    typing(&mut app, "one");
    key(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    typing(&mut app, "two");
    key(&mut app, KeyCode::Enter);
    assert_eq!(note_line(&app, 0), "My PIN is 1234 ok");
    assert!(
        screen(&mut app, 100, 24)
            .join("\n")
            .contains("the passwords don't match"),
    );
}

#[test]
fn meld_encrypts_text_is_decrypted_with_its_hint() {
    // Written by Meld Encrypt's own code.
    let meld = "%%🔐β 💡the usual💡AmAJFj9S7yAiBnXjg5YsQeiWCe1Loe4RrrpM3FAj+AuJDdvpWo7ZYoCcoHr+Arvp7fBGz6qXsPajsihE1ZrDI/Rzjunv8ajIlwGxUw== 🔐%%";
    let mut app = encrypt_app(
        "encrypt-meld",
        "remember_password = \"off\"\n",
        &format!("Key: {meld}\n\nend"),
    );
    for _ in 0..8 {
        key(&mut app, KeyCode::Right);
    }
    palette(&mut app, "decrypt");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("the usual"), "the hint: {rows}");
    typing(&mut app, "wrong");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("Wrong password"), "{rows}");
    palette(&mut app, "decrypt");
    typing(&mut app, "pässword");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(
        rows.contains("Ünïcode secret") && rows.contains("second line"),
        "{rows}"
    );
    // Close leaves it encrypted.
    key(&mut app, KeyCode::Esc);
    assert!(note_line(&app, 0).starts_with("Key: %%🔐β"));
    // Not remembered: asked again.
    palette(&mut app, "decrypt");
    assert!(matches!(app.prompt, Some(Prompt::Ask(_))));
    // Not inside encrypted text.
    key(&mut app, KeyCode::Esc);
    palette(&mut app, "encrypt selection");
    assert!(app.prompt.is_none());
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("on encrypted text"), "{rows}");
    // Nothing selected and no encrypted text at the cursor: the text is
    // typed in the window.
    while app.view().unwrap().editor.row == 0 {
        key(&mut app, KeyCode::Down);
    }
    palette(&mut app, "encrypt selection");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("Text"), "{rows}");
    typing(&mut app, "typed secret");
    for pw in ["pw", "pw"] {
        key(&mut app, KeyCode::Tab);
        typing(&mut app, pw);
    }
    key(&mut app, KeyCode::Enter);
    assert!(
        note_line(&app, 1).starts_with("🔐β "),
        "{}",
        note_line(&app, 1)
    );
}

#[test]
fn task_estimates_are_summed_by_short_queries() {
    let dir = vault(
        "estimates",
        &[
            (
                ".blackglass/plugins.toml",
                "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
            ),
            (
                "Work/Sprint.md",
                "- [ ] one [estimate:: 2h]\n- [ ] two [estimate:: 1.5h]\n- [x] three [estimate:: 30m]\n- [ ] four\n\nLeft: `= sum(filter(this.file.tasks, (t) => !t.completed).estimate)`\nAll: `= sum(this.file.tasks.estimate)`\n\n```dataview\nTABLE WITHOUT ID sum(rows.t.estimate) AS Folder\nFROM \"Work\"\nFLATTEN file.tasks AS t\nWHERE !t.completed\nGROUP BY true\n```\n",
            ),
            ("Work/Other.md", "- [ ] other [estimate:: 4hr 15min]\n"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Work/Sprint.md"));
    let rows = screen(&mut app, 100, 30).join("\n");
    assert!(rows.contains("Left: 3 hours, 30 minutes"), "{rows}");
    assert!(rows.contains("All: 4 hours"), "{rows}");
    assert!(rows.contains("7 hours, 45 minutes"), "the folder: {rows}");
}

fn mouse(app: &mut App, kind: MouseEventKind, x: u16, y: u16) -> Action {
    app.handle_mouse(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

/// Where `text` is on the screen: (column, row).
fn spot(rows: &[String], text: &str) -> (u16, u16) {
    rows.iter()
        .enumerate()
        .find_map(|(y, r)| {
            let i = r.find(text)?;
            // The screen column: wide characters (emoji) take two.
            Some((
                unicode_width::UnicodeWidthStr::width(&r[..i]) as u16,
                y as u16,
            ))
        })
        .unwrap_or_else(|| panic!("{text}: {rows:#?}"))
}

#[test]
fn query_blocks_under_the_mouse_keep_clicks_out_of_their_source() {
    let dir = vault(
        "query-mouse",
        &[
            (
                ".blackglass/plugins.toml",
                "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
            ),
            (
                "Note.md",
                "top\n\n```dataview\nLIST\nFROM \"Books\"\n```\n\nend",
            ),
            ("Books/Dune.md", "spice"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    let rows = screen(&mut app, 100, 24);
    assert!(!rows.join("\n").contains("</>"), "{rows:#?}");
    // The mouse over the block: a button for its source.
    let (x, y) = spot(&rows, "╰─");
    assert!(app.mouse_moved(MouseEvent {
        kind: MouseEventKind::Moved,
        column: x + 2,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(
        !app.mouse_moved(MouseEvent {
            kind: MouseEventKind::Moved,
            column: x + 3,
            row: y,
            modifiers: KeyModifiers::NONE,
        }),
        "the same block: no need to draw"
    );
    let rows = screen(&mut app, 100, 24);
    assert!(rows.join("\n").contains("</>"), "{rows:#?}");
    // A click in the block but not on a result: the source stays hidden.
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 2, y);
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(!rows.contains("FROM"), "{rows}");
    assert_eq!(app.view().unwrap().editor.row, 0);
    // A click on a result's link follows it.
    let rows = screen(&mut app, 100, 24);
    let (x, y) = spot(&rows, "Dune");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    assert!(
        app.view()
            .unwrap()
            .path
            .as_ref()
            .unwrap()
            .ends_with("Dune.md"),
        "the link"
    );
    app.open(&note(&app, "Note.md"));
    // The button: the source.
    let rows = screen(&mut app, 100, 24);
    let (x, y) = spot(&rows, "╰─");
    app.mouse_moved(MouseEvent {
        kind: MouseEventKind::Moved,
        column: x + 2,
        row: y,
        modifiers: KeyModifiers::NONE,
    });
    let rows = screen(&mut app, 100, 24);
    let (x, y) = spot(&rows, "</>");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("FROM \"Books\""), "{rows}");
    assert_eq!(app.view().unwrap().editor.row, 3);
}

#[test]
fn a_dataview_calendars_days_open_their_notes() {
    let dir = vault(
        "calendar-click",
        &[
            (
                ".blackglass/plugins.toml",
                "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
            ),
            (
                "Note.md",
                "top\n\n```dataview\nCALENDAR date\nFROM \"Log\"\n```\n",
            ),
            ("Log/Dentist.md", "---\ndate: 2026-03-04\n---\n"),
            ("Log/Haircut.md", "---\ndate: 2026-03-17\n---\n"),
            ("Log/Barber.md", "---\ndate: 2026-03-17\n---\n"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    let rows = screen(&mut app, 100, 30);
    // The 4th: one note, opened.
    let (x, y) = spot(&rows, " 4•");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    assert!(
        app.view()
            .unwrap()
            .path
            .as_ref()
            .unwrap()
            .ends_with("Dentist.md"),
        "{rows:#?}"
    );
    // The 17th: two, so which one?
    app.open(&note(&app, "Note.md"));
    let rows = screen(&mut app, 100, 30);
    let (x, y) = spot(&rows, "17•");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    let rows = screen(&mut app, 100, 30).join("\n");
    assert!(
        rows.contains("Barber") && rows.contains("Haircut"),
        "{rows}"
    );
    typing(&mut app, "hair");
    key(&mut app, KeyCode::Enter);
    assert!(
        app.view()
            .unwrap()
            .path
            .as_ref()
            .unwrap()
            .ends_with("Haircut.md")
    );
    // A day without notes: create one? No, then yes.
    app.open(&note(&app, "Note.md"));
    let rows = screen(&mut app, 100, 30);
    let (x, y) = spot(&rows, " 5 ");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    let rows = screen(&mut app, 100, 30).join("\n");
    assert!(rows.contains("Create 2026-03-05?"), "{rows}");
    key(&mut app, KeyCode::Esc);
    assert!(
        app.view()
            .unwrap()
            .path
            .as_ref()
            .unwrap()
            .ends_with("Note.md")
    );
    assert!(!dir.join("Log/2026-03-05.md").exists());
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    key(&mut app, KeyCode::Enter);
    assert!(
        app.view()
            .unwrap()
            .path
            .as_ref()
            .unwrap()
            .ends_with("2026-03-05.md"),
        "created and opened"
    );
    // Where new notes go: the folder chosen in the explorer.
    assert!(dir.join("Log/2026-03-05.md").is_file());
}

#[test]
fn a_dataview_calendars_empty_day_makes_its_daily_note() {
    let dir = vault(
        "calendar-daily",
        &[
            (
                ".blackglass/plugins.toml",
                "installed = [\"dataview\", \"periodic-notes\"]\nenabled = [\"dataview\", \"periodic-notes\"]\n",
            ),
            (
                ".blackglass/plugins/periodic-notes/settings.toml",
                "[daily]\nfolder = \"Journal\"\ntemplate = \"Templates/Daily\"\n",
            ),
            ("Templates/Daily.md", "# {{date:YYYY-MM-DD}}\n"),
            (
                "Note.md",
                "top\n\n```dataview\nCALENDAR file.day\nFROM \"Journal\"\n```\n",
            ),
            ("Journal/2026-03-04.md", "a day"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Note.md"));
    let rows = screen(&mut app, 100, 30);
    let (x, y) = spot(&rows, " 9 ");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    let rows = screen(&mut app, 100, 30).join("\n");
    assert!(rows.contains("Create Journal/2026-03-09.md"), "{rows}");
    key(&mut app, KeyCode::Enter);
    let made = dir.join("Journal/2026-03-09.md");
    assert!(made.is_file(), "the daily note, in its folder");
    assert_eq!(
        std::fs::read_to_string(&made).unwrap().trim(),
        "# 2026-03-09",
        "from its template"
    );
}

/// The keys `documentation/terminals.md` suggests where the terminal keeps
/// the mouse's side buttons (they're remapped to Alt+← / Alt+→).
const SIDE_BUTTON_KEYS: &str = "go-back = \"Alt+Left / Ctrl+Alt+Left\"\ngo-forward = \"Alt+Right / Ctrl+Alt+Right\"\nprevious-tab = \"Ctrl+PgUp\"\nnext-tab = \"Ctrl+PgDn\"\n";

#[test]
fn back_and_forward_can_take_the_side_buttons_keys() {
    let (mut app, config) = app_with_config("side-button-keys");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("keys.toml"), SIDE_BUTTON_KEYS).unwrap();
    app.load_user_config();
    assert!(app.message.is_empty(), "{}", app.message);
    let welcome = note(&app, "Welcome.md");
    let dune = note(&app, "Books/Dune.md");
    app.open(&welcome);
    app.open(&dune);
    alt(&mut app, KeyCode::Left);
    assert_eq!(active_path(&app), Some(welcome.clone()), "Alt+← goes back");
    alt(&mut app, KeyCode::Right);
    assert_eq!(active_path(&app), Some(dune.clone()), "Alt+→ goes forward");
    // The tabs keep Ctrl+PgUp / Ctrl+PgDn.
    let before = app.active;
    press(&mut app, KeyCode::PageUp, KeyModifiers::CONTROL);
    assert_ne!(app.active, before, "{}", app.message);
}

#[test]
fn the_task_window_has_an_estimate() {
    let mut app = tasks_app(
        "tasks-estimate",
        &[(
            "T.md",
            "- [ ] Paint the fence [estimate:: 2h] 📅 2026-10-12\n- \n",
        )],
    );
    app.open(&note(&app, "T.md"));
    alt(&mut app, KeyCode::Char('t'));
    let rows = screen(&mut app, 110, 40);
    let at = |name: &str| rows.iter().position(|r| r.contains(name)).unwrap();
    assert!(at("Estimate") < at("Due"), "before Due: {rows:#?}");
    assert!(rows[at("Estimate")].contains("2h"), "filled in: {rows:#?}");
    assert!(
        !rows[at("Description")].contains("estimate::"),
        "not in the description: {rows:#?}"
    );
    for _ in 0..3 {
        key(&mut app, KeyCode::Down); // estimate
    }
    ctrl(&mut app, 'u');
    typing(&mut app, "1h 30m");
    key(&mut app, KeyCode::Enter);
    assert_eq!(
        lines(&app)[0],
        "- [ ] Paint the fence [estimate:: 1h 30m] 📅 2026-10-12",
        "{}",
        app.message
    );
    // A new task with one; none written when it's empty.
    put_cursor(&mut app, 1, 2);
    alt(&mut app, KeyCode::Char('t'));
    typing(&mut app, "Buy paint");
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    typing(&mut app, "45m");
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[1], "- [ ] Buy paint [estimate:: 45m]");
    alt(&mut app, KeyCode::Char('t'));
    for _ in 0..3 {
        key(&mut app, KeyCode::Down);
    }
    ctrl(&mut app, 'u');
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[1], "- [ ] Buy paint");
}

/// A vault with Tasks and Periodic Notes (daily notes in Journal/, from a
/// template with a Tasks heading).
fn quick_task_app(name: &str) -> (App, PathBuf) {
    let dir = vault(
        name,
        &[
            (
                STATE_FILE,
                "installed = [\"tasks\", \"periodic-notes\"]\nenabled = [\"tasks\", \"periodic-notes\"]\n",
            ),
            (
                ".blackglass/plugins/periodic-notes/settings.toml",
                "[daily]\nfolder = \"Journal\"\ntemplate = \"Templates/Daily\"\n",
            ),
            (
                "Templates/Daily.md",
                "# Today\n\n## Tasks\n\n## Notes\n- a note\n",
            ),
        ],
    );
    (App::new(Vault::open(&dir).unwrap()), dir)
}

#[test]
fn with_no_note_open_tasks_go_to_todays_daily_note() {
    let (mut app, dir) = quick_task_app("quick-task");
    let daily = dir.join(format!("Journal/{}.md", day(0)));
    // The start screen offers both.
    let rows = screen(&mut app, 110, 30);
    let at = |name: &str| {
        rows.iter()
            .position(|r| r.contains(name))
            .unwrap_or_else(|| panic!("{name}: {rows:#?}"))
    };
    assert!(rows[at("Add quick task")].contains("Ctrl+T"), "{rows:#?}");
    assert!(
        rows[at("Add task (task window)")].contains("Alt+T"),
        "{rows:#?}"
    );
    assert!(at("Add quick task") < at("Add task (task window)"));
    // Ctrl+T: one line, into today's note (made from its template).
    ctrl(&mut app, 't');
    typing(&mut app, "Call the plumber");
    key(&mut app, KeyCode::Enter);
    let text = fs::read_to_string(&daily).unwrap_or_else(|e| panic!("{e}: {}", app.message));
    assert_eq!(
        text, "# Today\n\n## Tasks\n- [ ] Call the plumber\n\n## Notes\n- a note\n",
        "under its Tasks heading"
    );
    // Alt+T: the task window, the next task after it.
    app.close(app.active);
    while !app.tabs.is_empty() {
        app.close(0);
    }
    alt(&mut app, KeyCode::Char('t'));
    typing(&mut app, "Order paint");
    for _ in 0..3 {
        key(&mut app, KeyCode::Down); // estimate
    }
    typing(&mut app, "30m");
    key(&mut app, KeyCode::Enter);
    let text = fs::read_to_string(&daily).unwrap();
    assert!(
        text.contains(
            "## Tasks\n- [ ] Call the plumber\n- [ ] Order paint [estimate:: 30m]\n\n## Notes"
        ),
        "{text}"
    );
    // With a note open, Ctrl+T still makes the line a task.
    app.open(&daily);
    let row = lines(&app).iter().position(|l| l == "- a note").unwrap();
    put_cursor(&mut app, row, 0);
    ctrl(&mut app, 't');
    assert_eq!(lines(&app)[row], "- [ ] a note");
}

#[test]
fn the_start_screen_shows_how_to_move_around() {
    let mut app = app("start-moving");
    let rows = screen(&mut app, 110, 40);
    let at = |name: &str| {
        rows.iter()
            .position(|r| r.contains(name))
            .unwrap_or_else(|| panic!("{name}: {rows:#?}"))
    };
    let title = format!("blackglass v{}", env!("CARGO_PKG_VERSION"));
    assert!(at(&title) < at("No file is open"), "the title on top");
    assert!(at("Moving around") > at("Quit"), "a section of its own");
    let settings = at("Settings");
    assert!(rows[settings].contains("Alt+, / Ctrl+,"), "{rows:#?}");
    assert!(settings < at("Help"), "with the commands: {rows:#?}");
    for (what, keys) in [
        ("Sidebar / note", "Ctrl+B"),
        ("Next / previous sidebar tab", "Tab / Shift+Tab"),
        ("Choose in the sidebar", "↑↓ Enter"),
        ("Back to the note", "Esc"),
        ("Previous / next note tab", "Alt+Left / Alt+Right"),
        ("Back / forward", "Ctrl+Alt+Left / Ctrl+Alt+Right"),
        ("Hide / show the sidebar", "Alt+B"),
        ("Note mode: preview, source, view", "Alt+V"),
    ] {
        assert!(rows[at(what)].contains(keys), "{what} {keys}: {rows:#?}");
    }
    // A short terminal: the two sections side by side, nothing cut off.
    let rows = screen(&mut app, 140, 18);
    let title = rows.iter().find(|r| r.contains("No file is open")).unwrap();
    assert!(title.contains("Moving around"), "{rows:#?}");
    assert!(rows.iter().any(|r| r.contains("All commands")), "{rows:#?}");
}

#[test]
fn a_task_date_is_saved_as_the_calendar_shows_it() {
    let mut app = tasks_app("tasks-date-words", &[("T.md", "- [ ] Paint the shed\n")]);
    app.open(&note(&app, "T.md"));
    alt(&mut app, KeyCode::Char('t'));
    for _ in 0..4 {
        key(&mut app, KeyCode::Down); // estimate, due
    }
    typing(&mut app, "next friday");
    let now = chrono::Local::now().naive_local();
    let shown =
        blackglass::nldates::parse_date("next friday", now, chrono::Weekday::Mon).expect("a day");
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[0], format!("- [ ] Paint the shed 📅 {shown}"));
}

// Folder permissions: Unix's (a read-only folder on Windows still takes files).
#[cfg(unix)]
#[test]
fn a_read_only_vault_says_so() {
    use std::os::unix::fs::PermissionsExt;
    let dir = vault("read-only", &[("Note.md", "# hi")]);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let app = App::new(Vault::open(&dir).unwrap());
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(app.message.contains("read-only"), "{}", app.message);
    let writable = App::new(Vault::open(&dir).unwrap());
    assert!(
        !writable.message.contains("read-only"),
        "{}",
        writable.message
    );
}

#[test]
fn a_tasks_result_checks_on_its_box_and_opens_on_its_backlink() {
    let mut app = tasks_app(
        "tasks-box-click",
        &[
            ("T.md", "intro\n- [ ] Paint the shed\n"),
            ("Q.md", "top\n\n```tasks\nnot done\n```\n"),
        ],
    );
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 110, 24);
    let (x, y) = spot(&rows, "☐ Paint the shed");
    // The description: nothing (the query stays, the task too).
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 6, y);
    assert_eq!(
        fs::read_to_string(app.vault.root.join("T.md")).unwrap(),
        "intro\n- [ ] Paint the shed\n"
    );
    assert_eq!(app.view().unwrap().editor.row, 0, "the query stays shown");
    // The backlink: its note, at the task.
    let rows = screen(&mut app, 110, 24);
    let (bx, by) = spot(&rows, "(T)");
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        bx + 1,
        by,
    );
    assert!(app.view().unwrap().path.as_ref().unwrap().ends_with("T.md"));
    assert_eq!(app.view().unwrap().editor.row, 1, "at the task");
    // Back in the query: the box checks it.
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 110, 24);
    let (x, y) = spot(&rows, "☐ Paint the shed");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    let text = fs::read_to_string(app.vault.root.join("T.md")).unwrap();
    assert!(text.contains("- [x] Paint the shed"), "{text}");
}

#[test]
fn a_dataview_task_checks_on_its_box_only() {
    let dir = vault(
        "dataview-box-click",
        &[
            (
                ".blackglass/plugins.toml",
                "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
            ),
            ("T.md", "- [ ] Paint the shed\n"),
            ("Q.md", "top\n\n```dataview\nTASK FROM \"T\"\n```\n"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 100, 20);
    let (x, y) = spot(&rows, "☐ Paint the shed");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 6, y);
    assert_eq!(
        fs::read_to_string(dir.join("T.md")).unwrap(),
        "- [ ] Paint the shed\n",
        "the text: nothing"
    );
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    let text = fs::read_to_string(dir.join("T.md")).unwrap();
    assert!(text.starts_with("- [x] Paint the shed"), "the box: {text}");
}

#[test]
fn tasks_queries_filter_sort_and_group_by_function() {
    let tasks = "- [ ] Tiny\n- [ ] Short one 📅 2026-10-10 #home\n- [ ] A much longer description 📅 2026-10-20 #home\n- [ ] Middle-sized task #work\n";
    let query = "```tasks\nfilter by function task.description.length > 5\nsort by function reverse task.description.length\ngroup by function task.tags.length ? task.tags[0] : 'No tag'\nhide urgency\n```\n\n```tasks\nfilter by function task.due.moment?.isBefore(moment('2026-10-15'), 'day')\nhide urgency\n```\n\n```tasks\nfilter by function nope(\n```\n";
    let mut app = tasks_app(
        "tasks-functions",
        &[("T.md", tasks), ("Q.md", &format!("top\n\n{query}"))],
    );
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 120, 40);
    let at = |text: &str| {
        rows.iter()
            .position(|r| r.contains(text))
            .unwrap_or_else(|| panic!("{text}: {rows:#?}"))
    };
    // Grouped by the first tag; longest first in each group; Tiny is out.
    assert!(at("#home") < at("A much longer description"), "{rows:#?}");
    assert!(
        at("A much longer description") < at("Short one"),
        "{rows:#?}"
    );
    assert!(at("#work") > at("Short one") && at("#work") < at("Middle-sized task"));
    assert!(
        !rows[..at("#work") + 3].iter().any(|r| r.contains("Tiny")),
        "{rows:#?}"
    );
    // Dates as moments: only the task due before the 15th.
    let second = &rows[at("3 tasks") + 1..];
    let due_before: Vec<&String> = second
        .iter()
        .take_while(|r| !r.contains("1 task"))
        .filter(|r| r.contains('☐'))
        .collect();
    assert_eq!(due_before.len(), 1, "{rows:#?}");
    assert!(due_before[0].contains("Short one"), "{rows:#?}");
    // A broken function says so.
    let error = rows
        .iter()
        .find(|r| r.contains("Tasks query:"))
        .expect("an error");
    assert!(
        error.contains("function") && !error.contains("line 6"),
        "{error}"
    );
}

const GROUPED_TABLE: &str = "top\n```base\nfilters: type == \"book\"\nviews:\n  - type: table\n    name: Shelf\n    order: [file.name, rating]\n    groupBy:\n      property: status\n      direction: ASC\n    groupOrder:\n      - reading\n      - read\n```\nend\n";

fn block_text(app: &App) -> String {
    app.view().unwrap().editor.to_text()
}

#[test]
fn base_groups_follow_their_order_collapse_and_change_by_command() {
    let mut app = bases_app("bases-groups", &[("G.md", GROUPED_TABLE)]);
    app.open(&note(&app, "G.md"));
    let rows = screen(&mut app, 100, 30);
    let at = |rows: &[String], t: &str| rows.iter().position(|r| r.contains(t));
    // groupOrder: reading first; every group in it shown.
    assert!(
        at(&rows, "reading (1)").unwrap() < at(&rows, "read (2)").unwrap(),
        "{rows:#?}"
    );
    // A click on a group's heading collapses it, again expands it.
    let (x, y) = spot(&rows, "read (2)");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    let rows = screen(&mut app, 100, 30);
    assert!(at(&rows, "▸ read (2)").is_some(), "{rows:#?}");
    assert!(at(&rows, "Hobbit").is_none(), "its rows hidden: {rows:#?}");
    assert_eq!(app.view().unwrap().editor.row, 0, "the block stays shown");
    let (x, y) = spot(&rows, "▸ read (2)");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 3, y);
    let rows = screen(&mut app, 100, 30);
    assert!(at(&rows, "Hobbit").is_some(), "{rows:#?}");
    // The Group menu, as commands with the cursor in the block.
    put_cursor(&mut app, 3, 0);
    run_palette(&mut app, "bases add a group");
    typing(&mut app, "wishlist");
    key(&mut app, KeyCode::Enter);
    assert!(
        block_text(&app).contains("      - wishlist"),
        "{}",
        block_text(&app)
    );
    run_palette(&mut app, "bases show or hide a group");
    typing(&mut app, "reading");
    key(&mut app, KeyCode::Enter);
    let text = block_text(&app);
    assert!(!text.contains("      - reading"), "hidden: {text}");
    run_palette(&mut app, "bases reorder groups");
    typing(&mut app, "wishlist");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "first");
    key(&mut app, KeyCode::Enter);
    let text = block_text(&app);
    let wish = text.find("      - wishlist").unwrap();
    let read = text.find("      - read\n").unwrap();
    assert!(wish < read, "wishlist first: {text}");
}

#[test]
fn a_board_in_a_note_moves_cards_by_click() {
    let mut app = bases_app(
        "bases-board-click",
        &[(
            "K.md",
            "top\n```base\nfilters: type == \"book\"\nviews:\n  - type: kanban\n    groupBy: status\n    order: [file.name]\n```\nend\n",
        )],
    );
    app.open(&note(&app, "K.md"));
    let rows = screen(&mut app, 100, 30);
    let (x, y) = spot(&rows, "Emma");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x + 1, y);
    let rows = screen(&mut app, 100, 30).join("\n");
    assert!(
        rows.contains("Open Emma") && rows.contains("Move to read"),
        "{rows}"
    );
    typing(&mut app, "move to read");
    key(&mut app, KeyCode::Enter);
    let emma = fs::read_to_string(app.vault.root.join("Books/Emma.md")).unwrap();
    assert!(emma.contains("status: read\n"), "{emma}");
}

#[test]
fn a_checked_task_stays_in_its_query_for_a_moment() {
    let mut app = tasks_app(
        "tasks-grace",
        &[
            ("T.md", "- [ ] Paint the shed\n- [ ] Buy paint\n"),
            ("Q.md", "top\n\n```tasks\nnot done\nhide urgency\n```\n"),
        ],
    );
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 100, 20);
    let (x, y) = spot(&rows, "☐ Paint the shed");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    app.tick();
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(
        rows.contains("☑ Paint the shed"),
        "shown checked, not gone: {rows}"
    );
    let path = app.vault.root.join("T.md");
    let file = || fs::read_to_string(&path).unwrap();
    assert!(file().starts_with("- [x] Paint the shed"), "{}", file());
    // Clicked again in time: unchecked.
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    app.tick();
    assert!(file().starts_with("- [ ] Paint the shed"), "{}", file());
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(rows.contains("☐ Paint the shed"), "{rows}");
    // Checked, then after a few seconds the query is drawn anew.
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    std::thread::sleep(std::time::Duration::from_millis(3700));
    app.tick();
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(!rows.contains("Paint the shed"), "gone now: {rows}");
    assert!(rows.contains("☐ Buy paint"), "{rows}");
}

#[test]
fn a_dataview_checked_task_stays_for_a_moment() {
    let dir = vault(
        "dataview-grace",
        &[
            (
                ".blackglass/plugins.toml",
                "installed = [\"dataview\"]\nenabled = [\"dataview\"]\n",
            ),
            ("T.md", "- [ ] Paint the shed\n"),
            (
                "Q.md",
                "top\n\n```dataview\nTASK FROM \"T\"\nWHERE !completed\n```\n",
            ),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 100, 20);
    let (x, y) = spot(&rows, "☐ Paint the shed");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    app.tick();
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(rows.contains("☑ Paint the shed"), "shown checked: {rows}");
    assert!(
        fs::read_to_string(dir.join("T.md"))
            .unwrap()
            .starts_with("- [x]")
    );
}

#[test]
fn checked_tasks_can_go_at_once() {
    let mut app = tasks_app(
        "tasks-grace-off",
        &[
            ("T.md", "- [ ] Paint the shed\n- [ ] Buy paint\n"),
            ("Q.md", "top\n\n```tasks\nnot done\nhide urgency\n```\n"),
            (
                ".blackglass/plugins/tasks/settings.toml",
                "keep_checked = \"0\"\n",
            ),
        ],
    );
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 100, 20);
    let (x, y) = spot(&rows, "☐ Paint the shed");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    app.tick();
    let rows = screen(&mut app, 100, 20).join("\n");
    assert!(!rows.contains("Paint the shed"), "gone at once: {rows}");
}

#[test]
fn task_results_have_edit_and_postpone_buttons() {
    let mut app = tasks_app(
        "tasks-buttons",
        &[
            ("T.md", "- [ ] Paint the shed 📅 2026-10-20\n"),
            (
                "Q.md",
                "top\n\n```tasks\nnot done\nhide urgency\n```\n\n```tasks\nnot done\nhide urgency\nhide edit button\nhide postpone button\n```\n",
            ),
        ],
    );
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 110, 24);
    let shed: Vec<&String> = rows
        .iter()
        .filter(|r| r.contains("Paint the shed"))
        .collect();
    assert_eq!(shed.len(), 2, "{rows:#?}");
    assert!(
        shed[0].contains('✎') && shed[0].contains('⇥'),
        "{}",
        shed[0]
    );
    assert!(
        !shed[1].contains('✎') && !shed[1].contains('⇥'),
        "hidden: {}",
        shed[1]
    );
    // ✎: the task window, for that task.
    let (x, y) = spot(&rows, "✎");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    let screen_now = screen(&mut app, 110, 24).join("\n");
    assert!(
        screen_now.contains("Edit task") && screen_now.contains("2026-10-20"),
        "{screen_now}"
    );
    key(&mut app, KeyCode::Esc);
    // ⇥: postpone it.
    let rows = screen(&mut app, 110, 24);
    let (x, y) = spot(&rows, "⇥");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    typing(&mut app, "1 day");
    key(&mut app, KeyCode::Enter);
    let text = fs::read_to_string(app.vault.root.join("T.md")).unwrap();
    assert!(text.contains("📅 2026-10-21"), "{text}");
}

#[test]
fn task_fields_in_a_note_are_muted_chips() {
    let mut app = tasks_app(
        "tasks-chips",
        &[(
            "T.md",
            "top\n\n- [ ] Pay rent ⏫ 🔁 every month 📅 2026-10-01 #home\n",
        )],
    );
    app.open(&note(&app, "T.md"));
    let rows = screen(&mut app, 100, 12);
    let (x, y) = spot(&rows, "Pay rent");
    let text = bg_at(&mut app, 100, 12, x, y);
    for field in ["⏫", "🔁 every", "📅 2026", "2026-10-01"] {
        let (x, y) = spot(&rows, field);
        assert_ne!(bg_at(&mut app, 100, 12, x, y), text, "{field}: {rows:#?}");
    }
    let (x, y) = spot(&rows, "#home");
    let tag = bg_at(&mut app, 100, 12, x, y);
    let (x, y) = spot(&rows, "📅");
    assert_ne!(
        tag,
        bg_at(&mut app, 100, 12, x, y),
        "the tag isn't the date's"
    );
    assert!(rows[y as usize].contains("Pay rent ⏫ 🔁 every month 📅 2026-10-01"));
}

#[test]
fn a_quickadd_capture_asks_and_adds_a_line_to_todays_note() {
    let today = chrono::Local::now().date_naive();
    let day = today.format("%Y-%m-%d").to_string();
    let year = today.format("%Y").to_string();
    let dir = vault(
        "quickadd-capture",
        &[
            (
                STATE_FILE,
                "installed = [\"quickadd\"]\nenabled = [\"quickadd\"]\n",
            ),
            (
                ".blackglass/plugins/quickadd/settings.toml",
                "[Expense]\nformat = \"- [[{{NOTE:Budget/Envelopes}}]] {{VALUE:amount}} {{VALUE:what}}\"\nnote = \"\"\nheading = \"Money\"\n\n[Log]\nformat = \"- {{DATE:YYYY-MM-DD}} {{VALUE:size,small,big}} {{VALUE}}\"\nnote = \"Logs/{{DATE:YYYY}}.md\"\nheading = \"\"\n",
            ),
            ("Budget/Envelopes/Groceries.md", "monthly: 400"),
            ("Budget/Envelopes/Fun.md", "monthly: 100"),
            (&format!("{day}.md"), "# Today\n\n## Logs\n- woke up\n"),
        ],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    // Each capture is a command; its questions come one after the other.
    run_palette(&mut app, "quickadd expense");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("Groceries") && rows.contains("Fun"), "{rows}");
    typing(&mut app, "gro");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "12.50");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "milk");
    key(&mut app, KeyCode::Enter);
    let text = fs::read_to_string(dir.join(format!("{day}.md"))).unwrap();
    assert_eq!(
        text, "# Today\n\n## Logs\n- woke up\n\n## Money\n- [[Groceries]] 12.50 milk\n",
        "the heading is made"
    );
    // Again: under the heading, after its list.
    run_palette(&mut app, "quickadd expense");
    typing(&mut app, "fun");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "28");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "cinema");
    key(&mut app, KeyCode::Enter);
    let text = fs::read_to_string(dir.join(format!("{day}.md"))).unwrap();
    assert!(
        text.ends_with("## Money\n- [[Groceries]] 12.50 milk\n- [[Fun]] 28 cinema\n"),
        "{text}"
    );
    // "Run QuickAdd" lists the captures; a note by path is made.
    run_palette(&mut app, "quickadd run quickadd");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("Expense") && rows.contains("Log"), "{rows}");
    typing(&mut app, "log");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "big");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "a long walk");
    key(&mut app, KeyCode::Enter);
    let log = fs::read_to_string(dir.join(format!("Logs/{year}.md"))).unwrap();
    assert_eq!(log, format!("- {day} big a long walk\n"));
}

#[test]
fn task_dependencies_are_chosen_from_the_vaults_tasks() {
    let mut app = tasks_app(
        "tasks-depends-pick",
        &[
            (
                "T.md",
                "- [ ] Book the venue\n- [ ] Send invitations\n- [ ] Print flyers 🆔 flyers\n",
            ),
            ("Other.md", "- [ ] Order the cake\n"),
        ],
    );
    app.open(&note(&app, "T.md"));
    put_cursor(&mut app, 1, 0);
    alt(&mut app, KeyCode::Char('t'));
    for _ in 0..12 {
        key(&mut app, KeyCode::Down); // blocked by
    }
    typing(&mut app, "venue");
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(
        rows.contains("Blocked by") && rows.contains("Blocks"),
        "{rows}"
    );
    assert!(rows.contains("Book the venue"), "a match: {rows}");
    key(&mut app, KeyCode::Enter); // chosen, the window stays
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(rows.contains("Edit task"), "{rows}");
    typing(&mut app, "cake");
    key(&mut app, KeyCode::Enter); // a task in another note
    key(&mut app, KeyCode::Down); // blocks
    typing(&mut app, "flyers");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter); // saved
    let l = lines(&app);
    let id_of = |line: &str| {
        line.split("🆔 ")
            .nth(1)
            .map(|r| r.split_whitespace().next().unwrap().to_string())
            .unwrap_or_else(|| panic!("an id in {line:?}"))
    };
    let venue = id_of(&l[0]);
    let this = id_of(&l[1]);
    assert_ne!(venue, this);
    let cake_line = fs::read_to_string(note(&app, "Other.md")).unwrap();
    let cake = id_of(&cake_line);
    assert!(
        l[1].contains(&format!("⛔ {venue},{cake}")),
        "waits for both: {:?}",
        l[1]
    );
    assert_eq!(l[2], format!("- [ ] Print flyers 🆔 flyers ⛔ {this}"));
    // Edited again: what it waits for and blocks is shown, and taken off.
    put_cursor(&mut app, 1, 0);
    alt(&mut app, KeyCode::Char('t'));
    let rows = screen(&mut app, 120, 40).join("\n");
    assert!(rows.contains("Print flyers"), "blocks: {rows}");
    for _ in 0..13 {
        key(&mut app, KeyCode::Down); // blocks
    }
    key(&mut app, KeyCode::Backspace); // flyers off
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[2], "- [ ] Print flyers 🆔 flyers");
}

#[test]
fn a_tasks_toolbar_filters_and_copies_the_results() {
    let mut app = tasks_app(
        "tasks-toolbar",
        &[
            (
                "Shop.md",
                "- [ ] Buy milk\n- [ ] Buy bread #food\n  - [ ] Wholemeal\n",
            ),
            ("Work.md", "- [ ] Write the report\n"),
            (
                "Q.md",
                "top\n\n```tasks\nnot done\ngroup by filename\nhide edit button\nhide postpone button\n```\n\n```tasks\nnot done\nhide toolbar\n```\n",
            ),
        ],
    );
    let copied = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let seen = std::rc::Rc::clone(&copied);
    app.copier = Box::new(move |text| {
        *seen.borrow_mut() = text.to_string();
        Ok(())
    });
    app.open(&note(&app, "Q.md"));
    let rows = screen(&mut app, 110, 30);
    let toolbars = rows.iter().filter(|r| r.contains("Copy results")).count();
    assert_eq!(toolbars, 1, "hide toolbar: {rows:#?}");
    // Copy: the headings and the task lines as Markdown, no count.
    let (x, y) = spot(&rows, "Copy results");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    assert_eq!(
        *copied.borrow(),
        "#### Shop\n- [ ] Buy milk\n- [ ] Buy bread #food\n- [ ] Wholemeal\n\n#### Work\n- [ ] Write the report\n"
    );
    // Filter: by description, the query unchanged.
    let (x, y) = spot(&rows, "Filter results");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    typing(&mut app, "BREAD");
    key(&mut app, KeyCode::Enter);
    let rows = screen(&mut app, 110, 30);
    let text = rows.join("\n");
    assert!(
        text.contains("Buy bread") && text.contains("Filter: BREAD"),
        "{text}"
    );
    let first = rows
        .iter()
        .position(|r| r.contains("Copy results"))
        .unwrap();
    let second: Vec<&String> = rows[first..]
        .iter()
        .take_while(|r| !r.contains("tasks"))
        .collect();
    assert!(
        !second
            .iter()
            .any(|r| r.contains("Buy milk") || r.contains("Write the report")),
        "{text}"
    );
    assert!(
        fs::read_to_string(note(&app, "Q.md"))
            .unwrap()
            .contains("group by filename\nhide"),
        "the query is as it was"
    );
    // Cleared again.
    let (x, y) = spot(&rows, "Filter: BREAD");
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    ctrl(&mut app, 'u');
    key(&mut app, KeyCode::Enter);
    let text = screen(&mut app, 110, 30).join("\n");
    assert!(
        text.contains("Write the report") && text.contains("Filter results"),
        "{text}"
    );
}

#[test]
fn math_shows_as_unicode() {
    let dir = vault(
        "math-unicode",
        &[(
            "M.md",
            "top\n\nEnergy $e = mc^2$, prices $5 and $10.\n\n$$\n\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}\n$$\n",
        )],
    );
    let mut app = App::new(Vault::open(&dir).unwrap());
    app.open(&dir.join("M.md"));
    let text = screen(&mut app, 100, 20).join("\n");
    assert!(
        text.contains("Energy e = mc², prices $5 and $10."),
        "{text}"
    );
    assert!(text.contains("∑ᵢ₌₁ⁿ i = (n(n+1))/2"), "{text}");
    // The cursor in the block: its LaTeX.
    put_cursor(&mut app, 5, 0);
    let text = screen(&mut app, 100, 20).join("\n");
    assert!(text.contains("\\frac{n(n+1)}{2}"), "{text}");
}

fn quickadd_app(name: &str, settings: &str, files: &[(&str, &str)]) -> (App, PathBuf) {
    let mut all = files.to_vec();
    all.push((
        STATE_FILE,
        "installed = [\"quickadd\"]\nenabled = [\"quickadd\"]\n",
    ));
    all.push((".blackglass/plugins/quickadd/settings.toml", settings));
    let dir = vault(name, &all);
    (App::new(Vault::open(&dir).unwrap()), dir)
}

#[test]
fn quickadd_captures_go_where_they_are_told() {
    let (mut app, dir) = quickadd_app(
        "quickadd-places",
        "[Here]\nformat = \"({{VALUE:what}}{{CURSOR}})\"\nactive = \"true\"\nplace = \"cursor\"\n\n\
         [Below]\nformat = \"- {{VALUE:a,b|custom}} for {{LINKSECTION}}\"\nactive = \"true\"\nplace = \"line below cursor\"\n\n\
         [Todo]\nformat = \"{{VALUE}}\"\nnote = \"Projects/\"\nheading = \"Inbox\"\nfirst = \"true\"\nheading_missing = \"top\"\ntask = \"true\"\nper_line = \"true\"\nlink = \"true\"\n\n\
         [Status]\nformat = \"status: {{FIELD:status}}\"\nnote = \"Log\"\nplace = \"top\"\ncreate = \"false\"\n",
        &[
            ("N.md", "# Plan\nfirst line\nsecond line\n"),
            (
                "Projects/Alpha.md",
                "---\nstatus: open\n---\n# Alpha\n## Inbox\n- [ ] old\n",
            ),
            ("Projects/Beta.md", "---\nstatus: done\n---\nIntro\n"),
        ],
    );
    let n = dir.join("N.md");
    app.open(&n);
    // At the cursor: {{CURSOR}} leaves it inside the brackets.
    put_cursor(&mut app, 1, 5);
    run_palette(&mut app, "quickadd here");
    typing(&mut app, "x");
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[1], "first(x) line");
    assert_eq!(cursor(&app), (1, 7), "before the closing bracket");
    // A new line below the cursor; a suggestion takes new text too.
    run_palette(&mut app, "quickadd below");
    typing(&mut app, "zzz");
    key(&mut app, KeyCode::Enter);
    assert_eq!(lines(&app)[2], "- zzz for [[N#Plan]]");
    // A note chosen from a folder; under its heading, newest first; one
    // task a line; a link to it where the cursor was.
    put_cursor(&mut app, 3, 0);
    run_palette(&mut app, "quickadd todo");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(
        rows.contains("Projects/Alpha") && rows.contains("Projects/Beta"),
        "{rows}"
    );
    typing(&mut app, "beta");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "one");
    press(&mut app, KeyCode::Enter, KeyModifiers::ALT);
    typing(&mut app, "two");
    key(&mut app, KeyCode::Enter);
    let beta = fs::read_to_string(dir.join("Projects/Beta.md")).unwrap();
    assert_eq!(
        beta, "---\nstatus: done\n---\n## Inbox\n- [ ] one\n- [ ] two\n\nIntro\n",
        "the heading made at the top"
    );
    assert!(lines(&app)[3].starts_with("[[Beta]]"), "{:?}", lines(&app));
    run_palette(&mut app, "quickadd todo");
    typing(&mut app, "alpha");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "new");
    key(&mut app, KeyCode::Enter);
    let alpha = fs::read_to_string(dir.join("Projects/Alpha.md")).unwrap();
    assert!(
        alpha.contains("## Inbox\n- [ ] new\n- [ ] old\n"),
        "first: {alpha}"
    );
    // A property's values; a note that isn't there isn't made.
    run_palette(&mut app, "quickadd status");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("done") && rows.contains("open"), "{rows}");
    key(&mut app, KeyCode::Enter);
    assert!(!dir.join("Log.md").exists());
    assert!(app.message.contains("isn't there"), "{}", app.message);
}

#[test]
fn quickadd_templates_groups_and_days() {
    let (mut app, dir) = quickadd_app(
        "quickadd-templates",
        "[Meeting]\ntype = \"template\"\ntemplate = \"Templates/Meeting\"\nfile_name = \"{{VALUE:topic|case:title}}\"\nfolder = \"Meetings/{{DATE:YYYY}}\"\ngroup = \"Work\"\n\n\
         [Diary]\nformat = \"- {{DATE:YYYY-MM-DD}}: {{VALUE}}\"\nheading = \"Diary\"\nday = \"ask\"\n\n\
         [Global variables]\nfooter = \"made {{DATE:YYYY}}\"\n",
        &[(
            "Templates/Meeting.md",
            "# {{TITLE}}\nWhen: {{DATE}}\nWith: {{VALUE:who}}\n{{GLOBAL_VAR:footer}}",
        )],
    );
    let today = chrono::Local::now().date_naive();
    let year = today.format("%Y").to_string();
    // A group, then its template choice; its tokens asked too.
    run_palette(&mut app, "quickadd run quickadd");
    let rows = screen(&mut app, 100, 24).join("\n");
    assert!(rows.contains("Work ▸") && rows.contains("Diary"), "{rows}");
    typing(&mut app, "work");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter); // Meeting
    typing(&mut app, "budget review");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "Ada");
    key(&mut app, KeyCode::Enter);
    let made = dir.join(format!("Meetings/{year}/Budget Review.md"));
    let text = fs::read_to_string(&made).unwrap();
    assert_eq!(
        text,
        format!("# Budget Review\nWhen: {today}\nWith: Ada\nmade {year}")
    );
    assert_eq!(active_path(&app).as_deref(), Some(made.as_path()), "opened");
    // Again: a number, the name being taken.
    run_palette(&mut app, "quickadd meeting");
    typing(&mut app, "budget review");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter);
    assert!(
        dir.join(format!("Meetings/{year}/Budget Review 1.md"))
            .is_file()
    );
    // Which day: asked first; the dates and the daily note are its.
    run_palette(&mut app, "quickadd diary");
    typing(&mut app, "yesterday");
    key(&mut app, KeyCode::Enter);
    typing(&mut app, "rain");
    key(&mut app, KeyCode::Enter);
    let yesterday = today.pred_opt().unwrap();
    let note = fs::read_to_string(dir.join(format!("{yesterday}.md"))).unwrap();
    assert_eq!(note, format!("## Diary\n- {yesterday}: rain\n"));
}

#[test]
fn the_window_has_its_own_settings_page() {
    let (mut app, config) = app_with_config("window-settings");
    alt(&mut app, KeyCode::Char(','));
    let rows = screen(&mut app, 110, 32);
    assert!(
        find(&rows, "Window").is_none(),
        "only in the window: {rows:#?}"
    );
    key(&mut app, KeyCode::Esc);
    app.windowed = true;
    alt(&mut app, KeyCode::Char(','));
    typing(&mut app, "font size");
    let rows = screen(&mut app, 110, 32);
    assert!(find(&rows, "Font size").is_some(), "{rows:#?}");
    key(&mut app, KeyCode::Enter); // into the page
    key(&mut app, KeyCode::Enter); // edit its row
    ctrl(&mut app, 'u');
    typing(&mut app, "18");
    key(&mut app, KeyCode::Enter);
    let saved = fs::read_to_string(config.join("window.toml")).unwrap();
    assert!(saved.contains("size = \"18\""), "{saved}");
    assert_eq!(app.window_settings().size, 18.0);
}

#[test]
fn notes_with_windows_line_ends_keep_them() {
    // `\r\n` notes (written on Windows): read as any other, changed in
    // place, saved with their own line ends.
    let mut app = dv_app(
        "crlf",
        "",
        &[
            ("Q.md", "top\n```dataview\nTASK FROM \"Todo\"\n```"),
            ("Todo.md", "- [ ] one\r\n- [x] two\r\n"),
        ],
    );
    app.open(&note(&app, "Q.md"));
    view_mode(&mut app);
    screen(&mut app, 100, 20);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Enter);
    let todo = note(&app, "Todo.md");
    assert_eq!(
        fs::read_to_string(&todo).unwrap(),
        "- [x] one\r\n- [x] two\r\n",
        "{}",
        app.message
    );
    // Open, a plugin sees its lines without `\r` (Tasks' toggle).
    app.open(&todo);
    put_cursor(&mut app, 1, 0);
    let text = app.with_context(|ctx| ctx.note.unwrap().text.to_string());
    assert!(!text.contains('\r'), "{text:?}");
    ctrl(&mut app, 's');
    assert_eq!(
        fs::read_to_string(&todo).unwrap(),
        "- [x] one\r\n- [x] two\r\n",
        "saved with its own line ends"
    );
}
