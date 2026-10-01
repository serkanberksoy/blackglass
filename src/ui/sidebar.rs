//! Draws the sidebar: the panel tabs, a header (the vault's name, the
//! search input or the tag count), then the panel's list. Top-level
//! folders are tinted with their color, and a bar of that color runs down
//! the left of everything inside them.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

use super::theme::{
    ACCENT, BG_RAISED, BG_SELECTED, BG_SIDEBAR, FAINT, MUTED, Paint, TAG, TEXT, Theme,
};
use super::{fit, put};
use crate::sidebar::{FileRow, Panel, RowKind, SearchRow};
use crate::vault::search::Query;
use crate::workspace::{App, Focus};

/// Rows above the list: the panel tabs and the header.
const HEADER_ROWS: u16 = 2;

pub fn draw(buf: &mut Buffer, area: Rect, app: &mut App, theme: &Theme) {
    buf.set_style(area, theme.on(TEXT, BG_SIDEBAR));
    if area.height <= HEADER_ROWS {
        return;
    }
    draw_panel_tabs(buf, area, app, theme);
    let header = Rect::new(area.x, area.y + 1, area.width, 1);
    let list = Rect::new(
        area.x,
        area.y + HEADER_ROWS,
        area.width,
        area.height - HEADER_ROWS,
    );
    app.areas.list = list;
    app.sidebar.height = list.height as usize;
    // The list may have shrunk (a smaller window, a closed folder).
    let (_, len) = app.sidebar.cursor(&app.vault);
    app.sidebar.scroll_by(0, &app.vault);
    let focused = app.focus == Focus::Sidebar && app.prompt.is_none();
    let (at, _) = app.sidebar.cursor(&app.vault);
    let selected = Style::new().bg(theme.color(BG_SELECTED));
    let first = app.sidebar.scroll();
    let shown = (first..len).take(list.height as usize);
    match app.sidebar.panel {
        Panel::Files => {
            let right = format!("{} ", app.sidebar.sort.label());
            put(
                buf,
                header.x + 1,
                header.y,
                app.vault.name(),
                header.width.saturating_sub(right.width() as u16 + 2),
                theme.bold(TEXT, BG_SIDEBAR),
            );
            put_right(buf, header, &right, theme.on(FAINT, BG_SIDEBAR));
            let rows = app.sidebar.file_rows(&app.vault);
            let open = app.view().and_then(|v| v.path.clone());
            for (y, i) in (list.y..).zip(shown) {
                let row = &rows[i];
                let is_open = open.as_deref() == Some(row.path.as_path());
                let name = match row.kind {
                    RowKind::Note => app.shown_name(&row.path),
                    _ => row.name.clone(),
                };
                draw_file_row(
                    buf,
                    Rect::new(list.x, y, list.width, 1),
                    row,
                    &name,
                    is_open,
                    theme,
                );
                if i == at && focused {
                    let r = Rect::new(list.x + 1, y, list.width - 1, 1);
                    buf.set_style(r, selected);
                }
            }
        }
        Panel::Search => {
            draw_search_input(buf, header, &app.sidebar.query, theme);
            let rows = app.sidebar.search_rows();
            let highlight = Query::parse(&app.sidebar.query).highlight();
            for (y, i) in (list.y..).zip(shown) {
                let r = Rect::new(list.x, y, list.width, 1);
                draw_search_row(buf, r, rows[i], app, highlight.as_deref(), theme);
                if i == at && focused {
                    buf.set_style(r, selected);
                }
            }
            if rows.is_empty() && !app.sidebar.query.trim().is_empty() {
                put(
                    buf,
                    list.x + 2,
                    list.y,
                    "No matches",
                    list.width - 2,
                    theme.on(MUTED, BG_SIDEBAR),
                );
            }
        }
        Panel::Plugin(_) => {
            let title = app.sidebar.title(app.sidebar.panel);
            put(
                buf,
                header.x + 1,
                header.y,
                title,
                header.width,
                theme.bold(TEXT, BG_SIDEBAR),
            );
            let rows = app.sidebar_plugin_rows();
            put_right(
                buf,
                header,
                &format!("{} ", rows.len()),
                theme.on(FAINT, BG_SIDEBAR),
            );
            let open = app
                .view()
                .and_then(|v| v.path.as_deref())
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned());
            for (y, i) in (list.y..).zip(shown) {
                let Some(row) = rows.get(i) else {
                    break;
                };
                let r = Rect::new(list.x, y, list.width, 1);
                let color = if open.as_deref() == Some(row.label.as_str()) {
                    ACCENT
                } else {
                    TEXT
                };
                let x = put(
                    buf,
                    r.x + 2,
                    y,
                    &row.label,
                    r.width.saturating_sub(2),
                    theme.on(color, BG_SIDEBAR),
                );
                if !row.detail.is_empty() {
                    put(
                        buf,
                        x + 1,
                        y,
                        &row.detail,
                        r.right().saturating_sub(x + 1),
                        theme.on(FAINT, BG_SIDEBAR),
                    );
                }
                if i == at && focused {
                    buf.set_style(r, selected);
                }
            }
            if rows.is_empty() {
                put(
                    buf,
                    list.x + 2,
                    list.y,
                    "Nothing yet",
                    list.width - 2,
                    theme.on(MUTED, BG_SIDEBAR),
                );
            }
        }
        Panel::Tags => {
            put(
                buf,
                header.x + 1,
                header.y,
                "Tags",
                header.width,
                theme.bold(TEXT, BG_SIDEBAR),
            );
            put_right(
                buf,
                header,
                &format!("{} ", app.vault.tags.len()),
                theme.on(FAINT, BG_SIDEBAR),
            );
            let rows = app.sidebar.tag_rows(&app.vault);
            for (y, i) in (list.y..).zip(shown) {
                let r = Rect::new(list.x, y, list.width, 1);
                let Some(tag) = rows.get(i) else {
                    break;
                };
                let count = format!("{} ", tag.notes);
                let x = r.x + 2 * tag.depth as u16;
                let arrow = match (tag.nested, tag.open) {
                    (false, _) => "  ",
                    (true, true) => theme.glyph("▾ ", "v "),
                    (true, false) => theme.glyph("▸ ", "> "),
                };
                put(buf, x, y, arrow, r.width, theme.on(MUTED, BG_SIDEBAR));
                let room =
                    (r.right().saturating_sub(x + 2) as usize).saturating_sub(count.len() + 1);
                let name = fit(&format!("#{}", tag.label), room);
                put(
                    buf,
                    x + 2,
                    y,
                    &name,
                    r.right().saturating_sub(x + 2),
                    theme.on(TAG, BG_SIDEBAR),
                );
                put_right(buf, r, &count, theme.on(FAINT, BG_SIDEBAR));
                if i == at && focused {
                    buf.set_style(r, selected);
                }
            }
            if app.vault.tags.is_empty() {
                put(
                    buf,
                    list.x + 2,
                    list.y,
                    "No tags yet",
                    list.width - 2,
                    theme.on(MUTED, BG_SIDEBAR),
                );
            }
        }
    }
}

