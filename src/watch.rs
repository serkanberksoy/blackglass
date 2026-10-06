//! Watching the vault for changes made elsewhere (another editor, a sync
//! tool, Git): a thread looks at every file's size and modification time
//! (as the vault's scan sees them: no `.` files or folders) about once a
//! second and reports what was added, removed or modified. Polling needs no
//! platform code and costs a directory walk; the workspace decides which
//! changes are its own ([`crate::workspace::App::tick`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, SystemTime};

/// How often the vault is looked at.
pub const INTERVAL: Duration = Duration::from_millis(1000);

/// A file that changed (absolute path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Added(PathBuf),
    Removed(PathBuf),
    Modified(PathBuf),
}

impl Change {
    pub fn path(&self) -> &Path {
        match self {
            Change::Added(p) | Change::Removed(p) | Change::Modified(p) => p,
        }
    }
}

/// Each file's modification time and size.
type Snapshot = HashMap<PathBuf, (Option<SystemTime>, u64)>;

/// A thread watching a vault; it stops when this is dropped.
pub struct Watcher {
    rx: Receiver<Vec<Change>>,
    stop: Arc<AtomicBool>,
}

impl Watcher {
    /// Watches the vault at `root`, looking every `interval`.
    pub fn start(root: &Path, interval: Duration) -> Watcher {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let root = root.to_path_buf();
        // Now, not when the thread first runs: a change made meanwhile is
        // a change.
        let mut before = snapshot(&root);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(interval);
                if stopped.load(Ordering::Relaxed) {
                    return;
                }
                let now = snapshot(&root);
                let changes = diff(&before, &now);
                before = now;
                if !changes.is_empty() && tx.send(changes).is_err() {
                    return;
                }
            }
        });
        Watcher { rx, stop }
    }

    /// The changes seen since the last call.
    pub fn poll(&self) -> Vec<Change> {
        let mut all: Vec<Change> = self.rx.try_iter().flatten().collect();
        all.dedup();
        all
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Every file in the vault (not in `.` folders), with its time and size.
fn snapshot(root: &Path) -> Snapshot {
    fn walk(dir: &Path, out: &mut Snapshot) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                walk(&path, out);
            } else {
                out.insert(path, (meta.modified().ok(), meta.len()));
            }
        }
    }
    let mut out = Snapshot::new();
    walk(root, &mut out);
    out
}

/// What changed from `before` to `now`, sorted by path.
fn diff(before: &Snapshot, now: &Snapshot) -> Vec<Change> {
    let mut changes: Vec<Change> = now
        .iter()
        .filter_map(|(path, stamp)| match before.get(path) {
            None => Some(Change::Added(path.clone())),
            Some(old) if old != stamp => Some(Change::Modified(path.clone())),
            Some(_) => None,
        })
        .chain(
            before
                .keys()
                .filter(|p| !now.contains_key(*p))
                .map(|p| Change::Removed(p.clone())),
        )
        .collect();
    changes.sort_by(|a, b| a.path().cmp(b.path()));
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn a_change_right_after_starting_is_seen() {
        // The thread may start late: what it compares with is the vault
        // when watching began, not when the thread first looked.
        for round in 0..20 {
            let dir = scratch(&format!("watch-start-{round}"));
            write(&dir, &[("A.md", "a")]);
            let watcher = Watcher::start(&dir, Duration::from_millis(5));
            std::fs::write(dir.join("A.md"), "changed").unwrap();
            let mut seen = Vec::new();
            for _ in 0..200 {
                seen.extend(watcher.poll());
                if !seen.is_empty() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(seen, [Change::Modified(dir.join("A.md"))], "round {round}");
        }
    }

    #[test]
    fn added_removed_and_modified_files_are_seen() {
        let dir = scratch("watch-diff");
        write(
            &dir,
            &[("A.md", "a"), ("Sub/B.md", "b"), (".git/HEAD", "x")],
        );
        let before = snapshot(&dir);
        assert_eq!(before.len(), 2, "no . folders");
        std::fs::write(dir.join("A.md"), "a, longer").unwrap();
        std::fs::remove_file(dir.join("Sub/B.md")).unwrap();
        std::fs::write(dir.join("C.md"), "c").unwrap();
        std::fs::write(dir.join(".git/HEAD"), "y").unwrap();
        assert_eq!(
            diff(&before, &snapshot(&dir)),
            [
                Change::Modified(dir.join("A.md")),
                Change::Added(dir.join("C.md")),
                Change::Removed(dir.join("Sub/B.md")),
            ]
        );
    }

    #[test]
    fn a_watcher_reports_and_stops() {
        let dir = scratch("watch-thread");
        write(&dir, &[("A.md", "a")]);
        let watcher = Watcher::start(&dir, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(60));
        std::fs::write(dir.join("B.md"), "b").unwrap();
        let mut seen = Vec::new();
        for _ in 0..100 {
            seen.extend(watcher.poll());
            if !seen.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(seen, [Change::Added(dir.join("B.md"))]);
    }
}
