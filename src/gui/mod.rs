//! blackglass in a window of its own (`--gui`, the `gui` feature): the same
//! app and screens as in the terminal, drawn by eframe (egui) on macOS,
//! Windows and Linux. The screens are ratatui's, into a [`grid::Grid`]
//! painted with egui's GPU text; the window's keys, text, clipboard and
//! mouse come in as the terminal's events ([`input`]). The vault picker
//! comes first when no vault is given, as in the terminal.

pub mod emoji;
pub mod fonts;
pub mod grid;
pub mod images;
pub mod input;
#[cfg(test)]
mod tests;

use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use mdedit::terminal::Capabilities;
use ratatui::Terminal;
use ratatui::crossterm::event::MouseEventKind;
use ratatui::style::Modifier;

use crate::startup;
use crate::ui;
use crate::ui::theme::{BG_CHROME, TEXT};
use crate::vault_picker::{Picked, VaultPicker};
use crate::workspace::{Action, App};
use grid::{Grid, rgb};
use input::{Input, Translator};

/// How often the app's timers and background work run when idle.
const TICK: Duration = Duration::from_millis(500);

/// What the window shows.
enum Screen {
    /// The vault picker, before a vault is open.
    Picker {
        picker: Box<VaultPicker>,
        palette: ui::theme::Palette,
    },
    Vault(Box<App>),
}

/// The window's state.
struct Window {
    screen: Screen,
    terminal: Terminal<Grid>,
    input: Translator,
    last_tick: Instant,
    /// The system's fonts, and the font settings in use (their styles).
    fonts: Vec<PathBuf>,
    applied: Option<crate::window_settings::Settings>,
    styles: fonts::Styles,
    /// New fonts' styles, used from the next frame (when egui has them).
    waiting: Option<fonts::Styles>,
    /// The images drawn, and the cell size (pixels) the editor's image
    /// picker was made for.
    textures: images::Textures,
    picker_cell: Option<(u16, u16)>,
    /// The color emoji drawn.
    emoji: emoji::Emoji,
    /// Text the app copied, for the window's clipboard.
    copied: Clipboard,
    /// The window's settings as the user's file has them (for the picker,
    /// before an app keeps them; read once).
    user_settings: crate::window_settings::Settings,
    /// Whether apps use the user's config folder (off in tests).
    user_config: bool,
}

/// Text waiting to go on the window's clipboard.
type Clipboard = std::rc::Rc<std::cell::RefCell<Option<String>>>;

/// Opens `path` (a vault or a note; `None`: the vault picker first) in a
/// window, and runs until it's closed.
pub fn run(path: Option<PathBuf>) -> io::Result<()> {
    let (path, problem) = startup::first(path);
    let copied = Clipboard::default();
    let screen = open(path, problem, &copied, true);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("blackglass")
            .with_app_id("blackglass")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([480.0, 320.0])
            .with_icon(icon()),
        ..eframe::NativeOptions::default()
    };
    let window = Window::new(screen, copied, true);
    eframe::run_native(
        "blackglass",
        options,
        Box::new(|cc| {
            let mut window = window;
            window.start(&cc.egui_ctx);
            Ok(Box::new(window))
        }),
    )
    .map_err(|e| io::Error::other(e.to_string()))
}

