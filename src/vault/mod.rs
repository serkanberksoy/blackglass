//! The vault: a folder of notes, scanned into a tree for the file explorer,
//! with every note's text (for search) and tags (for the tag pane).
//! Hidden files and folders (`.obsidian`, `.git`, `.trash` …) are skipped.

pub mod properties;
pub mod search;
pub mod tags;

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A folder in the vault, with its sub-folders and files sorted by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folder {
    pub name: String,
    /// Absolute path.
    pub path: PathBuf,
    pub folders: Vec<Folder>,
    pub files: Vec<File>,
}

/// A file in the vault (a note or an attachment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The file name, with its extension.
    pub name: String,
    /// Absolute path.
    pub path: PathBuf,
    /// Last modification, for sorting by date (`None` if unknown).
    pub modified: Option<SystemTime>,
}

impl File {
    /// Whether this is a Markdown note.
    pub fn is_note(&self) -> bool {
        is_note(&self.path)
    }

    /// The name shown for the file: a note without `.md`, anything else
    /// with its extension (as Obsidian does).
    pub fn display_name(&self) -> &str {
        if self.is_note() {
            self.name
                .get(..self.name.len() - 3)
                .expect("a note's name ends with .md")
        } else {
            &self.name
        }
    }
}

/// A Markdown note, with what search and the tag pane need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// Absolute path.
    pub path: PathBuf,
    /// Path relative to the vault root.
    pub rel: PathBuf,
    pub lines: Vec<String>,
    /// Tags from the frontmatter and the text, without `#`, in order of
    /// appearance (see [`tags::extract`]).
    pub tags: Vec<String>,
    /// The frontmatter's properties (see [`properties::extract`]).
    pub properties: Vec<(String, String)>,
    /// Its other names (`aliases`).
    pub aliases: Vec<String>,
    pub modified: Option<SystemTime>,
}

