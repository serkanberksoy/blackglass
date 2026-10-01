//! Draws the workspace like Obsidian: the sidebar on the left (file
//! explorer, search, tags); on the right the tab bar, the note's folder
//! path, its name as a title, and the mdedit editor in a readable-width
//! column; a status bar at the bottom; prompts on top.

mod prompt;
mod sidebar;
pub mod theme;

use mdedit::ui::{EditorWidget, apply_terminal_workarounds};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::sidebar::Panel;
use crate::workspace::{App, Areas, Focus};
use theme::{
    ACCENT, BG_ACTIVE_TAB, BG_CHROME, BG_SELECTED, BG_SIDEBAR, DIRTY, FAINT, MUTED, TEXT, TITLE,
    Theme,
};

/// The sidebar's width, when the screen is wide enough.
pub const SIDEBAR_WIDTH: u16 = 32;
/// The editor column's widest (Obsidian's "readable line length").
pub const READABLE_WIDTH: u16 = 100;
/// A tab's widest title.
const TAB_TITLE_WIDTH: usize = 24;

/// The screen before a vault is open (`blackglass` without a folder): the
/// name, and the vault picker.
pub fn draw_launcher(
    frame: &mut Frame,
    picker: &crate::vault_picker::VaultPicker,
    caps: mdedit::terminal::Capabilities,
    palette: theme::Palette,
) {
    let theme = Theme::new(caps, palette);
    let area = frame.area();
    let buf = frame.buffer_mut();
    buf.set_style(area, theme.on(TEXT, BG_CHROME));
    let title = "blackglass";
    let x = area.x + area.width.saturating_sub(title.width() as u16) / 2;
    put(
        buf,
        x,
        area.y + 1,
        title,
        area.width,
        theme.bold(ACCENT, BG_CHROME),
    );
    let line = "A vault is a folder of notes: open one, or type a new folder's path";
    let x = area.x + area.width.saturating_sub(line.width() as u16) / 2;
    put(
        buf,
        x,
        area.y + 2,
        line,
        area.width,
        theme.on(MUTED, BG_CHROME),
    );
    let below = Rect::new(
        area.x,
        area.y + 3,
        area.width,
        area.height.saturating_sub(3),
    );
    if let Some(cursor) = prompt::draw_vault_picker(buf, below, picker, &theme) {
        frame.set_cursor_position(cursor);
    }
}

