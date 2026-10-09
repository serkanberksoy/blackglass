//! The window, without a display: its real frame code run by egui
//! headlessly, fed the window's raw input (keys, text, the mouse, copy
//! and paste, a close request, its size), and checked by what the app
//! did, what the window asked of the system (the clipboard, closing) and
//! what it painted (text, images, emoji).

use std::path::{Path, PathBuf};

use eframe::egui;
use ratatui::crossterm::event::KeyCode;

use super::*;
use crate::vault::Vault;
use crate::vault::tests::{scratch, write};
use crate::workspace::Prompt;

/// A window on a vault, run frame by frame.
struct Harness {
    ctx: egui::Context,
    window: Window,
    size: egui::Vec2,
    root: PathBuf,
}

impl Harness {
    /// A window on a new vault of `files`, `open` open in it.
    fn new(name: &str, files: &[(&str, &str)], open: &str) -> Self {
        let dir = scratch(name);
        write(&dir, files);
        let vault = Vault::open(&dir).unwrap();
        let root = vault.root.clone();
        let note = (!open.is_empty()).then(|| root.join(open));
        let copied = Clipboard::default();
        let screen = Screen::Vault(Box::new(app(vault, note, &copied, false)));
        Self::with(Window::new(screen, copied, false), root)
    }

    fn with(mut window: Window, root: PathBuf) -> Self {
        let ctx = egui::Context::default();
        window.start(&ctx);
        Harness {
            ctx,
            window,
            size: egui::vec2(1000.0, 700.0),
            root,
        }
    }

    /// A frame with these events (and the window's close button, `close`).
    fn frame_with(&mut self, events: Vec<egui::Event>, close: bool) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            events,
            ..egui::RawInput::default()
        };
        if close {
            let mut info = egui::ViewportInfo::default();
            info.events.push(egui::ViewportEvent::Close);
            input.viewports.insert(egui::ViewportId::ROOT, info);
        }
        let window = &mut self.window;
        self.ctx.run_ui(input, |ui| window.frame(ui))
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.frame_with(events, false)
    }

    fn app(&self) -> &App {
        match &self.window.screen {
            Screen::Vault(app) => app,
            Screen::Picker { .. } => panic!("the picker, not a vault"),
        }
    }

    fn lines(&self) -> Vec<String> {
        self.app().view().expect("a note").editor.lines.clone()
    }

    /// Where `text` is on the screen (the middle of its first cell).
    fn pos_of(&self, text: &str) -> egui::Pos2 {
        let grid = self.window.terminal.backend();
        let area = grid.buffer.area;
        for y in 0..area.height {
            let row: String = (0..area.width)
                .map(|x| grid.buffer[(x, y)].symbol().to_string())
                .collect();
            if let Some(i) = row.find(text) {
                let col = unicode_width::UnicodeWidthStr::width(&row[..i]) as f32;
                let cell = self.window.input.cell;
                return self.window.input.origin
                    + egui::vec2((col + 0.5) * cell.x, (y as f32 + 0.5) * cell.y);
            }
        }
        panic!("{text} isn't on the screen")
    }
}

