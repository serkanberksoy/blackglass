//! Starting up, the same in the terminal and in the window: the first
//! run's example vault, opening the vault (and note) chosen, and the app
//! with the user's settings, keys and recent emoji.

use std::io;
use std::path::{Path, PathBuf};

use mdedit::emoji::{self, Recent};

use crate::config;
use crate::ui;
use crate::vault::Vault;
use crate::workspace::App;

/// What to open first: `path`, or on the very first run without one the
/// example vault (written out); with a reason when that failed.
pub fn first(path: Option<PathBuf>) -> (Option<PathBuf>, Option<String>) {
    if path.is_some() || !config::first_run(config::config_dir().as_deref()) {
        return (path, None);
    }
    match crate::example::install(&crate::example::default_folder()) {
        Ok(welcome) => (Some(welcome), None),
        Err(e) => (None, Some(format!("The example vault: {e}"))),
    }
}

/// The vault `chosen` is (or is in), and the note it names; why not.
pub fn open(chosen: &Path) -> Result<(Vault, Option<PathBuf>), String> {
    let (folder, note) = config::vault_and_note(Some(chosen));
    Vault::open(&folder)
        .map(|vault| (vault, note))
        .map_err(|e| format!("Cannot open {}: {e}", folder.display()))
}

/// The app for `vault` with the user's editor settings, shortcuts and
/// recent emoji, `note` open in it.
pub fn app(vault: Vault, note: Option<PathBuf>) -> App {
    let mut app = App::new(vault);
    // Recent emoji are shared with mdedit.
    if let Some(recent) = emoji::default_recent_path() {
        app.shared.recent = Recent::load(recent);
    }
    // The editor settings (blackglass's config.toml, else mdedit's) and
    // the keyboard shortcuts, from the user's config folder.
    if let Some(path) = config::settings_path() {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let warnings = app.use_editor_config(&text).join("; ");
                if !warnings.is_empty() {
                    app.message = warnings;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => app.message = format!("Cannot read {}: {e}", path.display()),
        }
    }
    app.config_dir = config::config_dir();
    app.load_user_config();
    app.remember_vault();
    if let Some(note) = note
        && let Ok(note) = mdedit::platform::canonical(&note)
    {
        app.open(&note);
        app.sidebar.reveal(&note, &app.vault);
    }
    app
}

/// The vault picker before a vault is open: the recent vaults, the
/// theme in use, and `problem` (why the last folder couldn't be opened).
pub fn picker(problem: Option<String>) -> (crate::vault_picker::VaultPicker, ui::theme::Palette) {
    let recent = config::config_dir()
        .map(|dir| config::recent_vaults(&dir))
        .unwrap_or_default();
    let mut picker = crate::vault_picker::VaultPicker::new(recent, config::home());
    if let Some(problem) = problem {
        picker.note = problem;
    }
    let dir = config::config_dir();
    let palette = dir
        .as_deref()
        .and_then(|d| ui::theme::saved(&d.join(crate::workspace::APPEARANCE_FILE)))
        .and_then(|id| ui::theme::load(dir.as_deref(), &id))
        .map(|(palette, _)| palette)
        .unwrap_or_default();
    (picker, palette)
}
