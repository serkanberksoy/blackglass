//! Draws the open prompt in a box over the workspace: the quick switcher,
//! the command palette, the plugins window, the settings window and its
//! screens (editor, keyboard shortcuts, plugin settings), the new-note and
//! save-as name inputs, and the unsaved-changes question.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use unicode_width::UnicodeWidthStr;

use super::theme::{ACCENT, BG_PROMPT, BG_RAISED, BG_SELECTED, FAINT, MUTED, TEXT, Theme};
use super::{fit, put};
use crate::commands::Palette;
use crate::plugins::Question;
use crate::settings_window::{Item, Page, SettingsWindow};
use crate::switcher::Switcher;
use crate::vault_picker::VaultPicker;
use crate::workspace::{App, Ask, Prompt, PropertyInput, PropertyWindow, Then};

/// Draws `app`'s prompt, if any, centered in `screen`; returns where the
/// cursor goes.
pub fn draw(buf: &mut Buffer, screen: Rect, app: &mut App, theme: &Theme) -> Option<Position> {
    let prompt = app.prompt.as_ref()?;
    match prompt {
        Prompt::Switcher(s) => {
            let s = s.clone();
            draw_switcher(buf, screen, app, &s, theme)
        }
        Prompt::Palette(p) => {
            let p = p.clone();
            let hint = "↑↓ choose  ·  Enter run  ·  Esc close";
            draw_palette(buf, screen, app, &p, " Commands ", hint, theme)
        }
        Prompt::Ask(ask) => {
            let ask = ask.clone();
            draw_ask(buf, screen, app, &ask, theme)
        }
        Prompt::Settings(window) => {
            let window = window.clone();
            draw_settings(buf, screen, app, &window, theme)
        }
        Prompt::OpenVault(picker) => draw_vault_picker(buf, screen, picker, theme),
        Prompt::Properties(window) => {
            let window = window.clone();
            draw_properties(buf, screen, app, &window, theme)
        }
        Prompt::NewNote { folder, input } => {
            let place = match folder.strip_prefix(&app.vault.root) {
                Ok(rel) if !rel.as_os_str().is_empty() => {
                    format!("In {}/  (a / in the name makes folders)", rel.display())
                }
                _ => format!("In {}/  (a / in the name makes folders)", app.vault.name()),
            };
            let title = if app.extract.is_some() {
                " Extract to a new note "
            } else {
                " New note "
            };
            let inner = frame(buf, screen, title, 4, theme)?;
            put(
                buf,
                inner.x,
                inner.y,
                &place,
                inner.width,
                theme.on(MUTED, BG_PROMPT),
            );
            let cursor = input_row(buf, inner, 1, "Name: ", input, "Untitled", theme);
            hint(
                buf,
                inner,
                3,
                "Enter create  ·  Tab from a template  ·  Esc cancel",
                theme,
            );
            cursor
        }
        Prompt::Date { input } => {
            let inner = frame(buf, screen, " Date ", 3, theme)?;
            let cursor = input_row(buf, inner, 0, "Date in words: ", input, "tomorrow", theme);
            let now = chrono::Local::now().naive_local();
            let read = match crate::nldates::parse_date(input, now, app.dates.week_start) {
                Some(day) => format!("→ {}", crate::nldates::format_date(day, &app.dates)),
                None if input.trim().is_empty() => "today, next friday, in 3 days, oct 20 …".into(),
                None => "no date in that".into(),
            };
            put(
                buf,
                inner.x,
                inner.y + 1,
                &read,
                inner.width,
                theme.on(MUTED, BG_PROMPT),
            );
            hint(buf, inner, 2, "Enter insert  ·  Esc cancel", theme);
            cursor
        }
        Prompt::SaveAs { input } => {
            let inner = frame(buf, screen, " Save as ", 3, theme)?;
            let cursor = input_row(buf, inner, 0, "Path in the vault: ", input, "", theme);
            hint(
                buf,
                inner,
                2,
                "Enter save  ·  Esc cancel  ·  .md is added",
                theme,
            );
            cursor
        }
        Prompt::Unsaved { tab, then } => {
            let name = app
                .tabs
                .get(*tab)
                .map(crate::workspace::tab_title)
                .unwrap_or_default();
            let what = match then {
                Then::Close => "before closing",
                Then::Quit => "before quitting",
            };
            let inner = frame(buf, screen, " Unsaved changes ", 3, theme)?;
            let question = format!("Save {name} {what}?");
            put(
                buf,
                inner.x,
                inner.y,
                &question,
                inner.width,
                theme.on(TEXT, BG_PROMPT),
            );
            let answers = "[Y]es  [N]o  [C]ancel";
            put(
                buf,
                inner.x,
                inner.y + 2,
                answers,
                inner.width,
                theme.bold(ACCENT, BG_PROMPT),
            );
            None
        }
    }
}

/// A bordered box with `title` and `rows` inner rows, centered; returns
/// the inside (with a column of padding), or `None` if it doesn't fit.
fn frame(buf: &mut Buffer, screen: Rect, title: &str, rows: u16, theme: &Theme) -> Option<Rect> {
    frame_sized(buf, screen, title, 64, rows, theme)
}