/// The cursor, while the sidebar has the focus: in the search input.
pub fn cursor(app: &App) -> Option<Position> {
    if app.sidebar.panel != Panel::Search || !app.sidebar.visible {
        return None;
    }
    let list = app.areas.list;
    let shown = app
        .sidebar
        .query
        .width()
        .min(list.width.saturating_sub(5) as usize);
    let x = list.x + 3 + shown as u16;
    (x < list.right()).then(|| Position::new(x, list.y.saturating_sub(1)))
}

/// "Files  Search  Tags", the active one raised.
fn draw_panel_tabs(buf: &mut Buffer, area: Rect, app: &mut App, theme: &Theme) {
    let mut x = area.x;
    for panel in app.sidebar.panels() {
        let label = format!(" {} ", app.sidebar.title(panel));
        let width = label.width() as u16;
        if x + width > area.right() {
            break;
        }
        let style = if panel == app.sidebar.panel {
            theme.bold(TEXT, BG_RAISED)
        } else {
            theme.on(MUTED, BG_SIDEBAR)
        };
        put(buf, x, area.y, &label, width, style);
        app.areas
            .panel_tabs
            .push((Rect::new(x, area.y, width, 1), panel));
        x += width;
    }
}

fn draw_search_input(buf: &mut Buffer, header: Rect, query: &str, theme: &Theme) {
    let input = Rect::new(header.x + 1, header.y, header.width.saturating_sub(2), 1);
    buf.set_style(input, theme.on(TEXT, BG_RAISED));
    let icon_x = put(
        buf,
        input.x,
        input.y,
        theme.glyph("⌕ ", "> "),
        2,
        theme.on(MUTED, BG_RAISED),
    );
    if query.is_empty() {
        put(
            buf,
            icon_x,
            input.y,
            "Search… tag:name",
            input.width - 2,
            theme.on(FAINT, BG_RAISED),
        );
    } else {
        // The end of a long query stays visible.
        let room = (input.width as usize).saturating_sub(3);
        let skip = query.width().saturating_sub(room);
        let shown: String = query.chars().skip(skip).collect();
        put(
            buf,
            icon_x,
            input.y,
            &shown,
            input.width - 2,
            theme.on(TEXT, BG_RAISED),
        );
    }
}