/// The texts painted.
fn painted(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The pictures painted (meshes with a texture other than the fonts').
fn pictures(output: &egui::FullOutput) -> usize {
    output
        .shapes
        .iter()
        .filter(|c| {
            matches!(&c.shape, egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default())
        })
        .count()
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn click(pos: egui::Pos2) -> Vec<egui::Event> {
    [true, false]
        .map(|pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        })
        .to_vec()
}

#[test]
fn a_vault_opens_and_its_note_is_painted() {
    let mut h = Harness::new(
        "gui-paint",
        &[("Notes/Hello.md", "# Hello\n\nSome **bold** text.\n")],
        "Notes/Hello.md",
    );
    let out = h.frame(Vec::new());
    let text = painted(&out);
    for part in ["Files", "Notes", "Hello", "Some", "bold", "text."] {
        assert!(text.contains(part), "{part}: {text}");
    }
    // The grid fills the window: about a thousand points of cells.
    let size = h.window.terminal.backend().buffer.area;
    let cell = h.window.input.cell;
    assert!(cell.x > 4.0 && cell.y > 8.0, "{cell:?}");
    assert_eq!(size.width, (1000.0 / cell.x) as u16);
    assert_eq!(size.height, (700.0 / cell.y) as u16);
}

#[test]
fn typing_and_shortcuts_reach_the_app() {
    let mut h = Harness::new("gui-keys", &[("N.md", "start\n")], "N.md");
    h.frame(Vec::new());
    h.frame(vec![
        key(egui::Key::End, egui::Modifiers::NONE),
        egui::Event::Text(" and more".into()),
        key(egui::Key::Enter, egui::Modifiers::NONE),
        egui::Event::Ime(egui::ImeEvent::Commit("é".into())),
    ]);
    assert_eq!(h.lines()[..2], ["start and more", "é"]);
    // Ctrl+P: the command palette, Esc closes it.
    let out = h.frame(vec![key(egui::Key::P, egui::Modifiers::CTRL)]);
    assert!(matches!(h.app().prompt, Some(Prompt::Palette(_))));
    h.frame(Vec::new());
    let out2 = h.frame(Vec::new());
    assert!(painted(&out2).contains("Commands"), "{}", painted(&out));
    h.frame(vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    assert!(h.app().prompt.is_none());
    // Ctrl+S saves.
    h.frame(vec![key(egui::Key::S, egui::Modifiers::CTRL)]);
    let saved = std::fs::read_to_string(h.root.join("N.md")).unwrap();
    assert!(saved.starts_with("start and more\né"), "{saved}");
}

#[test]
fn the_mouse_clicks_and_scrolls_by_cell() {
    let body: String = (1..=200).map(|i| format!("line {i}\n")).collect();
    let mut h = Harness::new("gui-mouse", &[("A.md", "in A\n"), ("B.md", &body)], "A.md");
    h.frame(Vec::new());
    // A click on a note in the explorer opens it.
    let at = h.pos_of("B");
    h.frame(click(at));
    h.frame(Vec::new());
    let path = h.app().view().and_then(|v| v.path.clone()).unwrap();
    assert_eq!(path.file_name().unwrap(), "B.md");
    // A click in the text puts the cursor there.
    let at = h.pos_of("line 3");
    h.frame(click(at));
    assert_eq!(h.app().view().unwrap().editor.row, 2);
    // The wheel scrolls the note.
    let note = h.pos_of("line 1");
    h.frame(vec![egui::Event::PointerMoved(note)]);
    h.frame(vec![egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -10.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    }]);
    let out = h.frame(Vec::new());
    assert!(!painted(&out).contains("line 1\n"), "scrolled");
}

#[test]
fn copy_and_paste_use_the_windows_clipboard() {
    let mut h = Harness::new("gui-clipboard", &[("N.md", "alpha\nbeta\n")], "N.md");
    h.frame(Vec::new());
    // Ctrl+A, then copy: the note's text on the clipboard.
    h.frame(vec![key(egui::Key::A, egui::Modifiers::CTRL)]);
    let out = h.frame(vec![egui::Event::Copy]);
    let copied: Vec<&String> = out
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(copied.len(), 1);
    assert!(copied[0].starts_with("alpha\nbeta"), "{copied:?}");
    // Paste: the clipboard's text goes in (as one paste).
    h.frame(vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
    h.frame(vec![
        key(egui::Key::End, egui::Modifiers::CTRL),
        key(egui::Key::Enter, egui::Modifiers::NONE),
        egui::Event::Paste("gamma\n- [ ] delta".into()),
    ]);
    let lines = h.lines();
    assert!(lines.iter().any(|l| l == "gamma"), "{lines:?}");
    assert!(lines.iter().any(|l| l == "- [ ] delta"), "{lines:?}");
    // What blackglass copies itself goes there too.
    (h.window.copied.borrow_mut()).replace("from the app".into());
    let out = h.frame(Vec::new());
    assert!(
        out.platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText("from the app".into()))
    );
}

#[test]
fn closing_asks_about_unsaved_changes_first() {
    let mut h = Harness::new("gui-close", &[("N.md", "x\n")], "N.md");
    h.frame(Vec::new());
    // Nothing unsaved: the window closes (nothing cancels it).
    let out = h.frame_with(Vec::new(), true);
    let commands = &out.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(!commands.contains(&egui::ViewportCommand::CancelClose));
    // Unsaved: closing is cancelled and the question asked.
    h.frame(vec![egui::Event::Text("y".into())]);
    let out = h.frame_with(Vec::new(), true);
    let commands = &out.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(commands.contains(&egui::ViewportCommand::CancelClose));
    assert!(matches!(h.app().prompt, Some(Prompt::Unsaved { .. })));
}

#[test]
fn the_grid_follows_the_window_and_the_font() {
    let mut h = Harness::new("gui-size", &[("N.md", "x\n")], "N.md");
    h.frame(Vec::new());
    let big = h.window.terminal.backend().buffer.area;
    h.size = egui::vec2(600.0, 400.0);
    h.frame(Vec::new());
    let small = h.window.terminal.backend().buffer.area;
    assert!(small.width < big.width && small.height < big.height);
    // A bigger font: fewer cells.
    let cell = h.window.input.cell;
    let dir = scratch("gui-size-config");
    let size = crate::window_settings::Settings::schema()
        .into_iter()
        .find(|s| s.key == "size")
        .unwrap();
    if let Screen::Vault(app) = &mut h.window.screen {
        app.config_dir = Some(dir.clone());
        app.set_setting(crate::workspace::Target::Window, &size, "24")
            .unwrap();
    }
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        h.window.input.cell.y > cell.y,
        "{:?} > {cell:?}",
        h.window.input.cell
    );
    let fewer = h.window.terminal.backend().buffer.area;
    assert!(fewer.height < small.height);
    // A font that isn't there: said.
    let font = crate::window_settings::Settings::schema()
        .into_iter()
        .find(|s| s.key == "font")
        .unwrap();
    if let Screen::Vault(app) = &mut h.window.screen {
        app.set_setting(crate::workspace::Target::Window, &font, "NoSuchFont")
            .unwrap();
    }
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        h.app().message.contains("no font NoSuchFont"),
        "{}",
        h.app().message
    );
}

