//! blackglass's colors: a theme (a TOML file of named colors, W-69), by
//! default a dark one after Obsidian's with a rainbow of folder colors.
//! Every color goes through mdedit's [`fit_color`], so it also works in
//! 256- and 16-color terminals, and every symbol has an ASCII fallback.
//!
//! The drawing code names colors by their role ([`TEXT`], [`ACCENT`] …, each
//! a [`Paint`]); [`Theme`] turns a role into the chosen theme's color. The
//! built-in themes are in `themes/` (embedded); the user's own are in
//! `~/.config/blackglass/themes/` ([`themes`]).

use std::path::Path;
use std::sync::OnceLock;

use mdedit::terminal::{Capabilities, ColorDepth, fit_color};
use ratatui::style::{Color, Modifier, Style};

use crate::plugins::settings::Values;

/// A color's role in the interface; the theme gives each one its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Chrome,
    Sidebar,
    ActiveTab,
    Raised,
    Selected,
    Prompt,
    Text,
    Muted,
    Faint,
    Accent,
    Title,
    Tag,
    Dirty,
}

/// Every role with its key in a theme file, in [`Role`] order.
const ROLES: [(&str, Role); 13] = [
    ("chrome", Role::Chrome),
    ("sidebar", Role::Sidebar),
    ("active_tab", Role::ActiveTab),
    ("raised", Role::Raised),
    ("selected", Role::Selected),
    ("prompt", Role::Prompt),
    ("text", Role::Text),
    ("muted", Role::Muted),
    ("faint", Role::Faint),
    ("accent", Role::Accent),
    ("title", Role::Title),
    ("tag", Role::Tag),
    ("dirty", Role::Dirty),
];

/// What a cell is painted with: a role's color in the theme, or a fixed
/// color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    Role(Role),
    Fixed(Color),
}

impl From<Role> for Paint {
    fn from(role: Role) -> Self {
        Paint::Role(role)
    }
}

impl From<Color> for Paint {
    fn from(color: Color) -> Self {
        Paint::Fixed(color)
    }
}

/// Tab bar and status bar.
pub const BG_CHROME: Paint = Paint::Role(Role::Chrome);
pub const BG_SIDEBAR: Paint = Paint::Role(Role::Sidebar);
pub const BG_ACTIVE_TAB: Paint = Paint::Role(Role::ActiveTab);
/// Inputs, the sidebar's active panel tab and the open note's row.
pub const BG_RAISED: Paint = Paint::Role(Role::Raised);
/// The selected row while its list has the focus.
pub const BG_SELECTED: Paint = Paint::Role(Role::Selected);
pub const BG_PROMPT: Paint = Paint::Role(Role::Prompt);
pub const TEXT: Paint = Paint::Role(Role::Text);
pub const MUTED: Paint = Paint::Role(Role::Muted);
pub const FAINT: Paint = Paint::Role(Role::Faint);
/// The accent (purple in the default theme).
pub const ACCENT: Paint = Paint::Role(Role::Accent);
/// The note's name above its text (Obsidian's "inline title").
pub const TITLE: Paint = Paint::Role(Role::Title);
pub const TAG: Paint = Paint::Role(Role::Tag);
/// The unsaved-changes dot on a tab.
pub const DIRTY: Paint = Paint::Role(Role::Dirty);

/// How many folder colors a theme has.
const FOLDERS: usize = 8;

/// The built-in themes: (id, file), the default first.
pub const BUILTIN: [(&str, &str); 6] = [
    ("default", include_str!("../../themes/default.toml")),
    ("paper", include_str!("../../themes/paper.toml")),
    ("ember", include_str!("../../themes/ember.toml")),
    ("ocean", include_str!("../../themes/ocean.toml")),
    ("forest", include_str!("../../themes/forest.toml")),
    ("contrast", include_str!("../../themes/contrast.toml")),
];

/// A theme's colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    roles: [Color; ROLES.len()],
    /// Top-level folders take these colors in turn (alphabetically), and
    /// everything inside a folder shows its color.
    folders: [Color; FOLDERS],
    /// The editor's heading colors (`None`: the editor's own).
    pub headings: [Option<Color>; 6],
    /// The editor's colors: its text, its page, and what it draws instead
    /// of each named color (`color_red` …).
    pub editor: mdedit::palette::Palette,
}