/// One file explorer row: `▌▾ Folder` (tinted) at the top level, then the
/// folder's color bar, indentation, and a folder marker or the name.
/// A file explorer row; `name` is what a note is shown as.
fn draw_file_row(
    buf: &mut Buffer,
    area: Rect,
    row: &FileRow,
    name: &str,
    is_open: bool,
    theme: &Theme,
) {
    let color = row.color.map(|i| Paint::from(theme.folder(i)));
    let top_folder = row.depth == 0 && matches!(row.kind, RowKind::Folder { .. });
    let bg = match row.color {
        Some(i) if top_folder => theme.folder_tint(i).into(),
        _ if is_open => BG_RAISED,
        _ => BG_SIDEBAR,
    };
    buf.set_style(area, theme.on(TEXT, bg));
    let bar = match (color, top_folder) {
        (Some(c), true) => Some((theme.glyph("▌", "|"), c)),
        (Some(c), false) => Some((theme.glyph("▏", "|"), c)),
        (None, _) => None,
    };
    if let Some((symbol, c)) = bar {
        put(buf, area.x, area.y, symbol, 1, theme.on(c, bg));
    }
    let mut x = area.x + 1 + 2 * row.depth as u16;
    let marker = match row.kind {
        RowKind::Folder { expanded: true } => theme.glyph("▾ ", "v "),
        RowKind::Folder { expanded: false } => theme.glyph("▸ ", "> "),
        RowKind::Note | RowKind::Attachment => "  ",
    };
    let folder_fg = color.unwrap_or(TEXT);
    let (marker_fg, name_style) = match row.kind {
        RowKind::Folder { .. } if top_folder => (folder_fg, theme.bold(folder_fg, bg)),
        RowKind::Folder { .. } => (folder_fg, theme.on(folder_fg, bg)),
        RowKind::Note if is_open => (TEXT, theme.bold(TEXT, bg)),
        RowKind::Note => (TEXT, theme.on(TEXT, bg)),
        RowKind::Attachment => (MUTED, theme.on(MUTED, bg)),
    };
    x = put(buf, x, area.y, marker, 2, theme.on(marker_fg, bg));
    let room = area.right().saturating_sub(x) as usize;
    put(buf, x, area.y, &fit(name, room), room as u16, name_style);
}

/// One search result row: a note's path, a matching line with the match
/// highlighted, or how many more matches there are.
fn draw_search_row(
    buf: &mut Buffer,
    area: Rect,
    row: SearchRow,
    app: &App,
    highlight: Option<&str>,
    theme: &Theme,
) {
    match row {
        SearchRow::Note { hits } => {
            let h = &app.sidebar.results[hits];
            let count = format!("{} ", h.total);
            let note = &app.vault.notes[h.note];
            let shown = app.shown_name(&note.path);
            let name = match note.rel.parent().filter(|p| !p.as_os_str().is_empty()) {
                Some(folder) => format!("{}/{shown}", folder.display()),
                None => shown,
            };
            let room = (area.width as usize).saturating_sub(count.len() + 2);
            put(
                buf,
                area.x + 1,
                area.y,
                &fit(&name, room),
                room as u16,
                theme.bold(TEXT, BG_SIDEBAR),
            );
            if h.total > 0 {
                put_right(buf, area, &count, theme.on(FAINT, BG_SIDEBAR));
            }
        }
        SearchRow::Line { hits, line, col } => {
            let note = &app.vault.notes[app.sidebar.results[hits].note];
            let text = note.lines.get(line).map_or("", String::as_str);
            let room = (area.width as usize).saturating_sub(4);
            let (snippet, at) = snippet(text, col, room);
            let x = area.x + 3;
            put(
                buf,
                x,
                area.y,
                &snippet,
                room as u16,
                theme.on(MUTED, BG_SIDEBAR),
            );
            // The match itself, in mdedit's search highlight.
            if let Some(q) = highlight {
                let len = q.chars().count();
                let before: String = snippet.chars().take(at).collect();
                let found: String = snippet.chars().skip(at).take(len).collect();
                let mx = x + before.width() as u16;
                let room = area.right().saturating_sub(mx);
                put(buf, mx, area.y, &found, room, mdedit::ui::SEARCH_MATCH);
            }
        }
        SearchRow::More { count, .. } => {
            let text = format!("… {count} more");
            put(
                buf,
                area.x + 3,
                area.y,
                &text,
                area.width.saturating_sub(3),
                theme.on(FAINT, BG_SIDEBAR),
            );
        }
    }
}

/// The part of `line` to show, `width` columns, so the match at char
/// column `col` is visible; and where the match starts in it (in chars).
fn snippet(line: &str, col: usize, width: usize) -> (String, usize) {
    // Leading indentation says nothing.
    let indent = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .count()
        .min(col);
    let chars: Vec<char> = line.chars().skip(indent).collect();
    let col = col - indent;
    // Some context before the match, if the line is long.
    let start = if col + 12 > width {
        col.saturating_sub(width / 3)
    } else {
        0
    };
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..]);
    let at = col - start + usize::from(start > 0);
    (fit(&out, width), at)
}

/// `text` against the right edge of `area`.
fn put_right(buf: &mut Buffer, area: Rect, text: &str, style: Style) {
    let w = text.width() as u16;
    if w < area.width {
        put(buf, area.right() - w, area.y, text, w, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snippet_keeps_the_match_in_view() {
        assert_eq!(snippet("  - short line", 4, 20), ("- short line".into(), 2));
        let long = format!("{}MATCH end", "x".repeat(50));
        let (s, at) = snippet(&long, 50, 20);
        assert!(s.starts_with('…'), "{s}");
        assert_eq!(s.chars().skip(at).take(5).collect::<String>(), "MATCH");
    }
}
