//! Choosing a vault: any folder. The picker lists the vaults opened
//! lately; typing a path lists the folders there (Tab completes one), and
//! Enter opens it. A folder that doesn't exist yet is made after asking
//! (Enter again). It's used at start (`blackglass` without a folder) and
//! by "Open vault" in the palette.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// How many folders the picker lists.
const MOST: usize = 50;

/// What a key in the picker did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    None,
    /// Open this folder (it exists; its canonical path).
    Open(PathBuf),
    Cancel,
}

/// The vault picker's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultPicker {
    /// The typed path (`~` is the home folder).
    pub input: String,
    /// What's listed: the recent vaults (nothing typed), or the folders
    /// matching the typed path.
    pub rows: Vec<PathBuf>,
    pub selected: usize,
    /// A line for the user (why a path can't be opened, "Enter again …").
    pub note: String,
    recent: Vec<PathBuf>,
    home: PathBuf,
    /// The missing folder Enter was pressed on once (again: make it).
    create: Option<PathBuf>,
}

impl VaultPicker {
    /// The recent vaults to offer (newest first), and the home folder
    /// (`~`). Without recent vaults it starts at the home folder.
    pub fn new(recent: Vec<PathBuf>, home: PathBuf) -> Self {
        let recent: Vec<PathBuf> = recent.into_iter().filter(|p| p.is_dir()).collect();
        let mut picker = VaultPicker {
            input: if recent.is_empty() {
                "~/".into()
            } else {
                String::new()
            },
            rows: Vec::new(),
            selected: 0,
            note: String::new(),
            recent,
            home,
            create: None,
        };
        picker.update();
        picker
    }

