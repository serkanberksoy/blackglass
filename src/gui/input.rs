//! The window's input as the terminal's events: keys (Ctrl / Alt
//! shortcuts from key events, typed text from text events; AltGr's text
//! stays text, and so does an input method's composed text), copy, cut
//! and paste, and the mouse by cell (its side buttons as back / forward).

use eframe::egui;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// A terminal event the app handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
}

/// What the translation remembers between events.
#[derive(Debug, Clone, Default)]
pub struct Translator {
    /// A cell's size and the grid's top left, in points.
    pub cell: egui::Vec2,
    pub origin: egui::Pos2,
    /// The button held (a move is a drag), and the cell last reported.
    held: Option<MouseButton>,
    at: Option<(u16, u16)>,
    /// Wheel movement in points not yet a whole line.
    wheel: f32,
}

fn modifiers(m: egui::Modifiers) -> KeyModifiers {
    let mut out = KeyModifiers::NONE;
    // On macOS, Cmd is the shortcut key (Cmd+S saves).
    if m.ctrl || m.mac_cmd {
        out |= KeyModifiers::CONTROL;
    }
    if m.alt {
        out |= KeyModifiers::ALT;
    }
    if m.shift {
        out |= KeyModifiers::SHIFT;
    }
    out
}

/// A shortcut is Ctrl or Alt (not both: that's AltGr, which types).
fn shortcut(m: egui::Modifiers) -> bool {
    (m.ctrl || m.mac_cmd) != m.alt
}

/// A key that isn't text: its code.
fn named(key: egui::Key, shift: bool) -> Option<KeyCode> {
    use egui::Key as K;
    Some(match key {
        K::Enter => KeyCode::Enter,
        K::Tab if shift => KeyCode::BackTab,
        K::Tab => KeyCode::Tab,
        K::Backspace => KeyCode::Backspace,
        K::Delete => KeyCode::Delete,
        K::Insert => KeyCode::Insert,
        K::Escape => KeyCode::Esc,
        K::Home => KeyCode::Home,
        K::End => KeyCode::End,
        K::PageUp => KeyCode::PageUp,
        K::PageDown => KeyCode::PageDown,
        K::ArrowLeft => KeyCode::Left,
        K::ArrowRight => KeyCode::Right,
        K::ArrowUp => KeyCode::Up,
        K::ArrowDown => KeyCode::Down,
        K::F1 => KeyCode::F(1),
        K::F2 => KeyCode::F(2),
        K::F3 => KeyCode::F(3),
        K::F4 => KeyCode::F(4),
        K::F5 => KeyCode::F(5),
        K::F6 => KeyCode::F(6),
        K::F7 => KeyCode::F(7),
        K::F8 => KeyCode::F(8),
        K::F9 => KeyCode::F(9),
        K::F10 => KeyCode::F(10),
        K::F11 => KeyCode::F(11),
        K::F12 => KeyCode::F(12),
        _ => return None,
    })
}

impl Translator {
    /// The cell at `pos` (inside the grid).
    fn cell_at(&self, pos: egui::Pos2) -> (u16, u16) {
        let x = ((pos.x - self.origin.x) / self.cell.x.max(1.0)).max(0.0);
        let y = ((pos.y - self.origin.y) / self.cell.y.max(1.0)).max(0.0);
        (x as u16, y as u16)
    }