/// Draws the whole screen and places the cursor.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let caps = app.shared.config.apply(app.shared.caps);
    let theme = Theme::new(caps, app.palette);
    app.areas = Areas::default();
    app.refresh_plugin_tabs();
    let [main, status] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    let right = if app.sidebar.visible && main.width >= 40 {
        let width = SIDEBAR_WIDTH.min(main.width / 2);
        let [side, line, right] = Layout::horizontal([
            Constraint::Length(width - 1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(main);
        let side = draw_panel(frame.buffer_mut(), side, app, &theme);
        sidebar::draw(frame.buffer_mut(), side, app, &theme);
        let edge = theme.on(FAINT, BG_SIDEBAR);
        for y in line.top()..line.bottom() {
            frame.buffer_mut()[(line.x, y)]
                .set_symbol(theme.glyph("│", "|"))
                .set_style(edge);
        }
        right
    } else {
        main
    };
    draw_notes(frame, right, app, &theme);
    prompt::draw_suggest(frame.buffer_mut(), main, app, &theme);
    prompt::draw_plugin_suggest(frame.buffer_mut(), main, app, &theme);
    draw_status(frame.buffer_mut(), status, app, &theme);
    let screen = frame.area();
    draw_preview(frame, screen, app, &theme);
    let prompt_cursor = prompt::draw(frame.buffer_mut(), screen, app, &theme);

    let images = app
        .view()
        .map(|v| v.image_areas.clone())
        .unwrap_or_default();
    apply_terminal_workarounds(frame.buffer_mut(), &caps, &images);
    let cursor = match (&app.prompt, app.focus) {
        (Some(_), _) => prompt_cursor,
        (None, Focus::Sidebar) => sidebar::cursor(app),
        (None, Focus::Editor) => app.view().and_then(|v| v.cursor),
        (None, Focus::Panel | Focus::Backlinks) => None,
    };
    if let Some(cursor) = cursor {
        frame.set_cursor_position(cursor);
    }
}

/// The right side: tabs, then the active note (or what to do without one).
fn draw_notes(frame: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    let [tabs, path, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);
    draw_tabs(frame.buffer_mut(), tabs, app, theme);
    // The backlinks take the bottom of the note's side (Alt+L).
    let body = if app.backlinks && !app.tabs.is_empty() && body.height >= 8 {
        let height = (body.height / 3).clamp(3, 12);
        let [note, pane] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(height)]).areas(body);
        draw_backlinks(frame.buffer_mut(), pane, app, theme);
        note
    } else {
        body
    };
    // The editor column, centered, at most READABLE_WIDTH wide.
    let margin = if body.width >= 40 { 2 } else { 0 };
    let width = body
        .width
        .saturating_sub(2 * margin)
        .clamp(1, READABLE_WIDTH);
    let column = Rect::new(
        // `saturating_sub`: a 0-wide screen (some terminals report 0×0 at first).
        body.x + body.width.saturating_sub(width) / 2,
        body.y,
        width,
        body.height,
    );
    app.areas.editor = body;
    let shown = app.tabs.get(app.active).map(|v| app.tab_name(v));
    let Some(view) = app.tabs.get_mut(app.active) else {
        draw_empty(frame.buffer_mut(), body, app, theme);
        return;
    };
    let rel = view
        .path
        .as_deref()
        .and_then(|p| p.strip_prefix(&app.vault.root).ok())
        .map(|p| p.to_string_lossy().into_owned());
    let name = shown.unwrap_or_default();
    let name = name.strip_suffix(".md").unwrap_or(&name).to_string();
    // The folder path, as Obsidian shows it above the note.
    let crumbs: Vec<String> = match &rel {
        Some(rel) => {
            let mut parts: Vec<String> = rel
                .trim_end_matches(".md")
                .split(std::path::MAIN_SEPARATOR)
                .map(str::to_string)
                .collect();
            // The note itself as it's shown (its title).
            if let Some(last) = parts.last_mut() {
                last.clone_from(&name);
            }
            parts
        }
        None => vec![name.clone()],
    };
    let buf = frame.buffer_mut();
    buf.set_style(path, theme.on(MUTED, BG_CHROME));
    let mut x = path.x + 1;
    let last = crumbs.len() - 1;
    for (i, crumb) in crumbs.iter().enumerate() {
        let style = if i == last {
            theme.on(TEXT, BG_CHROME)
        } else {
            theme.on(MUTED, BG_CHROME)
        };
        x = put(buf, x, path.y, crumb, path.right().saturating_sub(x), style);
        if i < last {
            x = put(
                buf,
                x,
                path.y,
                " / ",
                path.right().saturating_sub(x),
                theme.on(FAINT, BG_CHROME),
            );
        }
    }
    // The inline title, then the text.
    let editor = if column.height >= 8 {
        let title = Rect::new(column.x, column.y + 1, column.width, 1);
        put(
            buf,
            title.x,
            title.y,
            &name,
            title.width,
            theme.fg(TITLE).add_modifier(Modifier::BOLD),
        );
        Rect::new(column.x, column.y + 3, column.width, column.height - 3)
    } else {
        column
    };
    frame.render_stateful_widget(
        EditorWidget::new(&mut app.shared).status_bar(false),
        editor,
        view,
    );
}

/// The tab bar: each open note's name (a dot while it has unsaved
/// changes, else a close button), then `+` for a new note. It scrolls to
/// keep the active tab in view.
fn draw_tabs(buf: &mut Buffer, area: Rect, app: &mut App, theme: &Theme) {
    buf.set_style(area, theme.on(MUTED, BG_CHROME));
    let labels: Vec<String> = app
        .tabs
        .iter()
        .map(|v| {
            let title = app.tab_name(v);
            fit(title.strip_suffix(".md").unwrap_or(&title), TAB_TITLE_WIDTH)
        })
        .collect();
    // " title × " is the title plus four columns.
    let widths: Vec<u16> = labels.iter().map(|l| l.width() as u16 + 4).collect();
    let plus = 3;
    let mut first = 0;
    while first < app.active && widths[first..=app.active].iter().sum::<u16>() + plus > area.width {
        first += 1;
    }
    let mut x = area.x;
    for (i, label) in labels.iter().enumerate().skip(first) {
        let width = widths[i];
        if x + width > area.right() {
            break;
        }
        let active = i == app.active;
        let bg = if active { BG_ACTIVE_TAB } else { BG_CHROME };
        let style = if active {
            theme.bold(TEXT, bg)
        } else {
            theme.on(MUTED, bg)
        };
        let tab = Rect::new(x, area.y, width, 1);
        buf.set_style(tab, style);
        put(buf, x + 1, area.y, label, width - 1, style);
        let close = Rect::new(tab.right() - 2, area.y, 1, 1);
        if app.tabs[i].is_dirty() {
            put(
                buf,
                close.x,
                area.y,
                theme.glyph("●", "*"),
                1,
                theme.on(DIRTY, bg),
            );
        } else {
            put(
                buf,
                close.x,
                area.y,
                theme.glyph("×", "x"),
                1,
                theme.on(FAINT, bg),
            );
        }
        app.areas.tabs.push((Rect::new(x, area.y, width - 2, 1), i));
        app.areas.closes.push((close, i));
        x += width;
    }
    if x + plus <= area.right() {
        put(buf, x + 1, area.y, "+", 1, theme.on(MUTED, BG_CHROME));
        app.areas.new_tab = Rect::new(x, area.y, plus, 1);
    }
}

