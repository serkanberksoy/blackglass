//! Where blackglass's settings are, and the editor's settings screen. The
//! editor settings are mdedit's (`mdedit::config::Config`): blackglass
//! reads its own `config.toml` if there is one, else mdedit's, so an
//! mdedit setup just works; changed in the settings window, they're saved
//! to blackglass's own. The keyboard shortcuts are in `keys.toml` next to
//! it ([`crate::keymap`]).

use std::path::{Path, PathBuf};

use crate::plugins::settings::{Kind, Setting, Values};

/// blackglass's config folder: `~/.config/blackglass` (`$XDG_CONFIG_HOME`
/// if set; `%APPDATA%\blackglass` on Windows).
pub fn config_dir() -> Option<PathBuf> {
    Some(mdedit::platform::config_home()?.join("blackglass"))
}

/// The home folder (`~`; `%USERPROFILE%` on Windows).
pub fn home() -> PathBuf {
    mdedit::platform::home().unwrap_or_else(|| PathBuf::from("/"))
}

/// The recent vaults' file in the config folder.
pub const VAULTS_FILE: &str = "vaults.toml";

/// How many recent vaults are kept.
const RECENT_VAULTS: usize = 10;

/// The vaults opened lately, newest first (from `vaults.toml` in `dir`).
pub fn recent_vaults(dir: &Path) -> Vec<PathBuf> {
    let text = std::fs::read_to_string(dir.join(VAULTS_FILE)).unwrap_or_default();
    text.lines()
        .map(|l| l.trim().trim_end_matches(','))
        .filter_map(|l| l.strip_prefix('"')?.strip_suffix('"').map(String::from))
        .map(|l| PathBuf::from(l.replace("\\\"", "\"").replace("\\\\", "\\")))
        .collect()
}

/// Whether blackglass runs for the first time: no vault has been opened
/// yet (there's no `vaults.toml` in the config folder `dir`).
pub fn first_run(dir: Option<&Path>) -> bool {
    dir.is_some_and(|d| !d.join(VAULTS_FILE).exists())
}

/// Puts `vault` first in `vaults.toml` in `dir` (at most ten are kept).
pub fn remember_vault(dir: &Path, vault: &Path) -> std::io::Result<()> {
    let mut all = recent_vaults(dir);
    all.retain(|p| p != vault);
    all.insert(0, vault.to_path_buf());
    all.truncate(RECENT_VAULTS);
    let mut text =
        String::from("# blackglass: the vaults opened lately, newest first.\nrecent = [\n");
    for path in &all {
        let path = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        text.push_str(&format!("    \"{path}\",\n"));
    }
    text.push_str("]\n");
    std::fs::create_dir_all(dir)?;
    mdedit::files::write_atomic(&dir.join(VAULTS_FILE), &text)
}

/// blackglass's `config.toml`, if it exists, else mdedit's.
pub fn settings_path() -> Option<PathBuf> {
    let own = config_dir()?.join("config.toml");
    Some(choose(own, mdedit::config::default_path()))
}

/// The editor settings the settings window shows (mdedit's `config.toml`
/// keys; `heading_size` is left out, as blackglass keeps it off).
pub fn editor_settings() -> Vec<Setting> {
    let choice = |values: &[&str]| Kind::Choice(values.iter().map(|v| v.to_string()).collect());
    let undo = mdedit::config::Config::default().undo_steps.to_string();
    let mut all = vec![
        Setting::new(
            "",
            "auto_pair",
            "Auto-pair brackets",
            "Typing ( [ { or a quote adds the closing one",
            choice(&["on", "off"]),
            "on",
        ),
        Setting::new(
            "",
            "undo_steps",
            "Undo steps",
            "How many changes Ctrl+Z can undo (1 to 10000)",
            Kind::Text,
            &undo,
        ),
        Setting::new(
            "",
            "indent_width",
            "Indentation",
            "Spaces Tab indents a list item, and per list level (1 to 8)",
            Kind::Text,
            &mdedit::config::Config::default().indent_width.to_string(),
        ),
        Setting::new(
            "",
            "done_style",
            "Done tasks",
            "Checked tasks struck through, or only grey",
            choice(&["strike", "grey"]),
            "strike",
        ),
        Setting::new(
            "",
            "images",
            "Images",
            "How images are drawn (used after a restart)",
            choice(&["auto", "kitty", "sixel", "iterm2", "halfblocks", "off"]),
            "auto",
        ),
        Setting::new(
            "",
            "colors",
            "Colors",
            "The terminal's colors: detected, or forced",
            choice(&["auto", "truecolor", "256", "16"]),
            "auto",
        ),
        Setting::new(
            "",
            "glyphs",
            "Symbols",
            "Unicode symbols, or ASCII for fonts without them",
            choice(&["auto", "unicode", "ascii"]),
            "auto",
        ),
    ];
    for level in 1..=6 {
        all.push(Setting::new(
            "",
            &format!("heading{level}_color"),
            &format!("Heading {level} color"),
            "A color name, #rrggbb or 0-255; empty for the theme's",
            Kind::Text,
            "",
        ));
    }
    all.push(Setting::new(
        "",
        "theme",
        "Theme",
        "The colors: choose a theme, previewed as you move (Choose theme)",
        Kind::Action("choose-theme"),
        "",
    ));
    all
}

/// One editor setting as a `config.toml` line.
pub fn editor_text_line(key: &str, value: &str) -> String {
    let mut values = Values::default();
    values.set("", key, value);
    values.to_text(&[])
}

