//! Bookmarks, after Obsidian's core plugin: notes (and other files),
//! headings, folders and searches kept in a Bookmarks tab of the sidebar.
//! Enter opens one (a heading's note at the heading, a folder in the file
//! explorer, a search run again), `r` gives it a title, Delete removes it.
//! Kept per vault in `.blackglass/plugins/bookmarks/bookmarks.json`, in the
//! shape of Obsidian's `bookmarks.json`; notes moved or renamed keep theirs.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde_json::{Value, json};

use super::{Answer, Context, Effect, Manifest, Plugin, PluginCommand, Question, SidebarRow};
use crate::vault::Vault;

/// The plugin's id.
const ID: &str = "bookmarks";

/// What a bookmark points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A note or other file, relative to the vault.
    File(PathBuf),
    /// A note's heading.
    Heading(PathBuf, String),
    /// A folder, relative to the vault.
    Folder(PathBuf),
    /// A vault search.
    Search(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub target: Target,
    /// The name shown instead of the target's, if given.
    pub title: Option<String>,
}

#[derive(Default)]
pub struct Bookmarks {
    items: Vec<Bookmark>,
    /// Where they're kept.
    file: Option<PathBuf>,
    /// The bookmark waiting for its new title.
    renaming: Option<usize>,
}

/// A relative path with `/` separators.
fn text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A file's name without `.md`.
fn shown_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match name.strip_suffix(".md") {
        Some(stem) => stem.to_string(),
        None => name,
    }
}

impl Bookmark {
    fn to_json(&self) -> Value {
        let mut v = match &self.target {
            Target::File(p) => json!({ "type": "file", "path": text(p) }),
            Target::Heading(p, h) => {
                json!({ "type": "file", "path": text(p), "subpath": format!("#{h}") })
            }
            Target::Folder(p) => json!({ "type": "folder", "path": text(p) }),
            Target::Search(q) => json!({ "type": "search", "query": q }),
        };
        if let Some(title) = &self.title {
            v["title"] = json!(title);
        }
        v
    }

    fn from_json(v: &Value) -> Option<Bookmark> {
        let s = |k: &str| v[k].as_str().map(String::from);
        let target = match v["type"].as_str()? {
            "file" => {
                let path = PathBuf::from(s("path")?);
                match s("subpath").as_deref().and_then(|p| p.strip_prefix('#')) {
                    Some(heading) if !heading.starts_with('^') => {
                        Target::Heading(path, heading.to_string())
                    }
                    _ => Target::File(path),
                }
            }
            "folder" => Target::Folder(PathBuf::from(s("path")?)),
            "search" => Target::Search(s("query")?),
            _ => return None,
        };
        Some(Bookmark {
            target,
            title: s("title").filter(|t| !t.is_empty()),
        })
    }

    /// The row in the Bookmarks tab.
    fn row(&self) -> SidebarRow {
        let parent = |p: &Path| p.parent().map(text).unwrap_or_default();
        let (label, detail) = match &self.target {
            Target::File(p) => (shown_name(p), parent(p)),
            Target::Heading(p, h) => (format!("{} › {h}", shown_name(p)), parent(p)),
            Target::Folder(p) => (format!("{}/", shown_name(p)), parent(p)),
            Target::Search(q) => (q.clone(), "search".into()),
        };
        SidebarRow {
            label: self.title.clone().unwrap_or(label),
            detail,
        }
    }
}

/// The heading the line `row` of `text` is under (its text), if any.
fn heading_above(text: &str, row: usize) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut fence = false;
    let mut found = None;
    for line in lines.iter().take(row + 1) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            found = Some(trimmed[hashes..].trim().to_string());
        }
    }
    found
}

/// The line of heading `heading` in `lines`, if it's there.
fn heading_line(lines: &[String], heading: &str) -> Option<usize> {
    lines.iter().position(|line| {
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        (1..=6).contains(&hashes)
            && mdedit::links::heading_matches(trimmed[hashes..].trim(), heading)
    })
}