/// [`frame`], at most `widest` columns wide.
fn frame_sized(
    buf: &mut Buffer,
    screen: Rect,
    title: &str,
    widest: u16,
    rows: u16,
    theme: &Theme,
) -> Option<Rect> {
    let width = screen.width.saturating_sub(4).min(widest);
    let height = rows + 2;
    if width < 20 || height > screen.height {
        return None;
    }
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 3,
        width,
        height,
    );
    Clear.render(area, buf);
    let border = if theme.unicode {
        BorderType::Rounded
    } else {
        BorderType::Plain
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(border)
        .border_style(theme.on(FAINT, BG_PROMPT))
        .style(theme.on(TEXT, BG_PROMPT))
        .title(title)
        .title_style(theme.bold(TEXT, BG_PROMPT))
        .render(area, buf);
    Some(Rect::new(area.x + 2, area.y + 1, width - 4, rows))
}

/// `label` and the typed `input` (or a faint `placeholder`) on inner row
/// `row`; returns the cursor at the input's end.
fn input_row(
    buf: &mut Buffer,
    inner: Rect,
    row: u16,
    label: &str,
    input: &str,
    placeholder: &str,
    theme: &Theme,
) -> Option<Position> {
    let y = inner.y + row;
    let x = put(
        buf,
        inner.x,
        y,
        label,
        inner.width,
        theme.on(MUTED, BG_PROMPT),
    );
    let room = inner.right().saturating_sub(x);
    if input.is_empty() {
        put(buf, x, y, placeholder, room, theme.on(FAINT, BG_PROMPT));
        return Some(Position::new(x, y));
    }
    let end = put(buf, x, y, input, room, theme.on(TEXT, BG_PROMPT));
    Some(Position::new(end.min(inner.right().saturating_sub(1)), y))
}

fn hint(buf: &mut Buffer, inner: Rect, row: u16, text: &str, theme: &Theme) {
    put(
        buf,
        inner.x,
        inner.y + row,
        text,
        inner.width,
        theme.on(FAINT, BG_PROMPT),
    );
}

/// The quick switcher: the input, the matching notes (name, then folder),
/// and "Create" when no note has that name.
fn draw_switcher(
    buf: &mut Buffer,
    screen: Rect,
    app: &mut App,
    s: &Switcher,
    theme: &Theme,
) -> Option<Position> {
    let total = s.rows(&app.vault);
    // Input, a blank row, the list (at least one row), a blank row, the hint.
    let list_rows = (total.max(1) as u16)
        .min(screen.height.saturating_sub(8))
        .min(14);
    let inner = frame(buf, screen, " Go to note ", list_rows + 4, theme)?;
    let cursor = input_row(
        buf,
        inner,
        0,
        theme.glyph("› ", "> "),
        &s.input,
        "Find or create a note…",
        theme,
    );
    let list = Rect::new(inner.x, inner.y + 2, inner.width, list_rows);
    let scroll = (s.selected + 1).saturating_sub(list_rows as usize);
    app.areas.popup_list = list;
    app.areas.popup_scroll = scroll;
    if total == 0 {
        put(
            buf,
            list.x,
            list.y,
            "No notes in this vault",
            list.width,
            theme.on(MUTED, BG_PROMPT),
        );
    }
    for (y, row) in (list.y..list.bottom()).zip(scroll..total) {
        let bg = if row == s.selected {
            BG_SELECTED
        } else {
            BG_PROMPT
        };
        let area = Rect::new(list.x, y, list.width, 1);
        buf.set_style(area, theme.on(TEXT, bg));
        match s.results.get(row) {
            Some(&i) => {
                let note = &app.vault.notes[i];
                let name = app.shown_name(&note.path);
                let folder = note
                    .rel
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                let mut x = put(
                    buf,
                    area.x + 1,
                    y,
                    &fit(&name, area.width as usize - 2),
                    area.width - 1,
                    theme.bold(TEXT, bg),
                );
                if let Some(alias) = s.aliases.get(&i) {
                    let room = area.right().saturating_sub(x + 1);
                    x = put(
                        buf,
                        x + 1,
                        y,
                        &fit(&format!("({alias})"), room as usize),
                        room,
                        theme.on(ACCENT, bg),
                    );
                }
                if !folder.is_empty() {
                    let room = area.right().saturating_sub(x + 2);
                    put(
                        buf,
                        x + 2,
                        y,
                        &fit(&folder, room as usize),
                        room,
                        theme.on(MUTED, bg),
                    );
                }
            }
            None => {
                let text = format!("+ Create “{}”", s.input.trim());
                put(
                    buf,
                    area.x + 1,
                    y,
                    &fit(&text, area.width as usize - 2),
                    area.width - 1,
                    theme.on(ACCENT, bg),
                );
            }
        }
    }
    let hint_text = "↑↓ choose  ·  Enter open  ·  Esc close";
    if hint_text.width() as u16 <= inner.width {
        hint(buf, inner, list_rows + 3, hint_text, theme);
    }
    cursor
}

/// The first row to show so row `selected` is in a list of `rows` rows.
fn scroll_to(selected: usize, rows: u16) -> usize {
    (selected + 1).saturating_sub(rows as usize)
}