    fn mouse(&self, kind: MouseEventKind, (column, row): (u16, u16), m: egui::Modifiers) -> Input {
        Input::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: modifiers(m),
        })
    }

    /// `event` as the terminal's events (none, one or several).
    pub fn translate(&mut self, event: &egui::Event) -> Vec<Input> {
        let key = |code: KeyCode, mods: KeyModifiers| Input::Key(KeyEvent::new(code, mods));
        match event {
            egui::Event::Key {
                key: k,
                pressed: true,
                modifiers: m,
                ..
            } => {
                if let Some(code) = named(*k, m.shift) {
                    let mut mods = modifiers(*m);
                    if code == KeyCode::BackTab {
                        mods -= KeyModifiers::SHIFT;
                    }
                    return vec![key(code, mods)];
                }
                // Letters, digits, punctuation, Space: the text event
                // types them, unless this is a shortcut.
                if !shortcut(*m) {
                    return Vec::new();
                }
                let symbol = match k {
                    egui::Key::Space => " ",
                    k => k.symbol_or_name(),
                };
                let mut chars = symbol.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => {
                        let c = if m.shift {
                            c.to_ascii_uppercase()
                        } else {
                            c.to_ascii_lowercase()
                        };
                        vec![key(KeyCode::Char(c), modifiers(*m))]
                    }
                    _ => Vec::new(),
                }
            }
            egui::Event::Text(text) | egui::Event::Ime(egui::ImeEvent::Commit(text)) => text
                .chars()
                .map(|c| {
                    let mods = if c.is_uppercase() {
                        KeyModifiers::SHIFT
                    } else {
                        KeyModifiers::NONE
                    };
                    key(KeyCode::Char(c), mods)
                })
                .collect(),
            egui::Event::Copy => vec![key(KeyCode::Char('c'), KeyModifiers::CONTROL)],
            egui::Event::Cut => vec![key(KeyCode::Char('x'), KeyModifiers::CONTROL)],
            egui::Event::Paste(text) => vec![Input::Paste(text.clone())],
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: m,
            } => {
                let button = match button {
                    egui::PointerButton::Primary => MouseButton::Left,
                    egui::PointerButton::Secondary => MouseButton::Right,
                    egui::PointerButton::Middle => MouseButton::Middle,
                    egui::PointerButton::Extra1 => MouseButton::Back,
                    egui::PointerButton::Extra2 => MouseButton::Forward,
                };
                let at = self.cell_at(*pos);
                self.at = Some(at);
                if *pressed {
                    self.held = Some(button);
                    vec![self.mouse(MouseEventKind::Down(button), at, *m)]
                } else {
                    self.held = None;
                    vec![self.mouse(MouseEventKind::Up(button), at, *m)]
                }
            }
            egui::Event::PointerMoved(pos) => {
                let at = self.cell_at(*pos);
                if self.at == Some(at) {
                    return Vec::new();
                }
                self.at = Some(at);
                let kind = match self.held {
                    Some(b) => MouseEventKind::Drag(b),
                    None => MouseEventKind::Moved,
                };
                vec![self.mouse(kind, at, egui::Modifiers::NONE)]
            }
            egui::Event::MouseWheel {
                unit,
                delta,
                modifiers: m,
                ..
            } => {
                let lines = match unit {
                    egui::MouseWheelUnit::Line => delta.y,
                    egui::MouseWheelUnit::Point => {
                        self.wheel += delta.y / self.cell.y.max(1.0);
                        let whole = self.wheel.trunc();
                        self.wheel -= whole;
                        whole
                    }
                    egui::MouseWheelUnit::Page => delta.y * 20.0,
                };
                let kind = if lines > 0.0 {
                    MouseEventKind::ScrollUp
                } else {
                    MouseEventKind::ScrollDown
                };
                let at = self.at.unwrap_or_default();
                (0..lines.abs().round() as usize)
                    .map(|_| self.mouse(kind, at, *m))
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, Pos2, Vec2};

    fn translator() -> Translator {
        Translator {
            cell: Vec2::new(10.0, 20.0),
            origin: Pos2::new(5.0, 5.0),
            ..Translator::default()
        }
    }

    fn key_event(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn keys(t: &mut Translator, events: &[Event]) -> Vec<(KeyCode, KeyModifiers)> {
        events
            .iter()
            .flat_map(|e| t.translate(e))
            .map(|i| match i {
                Input::Key(k) => (k.code, k.modifiers),
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn shortcuts_named_keys_and_typed_text() {
        let mut t = translator();
        let ctrl = Modifiers::CTRL;
        let alt = Modifiers::ALT;
        let shift_tab = Modifiers::SHIFT;
        assert_eq!(
            keys(
                &mut t,
                &[
                    key_event(Key::P, ctrl),
                    key_event(Key::V, alt),
                    key_event(Key::S, ctrl | Modifiers::SHIFT),
                    key_event(Key::Comma, alt),
                    key_event(Key::Enter, Modifiers::NONE),
                    key_event(Key::Tab, shift_tab),
                    key_event(Key::F2, Modifiers::NONE),
                    key_event(Key::A, Modifiers::NONE),
                    Event::Text("aB".into()),
                ]
            ),
            [
                (KeyCode::Char('p'), KeyModifiers::CONTROL),
                (KeyCode::Char('v'), KeyModifiers::ALT),
                (
                    KeyCode::Char('S'),
                    KeyModifiers::CONTROL | KeyModifiers::SHIFT
                ),
                (KeyCode::Char(','), KeyModifiers::ALT),
                (KeyCode::Enter, KeyModifiers::NONE),
                (KeyCode::BackTab, KeyModifiers::NONE),
                (KeyCode::F(2), KeyModifiers::NONE),
                (KeyCode::Char('a'), KeyModifiers::NONE),
                (KeyCode::Char('B'), KeyModifiers::SHIFT),
            ],
            "a plain letter comes as text, once"
        );
        // Composed text (an input method, dead keys) types too.
        assert_eq!(
            keys(&mut t, &[Event::Ime(egui::ImeEvent::Commit("é漢".into()))]),
            [
                (KeyCode::Char('é'), KeyModifiers::NONE),
                (KeyCode::Char('漢'), KeyModifiers::NONE)
            ]
        );
        // AltGr (Ctrl+Alt) types: its text, no shortcut.
        assert!(keys(&mut t, &[key_event(Key::Q, ctrl | alt)]).is_empty());
        assert_eq!(
            t.translate(&Event::Paste("hi".into())),
            [Input::Paste("hi".into())]
        );
        assert_eq!(
            keys(&mut t, &[Event::Copy]),
            [(KeyCode::Char('c'), KeyModifiers::CONTROL)]
        );
    }

    #[test]
    fn the_mouse_by_cell() {
        let mut t = translator();
        let press = |button, pressed| Event::PointerButton {
            pos: Pos2::new(36.0, 47.0),
            button,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let kinds = |t: &mut Translator, e: Event| -> Vec<(MouseEventKind, u16, u16)> {
            t.translate(&e)
                .into_iter()
                .map(|i| match i {
                    Input::Mouse(m) => (m.kind, m.column, m.row),
                    other => panic!("{other:?}"),
                })
                .collect()
        };
        assert_eq!(
            kinds(&mut t, press(egui::PointerButton::Primary, true)),
            [(MouseEventKind::Down(MouseButton::Left), 3, 2)]
        );
        assert_eq!(
            kinds(&mut t, Event::PointerMoved(Pos2::new(56.0, 47.0))),
            [(MouseEventKind::Drag(MouseButton::Left), 5, 2)]
        );
        assert!(
            kinds(&mut t, Event::PointerMoved(Pos2::new(58.0, 48.0))).is_empty(),
            "the same cell"
        );
        kinds(&mut t, press(egui::PointerButton::Primary, false));
        assert_eq!(
            kinds(&mut t, press(egui::PointerButton::Extra1, true)),
            [(MouseEventKind::Down(MouseButton::Back), 3, 2)],
            "the side buttons"
        );
        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: Vec2::new(0.0, -3.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(kinds(&mut t, wheel).len(), 3);
    }
}
