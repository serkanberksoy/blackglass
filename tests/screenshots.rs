//! The README's screenshots and its demo GIF, drawn by blackglass itself:
//! the example vault driven by keys, each screen written as an SVG
//! (`documentation/images/`; `scripts/screenshots.sh` turns them into
//! PNGs and the GIF). Ignored in the normal run:
//!
//! ```bash
//! scripts/screenshots.sh
//! ```

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use blackglass::ui;
use blackglass::vault::Vault;
use blackglass::workspace::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier};

/// A cell's size in the SVG (JetBrains Mono's advance is 0.6 em).
const FONT: f32 = 15.0;
const CELL_W: f32 = FONT * 0.6;
const CELL_H: f32 = 19.0;
/// The window frame around the screen: its title bar and margin.
const BAR: f32 = 30.0;
const PAD: f32 = 12.0;

const FONTS: &str =
    "'JetBrainsMono Nerd Font Mono', 'JetBrains Mono', 'DejaVu Sans Mono', monospace";

/// A copy of the example vault without a saved session.
fn example(name: &str) -> PathBuf {
    // Named as it is in the repository (the explorer shows the name).
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(name)
        .join("example_vault");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.parent().unwrap()).unwrap();
    let status = std::process::Command::new("cp")
        .args(["-r", "example_vault"])
        .arg(&dir)
        .status()
        .expect("cp runs");
    assert!(status.success());
    let _ = fs::remove_file(dir.join(".blackglass/workspace.json"));
    dir.canonicalize().unwrap()
}

fn out_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("documentation/images");
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    app.handle_key(KeyEvent::new(code, modifiers));
}