impl Note {
    /// The note's name: its file name without `.md`.
    pub fn name(&self) -> String {
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Its path relative to the vault, without `.md` (`Journal/2026-08-09`),
    /// as the quick switcher and search show it.
    pub fn rel_name(&self) -> String {
        let rel = slash(&self.rel);
        rel.strip_suffix(".md").unwrap_or(&rel).to_string()
    }
}

/// A path in the vault as text, its folders separated by `/` on every
/// system (as links, the session and queries write them): Windows' `\`
/// becomes `/`; elsewhere `\` is a character a name may have.
pub fn slash(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

/// A tag and how many notes have it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCount {
    /// As first written in the vault (tags are case-insensitive).
    pub name: String,
    pub notes: usize,
}

#[derive(Debug, Clone)]
pub struct Vault {
    /// The vault folder (absolute, canonical).
    pub root: PathBuf,
    /// The folder tree; `tree.path` is `root`.
    pub tree: Folder,
    /// Every note, sorted by relative path (ignoring case).
    pub notes: Vec<Note>,
    /// Every file (notes and attachments), relative to the root.
    pub files: Vec<PathBuf>,
    /// Every tag with its note count, sorted by name (case-insensitive).
    pub tags: Vec<TagCount>,
}

impl Vault {
    /// Scans the folder at `root`. `Err` if it isn't a readable folder;
    /// unreadable files and sub-folders inside it are skipped.
    pub fn open(root: &Path) -> io::Result<Vault> {
        let root = mdedit::platform::canonical(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("{} is not a folder", root.display()),
            ));
        }
        let mut found = Vec::new();
        let mut files = Vec::new();
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into(),
        );
        let tree = scan(&root, &root, name, &mut found, &mut files)?;
        // Reading and parsing the notes is most of the work: on every core.
        let mut notes = par_map(&found, |(path, rel, modified)| {
            read_note(path, rel, *modified)
        });
        notes.sort_by_cached_key(|n| (n.rel.to_string_lossy().to_lowercase(), n.rel.clone()));
        files.sort();
        let mut vault = Vault {
            root,
            tree,
            notes,
            files,
            tags: Vec::new(),
        };
        vault.count_tags();
        Ok(vault)
    }

    /// Scans the vault again (after files were created or renamed).
    pub fn rescan(&mut self) -> io::Result<()> {
        *self = Vault::open(&self.root)?;
        Ok(())
    }

    /// The vault's name: its folder's name.
    pub fn name(&self) -> &str {
        &self.tree.name
    }

    /// `path` relative to the vault root, if it's inside the vault.
    pub fn rel<'a>(&self, path: &'a Path) -> Option<&'a Path> {
        path.strip_prefix(&self.root).ok()
    }

    /// The note at `path` (absolute).
    pub fn note(&self, path: &Path) -> Option<&Note> {
        self.notes.iter().find(|n| n.path == path)
    }

    /// Updates the index for a note that was saved with `text`. Returns
    /// false if the note is new to the index (then [`Vault::rescan`]).
    pub fn update_note(&mut self, path: &Path, text: &str) -> bool {
        let Some(note) = self.notes.iter_mut().find(|n| n.path == path) else {
            return false;
        };
        note.lines = split_lines(text);
        let old = std::mem::replace(&mut note.tags, tags::extract(&note.lines));
        note.properties = properties::extract(&note.lines);
        note.aliases = properties::aliases(&note.properties);
        note.modified = modified(path);
        let new = note.tags.clone();
        // This note's tags out, and in again (a full count is O(vault)).
        self.adjust_tags(&old, false);
        self.adjust_tags(&new, true);
        true
    }

    /// Counts `tags` in (`add`) or out; a tag no note has any more goes.
    fn adjust_tags(&mut self, tags: &[String], add: bool) {
        for tag in tags {
            let key = tag.to_lowercase();
            let at = self
                .tags
                .binary_search_by(|t| t.name.to_lowercase().cmp(&key));
            match (at, add) {
                (Ok(i), true) => self.tags[i].notes += 1,
                (Err(i), true) => self.tags.insert(
                    i,
                    TagCount {
                        name: tag.clone(),
                        notes: 1,
                    },
                ),
                (Ok(i), false) => {
                    self.tags[i].notes -= 1;
                    if self.tags[i].notes == 0 {
                        self.tags.remove(i);
                    }
                }
                (Err(_), false) => {}
            }
        }
    }

    fn count_tags(&mut self) {
        // Case-insensitive, keeping the first spelling seen.
        let mut counts: Vec<(String, TagCount)> = Vec::new();
        for note in &self.notes {
            for tag in &note.tags {
                let key = tag.to_lowercase();
                match counts.binary_search_by(|(k, _)| k.as_str().cmp(&key)) {
                    Ok(i) => counts[i].1.notes += 1,
                    Err(i) => counts.insert(
                        i,
                        (
                            key,
                            TagCount {
                                name: tag.clone(),
                                notes: 1,
                            },
                        ),
                    ),
                }
            }
        }
        self.tags = counts.into_iter().map(|(_, t)| t).collect();
    }
}

/// Whether `path` is a Markdown note (`.md`, any case).
pub fn is_note(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// The lines of `text` (`\r\n` or `\n`), as the editor splits them.
pub fn split_lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Scans `dir` (inside `root`) into a [`Folder`], collecting its notes and
/// files. Symbolic links to folders are not followed (they can loop).
/// A note found by [`scan`], to read: its path, its path in the vault and
/// when it was changed.
type Found = (PathBuf, PathBuf, Option<SystemTime>);

/// `f` of each item, in order, on the machine's cores (in chunks).
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    if cores < 2 || items.len() < 64 {
        return items.iter().map(f).collect();
    }
    let chunk = items.len().div_ceil(cores);
    std::thread::scope(|s| {
        let f = &f;
        let parts: Vec<_> = items
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(f).collect::<Vec<R>>()))
            .collect();
        parts
            .into_iter()
            .flat_map(|p| p.join().expect("a scan thread doesn't panic"))
            .collect()
    })
}

/// Reads a note and what's in it (a file that can't be read is empty).
fn read_note(path: &Path, rel: &Path, modified: Option<SystemTime>) -> Note {
    let text = std::fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let lines = split_lines(&text);
    let props = properties::extract(&lines);
    Note {
        path: path.to_path_buf(),
        rel: rel.to_path_buf(),
        tags: tags::extract(&lines),
        aliases: properties::aliases(&props),
        properties: props,
        lines,
        modified,
    }
}