/// The editor's color keys after `editor_text` and `editor_bg`: one per
/// named color, in `mdedit::palette::NAMED` order.
const EDITOR_COLORS: [&str; 16] = [
    "color_black",
    "color_red",
    "color_green",
    "color_yellow",
    "color_blue",
    "color_magenta",
    "color_cyan",
    "color_gray",
    "color_darkgray",
    "color_lightred",
    "color_lightgreen",
    "color_lightyellow",
    "color_lightblue",
    "color_lightmagenta",
    "color_lightcyan",
    "color_white",
];

/// Where an editor key's color goes in the editor's palette.
fn editor_slot<'a>(
    editor: &'a mut mdedit::palette::Palette,
    key: &str,
) -> Option<&'a mut Option<Color>> {
    match key {
        "editor_text" => Some(&mut editor.text),
        "editor_bg" => Some(&mut editor.background),
        _ => {
            let i = EDITOR_COLORS.iter().position(|k| *k == key)?;
            Some(&mut editor.named[i])
        }
    }
}

thread_local! {
    /// The theme in use, for what draws without a [`Theme`] at hand (the
    /// plugins' results).
    static CURRENT: std::cell::Cell<Option<Palette>> = const { std::cell::Cell::new(None) };
}

thread_local! {
    /// Whether the terminal shows Unicode symbols, for [`glyph`].
    static UNICODE: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Sets what [`glyph`] chooses by (each frame, from the terminal).
pub fn set_unicode(unicode: bool) {
    UNICODE.with(|u| u.set(unicode));
}

/// `unicode`, or `ascii` where the terminal can't show it, for what draws
/// without a [`Theme`] at hand (a plugin's rendered text).
pub fn glyph(unicode: &'static str, ascii: &'static str) -> &'static str {
    if UNICODE.with(std::cell::Cell::get) {
        unicode
    } else {
        ascii
    }
}

/// Uses `palette` for [`current`].
pub fn set_current(palette: Palette) {
    CURRENT.with(|c| c.set(Some(palette)));
}

/// The theme in use (the default before one is chosen).
pub fn current() -> Palette {
    CURRENT.with(|c| c.get()).unwrap_or_default()
}

/// `paint`'s color in the theme in use (plugins draw with it; the frame is
/// fitted to the terminal at the end).
pub fn paint(paint: impl Into<Paint>) -> Color {
    match paint.into() {
        Paint::Role(role) => current().role(role),
        Paint::Fixed(color) => color,
    }
}

impl Default for Palette {
    /// The default theme's colors.
    fn default() -> Self {
        static DEFAULT: OnceLock<Palette> = OnceLock::new();
        *DEFAULT.get_or_init(|| {
            let (palette, warnings) = Palette::over(Palette::BLANK, BUILTIN[0].1);
            assert!(
                warnings.is_empty(),
                "the default theme is valid: {warnings:?}"
            );
            palette
        })
    }
}

impl Palette {
    /// Only for reading the default theme over.
    const BLANK: Palette = Palette {
        roles: [Color::Reset; ROLES.len()],
        folders: [Color::Reset; FOLDERS],
        headings: [None; 6],
        editor: mdedit::palette::Palette {
            named: [None; 16],
            text: None,
            background: None,
        },
    };

    /// A theme file's colors; keys it leaves out are the default theme's.
    /// Returns a warning per bad value.
    pub fn parse(text: &str) -> (Palette, Vec<String>) {
        Palette::over(Palette::default(), text)
    }