    /// `path` as shown: under the home folder as `~/…`.
    pub fn shown(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rel) if rel.as_os_str().is_empty() => "~".into(),
            Ok(rel) => format!("~/{}", rel.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// The typed path, with `~` as the home folder.
    pub fn path(&self) -> PathBuf {
        let text = self.input.trim();
        match text.strip_prefix('~') {
            Some(rest) => self.home.join(rest.trim_start_matches('/')),
            None => PathBuf::from(text),
        }
    }

    /// Lists the recent vaults, or the folders the typed path can go on
    /// to (those in its folder whose name starts with what follows the
    /// last `/`; hidden ones only when that starts with a dot).
    fn update(&mut self) {
        self.selected = 0;
        if self.input.trim().is_empty() {
            self.rows = self.recent.clone();
            return;
        }
        let typed = self.path();
        let (dir, start) = if self.input.ends_with('/') {
            (typed.clone(), String::new())
        } else {
            let name = typed
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            (
                typed.parent().map(Path::to_path_buf).unwrap_or_default(),
                name,
            )
        };
        let mut rows: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().to_lowercase();
                        name.starts_with(&start)
                            && (!name.starts_with('.') || start.starts_with('.'))
                    })
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default();
        rows.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        rows.truncate(MOST);
        self.rows = rows;
    }

    /// A key: typing edits the path, ↑/↓ choose, Tab (→) takes the chosen
    /// folder into the path, Enter opens (the chosen recent vault when
    /// nothing is typed), Esc (Ctrl+Q, Ctrl+C) cancels.
    pub fn key(&mut self, key: KeyEvent) -> Picked {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => return Picked::Cancel,
            KeyCode::Char('q' | 'c') if ctrl => return Picked::Cancel,
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1));
            }
            KeyCode::Tab | KeyCode::Right => {
                if let Some(row) = self.rows.get(self.selected) {
                    self.input = format!("{}/", self.shown(row));
                    self.edited();
                }
            }
            KeyCode::Enter => {
                let target = if self.input.trim().is_empty() {
                    match self.rows.get(self.selected) {
                        Some(row) => row.clone(),
                        None => return Picked::None,
                    }
                } else {
                    self.path()
                };
                return self.open(target);
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.edited();
            }
            KeyCode::Char('u') if ctrl => {
                self.input.clear();
                self.edited();
            }
            KeyCode::Char(c) if !ctrl => {
                self.input.push(c);
                self.edited();
            }
            _ => {}
        }
        Picked::None
    }

    /// Pasted text goes in the path (on one line).
    pub fn paste(&mut self, text: &str) {
        self.input.push_str(text.trim());
        self.edited();
    }

    /// After the path changed: list again, forget a pending "create".
    fn edited(&mut self) {
        self.create = None;
        self.note.clear();
        self.update();
    }

    /// Opens `target`: a folder opens; a missing one is made on the second
    /// Enter; a file is refused.
    fn open(&mut self, target: PathBuf) -> Picked {
        if target.is_dir() {
            return match mdedit::platform::canonical(&target) {
                Ok(path) => Picked::Open(path),
                Err(e) => {
                    self.note = format!("Cannot open {}: {e}", self.shown(&target));
                    Picked::None
                }
            };
        }
        if target.exists() {
            self.note = format!("{} isn't a folder", self.shown(&target));
            return Picked::None;
        }
        if self.create.as_ref() != Some(&target) {
            self.note = format!("Enter again creates it (new): {}", self.shown(&target));
            self.create = Some(target);
            return Picked::None;
        }
        if let Err(e) = std::fs::create_dir_all(&target) {
            self.note = format!("Cannot create {}: {e}", self.shown(&target));
            return Picked::None;
        }
        self.open(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(p: &mut VaultPicker, code: KeyCode) -> Picked {
        p.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn typing(p: &mut VaultPicker, text: &str) {
        for c in text.chars() {
            press(p, KeyCode::Char(c));
        }
    }

    #[test]
    fn recent_vaults_come_first_and_folders_complete() {
        let home = scratch("picker-home");
        write(
            &home,
            &[
                ("Notes/a.md", ""),
                ("Nothing/x", ""),
                (".hidden/y", ""),
                ("Work/b.md", ""),
            ],
        );
        let recent = vec![home.join("Work")];
        let mut p = VaultPicker::new(recent.clone(), home.clone());
        assert_eq!(p.rows, recent, "the recent vaults");
        assert_eq!(
            press(&mut p, KeyCode::Enter),
            Picked::Open(mdedit::platform::canonical(&home.join("Work")).unwrap())
        );
        typing(&mut p, "~/No");
        assert_eq!(
            p.rows,
            [home.join("Notes"), home.join("Nothing")],
            "folders starting so"
        );
        press(&mut p, KeyCode::Tab);
        assert_eq!(p.input, "~/Notes/");
        assert_eq!(
            press(&mut p, KeyCode::Enter),
            Picked::Open(mdedit::platform::canonical(&home.join("Notes")).unwrap())
        );
        assert_eq!(press(&mut p, KeyCode::Esc), Picked::Cancel);
        for c in ['q', 'c'] {
            let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
            assert_eq!(p.key(key), Picked::Cancel, "Ctrl+{c} leaves too");
        }
    }

    #[test]
    fn a_new_folder_is_made_after_asking() {
        let home = scratch("picker-new");
        let mut p = VaultPicker::new(Vec::new(), home.clone());
        assert_eq!(p.input, "~/", "without recent vaults: the home folder");
        typing(&mut p, "New/Vault");
        assert_eq!(press(&mut p, KeyCode::Enter), Picked::None);
        assert!(p.note.contains("Enter again creates it"), "{}", p.note);
        assert!(!home.join("New").exists());
        let Picked::Open(path) = press(&mut p, KeyCode::Enter) else {
            panic!("opened");
        };
        assert!(path.is_dir() && path.ends_with("New/Vault"));
        // A file isn't a vault.
        write(&home, &[("file.txt", "")]);
        let mut p = VaultPicker::new(Vec::new(), home.clone());
        typing(&mut p, "file.txt");
        assert_eq!(press(&mut p, KeyCode::Enter), Picked::None);
        assert!(p.note.contains("isn't a folder"), "{}", p.note);
    }
}