impl Bookmarks {
    pub fn new() -> Self {
        Bookmarks::default()
    }

    fn save(&self) {
        let Some(file) = &self.file else {
            return;
        };
        let items: Vec<Value> = self.items.iter().map(Bookmark::to_json).collect();
        let text = serde_json::to_string_pretty(&json!({ "items": items }))
            .expect("JSON values serialize");
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = mdedit::files::write_atomic(file, &text);
    }

    /// Adds `target`, or removes it if it's there already (a toggle, as
    /// Obsidian's "Bookmark…" commands): what was done, to say.
    fn toggle(&mut self, target: Target, what: &str) -> Effect {
        if let Some(i) = self.items.iter().position(|b| b.target == target) {
            self.items.remove(i);
            self.save();
            return Effect::Message(format!("{what} is no longer bookmarked"));
        }
        self.items.push(Bookmark {
            target,
            title: None,
        });
        self.save();
        Effect::Message(format!("Bookmarked {what}"))
    }

    /// The active note's path in the vault.
    fn active(ctx: &Context) -> Option<PathBuf> {
        let path = ctx.note?.path?;
        Some(path.strip_prefix(&ctx.vault.root).ok()?.to_path_buf())
    }

    /// Opens bookmark `i`.
    fn open(&self, i: usize, ctx: &Context) -> Effect {
        let root = &ctx.vault.root;
        match &self.items[i].target {
            Target::File(p) => Effect::Open { path: root.join(p) },
            Target::Heading(p, heading) => {
                let path = root.join(p);
                let line = ctx
                    .vault
                    .note(&path)
                    .and_then(|n| heading_line(&n.lines, heading));
                match line {
                    Some(line) => Effect::Many(vec![Effect::Open { path }, Effect::GoToLine(line)]),
                    None => Effect::Many(vec![
                        Effect::Open { path },
                        Effect::Message(format!("No heading {heading} in it any more")),
                    ]),
                }
            }
            Target::Folder(p) => Effect::Reveal(root.join(p)),
            Target::Search(q) => Effect::Search(q.clone()),
        }
    }
}