/// The window's icon (`packaging/icon/`).
fn icon() -> egui::IconData {
    let png = include_bytes!("../../packaging/icon/blackglass-256.png");
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("the icon is a PNG")
        .to_rgba8();
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

/// The vault `path` names, as an app; else the picker (saying why).
/// (`user_config`: with the user's config folder; else none, as in tests.)
fn open(
    path: Option<PathBuf>,
    problem: Option<String>,
    copied: &Clipboard,
    user_config: bool,
) -> Screen {
    if let Some(path) = path {
        match startup::open(&path) {
            Ok((vault, note)) => {
                return Screen::Vault(Box::new(app(vault, note, copied, user_config)));
            }
            Err(e) => return picker(Some(e), user_config),
        }
    }
    picker(problem, user_config)
}

fn picker(problem: Option<String>, user_config: bool) -> Screen {
    let (mut picker, palette) = if user_config {
        startup::picker(None)
    } else {
        (
            VaultPicker::new(Vec::new(), crate::config::home()),
            ui::theme::Palette::default(),
        )
    };
    if let Some(problem) = problem {
        picker.note = problem;
    }
    Screen::Picker {
        picker: Box::new(picker),
        palette,
    }
}

/// The app for the window: every color (no terminal to fit); images as
/// half blocks until the window knows its cells' size (then as pictures).
fn app(
    vault: crate::vault::Vault,
    note: Option<PathBuf>,
    copied: &Clipboard,
    user_config: bool,
) -> App {
    let mut app = if user_config {
        startup::app(vault, note)
    } else {
        let mut app = App::new(vault);
        if let Some(note) = note.and_then(|n| mdedit::platform::canonical(&n).ok()) {
            app.open(&note);
        }
        app
    };
    app.windowed = true;
    // What blackglass copies goes on the window's clipboard.
    let outbox = std::rc::Rc::clone(copied);
    app.copier = Box::new(move |text: &str| {
        *outbox.borrow_mut() = Some(text.to_string());
        Ok(())
    });
    app.shared.caps = Capabilities::default();
    app.shared.picker = match app.shared.config.images {
        mdedit::config::Images::Off => None,
        _ => mdedit::images::picker_for(mdedit::config::Images::Halfblocks),
    };
    app
}

impl Window {
    fn new(screen: Screen, copied: Clipboard, user_config: bool) -> Self {
        let found = fonts::found();
        Window {
            screen,
            terminal: Terminal::new(Grid::new(80, 24)).expect("the grid has no I/O to fail"),
            input: Translator::default(),
            last_tick: Instant::now(),
            emoji: emoji::Emoji::new(&found),
            copied,
            user_settings: if user_config {
                crate::window_settings::Settings::load(crate::config::config_dir().as_deref())
            } else {
                crate::window_settings::Settings::default()
            },
            user_config,
            fonts: found,
            applied: None,
            styles: fonts::Styles::default(),
            waiting: None,
            textures: images::Textures::default(),
            picker_cell: None,
        }
    }

    /// Before the first frame: the fonts, so it's drawn in them.
    fn start(&mut self, ctx: &egui::Context) {
        let settings = self.settings();
        let styles = fonts::install(ctx, &settings.font, &self.fonts);
        self.applied = Some(settings);
        self.use_styles(styles);
    }

    /// The window's settings: the app's, or (the picker) the user's file's.
    fn settings(&self) -> crate::window_settings::Settings {
        match &self.screen {
            Screen::Vault(app) => app.window_settings(),
            Screen::Picker { .. } => self.user_settings.clone(),
        }
    }

    /// The fonts' styles in use; a font that isn't there is said (the
    /// default is used).
    fn use_styles(&mut self, styles: fonts::Styles) {
        if !styles.missing.is_empty()
            && let Screen::Vault(app) = &mut self.screen
        {
            app.message = format!(
                "Window: no font {0} here ({0}-Regular.ttf, {0}.ttf …); the default is used",
                styles.missing
            );
        }
        self.styles = styles;
    }

    /// The window's events, to the picker or the app; whether to close.
    fn handle(&mut self, events: &[Input]) -> bool {
        for event in events {
            match &mut self.screen {
                Screen::Picker { picker, .. } => {
                    let picked = match event {
                        Input::Key(key) => picker.key(*key),
                        Input::Paste(text) => {
                            picker.paste(text);
                            Picked::None
                        }
                        Input::Mouse(_) => Picked::None,
                    };
                    match picked {
                        Picked::Open(path) => {
                            self.screen = open(Some(path), None, &self.copied, self.user_config);
                        }
                        Picked::Cancel => return true,
                        Picked::None => {}
                    }
                }
                Screen::Vault(app) => {
                    let action = match event {
                        Input::Key(key) => app.handle_key(*key),
                        Input::Mouse(m) if m.kind == MouseEventKind::Moved => {
                            app.mouse_moved(*m);
                            Action::Continue
                        }
                        Input::Mouse(m) => app.handle_mouse(*m),
                        Input::Paste(text) => {
                            app.handle_paste(text);
                            Action::Continue
                        }
                    };
                    if action == Action::Quit || app.quit_requested {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Draws the screen into the grid.
    fn draw(&mut self) {
        let caps = Capabilities::default();
        let drawn = match &mut self.screen {
            Screen::Picker {
                picker, palette, ..
            } => self
                .terminal
                .draw(|f| ui::draw_launcher(f, picker, caps, *palette)),
            Screen::Vault(app) => self.terminal.draw(|f| ui::draw(f, app)),
        };
        drawn.expect("the grid has no I/O to fail");
    }
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
}

impl Window {
    /// A frame: the window's input to the app, its timers, then the
    /// screen drawn and painted.
    fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let settings = self.settings();
        if self.applied.as_ref() != Some(&settings) {
            // egui has new fonts from the next frame: their styles then.
            self.waiting = Some(fonts::install(&ctx, &settings.font, &self.fonts));
            self.styles = fonts::Styles::default();
            self.applied = Some(settings.clone());
            ctx.request_repaint();
        } else if let Some(styles) = self.waiting.take() {
            self.use_styles(styles);
        }
        let font = egui::FontId::monospace(settings.size);
        // A cell: the font's advance (exact, so long runs of text and
        // cells placed one by one line up) and its line height.
        let cell = egui::vec2(
            ctx.fonts_mut(|f| f.glyph_width(&font, 'M')),
            ctx.fonts_mut(|f| f.row_height(&font)).round(),
        );
        let area = ui.max_rect();
        // Images: an iTerm2 picker for the cells' size in pixels.
        let ppp = ctx.pixels_per_point();
        let px = ((cell.x * ppp).round() as u16, (cell.y * ppp).round() as u16);
        if let Screen::Vault(app) = &mut self.screen
            && self.picker_cell != Some(px)
            && app.shared.config.images != mdedit::config::Images::Off
        {
            #[expect(
                deprecated,
                reason = "the window has no terminal to ask: it knows its cells' size"
            )]
            let mut picker = ratatui_image::picker::Picker::from_fontsize(px.into());
            picker.set_protocol_type(ratatui_image::picker::ProtocolType::Iterm2);
            app.shared.picker = Some(picker);
            app.shared.image_cache = mdedit::images::ImageCache::default();
            self.picker_cell = Some(px);
        }
        self.input.cell = cell;
        self.input.origin = area.min;
        let cols = ((area.width() / cell.x) as u16).max(10);
        let rows = ((area.height() / cell.y) as u16).max(5);
        self.terminal.backend_mut().resize(cols, rows);

        // Input: the window's close button asks about unsaved notes first.
        let mut close = false;
        if ctx.input(|i| i.viewport().close_requested())
            && let Screen::Vault(app) = &mut self.screen
            && app.quit() == Action::Continue
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        // Copy (Ctrl+C): the editor's selection, if any, on the clipboard.
        let selection = match &self.screen {
            Screen::Vault(app) => app.view().and_then(|v| v.editor.selected_text()),
            Screen::Picker { .. } => None,
        };
        let mut copy = None;
        let events: Vec<Input> = ctx.input(|i| {
            i.events
                .iter()
                .flat_map(|e| match (e, &selection) {
                    (egui::Event::Copy, Some(text)) => {
                        copy = Some(text.clone());
                        Vec::new()
                    }
                    _ => self.input.translate(e),
                })
                .collect()
        });
        if let Some(text) = copy {
            ctx.copy_text(text);
        }
        close |= self.handle(&events);
        // Timers and background work.
        if self.last_tick.elapsed() >= TICK {
            self.last_tick = Instant::now();
            if let Screen::Vault(app) = &mut self.screen {
                app.tick();
                close |= app.quit_requested;
            }
        }
        if let Some(text) = self.copied.borrow_mut().take() {
            ctx.copy_text(text);
        }
        if close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if let Screen::Vault(app) = &self.screen {
            let name = app
                .vault
                .root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} · blackglass")));
        }
        self.draw();
        let emoji_px = (font.size * ppp).round() as u32;
        paint(
            ui.painter(),
            area,
            cell,
            &font,
            &self.styles,
            self.terminal.backend(),
            &mut |text: &str| self.emoji.get(&ctx, text, emoji_px),
        );
        self.emoji.sweep();
        self.paint_images(ui.painter(), area, cell, ppp);
        ctx.request_repaint_after(TICK);
    }
}

impl Window {
    /// Paints the grid's images over their cells, at their size in pixels.
    fn paint_images(
        &mut self,
        painter: &egui::Painter,
        area: egui::Rect,
        cell: egui::Vec2,
        ppp: f32,
    ) {
        let grid = self.terminal.backend();
        let ctx = painter.ctx().clone();
        for y in 0..grid.buffer.area.height {
            for run in grid::runs(&grid.buffer, y).into_iter().filter(|r| r.image) {
                let Some((id, [w, h])) = self.textures.get(&ctx, &run.text) else {
                    continue;
                };
                let at = area.min + egui::vec2(run.x as f32 * cell.x, y as f32 * cell.y);
                let rect = egui::Rect::from_min_size(at, egui::vec2(w as f32, h as f32) / ppp);
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(
                    id,
                    rect.intersect(area),
                    uv_for(rect, area, uv),
                    egui::Color32::WHITE,
                );
            }
        }
        self.textures.sweep();
    }
}

/// The part of an image's texture shown when `rect` is cut to `area`.
fn uv_for(rect: egui::Rect, area: egui::Rect, uv: egui::Rect) -> egui::Rect {
    let shown = rect.intersect(area);
    let t = |p: egui::Pos2| {
        egui::pos2(
            uv.min.x + (p.x - rect.min.x) / rect.width().max(1.0) * uv.width(),
            uv.min.y + (p.y - rect.min.y) / rect.height().max(1.0) * uv.height(),
        )
    };
    egui::Rect::from_min_max(t(shown.min), t(shown.max))
}

/// Paints the grid in `area`: each run's background, then its text, in
/// its style's face (bold and italic drawn when the font has no face for
/// them).
fn paint(
    painter: &egui::Painter,
    area: egui::Rect,
    cell: egui::Vec2,
    font: &egui::FontId,
    styles: &fonts::Styles,
    grid: &Grid,
    emoji: &mut dyn FnMut(&str) -> Option<egui::TextureId>,
) {
    let text_rgb = rgb(ui::theme::paint(TEXT), [0xdd, 0xdd, 0xdd]);
    let back_rgb = rgb(ui::theme::paint(BG_CHROME), [0x1e, 0x1e, 0x1e]);
    let color = |c: [u8; 3]| egui::Color32::from_rgb(c[0], c[1], c[2]);
    painter.rect_filled(area, 0.0, color(back_rgb));
    let height = grid.buffer.area.height;
    for y in 0..height {
        for run in grid::runs(&grid.buffer, y) {
            let (mut fg, mut bg) = (rgb(run.fg, text_rgb), rgb(run.bg, back_rgb));
            if run.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if run.modifier.contains(Modifier::DIM) {
                fg = [0, 1, 2].map(|i| ((fg[i] as u16 + bg[i] as u16) / 2) as u8);
            }
            let at = area.min + egui::vec2(run.x as f32 * cell.x, y as f32 * cell.y);
            let rect = egui::Rect::from_min_size(at, egui::vec2(run.width as f32 * cell.x, cell.y));
            if bg != back_rgb {
                painter.rect_filled(rect, 0.0, color(bg));
            }
            // A color emoji: its drawing, in its cells.
            if run.width == 2
                && emoji::is_emoji(&run.text)
                && let Some(id) = emoji(&run.text)
            {
                let size = emoji::size(cell.y);
                let image = egui::Rect::from_center_size(rect.center(), size);
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(id, image, uv, egui::Color32::WHITE);
                continue;
            }
            if run.image || run.text.trim().is_empty() || run.modifier.contains(Modifier::HIDDEN) {
                continue;
            }
            let stroke = egui::Stroke::new(1.0, color(fg));
            let bold = run.modifier.contains(Modifier::BOLD);
            let italic = run.modifier.contains(Modifier::ITALIC);
            let family = match (bold && styles.bold, italic && styles.italic) {
                (true, true) => egui::FontFamily::Name(fonts::BOLD_ITALIC.into()),
                (true, false) => egui::FontFamily::Name(fonts::BOLD.into()),
                (false, true) => egui::FontFamily::Name(fonts::ITALIC.into()),
                (false, false) => egui::FontFamily::Monospace,
            };
            let format = egui::TextFormat {
                font_id: egui::FontId::new(font.size, family),
                color: color(fg),
                italics: italic && !styles.italic,
                underline: if run.modifier.contains(Modifier::UNDERLINED) {
                    stroke
                } else {
                    egui::Stroke::NONE
                },
                strikethrough: if run.modifier.contains(Modifier::CROSSED_OUT) {
                    stroke
                } else {
                    egui::Stroke::NONE
                },
                ..egui::TextFormat::default()
            };
            let mut job = egui::text::LayoutJob::single_section(run.text.clone(), format);
            job.wrap.max_width = f32::INFINITY;
            let galley = painter.layout_job(job);
            painter.galley(at, galley.clone(), color(fg));
            // Bold: the text again, half a pixel over (the font has no bold).
            if bold && !styles.bold {
                painter.galley(at + egui::vec2(0.6, 0.0), galley, color(fg));
            }
        }
    }
    if grid.cursor_shown {
        let at =
            area.min + egui::vec2(grid.cursor.x as f32 * cell.x, grid.cursor.y as f32 * cell.y);
        let bar = egui::Rect::from_min_size(at, egui::vec2(2.0, cell.y));
        painter.rect_filled(bar, 0.0, color(text_rgb));
    }
}
