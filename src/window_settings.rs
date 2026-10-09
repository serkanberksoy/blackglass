//! The window's settings (`--gui`): its font and the text's size, kept
//! for the user (`window.toml` in the config folder) on the settings
//! window's Window page, shown while blackglass runs in a window.

use std::path::Path;

use crate::plugins::settings::{Kind, Setting, Values};

/// The settings file, in the config folder.
pub const FILE: &str = "window.toml";

/// The window's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// A font family by its files' name (`JetBrainsMono`: its `-Regular`,
    /// `-Bold`, `-Italic` files); empty: the default.
    pub font: String,
    /// The text's size in points.
    pub size: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            font: String::new(),
            size: 15.0,
        }
    }
}

impl Settings {
    /// The settings window's rows.
    pub fn schema() -> Vec<Setting> {
        vec![
            Setting::new(
                "",
                "font",
                "Font",
                "A monospace font by its files' name (DejaVuSansMono, JetBrainsMono …); empty: the default",
                Kind::Text,
                "",
            ),
            Setting::new(
                "",
                "size",
                "Font size",
                "The text's size in points (8 … 40); Ctrl+= and Ctrl+- zoom too",
                Kind::Text,
                "15",
            ),
        ]
    }

    /// The settings file's values (none without a config folder).
    pub fn values(config_dir: Option<&Path>) -> Values {
        let text = config_dir
            .and_then(|d| std::fs::read_to_string(d.join(FILE)).ok())
            .unwrap_or_default();
        Values::parse(&text)
    }

    /// The settings, from the config folder.
    pub fn load(config_dir: Option<&Path>) -> Settings {
        let values = Settings::values(config_dir);
        let default = Settings::default();
        Settings {
            font: values
                .get("", "font")
                .unwrap_or_default()
                .trim()
                .to_string(),
            size: values
                .get("", "size")
                .and_then(|s| s.trim().parse::<f32>().ok())
                .filter(|s| s.is_finite())
                .map_or(default.size, |s| s.clamp(8.0, 40.0)),
        }
    }
}