fn typing(app: &mut App, text: &str) {
    for c in text.chars() {
        key(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

fn draw(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    terminal.backend().buffer().clone()
}

/// A named color as a typical dark terminal shows it.
fn named(color: Color) -> Option<&'static str> {
    Some(match color {
        Color::Black => "#1d1f21",
        Color::Red => "#cc6666",
        Color::Green => "#b5bd68",
        Color::Yellow => "#f0c674",
        Color::Blue => "#81a2be",
        Color::Magenta => "#b294bb",
        Color::Cyan => "#8abeb7",
        Color::Gray => "#c5c8c6",
        Color::DarkGray => "#707880",
        Color::LightRed => "#d54e53",
        Color::LightGreen => "#b9ca4a",
        Color::LightYellow => "#e7c547",
        Color::LightBlue => "#7aa6da",
        Color::LightMagenta => "#c397d8",
        Color::LightCyan => "#70c0b1",
        Color::White => "#eaeaea",
        _ => return None,
    })
}

fn hex(color: Color, default: &str) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(i) => format!("#{0:02x}{0:02x}{0:02x}", i),
        c => named(c).unwrap_or(default).to_string(),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The screen as an SVG: in a window frame titled `title`, or (`None`) a
/// detail with a plain rounded frame.
fn svg(buf: &Buffer, title: Option<&str>) -> String {
    let area = buf.area;
    let (fg0, bg0) = ("#d8dee9", "#1e2129");
    let bar = if title.is_some() { BAR } else { PAD };
    let width = area.width as f32 * CELL_W + 2.0 * PAD;
    let height = area.height as f32 * CELL_H + bar + PAD;
    let mut out = String::new();
    let _ = write!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<rect width="{width}" height="{height}" rx="10" fill="{frame}"/>
"##,
        frame = if title.is_some() { "#15171c" } else { bg0 },
    );
    if let Some(title) = title {
        let _ = write!(
            out,
            r##"<circle cx="20" cy="15" r="6" fill="#ff5f57"/><circle cx="40" cy="15" r="6" fill="#febc2e"/><circle cx="60" cy="15" r="6" fill="#28c840"/>
<text x="{cx}" y="20" fill="#8a8f98" font-family="{FONTS}" font-size="13" text-anchor="middle">{title}</text>
"##,
            cx = width / 2.0,
            title = escape(title),
        );
    }
    let _ = write!(
        out,
        r##"<g transform="translate({PAD},{bar})" font-family="{FONTS}" font-size="{FONT}" style="white-space:pre">
<rect width="{w}" height="{h}" fill="{bg0}"/>
"##,
        w = area.width as f32 * CELL_W,
        h = area.height as f32 * CELL_H,
    );
    for y in 0..area.height {
        let top = y as f32 * CELL_H;
        let mut x = 0;
        while x < area.width {
            let cell = &buf[(x, y)];
            let reversed = cell.modifier.contains(Modifier::REVERSED);
            let (fg, bg) = if reversed {
                (hex(cell.bg, bg0), hex(cell.fg, fg0))
            } else {
                (hex(cell.fg, fg0), hex(cell.bg, bg0))
            };
            let symbol = cell.symbol();
            let wide = unicode_width::UnicodeWidthStr::width(symbol).max(1) as u16;
            // A run of cells with the same look (a wide character alone).
            let mut text = symbol.to_string();
            let mut end = x + wide;
            if wide == 1 {
                while end < area.width {
                    let next = &buf[(end, y)];
                    let w = unicode_width::UnicodeWidthStr::width(next.symbol());
                    if w != 1
                        || next.fg != cell.fg
                        || next.bg != cell.bg
                        || next.modifier != cell.modifier
                    {
                        break;
                    }
                    text.push_str(next.symbol());
                    end += 1;
                }
            }
            let left = x as f32 * CELL_W;
            let cells = f32::from(end - x);
            if bg != bg0 {
                let _ = writeln!(
                    out,
                    r#"<rect x="{left}" y="{top}" width="{}" height="{CELL_H}" fill="{bg}"/>"#,
                    cells * CELL_W + 0.5
                );
            }
            if !text.trim().is_empty() {
                let mut style = String::new();
                if cell.modifier.contains(Modifier::BOLD) {
                    style.push_str(r#" font-weight="bold""#);
                }
                if cell.modifier.contains(Modifier::ITALIC) {
                    style.push_str(r#" font-style="italic""#);
                }
                if cell.modifier.contains(Modifier::UNDERLINED) {
                    // One line under the whole run, spaces too.
                    let _ = writeln!(
                        out,
                        r#"<rect x="{left}" y="{}" width="{}" height="1" fill="{fg}"/>"#,
                        top + CELL_H * 0.9,
                        cells * CELL_W
                    );
                }
                if cell.modifier.contains(Modifier::DIM) {
                    style.push_str(r#" opacity="0.6""#);
                }
                // Word by word, each at its own cell: viewers treat spaces
                // in SVG text differently.
                let chars: Vec<char> = text.chars().collect();
                let single = wide == 1;
                let mut i = 0;
                while i < chars.len() {
                    if chars[i] == ' ' {
                        i += 1;
                        continue;
                    }
                    let start = i;
                    while i < chars.len() && (chars[i] != ' ' || !single) {
                        i += 1;
                    }
                    let word: String = chars[start..i].iter().collect();
                    let at = left + if single { start as f32 * CELL_W } else { 0.0 };
                    let _ = writeln!(
                        out,
                        r#"<text x="{at}" y="{}" fill="{fg}"{style}>{}</text>"#,
                        top + CELL_H * 0.78,
                        escape(&word)
                    );
                }
            }
            x = end;
        }
    }
    out.push_str("</g>\n</svg>\n");
    out
}

fn save(app: &mut App, name: &str, title: &str, width: u16, height: u16) {
    let buf = draw(app, width, height);
    fs::write(
        out_dir().join(format!("{name}.svg")),
        svg(&buf, Some(title)),
    )
    .unwrap();
}

fn open(app: &mut App, vault: &Path, rel: &str) {
    app.open(&vault.join(rel));
}

#[test]
#[ignore = "writes the README's images: scripts/screenshots.sh"]
fn screenshots() {
    let vault = example("screenshots-vault");
    let (w, h) = (130, 38);
    // The workspace: the explorer, tabs, a note in live preview.
    let mut app = App::new(Vault::open(&vault).unwrap());
    open(&mut app, &vault, "Guide/Writing.md");
    open(&mut app, &vault, "Welcome.md");
    // Off the title, so it shows as a heading.
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    save(&mut app, "workspace", "blackglass — Welcome", w, h);
    // Queries: Dataview tables over the vault's tasks.
    open(&mut app, &vault, "Plugins/Dataview/Time estimates.md");
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    save(&mut app, "queries", "blackglass — Dataview queries", w, h);
    // The task window, every field at once, with its calendar.
    open(&mut app, &vault, "Plugins/Tasks.md");
    // A blank line: a new task.
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    key(&mut app, KeyCode::Char('t'), KeyModifiers::ALT);
    typing(&mut app, "Water the garden #home");
    for _ in 0..4 {
        key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    }
    typing(&mut app, "friday");
    save(
        &mut app,
        "task-window",
        "blackglass — the task window",
        w,
        h,
    );
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    // The command palette.
    open(&mut app, &vault, "Welcome.md");
    key(&mut app, KeyCode::Char('p'), KeyModifiers::CONTROL);
    typing(&mut app, "insert");
    save(
        &mut app,
        "palette",
        "blackglass — every command in the palette",
        w,
        h,
    );
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    // Themes, previewed as you move: Ember.
    key(&mut app, KeyCode::Char('p'), KeyModifiers::CONTROL);
    typing(&mut app, "choose theme");
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    for _ in 0..2 {
        key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    }
    save(
        &mut app,
        "themes",
        "blackglass — themes, previewed as you choose",
        w,
        h,
    );
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
}

/// The demo GIF's frames: a short session, each step held for a while.
#[test]
#[ignore = "writes the README's demo frames: scripts/screenshots.sh"]
fn demo_frames() {
    let vault = example("demo-vault");
    let dir = out_dir().join("demo-frames");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let (w, h) = (110, 30);
    let mut app = App::new(Vault::open(&vault).unwrap());
    let mut n = 0;
    // Each frame: its SVG and how long it shows (hundredths of a second).
    let mut frame = |app: &mut App, hold: u32| {
        let buf = draw(app, w, h);
        fs::write(
            dir.join(format!("{n:03}-{hold}.svg")),
            svg(&buf, Some("blackglass")),
        )
        .unwrap();
        n += 1;
    };
    frame(&mut app, 200);
    // Go to a note by typing part of its name.
    key(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
    frame(&mut app, 50);
    for c in "time est".chars() {
        key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
        frame(&mut app, 10);
    }
    frame(&mut app, 70);
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    frame(&mut app, 250);
    // A new task at the end of the list, in the task window: its estimate
    // and a date in words.
    for _ in 0..8 {
        key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    }
    key(&mut app, KeyCode::End, KeyModifiers::NONE);
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    key(&mut app, KeyCode::Char('t'), KeyModifiers::ALT);
    frame(&mut app, 80);
    for c in "Paint the shed".chars() {
        key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
        frame(&mut app, 8);
    }
    for _ in 0..3 {
        key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    }
    for c in "2h".chars() {
        key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
        frame(&mut app, 12);
    }
    key(&mut app, KeyCode::Down, KeyModifiers::NONE);
    for c in "next friday".chars() {
        key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
        frame(&mut app, 8);
    }
    frame(&mut app, 200);
    // Saved: the task in the note, the totals counting it (unsaved edits
    // reach the queries at the app's next tick).
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    key(&mut app, KeyCode::Up, KeyModifiers::NONE);
    app.tick();
    frame(&mut app, 400);
}

/// A row's text.
fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

/// The first row from `from` with `text` in it, and the column it's at.
fn find(buf: &Buffer, text: &str, from: u16) -> (u16, u16) {
    (from..buf.area.height)
        .find_map(|y| {
            let row = row_text(buf, y);
            row.find(text).map(|i| (row[..i].chars().count() as u16, y))
        })
        .unwrap_or_else(|| panic!("{text:?} isn't on the screen"))
}

/// Rows `top..=bottom` and columns `left..` (to the last one with text),
/// as a screen of their own.
fn part(buf: &Buffer, top: u16, bottom: u16, left: u16) -> Buffer {
    let right = (top..=bottom)
        .filter_map(|y| {
            (left..buf.area.width)
                .rev()
                .find(|&x| !buf[(x, y)].symbol().trim().is_empty())
        })
        .max()
        .unwrap_or(left)
        + 2;
    let (w, h) = (right.min(buf.area.width) - left, bottom - top + 1);
    let mut out = Buffer::empty(ratatui::layout::Rect::new(0, 0, w, h));
    for y in 0..h {
        for x in 0..w {
            out[(x, y)] = buf[(left + x, top + y)].clone();
        }
    }
    out
}

/// The framed block (`╭` … `╰`) around the row with `anchor`.
fn block(buf: &Buffer, anchor: &str) -> Buffer {
    let (ax, ay) = find(buf, anchor, 0);
    // Up to the nearest frame corner left of the anchor.
    let (x, top) = (0..=ay)
        .rev()
        .find_map(|y| {
            let row = row_text(buf, y);
            let i = row.find('╭')?;
            let x = row[..i].chars().count() as u16;
            (x <= ax).then_some((x, y))
        })
        .expect("a block's corner above the anchor");
    let bottom = (ay..buf.area.height)
        .find(|&y| buf[(x, y)].symbol() == "╰")
        .expect("the block's end");
    part(buf, top, bottom, x.saturating_sub(1))
}

/// The rows from the one with `first` to the one with `last`.
fn lines(buf: &Buffer, first: &str, last: &str) -> Buffer {
    let (x, top) = find(buf, first, 0);
    let (_, bottom) = find(buf, last, top);
    part(buf, top, bottom, x.saturating_sub(3))
}

fn detail(name: &str, buf: &Buffer) {
    fs::write(out_dir().join(format!("{name}.svg")), svg(buf, None)).unwrap();
}

/// Details of pages: a query's results, a board, a diagram, lists.
#[test]
#[ignore = "writes the README's images: scripts/screenshots.sh"]
fn details() {
    let vault = example("details-vault");
    let mut app = App::new(Vault::open(&vault).unwrap());
    // The note alone, as tall as the page.
    key(&mut app, KeyCode::Char('b'), KeyModifiers::ALT);
    let page = |app: &mut App, rel: &str| {
        open(app, &vault, rel);
        draw(app, 120, 300)
    };
    let buf = page(&mut app, "Plugins/Dataview/Time estimates.md");
    detail("detail-days", &block(&buf, "Planned"));
    let buf = page(&mut app, "Plugins/Bases/Kanban.md");
    detail("detail-board", &block(&buf, "Plugins/Bases/Projects.base"));
    let buf = page(&mut app, "Plugins/Bases.md");
    detail("detail-cards", &block(&buf, "Library.base › Cards"));
    let buf = page(&mut app, "Plugins/Mermaid.md");
    detail("detail-chart", &block(&buf, "Where the time goes"));
    let buf = page(&mut app, "Plugins/Tasks.md");
    detail(
        "detail-tasks",
        &lines(&buf, "Renew passport", "Called the bank"),
    );
    let buf = page(&mut app, "Formatting/Lists.md");
    detail(
        "detail-states",
        &lines(&buf, "in progress", "any letter you like"),
    );
    let buf = page(&mut app, "Guide/Dates in words.md");
    detail("detail-dates", &lines(&buf, "@today", "@time"));
}
