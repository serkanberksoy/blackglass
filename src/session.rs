//! The workspace between sessions (W-33): the open tabs (with their
//! cursors), the active one, and the file explorer's open folders, kept in
//! the vault (`.blackglass/workspace.json`, as Obsidian keeps
//! `workspace.json`). Written whenever it changes and on quit, read when
//! the vault opens.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The file, relative to the vault.
pub const FILE: &str = ".blackglass/workspace.json";

/// A tab: its note (relative to the vault) and cursor (line, char).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub path: PathBuf,
    pub cursor: (usize, usize),
}

/// What's restored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    pub tabs: Vec<Tab>,
    pub active: usize,
    /// Open folders, relative to the vault.
    pub expanded: Vec<PathBuf>,
}

/// A relative path with `/` separators.
fn text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl Session {
    /// As the file has it.
    pub fn to_json(&self) -> String {
        let tabs: Vec<Value> = self
            .tabs
            .iter()
            .map(|t| json!({ "path": text(&t.path), "line": t.cursor.0, "column": t.cursor.1 }))
            .collect();
        let mut expanded: Vec<String> = self.expanded.iter().map(|p| text(p)).collect();
        expanded.sort();
        let value = json!({ "tabs": tabs, "active": self.active, "expanded": expanded });
        serde_json::to_string_pretty(&value).expect("JSON values serialize")
    }

    /// Read from the file's text; `None` if it isn't one.
    pub fn from_json(text: &str) -> Option<Session> {
        let v: Value = serde_json::from_str(text).ok()?;
        let num = |v: &Value| v.as_u64().unwrap_or(0) as usize;
        let tabs = v["tabs"]
            .as_array()?
            .iter()
            .filter_map(|t| {
                Some(Tab {
                    path: PathBuf::from(t["path"].as_str()?),
                    cursor: (num(&t["line"]), num(&t["column"])),
                })
            })
            .collect();
        let expanded = v["expanded"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(PathBuf::from))
            .collect();
        Some(Session {
            tabs,
            active: num(&v["active"]),
            expanded,
        })
    }
}

/// The vault's last session, if it has one.
pub fn load(root: &Path) -> Option<Session> {
    Session::from_json(&std::fs::read_to_string(root.join(FILE)).ok()?)
}

/// Writes `session` for the vault at `root` (its `.blackglass` folder
/// made if needed).
pub fn save(root: &Path, session: &Session) -> std::io::Result<()> {
    let file = root.join(FILE);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    mdedit::files::write_atomic(&file, &session.to_json())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_is_written_and_read_back() {
        let session = Session {
            tabs: vec![
                Tab {
                    path: "Books/Dune.md".into(),
                    cursor: (2, 3),
                },
                Tab {
                    path: "Top.md".into(),
                    cursor: (0, 0),
                },
            ],
            active: 1,
            expanded: vec!["Books".into(), "Books/Old".into()],
        };
        assert_eq!(Session::from_json(&session.to_json()), Some(session));
        assert_eq!(Session::from_json("not json"), None);
        assert_eq!(
            Session::from_json("{\"tabs\": [{\"path\": 3}], \"active\": 0}"),
            Some(Session::default()),
            "what it can't read is left out"
        );
    }
}
