//! Undoing changes to other notes (W-133): what a command did to files
//! (a kanban card moved, a task edited from a query's results, a property
//! set from a base, a note renamed or moved with every link to it, a note
//! deleted) is kept as the files' contents before and after, so it can be
//! undone and redone. A change is recorded between [`Files::begin`] and
//! [`Files::end`] (nested calls are one change); every write, move or
//! delete in it calls [`Files::touch`] first. A file changed again since
//! isn't overwritten.

use std::path::{Path, PathBuf};

/// The most changes kept.
const MOST: usize = 100;

/// A file's contents (`None`: there's no file).
type Contents = Option<Vec<u8>>;

/// One change: each file's contents before and after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub files: Vec<(PathBuf, Contents, Contents)>,
}

impl Change {
    /// What it changed, for a message: a file's name, or how many.
    fn what(&self) -> String {
        match self.files.as_slice() {
            [(path, ..)] => path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            files => format!("{} files", files.len()),
        }
    }
}

/// A change being recorded.
#[derive(Debug, Default)]
struct Recording {
    depth: usize,
    before: Vec<(PathBuf, Contents)>,
}

/// The changes to files that can be undone and redone.
#[derive(Debug, Default)]
pub struct Files {
    recording: Option<Recording>,
    undo: Vec<Change>,
    redo: Vec<Change>,
}

fn read(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Puts `contents` at `path` (`None`: no file there).
fn put(path: &Path, contents: Option<&[u8]>) -> std::io::Result<()> {
    match contents {
        Some(bytes) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, bytes)
        }
        None if path.exists() => std::fs::remove_file(path),
        None => Ok(()),
    }
}

impl Files {
    /// Starts recording a change (or goes on with the one being recorded).
    pub fn begin(&mut self) {
        self.recording.get_or_insert_with(Recording::default).depth += 1;
    }

    /// `path` is about to be written, moved or deleted: its contents now
    /// are what undoing puts back.
    pub fn touch(&mut self, path: &Path) {
        if let Some(r) = &mut self.recording
            && !r.before.iter().any(|(p, _)| p == path)
        {
            r.before.push((path.to_path_buf(), read(path)));
        }
    }

    /// Ends the change begun last; at the outermost, it's kept if it
    /// changed anything (and what was undone can't be redone any more).
    pub fn end(&mut self) {
        let Some(r) = &mut self.recording else {
            return;
        };
        r.depth -= 1;
        if r.depth > 0 {
            return;
        }
        let r = self.recording.take().expect("just checked");
        let files: Vec<_> = r
            .before
            .into_iter()
            .map(|(path, before)| {
                let after = read(&path);
                (path, before, after)
            })
            .filter(|(_, before, after)| before != after)
            .collect();
        if files.is_empty() {
            return;
        }
        self.undo.push(Change { files });
        if self.undo.len() > MOST {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undoes (`back`) or redoes the last change: the files it changed,
    /// and a message; an error if one of them changed again since.
    pub fn step(&mut self, back: bool) -> Result<(Vec<PathBuf>, String), String> {
        let stack = if back { &mut self.undo } else { &mut self.redo };
        let Some(change) = stack.pop() else {
            let what = if back { "undo" } else { "redo" };
            return Err(format!("Nothing to {what} in other notes"));
        };
        // Each file as the change left it (or as undoing it left it).
        for (path, before, after) in &change.files {
            let now = if back { after } else { before };
            if read(path) != *now {
                let name = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                stack.push(change);
                return Err(format!("{name} changed since; not undone"));
            }
        }
        for (path, before, after) in &change.files {
            let to = if back { before } else { after };
            put(path, to.as_deref())
                .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
        }
        let message = format!(
            "{} the change to {}",
            if back { "Undid" } else { "Redid" },
            change.what()
        );
        let paths = change.files.iter().map(|(p, ..)| p.clone()).collect();
        if back {
            self.redo.push(change);
        } else {
            self.undo.push(change);
        }
        Ok((paths, message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::scratch;

    #[test]
    fn changes_are_undone_redone_and_kept_safe() {
        let dir = scratch("journal");
        let (a, b) = (dir.join("a.md"), dir.join("b.md"));
        std::fs::write(&a, "one").unwrap();
        let mut files = Files::default();
        // A rename: a gone, b made; nested begins are one change.
        files.begin();
        files.begin();
        files.touch(&a);
        files.touch(&b);
        std::fs::rename(&a, &b).unwrap();
        files.end();
        files.end();
        assert!(files.can_undo() && !files.can_redo());
        let (_, message) = files.step(true).unwrap();
        assert_eq!(message, "Undid the change to 2 files");
        assert!(a.is_file() && !b.exists());
        files.step(false).unwrap();
        assert!(!a.exists() && std::fs::read_to_string(&b).unwrap() == "one");
        // Changed again since: left alone.
        std::fs::write(&b, "two").unwrap();
        assert!(files.step(true).unwrap_err().contains("changed since"));
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "two");
        // Nothing changed: nothing kept.
        let mut quiet = Files::default();
        quiet.begin();
        quiet.touch(&b);
        quiet.end();
        assert!(!quiet.can_undo());
    }
}