/// A linked note's preview (Alt+P): a box over the workspace with the
/// note in view mode.
fn draw_preview(frame: &mut Frame, screen: Rect, app: &mut App, theme: &Theme) {
    let Some(preview) = app.preview.as_mut() else {
        return;
    };
    let width = (screen.width * 4 / 5).clamp(20.min(screen.width), READABLE_WIDTH + 4);
    let height = (screen.height * 3 / 4).max(5.min(screen.height));
    if width < 12 || height < 5 {
        return;
    }
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 3,
        width,
        height,
    );
    let name = preview
        .path
        .as_deref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let title = format!(" {name} ");
    let buf = frame.buffer_mut();
    ratatui::widgets::Widget::render(ratatui::widgets::Clear, area, buf);
    let border = if theme.unicode {
        ratatui::widgets::BorderType::Rounded
    } else {
        ratatui::widgets::BorderType::Plain
    };
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(border)
        .border_style(theme.on(FAINT, BG_CHROME))
        .style(theme.on(TEXT, BG_CHROME))
        .title(title)
        .title_style(theme.bold(TITLE, BG_CHROME))
        .title_bottom(" ↑↓ scroll · Enter open · Esc close ");
    let inner = block.inner(area);
    ratatui::widgets::Widget::render(block, area, buf);
    frame.render_stateful_widget(
        EditorWidget::new(&mut app.shared).status_bar(false),
        inner,
        preview.as_mut(),
    );
}

/// The pane under the note (Alt+L): its backlinks (each linking note,
/// then its lines), unlinked mentions and outgoing links, each section
/// with its count; the chosen row highlighted while it has the focus.
fn draw_backlinks(buf: &mut Buffer, area: Rect, app: &mut App, theme: &Theme) {
    use crate::workspace::PaneItem;
    buf.set_style(area, theme.on(TEXT, BG_SIDEBAR));
    let items = app.link_pane();
    let count = |f: fn(&PaneItem) -> bool| items.iter().filter(|i| f(i)).count();
    let backs = count(|i| matches!(i, PaneItem::Backlink(_)));
    let mentions = count(|i| matches!(i, PaneItem::Mention(_)));
    let outs = count(|i| matches!(i, PaneItem::Outgoing(_)));
    let rule = theme.glyph("─", "-").repeat(area.width as usize);
    put(
        buf,
        area.x,
        area.y,
        &rule,
        area.width,
        theme.on(FAINT, BG_SIDEBAR),
    );
    let title = format!(" Backlinks ({backs}) ");
    put(
        buf,
        area.x + 2,
        area.y,
        &title,
        area.width.saturating_sub(2),
        theme.bold(MUTED, BG_SIDEBAR),
    );
    let list = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    // Rows: (what's written, style kind, the item it chooses).
    enum Kind {
        Heading,
        Note,
        Line,
        Empty,
    }
    let mut rows: Vec<(String, Kind, Option<usize>)> = Vec::new();
    if backs == 0 {
        rows.push(("No backlinks".into(), Kind::Empty, None));
    }
    let mut last_note: Option<std::path::PathBuf> = None;
    for (i, item) in items.iter().enumerate() {
        match item {
            PaneItem::Backlink(link) | PaneItem::Mention(link) => {
                if matches!(item, PaneItem::Mention(_))
                    && (i == 0 || !matches!(items[i - 1], PaneItem::Mention(_)))
                {
                    rows.push((
                        format!("Unlinked mentions ({mentions})  l links one"),
                        Kind::Heading,
                        None,
                    ));
                    last_note = None;
                }
                if last_note.as_ref() != Some(&link.note) {
                    let name = app.shown_name(&link.note);
                    let folder = app
                        .vault
                        .rel(&link.note)
                        .and_then(|r| r.parent())
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    rows.push((format!("{name}  {folder}"), Kind::Note, Some(i)));
                    last_note = Some(link.note.clone());
                }
                rows.push((link.text.trim().to_string(), Kind::Line, Some(i)));
            }
            PaneItem::Outgoing(out) => {
                if i == 0 || !matches!(items[i - 1], PaneItem::Outgoing(_)) {
                    rows.push((format!("Outgoing links ({outs})"), Kind::Heading, None));
                }
                let missing = if out.path.is_none() { " (missing)" } else { "" };
                rows.push((
                    format!("{} {}{missing}", theme.glyph("→", "->"), out.name),
                    Kind::Line,
                    Some(i),
                ));
            }
        }
    }
    let focused = app.focus == Focus::Backlinks;
    let chosen = app.backlink.min(items.len().saturating_sub(1));
    let at = rows
        .iter()
        .rposition(|r| matches!(r.1, Kind::Line) && r.2 == Some(chosen))
        .unwrap_or(0);
    let scroll = (at + 1).saturating_sub(list.height as usize);
    for (y, (text, kind, item)) in (list.y..list.bottom()).zip(rows.iter().skip(scroll)) {
        let row = Rect::new(list.x, y, list.width, 1);
        if let Some(i) = item {
            app.areas.backlinks.push((row, *i));
        }
        let selected = focused && *item == Some(chosen) && matches!(kind, Kind::Line);
        let bg = if selected { BG_SELECTED } else { BG_SIDEBAR };
        if selected {
            buf.set_style(row, theme.on(TEXT, bg));
        }
        let (x, style) = match kind {
            Kind::Heading => (row.x + 2, theme.bold(MUTED, bg)),
            Kind::Note => (row.x + 2, theme.bold(ACCENT, bg)),
            Kind::Line => (row.x + 4, theme.on(MUTED, bg)),
            Kind::Empty => (row.x + 2, theme.on(FAINT, bg)),
        };
        let text = fit(text, row.right().saturating_sub(x + 1) as usize);
        put(buf, x, y, &text, row.right().saturating_sub(x), style);
    }
}