/// The command palette: the input, then each command with its key.
fn draw_palette(
    buf: &mut Buffer,
    screen: Rect,
    app: &mut App,
    p: &Palette,
    title: &str,
    keys: &str,
    theme: &Theme,
) -> Option<Position> {
    let list_rows = (p.results.len().max(1) as u16)
        .min(screen.height.saturating_sub(8))
        .min(16);
    let inner = frame(buf, screen, title, list_rows + 4, theme)?;
    let cursor = input_row(
        buf,
        inner,
        0,
        theme.glyph("› ", "> "),
        &p.input,
        "Type a command…",
        theme,
    );
    let list = Rect::new(inner.x, inner.y + 2, inner.width, list_rows);
    let scroll = scroll_to(p.selected, list_rows);
    app.areas.popup_list = list;
    app.areas.popup_scroll = scroll;
    if p.results.is_empty() {
        put(
            buf,
            list.x,
            list.y,
            "No matching command",
            list.width,
            theme.on(MUTED, BG_PROMPT),
        );
    }
    for (y, row) in (list.y..list.bottom()).zip(scroll..p.results.len()) {
        let command = &p.commands[p.results[row]];
        let bg = if row == p.selected {
            BG_SELECTED
        } else {
            BG_PROMPT
        };
        let area = Rect::new(list.x, y, list.width, 1);
        buf.set_style(area, theme.on(TEXT, bg));
        let key_width = command.key.width() as u16;
        let room = area.width.saturating_sub(key_width + 3);
        put(
            buf,
            area.x + 1,
            y,
            &fit(&command.name, room as usize),
            room,
            theme.on(TEXT, bg),
        );
        if key_width > 0 && key_width + 2 < area.width {
            put(
                buf,
                area.right() - key_width - 1,
                y,
                &command.key,
                key_width,
                theme.on(MUTED, bg),
            );
        }
    }
    hint(buf, inner, list_rows + 3, keys, theme);
    cursor
}

/// The vault picker: the path, a note (why it can't open, "Enter again
/// …"), then the recent vaults or the folders the path goes on to.
pub fn draw_vault_picker(
    buf: &mut Buffer,
    screen: Rect,
    picker: &VaultPicker,
    theme: &Theme,
) -> Option<Position> {
    let list_rows = (picker.rows.len().max(1) as u16)
        .min(screen.height.saturating_sub(10))
        .min(12);
    let inner = frame(buf, screen, " Open vault ", list_rows + 6, theme)?;
    let cursor = input_row(
        buf,
        inner,
        0,
        "Folder: ",
        &picker.input,
        "a path, ~ for home",
        theme,
    );
    if !picker.note.is_empty() {
        put(
            buf,
            inner.x,
            inner.y + 1,
            &fit(&picker.note, inner.width as usize),
            inner.width,
            theme.on(ACCENT, BG_PROMPT),
        );
    }
    let heading = if picker.input.trim().is_empty() {
        "Recent vaults"
    } else {
        "Folders"
    };
    put(
        buf,
        inner.x,
        inner.y + 2,
        heading,
        inner.width,
        theme.bold(MUTED, BG_PROMPT),
    );
    let list = Rect::new(inner.x, inner.y + 3, inner.width, list_rows);
    if picker.rows.is_empty() {
        let empty = if picker.input.trim().is_empty() {
            "None yet: type a folder's path"
        } else {
            "No folders here (Enter opens or makes this one)"
        };
        put(
            buf,
            list.x,
            list.y,
            empty,
            list.width,
            theme.on(FAINT, BG_PROMPT),
        );
    }
    let scroll = scroll_to(picker.selected, list_rows);
    for (y, (i, row)) in (list.y..list.bottom()).zip(picker.rows.iter().enumerate().skip(scroll)) {
        let bg = if i == picker.selected {
            BG_SELECTED
        } else {
            BG_PROMPT
        };
        let area = Rect::new(list.x, y, list.width, 1);
        buf.set_style(area, theme.on(TEXT, bg));
        let shown = fit(&picker.shown(row), list.width.saturating_sub(2) as usize);
        put(
            buf,
            list.x + 1,
            y,
            &shown,
            list.width - 1,
            theme.on(TEXT, bg),
        );
    }
    let keys = "↑↓ choose  ·  Tab into it  ·  Enter open (a new folder is made)  ·  Esc cancel";
    hint(
        buf,
        inner,
        list_rows + 5,
        &fit(keys, inner.width as usize),
        theme,
    );
    cursor
}