/// The editor's `config.toml`: every setting with a value (empty ones are
/// left out: they mean the default), then the file's other lines.
pub fn editor_text(values: &Values) -> String {
    let mut text: String = values
        .to_text(&editor_settings())
        .lines()
        .filter(|line| !line.ends_with("= \"\""))
        .map(|line| format!("{line}\n"))
        .collect();
    if !text.is_empty() {
        text.insert_str(
            0,
            "# blackglass editor settings (mdedit's config.toml keys)\n",
        );
    }
    text
}

fn choose(own: PathBuf, mdedit: Option<PathBuf>) -> PathBuf {
    match mdedit {
        Some(m) if !own.exists() => m,
        _ => own,
    }
}

/// Folders that mark a vault: blackglass's own, or Obsidian's.
const VAULT_MARKERS: [&str; 2] = [".blackglass", ".obsidian"];

/// The nearest folder from `dir` up that `is_vault`, else `dir`.
fn nearest_vault(dir: &Path, is_vault: impl Fn(&Path) -> bool) -> PathBuf {
    dir.ancestors()
        // The last ancestor of a relative path is "": the current folder.
        .map(|d| {
            if d.as_os_str().is_empty() {
                Path::new(".")
            } else {
                d
            }
        })
        .find(|d| is_vault(d))
        .unwrap_or(dir)
        .to_path_buf()
}

/// The vault folder and the note to open for a command-line path: a
/// folder is the vault; a note opens in the nearest folder above it that
/// is a vault (has `.blackglass/` or `.obsidian/`), else in its own
/// folder; nothing is the current folder.
pub fn vault_and_note(path: Option<&Path>) -> (PathBuf, Option<PathBuf>) {
    match path {
        Some(p) if p.is_file() => {
            let dir = p
                .parent()
                .filter(|d| !d.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let is_vault = |d: &Path| VAULT_MARKERS.iter().any(|m| d.join(m).is_dir());
            (nearest_vault(dir, is_vault), Some(p.to_path_buf()))
        }
        Some(p) => (p.to_path_buf(), None),
        None => (PathBuf::from("."), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn blackglass_settings_win_when_they_exist() {
        let dir = scratch("config");
        let own = dir.join("blackglass.toml");
        let theirs = dir.join("mdedit.toml");
        assert_eq!(choose(own.clone(), Some(theirs.clone())), theirs);
        assert_eq!(choose(own.clone(), None), own);
        write(&dir, &[("blackglass.toml", "")]);
        assert_eq!(choose(own.clone(), Some(theirs)), own);
    }

    #[test]
    fn editor_settings_are_written_without_empty_ones() {
        let mut values = Values::parse("heading_size = \"on\"\nheading2_color = \"red\"\n");
        values.set("", "auto_pair", "off");
        let text = editor_text(&values);
        assert!(text.contains("auto_pair = \"off\"\n"), "{text}");
        assert!(text.contains("heading2_color = \"red\"\n"), "{text}");
        assert!(
            !text.contains("heading1_color"),
            "empty: the default: {text}"
        );
        assert!(
            text.contains("heading_size = \"on\""),
            "other lines are kept: {text}"
        );
        let (config, warnings) = mdedit::config::Config::parse(&text);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(!config.auto_pair);
        assert_eq!(editor_text_line("undo_steps", "9"), "undo_steps = \"9\"\n");
    }

    #[test]
    fn recent_vaults_are_kept_newest_first() {
        let dir = scratch("recent-vaults");
        assert!(recent_vaults(&dir).is_empty());
        assert!(first_run(Some(&dir)), "no vault opened yet");
        remember_vault(&dir, Path::new("/a")).unwrap();
        remember_vault(&dir, Path::new("/b \"q\"")).unwrap();
        remember_vault(&dir, Path::new("/a")).unwrap();
        assert_eq!(
            recent_vaults(&dir),
            [PathBuf::from("/a"), PathBuf::from("/b \"q\"")]
        );
        assert!(!first_run(Some(&dir)), "a vault was opened");
        assert!(!first_run(None), "no config folder: not asked");
    }

    #[test]
    fn a_note_opens_in_its_folder() {
        let dir = scratch("vault-and-note");
        write(&dir, &[("a.md", "")]);
        let note = dir.join("a.md");
        assert_eq!(vault_and_note(Some(&note)), (dir.clone(), Some(note)));
        assert_eq!(vault_and_note(Some(&dir)), (dir, None));
        assert_eq!(vault_and_note(None), (PathBuf::from("."), None));
    }

    #[test]
    fn a_note_in_a_vaults_subfolder_opens_in_that_vault() {
        let dir = scratch("vault-marker");
        write(
            &dir,
            &[
                (".blackglass/plugins.toml", ""),
                ("JavaScript/Charts.md", ""),
                ("Obsidian/.obsidian/app.json", "{}"),
                ("Obsidian/Deep/er/Note.md", ""),
            ],
        );
        let charts = dir.join("JavaScript/Charts.md");
        assert_eq!(
            vault_and_note(Some(&charts)),
            (dir.clone(), Some(charts)),
            ".blackglass"
        );
        let deep = dir.join("Obsidian/Deep/er/Note.md");
        assert_eq!(
            vault_and_note(Some(&deep)),
            (dir.join("Obsidian"), Some(deep)),
            "the nearest vault: an Obsidian one"
        );
    }

    #[test]
    fn a_relative_path_can_find_the_current_folder() {
        let dot = |d: &Path| d == Path::new(".");
        assert_eq!(
            nearest_vault(Path::new("sub/deeper"), dot),
            PathBuf::from(".")
        );
        assert_eq!(
            nearest_vault(Path::new("sub/deeper"), |_| false),
            PathBuf::from("sub/deeper"),
            "no vault: the note's folder"
        );
    }
}