/// A 40×20 PNG.
fn png(path: &Path) {
    image::RgbaImage::from_pixel(40, 20, image::Rgba([200, 60, 30, 255]))
        .save(path)
        .unwrap();
}

#[test]
fn images_are_painted_as_pictures() {
    let mut h = Harness::new(
        "gui-image",
        &[("P.md", "top\n\n![[pic.png]]\n\nend\n")],
        "P.md",
    );
    png(&h.root.join("pic.png"));
    let mut seen = 0;
    for _ in 0..4 {
        seen = seen.max(pictures(&h.frame(Vec::new())));
    }
    assert!(seen >= 1, "the image is painted");
    assert!(
        h.window.picker_cell.is_some(),
        "an iTerm2 picker for the cells"
    );
}

#[test]
fn emoji_are_painted_in_color_when_the_system_can() {
    let mut h = Harness::new("gui-emoji", &[("E.md", "top\n\nparty 🎉 time\n")], "E.md");
    let out = h.frame(Vec::new());
    let can = emoji::Emoji::new(&fonts::found()).draw("🎉", 16).is_some();
    if can {
        assert!(pictures(&out) >= 1, "a color emoji");
        assert!(!painted(&out).contains('🎉'), "not as text too");
    } else {
        assert!(painted(&out).contains('🎉'), "as text");
    }
}

#[test]
fn the_picker_opens_a_vault() {
    let dir = scratch("gui-picker");
    write(&dir, &[("Inside.md", "inside\n")]);
    let copied = Clipboard::default();
    let window = Window::new(picker(None, false), copied, false);
    let mut h = Harness::with(window, dir.clone());
    let out = h.frame(Vec::new());
    assert!(painted(&out).contains("Open vault"), "{}", painted(&out));
    // The field starts at ~/: cleared, then the vault's path.
    let backspace = key(egui::Key::Backspace, egui::Modifiers::NONE);
    h.frame(vec![backspace.clone(), backspace]);
    h.frame(vec![egui::Event::Text(dir.display().to_string())]);
    h.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    let out = h.frame(Vec::new());
    assert!(
        matches!(h.window.screen, Screen::Vault(_)),
        "{}",
        painted(&out)
    );
    assert!(painted(&out).contains("Inside"), "{}", painted(&out));
}

#[test]
fn key_names_match_what_the_app_expects() {
    // The window's keys come in as the terminal's (Ctrl+Shift+S as 'S').
    let mut t = Translator::default();
    let got = t.translate(&key(
        egui::Key::S,
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    ));
    match &got[..] {
        [Input::Key(k)] => assert_eq!(k.code, KeyCode::Char('S')),
        other => panic!("{other:?}"),
    }
}