/// What to do when no note is open, with the keys those commands have now.
fn draw_empty(buf: &mut Buffer, area: Rect, app: &App, theme: &Theme) {
    let keys = |id: &str| crate::keymap::describe(app.keymap.keys(id));
    let mut lines = vec![
        ("No file is open".to_string(), String::new()),
        (String::new(), String::new()),
        ("Create new note".to_string(), keys("new-note")),
        ("Go to file".to_string(), keys("go-to-note")),
        ("Search the vault".to_string(), keys("search-the-vault")),
        ("Help".to_string(), keys("help")),
        ("Quit".to_string(), keys("quit")),
    ];
    // The plugins' commands that have keys of their own (today's note).
    let at = lines.len() - 2;
    lines.splice(at..at, app.keyed_plugin_commands());
    let widest = lines
        .iter()
        .map(|(text, key)| text.width() + key.width() + 3)
        .max()
        .unwrap_or_default() as u16;
    let width = widest.max(30).min(area.width);
    let x = area.x + (area.width - width) / 2;
    let top = area.y + area.height.saturating_sub(lines.len() as u16) / 3;
    for (i, (text, key)) in lines.iter().enumerate() {
        let y = top + i as u16;
        if y >= area.bottom() {
            break;
        }
        let style = if i == 0 {
            theme.fg(MUTED).add_modifier(Modifier::BOLD)
        } else {
            theme.fg(TEXT)
        };
        put(buf, x, y, text, width, style);
        let key_x = x + width.saturating_sub(key.width() as u16);
        put(buf, key_x, y, key, width, theme.fg(FAINT));
    }
}