    fn over(mut palette: Palette, text: &str) -> (Palette, Vec<String>) {
        let numbered = |key: &str, prefix: &str, max: usize| {
            key.strip_prefix(prefix)
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|n| (1..=max).contains(n))
        };
        let mut warnings = Vec::new();
        for (key, value) in Values::parse(text).section("") {
            let role = ROLES.iter().position(|(k, _)| *k == key);
            let folder = numbered(key, "folder", FOLDERS);
            let heading = numbered(key, "heading", 6);
            if key == "name" {
                continue;
            }
            if let Some(slot) = editor_slot(&mut palette.editor, key) {
                if value == "default" {
                    *slot = None;
                } else {
                    match mdedit::config::parse_color(value) {
                        Some(color) => *slot = Some(color),
                        None => warnings.push(format!("{key} must be a color, not {value:?}")),
                    }
                }
                continue;
            }
            if role.is_none() && folder.is_none() && heading.is_none() {
                warnings.push(format!("unknown key {key:?}"));
                continue;
            }
            if let Some(n) = heading
                && value == "default"
            {
                palette.headings[n - 1] = None;
                continue;
            }
            let Some(color) = mdedit::config::parse_color(value) else {
                warnings.push(format!("{key} must be a color, not {value:?}"));
                continue;
            };
            match (role, folder, heading) {
                (Some(i), _, _) => palette.roles[i] = color,
                (_, Some(n), _) => palette.folders[n - 1] = color,
                (_, _, Some(n)) => palette.headings[n - 1] = Some(color),
                _ => unreachable!("checked above"),
            }
        }
        (palette, warnings)
    }

    pub fn role(&self, role: Role) -> Color {
        self.roles[role as usize]
    }
}

/// Theme `id`'s colors and its file's problems; `None` if there's no such
/// theme.
pub fn load(config_dir: Option<&Path>, id: &str) -> Option<(Palette, Vec<String>)> {
    themes(config_dir)
        .into_iter()
        .find(|(i, _)| i == id)
        .map(|(_, text)| Palette::parse(&text))
}

/// The id of the theme saved in `file` (`appearance.toml`).
pub fn saved(file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    Values::parse(&text).get("", "theme").map(String::from)
}

/// A theme's name in "Choose theme": its `name`, or its id.
pub fn name(id: &str, text: &str) -> String {
    Values::parse(text)
        .get("", "name")
        .map_or_else(|| id.to_string(), String::from)
}

/// Every theme: (id, file text), the built-in ones, then the user's from
/// `themes/` in `config_dir` (by name); a user's file with a built-in's id
/// replaces it.
pub fn themes(config_dir: Option<&Path>) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = BUILTIN
        .iter()
        .map(|(id, text)| (id.to_string(), text.to_string()))
        .collect();
    let Some(dir) = config_dir else {
        return all;
    };
    let Ok(entries) = std::fs::read_dir(dir.join("themes")) else {
        return all;
    };
    let mut own: Vec<(String, String)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .filter_map(|p| {
            let id = p.file_stem()?.to_str()?.to_string();
            Some((id, std::fs::read_to_string(&p).ok()?))
        })
        .collect();
    own.sort_by(|a, b| a.0.cmp(&b.0));
    for (id, text) in own {
        match all.iter_mut().find(|(i, _)| *i == id) {
            Some(theme) => theme.1 = text,
            None => all.push((id, text)),
        }
    }
    all
}

/// How strongly a top-level folder's row is tinted with its color.
const FOLDER_TINT: f32 = 0.16;

/// The colors and symbols for one frame, fitted to the terminal.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    depth: ColorDepth,
    pub unicode: bool,
    palette: Palette,
}

impl Theme {
    pub fn new(caps: Capabilities, palette: Palette) -> Self {
        Theme {
            depth: caps.colors,
            unicode: caps.unicode,
            palette,
        }
    }

    /// `paint`'s color in this theme (RGB).
    fn rgb(&self, paint: impl Into<Paint>) -> Color {
        match paint.into() {
            Paint::Role(role) => self.palette.role(role),
            Paint::Fixed(color) => color,
        }
    }

    /// `paint`'s color, fitted to the terminal.
    pub fn color(&self, paint: impl Into<Paint>) -> Color {
        fit_color(self.rgb(paint), self.depth)
    }

    /// Text in `fg` on the terminal's own background (the editor's).
    pub fn fg(&self, fg: impl Into<Paint>) -> Style {
        Style::new().fg(self.color(fg))
    }

    /// Text in `fg` on `bg`.
    pub fn on(&self, fg: impl Into<Paint>, bg: impl Into<Paint>) -> Style {
        Style::new().fg(self.color(fg)).bg(self.color(bg))
    }