/// The property editor: each property's name, value and type; the one
/// being typed (a value, a new `name: value`, a new name) with the cursor.
fn draw_properties(
    buf: &mut Buffer,
    screen: Rect,
    app: &mut App,
    w: &PropertyWindow,
    theme: &Theme,
) -> Option<Position> {
    use crate::properties::Value;
    let props = app.note_properties();
    let adding = matches!(w.input, Some((PropertyInput::Add, _)));
    let rows = (props.len().max(1) as u16 + u16::from(adding))
        .min(screen.height.saturating_sub(8))
        .max(1);
    let inner = frame(buf, screen, " Properties ", rows + 2, theme)?;
    let list = Rect::new(inner.x, inner.y, inner.width, rows);
    let key_width = props
        .iter()
        .map(|p| p.key.width())
        .max()
        .unwrap_or(0)
        .clamp(4, 20) as u16;
    let mut cursor = None;
    if props.is_empty() && !adding {
        put(
            buf,
            list.x,
            list.y,
            "No properties: a adds one",
            list.width,
            theme.on(FAINT, BG_PROMPT),
        );
    }
    let scroll = scroll_to(w.selected, rows);
    for (y, (i, p)) in (list.y..list.bottom()).zip(props.iter().enumerate().skip(scroll)) {
        let chosen = i == w.selected && !adding;
        let bg = if chosen { BG_SELECTED } else { BG_PROMPT };
        let area = Rect::new(list.x, y, list.width, 1);
        buf.set_style(area, theme.on(TEXT, bg));
        let typing = w.input.as_ref().filter(|_| chosen);
        let (key, value) = match typing {
            Some((PropertyInput::Rename, input)) => (input.clone(), p.value.edit_text()),
            Some((_, input)) => (p.key.clone(), input.clone()),
            None => {
                let shown = match &p.value {
                    Value::Checkbox(true) => theme.glyph("☑", "[x]").to_string(),
                    Value::Checkbox(false) => theme.glyph("☐", "[ ]").to_string(),
                    other => other.edit_text(),
                };
                (p.key.clone(), shown)
            }
        };
        let end = put(
            buf,
            area.x + 1,
            y,
            &fit(&key, key_width as usize),
            key_width,
            theme.on(MUTED, bg),
        );
        let kind = p.value.kind();
        let kind_x = area.right().saturating_sub(kind.width() as u16 + 1);
        let value_x = area.x + key_width + 3;
        let room = kind_x.saturating_sub(value_x + 1);
        let value_end = put(
            buf,
            value_x,
            y,
            &fit(&value, room as usize),
            room,
            theme.on(ACCENT, bg),
        );
        put(
            buf,
            kind_x,
            y,
            kind,
            kind.width() as u16,
            theme.on(FAINT, bg),
        );
        match typing {
            Some((PropertyInput::Rename, _)) => cursor = Some(Position::new(end, y)),
            Some(_) => cursor = Some(Position::new(value_end, y)),
            None => {}
        }
    }
    if let Some((PropertyInput::Add, input)) = &w.input {
        let y = list.y + (props.len() as u16).min(rows - 1);
        let area = Rect::new(list.x, y, list.width, 1);
        buf.set_style(area, theme.on(TEXT, BG_SELECTED));
        let shown = if input.is_empty() {
            "name: value"
        } else {
            input.as_str()
        };
        let x = put(buf, area.x + 1, y, "+ ", 2, theme.on(MUTED, BG_SELECTED));
        let end = put(
            buf,
            x,
            y,
            shown,
            area.width.saturating_sub(3),
            theme.on(TEXT, BG_SELECTED),
        );
        cursor = Some(Position::new(if input.is_empty() { x } else { end }, y));
    }
    let keys = if w.input.is_some() {
        "Enter save  ·  Ctrl+U clear  ·  Esc cancel"
    } else {
        "Enter edit  ·  a add  ·  r rename  ·  Delete remove  ·  Esc close"
    };
    hint(
        buf,
        inner,
        rows + 1,
        &fit(keys, inner.width as usize),
        theme,
    );
    cursor
}