/// The status bar: a message, or the note and cursor position; key hints
/// for what has the focus on the right.
fn draw_status(buf: &mut Buffer, area: Rect, app: &App, theme: &Theme) {
    buf.set_style(area, theme.on(MUTED, BG_CHROME));
    let left = if !app.message.is_empty() {
        format!(" {}", app.message)
    } else if let Some(view) = app.view() {
        let rel = view
            .path
            .as_deref()
            .map(|p| app.vault.rel(p).unwrap_or(p).to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".into());
        let dirty = if view.is_dirty() { " [+]" } else { "" };
        let mode = if view.source_mode {
            "SOURCE │ "
        } else if view.reading {
            "VIEW │ "
        } else {
            ""
        };
        let (row, col) = (view.editor.row + 1, view.editor.col + 1);
        let message = if view.status.is_empty() {
            String::new()
        } else {
            format!("  │ {}", view.status)
        };
        let (words, chars) = crate::words::count_lines(&view.editor.lines);
        let count = match view.editor.selected_text() {
            Some(selected) if !selected.is_empty() => {
                format!("{} of {words} words", crate::words::count(&selected).0)
            }
            _ => format!("{words} words · {chars} chars"),
        };
        let plugins = app.plugin_status();
        let plugins = if plugins.is_empty() {
            String::new()
        } else {
            format!("  {plugins}")
        };
        format!(" {mode}{rel}{dirty}  Ln {row}, Col {col}  {count}{plugins}{message}")
    } else {
        format!(" {}", app.vault.name())
    };
    let end = put(
        buf,
        area.x,
        area.y,
        &left,
        area.width,
        theme.on(TEXT, BG_CHROME),
    );
    // As many hints as fit, dropping the last ones first.
    let room = area.right().saturating_sub(end + 2) as usize;
    let hints = hints(app);
    let mut hints: Vec<&str> = hints.split("  ").collect();
    while hints.join("  ").width() + 1 > room && !hints.is_empty() {
        hints.pop();
    }
    let hints = hints.join("  ");
    if !hints.is_empty() {
        let x = area.right() - hints.width() as u16 - 1;
        put(
            buf,
            x,
            area.y,
            &hints,
            room as u16,
            theme.on(FAINT, BG_CHROME),
        );
    }
}

/// The plugin panel (the calendar), when it's shown, at the bottom of
/// the sidebar `area`: a rule (accented while the panel has the focus),
/// then the panel's lines. Returns the rest of the sidebar.
fn draw_panel(buf: &mut Buffer, area: Rect, app: &mut App, theme: &Theme) -> Rect {
    if !app.panel {
        return area;
    }
    let Some((id, lines)) = app.panel_lines(area.width) else {
        return area;
    };
    let height = (lines.len() as u16 + 1).min(area.height.saturating_sub(4));
    if height < 2 {
        return area;
    }
    let top = Rect::new(area.x, area.y, area.width, area.height - height);
    let rule = Rect::new(area.x, top.bottom(), area.width, 1);
    let body = Rect::new(area.x, rule.bottom(), area.width, height - 1);
    buf.set_style(
        Rect::new(area.x, rule.y, area.width, height),
        theme.on(TEXT, BG_SIDEBAR),
    );
    let color = if app.focus == Focus::Panel {
        ACCENT
    } else {
        FAINT
    };
    put(
        buf,
        rule.x,
        rule.y,
        &theme.glyph("─", "-").repeat(rule.width as usize),
        rule.width,
        theme.on(color, BG_SIDEBAR),
    );
    for (y, line) in (body.y..body.bottom()).zip(lines) {
        buf.set_line(body.x, y, &line, body.width);
    }
    app.areas.panel = body;
    app.areas.panel_plugin = Some(id);
    top
}

/// Key hints for what has the focus.
fn hints(app: &App) -> String {
    let text = match (app.focus, app.sidebar.panel) {
        (Focus::Panel, _) => {
            "←→↑↓ day  PgUp/PgDn month  ⏎ day's note  w week  m month  t today  Esc back"
        }
        (Focus::Backlinks, _) => "↑↓ choose  ⏎ open  l link a mention  Esc note  Alt+L hide",
        (Focus::Editor, _) if app.view().is_some_and(|v| v.reading) => {
            "Tab next link  ⏎ follow  ↑↓ move  Esc edit  Alt+V mode"
        }
        (Focus::Editor, _) => "^O go to  ^N new  ^W close  ^G search  ^B sidebar  ^Q quit",
        (Focus::Sidebar, Panel::Files) => "⏎ open  ←→ fold  s sort  Tab panel  Esc editor",
        (Focus::Sidebar, Panel::Search) => "tag:name for tags  ⏎ open  Tab panel  Esc editor",
        (Focus::Sidebar, Panel::Tags) => "⏎ notes with the tag  Tab panel  Esc editor",
        (Focus::Sidebar, Panel::Plugin(id)) => {
            let keys = app.plugins.borrow().sidebar_hint(id);
            return format!("{keys}  Tab panel  Esc editor");
        }
    };
    text.to_string()
}

/// Writes `text` at (`x`, `y`), at most `width` columns; returns the
/// column after it.
fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, width: u16, style: Style) -> u16 {
    if width == 0 || !buf.area.contains(Position::new(x, y)) {
        return x;
    }
    buf.set_stringn(x, y, text, width as usize, style).0
}

/// `text` cut to `width` columns, ending in `…` if it was cut.
fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_names_are_cut_with_an_ellipsis() {
        assert_eq!(fit("short", 10), "short");
        assert_eq!(fit("a long name", 6), "a lon…");
        assert_eq!(fit("日本語の名前", 5), "日本…");
    }
}