    pub fn bold(&self, fg: impl Into<Paint>, bg: impl Into<Paint>) -> Style {
        self.on(fg, bg).add_modifier(Modifier::BOLD)
    }

    /// `unicode`, or `ascii` where the terminal can't show it.
    pub fn glyph(&self, unicode: &'static str, ascii: &'static str) -> &'static str {
        if self.unicode { unicode } else { ascii }
    }

    /// The color of top-level folder number `i`.
    pub fn folder(&self, i: usize) -> Color {
        self.palette.folders[i % FOLDERS]
    }

    /// A top-level folder row's background: its color, faintly.
    pub fn folder_tint(&self, i: usize) -> Color {
        mix(self.folder(i), self.rgb(BG_SIDEBAR), FOLDER_TINT)
    }
}

/// `amount` of `color` over `base` (RGB colors; anything else is `base`).
pub fn mix(color: Color, base: Color, amount: f32) -> Color {
    let (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) = (color, base) else {
        return base;
    };
    let channel = |c: u8, base: u8| {
        (f32::from(base) + (f32::from(c) - f32::from(base)) * amount).round() as u8
    };
    Color::Rgb(channel(r, br), channel(g, bg), channel(b, bb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixing_moves_toward_the_color() {
        let black = Color::Rgb(0, 0, 0);
        assert_eq!(
            mix(Color::Rgb(200, 100, 0), black, 0.5),
            Color::Rgb(100, 50, 0)
        );
        assert_eq!(mix(Color::Red, black, 0.5), black, "not RGB");
    }

    #[test]
    fn colors_fit_the_terminal() {
        let basic = Theme::new(
            Capabilities {
                colors: ColorDepth::Basic16,
                ..Capabilities::default()
            },
            Palette::default(),
        );
        assert!(!matches!(basic.color(ACCENT), Color::Rgb(..)));
        let ascii = Theme::new(
            Capabilities {
                unicode: false,
                ..Capabilities::default()
            },
            Palette::default(),
        );
        assert_eq!(ascii.glyph("▾", "v"), "v");
        assert_eq!(ascii.folder(9), ascii.folder(1), "colors repeat");
    }

    #[test]
    fn every_built_in_theme_sets_every_key() {
        for (id, text) in BUILTIN {
            let (_, warnings) = Palette::parse(text);
            assert!(warnings.is_empty(), "{id}: {warnings:?}");
            let values = Values::parse(text);
            let keys = ROLES
                .iter()
                .map(|(k, _)| k.to_string())
                .chain((1..=FOLDERS).map(|n| format!("folder{n}")))
                .chain((1..=6).map(|n| format!("heading{n}")))
                .chain(["editor_text".to_string(), "editor_bg".to_string()])
                .chain(EDITOR_COLORS.iter().map(|k| k.to_string()))
                .chain(["name".to_string()]);
            for key in keys {
                assert!(values.get("", &key).is_some(), "{id} sets {key}");
            }
        }
        let names: Vec<String> = BUILTIN.iter().map(|(id, t)| name(id, t)).collect();
        assert_eq!(
            names,
            [
                "Default",
                "Paper",
                "Ember",
                "Ocean",
                "Forest",
                "High contrast"
            ]
        );
    }

    #[test]
    fn a_partial_theme_falls_back_to_the_default() {
        let (palette, warnings) = Palette::parse(
            "accent = \"#ff0000\"\nheading1 = \"blue\"\ntext = \"nope\"\nwho = \"red\"",
        );
        assert_eq!(palette.role(Role::Accent), Color::Rgb(0xff, 0, 0));
        assert_eq!(palette.headings[0], Some(Color::Blue));
        let default = Palette::default();
        assert_eq!(palette.role(Role::Text), default.role(Role::Text));
        assert_eq!(palette.role(Role::Chrome), Color::Rgb(0x14, 0x14, 0x14));
        assert_eq!(palette.headings[1], None, "the editor's own");
        assert_eq!(
            warnings,
            ["text must be a color, not \"nope\"", "unknown key \"who\""]
        );
    }
}