impl Plugin for Bookmarks {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Bookmarks",
            version: "0.1.0",
            author: "blackglass, after the core Bookmarks plugin",
            description: "Notes, headings, folders and searches you keep coming back to, in a \
                          Bookmarks tab of the sidebar: Enter opens one, r renames it, Delete \
                          removes it. Kept per vault.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        let file = super::settings_file(vault, ID).with_file_name("bookmarks.json");
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        self.items = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                v["items"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Bookmark::from_json).collect())
            })
            .unwrap_or_default();
        self.file = Some(file);
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("open", "Open"),
            PluginCommand::new("bookmark-note", "Bookmark this note"),
            PluginCommand::new("bookmark-heading", "Bookmark this heading"),
            PluginCommand::new("bookmark-folder", "Bookmark this folder"),
            PluginCommand::new("bookmark-search", "Bookmark the search"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        match id {
            "open" => Effect::ShowSidebarTab,
            "bookmark-note" => match Self::active(ctx) {
                Some(rel) => {
                    let what = shown_name(&rel);
                    self.toggle(Target::File(rel), &what)
                }
                None => Effect::Message("Bookmarks: open a note first".into()),
            },
            "bookmark-heading" => {
                let (Some(rel), Some(note)) = (Self::active(ctx), ctx.note) else {
                    return Effect::Message("Bookmarks: open a note first".into());
                };
                match heading_above(note.text, note.row) {
                    Some(h) => {
                        let what = format!("{} › {h}", shown_name(&rel));
                        self.toggle(Target::Heading(rel, h), &what)
                    }
                    None => Effect::Message("Bookmarks: no heading above the cursor".into()),
                }
            }
            "bookmark-folder" => match ctx.folder.strip_prefix(&ctx.vault.root) {
                Ok(rel) if !rel.as_os_str().is_empty() => {
                    let what = format!("{}/", text(rel));
                    self.toggle(Target::Folder(rel.to_path_buf()), &what)
                }
                _ => Effect::Message("Bookmarks: choose a folder in the file explorer".into()),
            },
            "bookmark-search" => match ctx.query.trim() {
                "" => Effect::Message("Bookmarks: search the vault first".into()),
                q => self.toggle(Target::Search(q.to_string()), &format!("the search {q}")),
            },
            _ => Effect::None,
        }
    }

    fn sidebar_tab(&self) -> Option<&'static str> {
        Some("Bookmarks")
    }

    fn sidebar_hint(&self) -> &'static str {
        "⏎ open  r rename  Delete remove"
    }

    fn sidebar_rows(&self, _ctx: &Context) -> Vec<SidebarRow> {
        self.items.iter().map(Bookmark::row).collect()
    }

    fn sidebar_key(&mut self, row: usize, key: KeyEvent, ctx: &Context) -> Effect {
        if row >= self.items.len() {
            return Effect::None;
        }
        match key.code {
            KeyCode::Enter => self.open(row, ctx),
            KeyCode::Delete => {
                self.items.remove(row);
                self.save();
                Effect::None
            }
            KeyCode::Char('r') => {
                self.renaming = Some(row);
                Effect::Ask(vec![Question::Text {
                    prompt: "Bookmark's title (empty: its name)".into(),
                    default: self.items[row].row().label,
                }])
            }
            _ => Effect::None,
        }
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], _ctx: &Context) -> Effect {
        if let (Some(i), [Answer::Text(title)]) = (self.renaming.take(), answers)
            && let Some(b) = self.items.get_mut(i)
        {
            let title = title.trim();
            b.title = (!title.is_empty()).then(|| title.to_string());
            self.save();
        }
        Effect::None
    }

    fn on_note_moved(&mut self, from: &Path, to: &Path, vault: &Vault) {
        let (Ok(from), Ok(to)) = (from.strip_prefix(&vault.root), to.strip_prefix(&vault.root))
        else {
            return;
        };
        let mut moved = false;
        for b in &mut self.items {
            match &mut b.target {
                Target::File(p) | Target::Heading(p, _) if p == from => {
                    *p = to.to_path_buf();
                    moved = true;
                }
                _ => {}
            }
        }
        if moved {
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_are_kept_as_the_original_keeps_them() {
        let all = vec![
            Bookmark {
                target: Target::File("Books/Dune.md".into()),
                title: Some("Desert".into()),
            },
            Bookmark {
                target: Target::Heading("Books/Dune.md".into(), "Spice".into()),
                title: None,
            },
            Bookmark {
                target: Target::Folder("Books".into()),
                title: None,
            },
            Bookmark {
                target: Target::Search("melange".into()),
                title: None,
            },
        ];
        for b in &all {
            assert_eq!(Bookmark::from_json(&b.to_json()).as_ref(), Some(b));
        }
        assert_eq!(
            all[1].to_json(),
            json!({ "type": "file", "path": "Books/Dune.md", "subpath": "#Spice" })
        );
        let rows: Vec<String> = all.iter().map(|b| b.row().label).collect();
        assert_eq!(rows, ["Desert", "Dune › Spice", "Books/", "melange"]);
        assert_eq!(Bookmark::from_json(&json!({ "type": "graph" })), None);
    }

    #[test]
    fn the_heading_above_the_cursor() {
        let text = "intro\n# One\n```\n# not a heading\n```\n## Two\nbody";
        assert_eq!(heading_above(text, 0), None);
        assert_eq!(
            heading_above(text, 4).as_deref(),
            Some("One"),
            "not in code"
        );
        assert_eq!(heading_above(text, 6).as_deref(), Some("Two"));
        let lines: Vec<String> = text.lines().map(String::from).collect();
        assert_eq!(heading_line(&lines, "Two"), Some(5));
    }
}