/// A plugin's suggestions (Tasks' fields and dates), under the line being
/// typed (above it near the bottom), from where the replaced text starts.
pub fn draw_plugin_suggest(buf: &mut Buffer, screen: Rect, app: &App, theme: &Theme) {
    let Some(p) = &app.plugin_suggest else {
        return;
    };
    let Some((view, cursor)) = app.view().and_then(|v| Some((v, v.cursor?))) else {
        return;
    };
    let rows = p.items.len() as u16;
    let widest = p.items.iter().map(|(l, _)| l.width()).max().unwrap_or(0) as u16;
    let width = (widest + 4).clamp(16, 44).min(screen.width);
    let height = rows + 2;
    if width < 12 || height > screen.height {
        return;
    }
    let typed: String = view.editor.lines[p.row]
        .chars()
        .skip(p.start)
        .take(view.editor.col.saturating_sub(p.start))
        .collect();
    let x = cursor
        .x
        .saturating_sub(typed.width() as u16 + 1)
        .min(screen.right().saturating_sub(width));
    let y = if cursor.y + 1 + height <= screen.bottom() {
        cursor.y + 1
    } else {
        cursor.y.saturating_sub(height)
    };
    let area = Rect::new(x, y, width, height);
    Clear.render(area, buf);
    let border = if theme.unicode {
        BorderType::Rounded
    } else {
        BorderType::Plain
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(border)
        .border_style(theme.on(FAINT, BG_PROMPT))
        .style(theme.on(TEXT, BG_PROMPT))
        .render(area, buf);
    for (row, (label, _)) in p.items.iter().enumerate() {
        let y = area.y + 1 + row as u16;
        let bg = if row == p.selected {
            BG_SELECTED
        } else {
            BG_PROMPT
        };
        buf.set_style(
            Rect::new(area.x + 1, y, area.width - 2, 1),
            theme.on(TEXT, bg),
        );
        put(
            buf,
            area.x + 2,
            y,
            label,
            area.width - 3,
            theme.on(TEXT, bg),
        );
    }
}

/// The link suggestions (W-23), under the line being typed (above it near
/// the bottom), lined up with the link's name.
pub fn draw_suggest(buf: &mut Buffer, screen: Rect, app: &mut App, theme: &Theme) {
    let Some(s) = &app.suggest else {
        return;
    };
    let Some(cursor) = app.view().and_then(|v| v.cursor) else {
        return;
    };
    let rows = s.switcher.results.len().max(1) as u16;
    let width = 44.min(screen.width);
    let height = rows + 2;
    if width < 12 || height > screen.height {
        return;
    }
    let typed = s.switcher.input.width() as u16;
    let x = cursor
        .x
        .saturating_sub(typed + 1)
        .min(screen.right().saturating_sub(width));
    let y = if cursor.y + 1 + height <= screen.bottom() {
        cursor.y + 1
    } else {
        cursor.y.saturating_sub(height)
    };
    let area = Rect::new(x, y, width, height);
    Clear.render(area, buf);
    let border = if theme.unicode {
        BorderType::Rounded
    } else {
        BorderType::Plain
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(border)
        .border_style(theme.on(FAINT, BG_PROMPT))
        .style(theme.on(TEXT, BG_PROMPT))
        .render(area, buf);
    let list = Rect::new(area.x + 1, area.y + 1, area.width - 2, rows);
    app.areas.suggest = list;
    if s.switcher.results.is_empty() {
        put(
            buf,
            list.x + 1,
            list.y,
            "No matching note",
            list.width - 1,
            theme.on(MUTED, BG_PROMPT),
        );
        return;
    }
    for (row, &i) in s.switcher.results.iter().enumerate() {
        let y = list.y + row as u16;
        let bg = if row == s.switcher.selected {
            BG_SELECTED
        } else {
            BG_PROMPT
        };
        buf.set_style(Rect::new(list.x, y, list.width, 1), theme.on(TEXT, bg));
        let note = &app.vault.notes[i];
        let name = app.shown_name(&note.path);
        let mut x = put(
            buf,
            list.x + 1,
            y,
            &fit(&name, list.width as usize - 2),
            list.width - 1,
            theme.bold(TEXT, bg),
        );
        if let Some(alias) = s.switcher.aliases.get(&i) {
            let room = list.right().saturating_sub(x + 1);
            x = put(
                buf,
                x + 1,
                y,
                &fit(&format!("({alias})"), room as usize),
                room,
                theme.on(ACCENT, bg),
            );
        }
        if let Some(folder) = note.rel.parent().filter(|p| !p.as_os_str().is_empty()) {
            let room = list.right().saturating_sub(x + 2);
            put(
                buf,
                x + 2,
                y,
                &fit(&folder.display().to_string(), room as usize),
                room,
                theme.on(MUTED, bg),
            );
        }
    }
}

/// A plugin's question: a text input, or a list to choose from.
/// A form: a row per field (its label and value; a choice as `‹ item ›`),
/// the edited one highlighted, its help under them.
fn draw_form(
    buf: &mut Buffer,
    screen: Rect,
    app: &mut App,
    ask: &Ask,
    title: &str,
    fields: &[crate::plugins::FormField],
    theme: &Theme,
) -> Option<Position> {
    use crate::plugins::FieldValue;
    app.areas.form_days.clear();
    let rows = fields.len() as u16;
    // Dates: a calendar column on the right.
    let dates = ask.form.iter().any(|v| matches!(v, FieldValue::Date(_)));
    let widest = if dates { 64 + CALENDAR } else { 64 };
    let inner = frame_sized(
        buf,
        screen,
        &format!(" {title} "),
        widest,
        rows.max(9) + 4,
        theme,
    )?;
    let calendar = (dates && inner.width > CALENDAR + 30)
        .then(|| Rect::new(inner.right() - CALENDAR, inner.y, CALENDAR, 9));
    let full = inner;
    let inner = match calendar {
        Some(c) => Rect::new(inner.x, inner.y, c.x - inner.x - 1, inner.height),
        None => inner,
    };
    let label_w = fields
        .iter()
        .map(|f| f.label.chars().count())
        .max()
        .unwrap_or(0) as u16
        + 3;
    let mut cursor = None;
    for (i, (field, value)) in fields.iter().zip(&ask.form).enumerate() {
        let y = inner.y + i as u16;
        let here = i == ask.field;
        let bg = if here { BG_SELECTED } else { BG_PROMPT };
        buf.set_style(Rect::new(inner.x, y, inner.width, 1), theme.on(TEXT, bg));
        let label_style = if here {
            theme.bold(ACCENT, bg)
        } else {
            theme.on(MUTED, bg)
        };
        put(buf, inner.x + 1, y, &field.label, label_w, label_style);
        let x = inner.x + 1 + label_w;
        let width = inner.right().saturating_sub(x + 1);
        match value {
            FieldValue::Text(t) | FieldValue::Date(t) if t.is_empty() => {
                put(buf, x, y, theme.glyph("—", "-"), width, theme.on(FAINT, bg));
                if here {
                    cursor = Some(Position::new(x, y));
                }
            }
            FieldValue::Text(t) | FieldValue::Date(t) => {
                // The end shows while it's long.
                let len = t.chars().count();
                let shown: String = t
                    .chars()
                    .skip(len.saturating_sub(width as usize - 1))
                    .collect();
                put(buf, x, y, &shown, width, theme.on(TEXT, bg));
                if here {
                    cursor = Some(Position::new(x + shown.chars().count() as u16, y));
                }
            }
            FieldValue::Choice(items, chosen) => {
                let item = items.get(*chosen).map(String::as_str).unwrap_or_default();
                let text = format!("{} {item} {}", theme.glyph("‹", "<"), theme.glyph("›", ">"));
                put(buf, x, y, &text, width, theme.on(TEXT, bg));
            }
        }
    }
    // The edited date's month, its day chosen (or today's).
    if let (Some(area), Some(value @ FieldValue::Date(_))) = (calendar, ask.form.get(ask.field)) {
        let today = chrono::Local::now().date_naive();
        draw_calendar(buf, app, area, value.day(), today, theme);
    }
    let rows = rows.max(9);
    let inner = full;
    if let Some(field) = fields.get(ask.field) {
        put(
            buf,
            inner.x + 1,
            inner.y + rows + 1,
            &field.help,
            inner.width.saturating_sub(2),
            theme.on(MUTED, BG_PROMPT),
        );
    }
    hint(
        buf,
        inner,
        rows + 3,
        if matches!(ask.form.get(ask.field), Some(FieldValue::Date(_))) {
            "↑↓ field · ⇧←→ day · ⇧↑↓ week · PgUp/PgDn month · Enter save · Esc cancel"
        } else {
            "↑↓ field  ·  ←→ choose  ·  Enter save  ·  Esc cancel"
        },
        theme,
    );
    cursor
}

/// The calendar column's width.
const CALENDAR: u16 = 22;

/// A month (the chosen day's, else today's) with the chosen day marked
/// and today underlined; each day's cell clickable.
fn draw_calendar(
    buf: &mut Buffer,
    app: &mut App,
    area: Rect,
    chosen: Option<chrono::NaiveDate>,
    today: chrono::NaiveDate,
    theme: &Theme,
) {
    use chrono::Datelike;
    let shown = chosen.unwrap_or(today);
    let first = shown.with_day(1).expect("every month has a 1st");
    let title = first.format("%B %Y").to_string();
    let x0 = area.x + 1;
    let pad = (20usize.saturating_sub(title.chars().count()) / 2) as u16;
    put(
        buf,
        x0 + pad,
        area.y,
        &title,
        20,
        theme.bold(ACCENT, BG_PROMPT),
    );
    put(
        buf,
        x0,
        area.y + 1,
        "Mo Tu We Th Fr Sa Su",
        20,
        theme.on(MUTED, BG_PROMPT),
    );
    let start = first - chrono::Duration::days(i64::from(first.weekday().num_days_from_monday()));
    for week in 0..6u16 {
        for d in 0..7u16 {
            let day = start + chrono::Duration::days(i64::from(week * 7 + d));
            let cell = Rect::new(x0 + d * 3, area.y + 2 + week, 2, 1);
            let mut style = if day.month() == first.month() {
                theme.on(TEXT, BG_PROMPT)
            } else {
                theme.on(FAINT, BG_PROMPT)
            };
            if Some(day) == chosen {
                style = theme.bold(TEXT, BG_SELECTED);
            }
            if day == today {
                style = style.add_modifier(ratatui::style::Modifier::UNDERLINED);
            }
            put(buf, cell.x, cell.y, &format!("{:>2}", day.day()), 2, style);
            app.areas.form_days.push((cell, day));
        }
    }
}

fn draw_ask(
    buf: &mut Buffer,
    screen: Rect,
    app: &mut App,
    ask: &Ask,
    theme: &Theme,
) -> Option<Position> {
    match ask.current() {
        Question::Form { title, fields } => draw_form(buf, screen, app, ask, title, fields, theme),
        Question::Text { prompt, default } => {
            let title = format!(" {prompt} ");
            let inner = frame(buf, screen, &title, 3, theme)?;
            let cursor = input_row(
                buf,
                inner,
                0,
                theme.glyph("› ", "> "),
                &ask.input,
                default,
                theme,
            );
            hint(buf, inner, 2, "Enter ok  ·  Esc cancel", theme);
            cursor
        }
        Question::Lines { prompt, default } => {
            let lines: Vec<&str> = ask.input.split('\n').collect();
            let shown = (lines.len() as u16)
                .min(screen.height.saturating_sub(8))
                .max(1);
            let title = format!(" {prompt} ");
            let inner = frame(buf, screen, &title, shown + 2, theme)?;
            // The last lines, if there are more than fit.
            let first = lines.len() - shown as usize;
            let mut cursor = None;
            for (row, line) in lines[first..].iter().enumerate() {
                let label = if first + row == 0 {
                    theme.glyph("› ", "> ")
                } else {
                    "  "
                };
                let placeholder = if ask.input.is_empty() { default } else { "" };
                cursor = input_row(buf, inner, row as u16, label, line, placeholder, theme);
            }
            hint(
                buf,
                inner,
                shown + 1,
                "Alt+Enter new line  ·  Enter ok  ·  Esc cancel",
                theme,
            );
            cursor
        }
        Question::Choose { prompt, items } | Question::Many { prompt, items } => {
            let many = matches!(ask.current(), Question::Many { .. });
            let list_rows = (ask.results.len().max(1) as u16)
                .min(screen.height.saturating_sub(8))
                .min(14);
            let title = format!(" {prompt} ");
            let inner = frame(buf, screen, &title, list_rows + 4, theme)?;
            let cursor = input_row(
                buf,
                inner,
                0,
                theme.glyph("› ", "> "),
                &ask.input,
                "Type to filter…",
                theme,
            );
            let list = Rect::new(inner.x, inner.y + 2, inner.width, list_rows);
            let scroll = scroll_to(ask.selected, list_rows);
            app.areas.popup_list = list;
            app.areas.popup_scroll = scroll;
            if ask.results.is_empty() {
                put(
                    buf,
                    list.x,
                    list.y,
                    "Nothing matches",
                    list.width,
                    theme.on(MUTED, BG_PROMPT),
                );
            }
            for (y, row) in (list.y..list.bottom()).zip(scroll..ask.results.len()) {
                let bg = if row == ask.selected {
                    BG_SELECTED
                } else {
                    BG_PROMPT
                };
                buf.set_style(Rect::new(list.x, y, list.width, 1), theme.on(TEXT, bg));
                let item = &items[ask.results[row]];
                let item = if many {
                    let marked = ask.marked.contains(&ask.results[row]);
                    format!("{} {item}", if marked { "[x]" } else { "[ ]" })
                } else {
                    item.clone()
                };
                put(
                    buf,
                    list.x + 1,
                    y,
                    &fit(&item, list.width as usize - 2),
                    list.width - 1,
                    theme.on(TEXT, bg),
                );
            }
            let keys = if many {
                "↑↓ choose  ·  Tab mark  ·  Enter ok  ·  Esc cancel"
            } else {
                "↑↓ choose  ·  Enter ok  ·  Esc cancel"
            };
            hint(buf, inner, list_rows + 3, keys, theme);
            cursor
        }
    }
}

/// The settings window: the search box and the pages in their groups on
/// the left; the page on the right, each row with its name and value, what
/// it does below; the keys at the bottom. Returns where the cursor goes.
fn draw_settings(
    buf: &mut Buffer,
    screen: Rect,
    app: &mut App,
    w: &SettingsWindow,
    theme: &Theme,
) -> Option<Position> {
    let height = screen.height.saturating_sub(2).min(40);
    let inner = frame_sized(
        buf,
        screen,
        " Settings ",
        110,
        height.saturating_sub(2),
        theme,
    )?;
    let left = Rect::new(
        inner.x,
        inner.y,
        (inner.width / 3).clamp(16, 26),
        inner.height,
    );
    let sep = left.right() + 1;
    for y in inner.y..inner.bottom() {
        put(
            buf,
            sep,
            y,
            theme.glyph("│", "|"),
            1,
            theme.on(FAINT, BG_PROMPT),
        );
    }
    let right = Rect::new(
        sep + 2,
        inner.y,
        inner.right().saturating_sub(sep + 2),
        inner.height,
    );
    let mut cursor = draw_settings_nav(buf, left, app, w, theme);
    let page = draw_settings_page(buf, right, app, w, theme);
    if w.on_page || w.editing.is_some() {
        cursor = page;
    }
    let keys = if w.capturing {
        "Press the new key  ·  Esc cancel"
    } else if w.editing.is_some() {
        "Enter save  ·  Ctrl+U clear  ·  Esc cancel"
    } else if !w.on_page {
        "↑↓ pages  ·  Enter open  ·  type to search  ·  Esc close"
    } else {
        match w.page {
            Page::Shortcuts => "Enter set a key  ·  Delete no key  ·  Ctrl+D default  ·  Esc back",
            Page::Plugins => "Enter install/uninstall  ·  Space on/off  ·  → settings  ·  Esc back",
            _ => "↑↓ choose  ·  Enter change  ·  ←→ choose a value  ·  Esc back",
        }
    };
    let hint_y = right.bottom().saturating_sub(1);
    put(
        buf,
        right.x,
        hint_y,
        &fit(keys, right.width as usize),
        right.width,
        theme.on(FAINT, BG_PROMPT),
    );
    cursor
}

/// The settings window's left pane: the search box, then the pages in
/// their groups. Returns the search box's cursor.
fn draw_settings_nav(
    buf: &mut Buffer,
    left: Rect,
    app: &mut App,
    w: &SettingsWindow,
    theme: &Theme,
) -> Option<Position> {
    let glyph = theme.glyph("⌕ ", "> ");
    let x = put(buf, left.x, left.y, glyph, 2, theme.on(MUTED, BG_PROMPT));
    let room = left.right().saturating_sub(x);
    let cursor = if w.search.is_empty() {
        put(
            buf,
            x,
            left.y,
            "Search settings…",
            room,
            theme.on(FAINT, BG_PROMPT),
        );
        Position::new(x, left.y)
    } else {
        let end = put(buf, x, left.y, &w.search, room, theme.on(TEXT, BG_PROMPT));
        Position::new(end.min(left.right().saturating_sub(1)), left.y)
    };
    let mut y = left.y + 2;
    let mut group = "";
    for item in app.settings_nav(&w.search) {
        if item.group != group {
            if !group.is_empty() {
                y += 1;
            }
            group = item.group;
            if y >= left.bottom() {
                break;
            }
            put(
                buf,
                left.x,
                y,
                group,
                left.width,
                theme.bold(MUTED, BG_PROMPT),
            );
            y += 1;
        }
        if y >= left.bottom() {
            break;
        }
        let area = Rect::new(left.x, y, left.width, 1);
        let bg = match (item.page == w.page, w.on_page) {
            (true, false) => BG_SELECTED,
            (true, true) => BG_RAISED,
            _ => BG_PROMPT,
        };
        buf.set_style(area, theme.on(TEXT, bg));
        put(
            buf,
            area.x + 2,
            y,
            &item.title,
            area.width - 2,
            theme.on(TEXT, bg),
        );
        app.areas.settings_nav.push((area, item.page));
        y += 1;
    }
    Some(cursor)
}

/// The settings window's right pane: the page's title, then its rows (a
/// section's heading before its first row), each with what it does on a
/// second line. Returns the cursor of a text being edited.
fn draw_settings_page(
    buf: &mut Buffer,
    right: Rect,
    app: &mut App,
    w: &SettingsWindow,
    theme: &Theme,
) -> Option<Position> {
    use crate::plugins::settings::Kind;
    enum LineOf {
        Heading(String),
        /// A row's name line (false) or help line (true).
        Row(usize, bool),
    }
    let title = app.settings_title(w.page);
    put(
        buf,
        right.x,
        right.y,
        &title,
        right.width,
        theme.bold(TEXT, BG_PROMPT),
    );
    let rows = app.settings_rows(w.page, &w.search);
    let body = Rect::new(
        right.x,
        right.y + 2,
        right.width,
        right.height.saturating_sub(4),
    );
    let mut lines = Vec::new();
    let mut section = "";
    for (i, row) in rows.iter().enumerate() {
        if row.section != section {
            section = &row.section;
            if !section.is_empty() {
                let text = section.replace('_', " ");
                let heading = format!("{}{}", text[..1].to_uppercase(), &text[1..]);
                lines.push(LineOf::Heading(heading));
            }
        }
        lines.push(LineOf::Row(i, false));
        if !row.help.is_empty() {
            lines.push(LineOf::Row(i, true));
        }
    }
    if rows.is_empty() {
        put(
            buf,
            body.x,
            body.y,
            "Nothing matches",
            body.width,
            theme.on(MUTED, BG_PROMPT),
        );
    }
    let last = lines
        .iter()
        .rposition(|l| matches!(l, LineOf::Row(i, _) if *i == w.row))
        .unwrap_or(0);
    let scroll = scroll_to(last, body.height);
    let mut cursor = None;
    for (y, line) in (body.y..body.bottom()).zip(lines.iter().skip(scroll)) {
        let (i, help) = match line {
            LineOf::Heading(text) => {
                put(
                    buf,
                    body.x,
                    y,
                    text,
                    body.width,
                    theme.bold(MUTED, BG_PROMPT),
                );
                continue;
            }
            LineOf::Row(i, help) => (*i, *help),
        };
        let row = &rows[i];
        let chosen = i == w.row;
        let bg = match (chosen, w.on_page) {
            (true, true) => BG_SELECTED,
            (true, false) => BG_RAISED,
            _ => BG_PROMPT,
        };
        let area = Rect::new(body.x, y, body.width, 1);
        buf.set_style(area, theme.on(TEXT, bg));
        app.areas.settings_rows.push((area, i));
        if help {
            let text = fit(&row.help, area.width.saturating_sub(2) as usize);
            put(
                buf,
                area.x + 1,
                y,
                &text,
                area.width - 1,
                theme.on(MUTED, bg),
            );
            continue;
        }
        let (value, style) = match &row.item {
            _ if chosen && w.capturing => ("Press the new key…".to_string(), theme.on(ACCENT, bg)),
            Item::Setting(_) if chosen && w.editing.is_some() => (
                w.editing.clone().unwrap_or_default(),
                theme.on(TEXT, BG_PROMPT),
            ),
            Item::Setting(s) => match &s.kind {
                Kind::Toggle => (
                    if row.value == "true" { "on" } else { "off" }.to_string(),
                    theme.on(ACCENT, bg),
                ),
                Kind::Choice(_) => (format!("‹ {} ›", row.value), theme.on(ACCENT, bg)),
                Kind::Text if row.value.is_empty() => ("(empty)".into(), theme.on(FAINT, bg)),
                Kind::Text => (row.value.clone(), theme.on(ACCENT, bg)),
                Kind::Action(_) => (theme.glyph("↵", "Enter").to_string(), theme.on(ACCENT, bg)),
            },
            Item::Command(_) if row.value.is_empty() => ("no key".into(), theme.on(FAINT, bg)),
            Item::Command(_) => (row.value.clone(), theme.on(ACCENT, bg)),
            Item::Plugin(_) => {
                let color = if row.value.ends_with("enabled") {
                    ACCENT
                } else if row.value.starts_with("not") {
                    FAINT
                } else {
                    MUTED
                };
                (row.value.clone(), theme.on(color, bg))
            }
        };
        let room = (area.width / 2).saturating_sub(1);
        let value = fit(&value, room as usize);
        let value_x = area.right().saturating_sub(value.width() as u16 + 1);
        let label_room = value_x.saturating_sub(area.x + 2);
        let label = fit(&row.label, label_room as usize);
        put(buf, area.x + 1, y, &label, label_room, theme.on(TEXT, bg));
        let end = put(buf, value_x, y, &value, room, style);
        if chosen && w.editing.is_some() {
            cursor = Some(Position::new(end.min(area.right().saturating_sub(1)), y));
        }
        // ⚙: the plugin's settings page, for installed plugins that have some.
        if let Item::Plugin(p) = row.item {
            let has = !app.plugins.borrow().settings(p).is_empty();
            if has && row.value.starts_with("installed") && value_x > area.x + 4 {
                let gear = Rect::new(value_x - 3, y, 1, 1);
                put(
                    buf,
                    gear.x,
                    y,
                    theme.glyph("⚙", "*"),
                    1,
                    theme.on(MUTED, bg),
                );
                app.areas.gears.push((gear, p));
            }
        }
    }
    cursor
}