fn scan(
    root: &Path,
    dir: &Path,
    name: String,
    notes: &mut Vec<Found>,
    files: &mut Vec<PathBuf>,
) -> io::Result<Folder> {
    let mut folder = Folder {
        name,
        path: dir.to_path_buf(),
        folders: Vec::new(),
        files: Vec::new(),
    };
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if let Ok(sub) = scan(root, &path, name, notes, files) {
                folder.folders.push(sub);
            }
            continue;
        }
        // A symbolic link counts as what it points to, if that's a file.
        if kind.is_symlink() && !path.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .expect("scanned paths are inside the root")
            .to_path_buf();
        let file = File {
            name,
            modified: modified(&path),
            path,
        };
        if file.is_note() {
            notes.push((file.path.clone(), rel.clone(), file.modified));
        }
        files.push(rel);
        folder.files.push(file);
    }
    let by_name = |a: &str, b: &str| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b));
    folder.folders.sort_by(|a, b| by_name(&a.name, &b.name));
    folder.files.sort_by(|a, b| by_name(&a.name, &b.name));
    Ok(folder)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh folder under the system temp dir for one test.
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("blackglass-tests").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// Writes `files` (relative path, contents) under `dir`.
    pub(crate) fn write(dir: &Path, files: &[(&str, &str)]) {
        for (rel, text) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    #[test]
    fn scans_folders_notes_and_attachments_sorted_by_name() {
        let dir = scratch("scan");
        write(
            &dir,
            &[
                ("b.md", "# B"),
                ("A.md", "text #idea"),
                ("Journal/2026-08-09.md", "---\ntags: [daily]\n---\n#idea"),
                ("assets/pic.png", "not really"),
                (".obsidian/app.json", "{}"),
                ("Journal/.hidden.md", ""),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        let names = |f: &Folder| f.folders.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&vault.tree), ["assets", "Journal"]);
        let files: Vec<_> = vault.tree.files.iter().map(File::display_name).collect();
        assert_eq!(files, ["A", "b"]);
        assert_eq!(vault.tree.folders[0].files[0].display_name(), "pic.png");
        let rels: Vec<_> = vault.notes.iter().map(Note::rel_name).collect();
        assert_eq!(rels, ["A", "b", "Journal/2026-08-09"]);
        assert_eq!(vault.files.len(), 4, "{:?}", vault.files);
        assert_eq!(
            vault.tags,
            [
                TagCount {
                    name: "daily".into(),
                    notes: 1
                },
                TagCount {
                    name: "idea".into(),
                    notes: 2
                },
            ]
        );
    }

    #[test]
    fn a_saved_note_updates_its_tags_without_a_rescan() {
        let dir = scratch("update");
        write(&dir, &[("a.md", "#one")]);
        let mut vault = Vault::open(&dir).unwrap();
        let path = vault.notes[0].path.clone();
        assert!(vault.update_note(&path, "#two\n#Two"));
        assert_eq!(vault.notes[0].lines, ["#two", "#Two"]);
        assert_eq!(vault.tags.len(), 1);
        assert_eq!(vault.tags[0].name, "two");
        assert!(!vault.update_note(&dir.join("new.md"), ""), "unknown note");
    }

    #[test]
    fn tag_counts_follow_saves_like_a_full_count() {
        let dir = scratch("tag-counts");
        write(&dir, &[("a.md", "#x #y"), ("b.md", "#Y #z"), ("c.md", "")]);
        let mut vault = Vault::open(&dir).unwrap();
        let paths: Vec<PathBuf> = vault.notes.iter().map(|n| n.path.clone()).collect();
        for (i, text) in ["#y", "#new #x", "", "#z #z #Q", "#q"].iter().enumerate() {
            vault.update_note(&paths[i % 3], text);
            let mut full = vault.clone();
            full.count_tags();
            let counts = |v: &Vault| {
                v.tags
                    .iter()
                    .map(|t| (t.name.to_lowercase(), t.notes))
                    .collect::<Vec<_>>()
            };
            assert_eq!(counts(&vault), counts(&full), "after {text:?}");
        }
    }

    #[test]
    fn a_file_is_not_a_vault() {
        let dir = scratch("not-a-folder");
        write(&dir, &[("a.md", "")]);
        assert!(Vault::open(&dir.join("a.md")).is_err());
        assert!(Vault::open(&dir.join("missing")).is_err());
    }
}
