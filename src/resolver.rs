//! Finds link targets anywhere in the vault, as Obsidian does: `[[Note]]`
//! is the note named `Note` in any folder. This is mdedit's
//! [`Resolver`], so links (Ctrl+Enter), note embeds and image embeds all
//! use it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use mdedit::resolver::Resolver;

use crate::vault::Vault;

/// A snapshot of the vault's files; rebuilt when the vault is scanned
/// again.
#[derive(Debug, Clone)]
pub struct VaultResolver {
    root: PathBuf,
    /// Every file relative to the root, lowercased, with `/` separators,
    /// paired with its real relative path.
    files: Vec<(String, PathBuf)>,
    /// Every file's name, lowercased (`dune.md`), for telling quickly
    /// whether a link points anywhere (drawn every frame).
    names: HashSet<String>,
}

impl VaultResolver {
    pub fn new(vault: &Vault) -> Self {
        let files = vault
            .files
            .iter()
            .map(|rel| (key(&rel.to_string_lossy()), rel.clone()))
            .collect::<Vec<(String, PathBuf)>>();
        let names = files
            .iter()
            .map(|(file, _)| file.rsplit('/').next().unwrap_or(file).to_string())
            .collect();
        VaultResolver {
            root: vault.root.clone(),
            files,
            names,
        }
    }
}

impl VaultResolver {
    /// `path` as the vault spells it: a file system that ignores case
    /// (Windows, macOS) finds `crane.md` for the note `Crane.md`.
    fn spelled(&self, path: PathBuf) -> PathBuf {
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return path;
        };
        let wanted = key(&rel.to_string_lossy());
        self.files
            .iter()
            .find(|(file, _)| *file == wanted)
            .map_or(path.clone(), |(_, real)| self.root.join(real))
    }
}

/// How paths are compared: lowercase, `/` separators.
fn key(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

impl Resolver for VaultResolver {
    /// In order: relative to the note (`./sub/Note`, a sibling), relative to
    /// the vault root (`Folder/Note`), then by name or path ending anywhere
    /// in the vault; with several matches, the shortest path wins.
    fn resolve(&self, from: Option<&Path>, target: &str) -> Result<PathBuf, String> {
        let base = from
            .and_then(Path::parent)
            .filter(|d| !d.as_os_str().is_empty());
        for dir in base.into_iter().chain([self.root.as_path()]) {
            if let Ok(path) = mdedit::links::resolve(dir, target) {
                return Ok(self.spelled(path));
            }
        }
        let wanted = key(target.trim_start_matches("./"));
        let note = format!("{wanted}.md");
        let ends_with = |file: &str, name: &str| {
            file == name
                || file
                    .strip_suffix(name)
                    .is_some_and(|dir| dir.ends_with('/'))
        };
        self.files
            .iter()
            .filter(|(file, _)| ends_with(file, &wanted) || ends_with(file, &note))
            .min_by_key(|(file, _)| (file.matches('/').count(), file.len()))
            .map(|(_, rel)| self.root.join(rel))
            .ok_or_else(|| format!("{target} (not in the vault)"))
    }

    /// Quick for a name (`[[Dune]]`: a file with that name anywhere); a
    /// path is resolved.
    fn exists(&self, from: Option<&Path>, target: &str) -> bool {
        let wanted = key(target);
        let name = wanted.rsplit('/').next().unwrap_or(&wanted);
        let named = self.names.contains(name) || self.names.contains(&format!("{name}.md"));
        if !named {
            return false;
        }
        !wanted.contains('/') || self.resolve(from, target).is_ok()
    }

    /// A note's part as mdedit embeds it; a `.base` file as its base (one
    /// view with `#View`), for the Bases plugin to draw.
    fn embed(&self, path: &Path, fragment: Option<&str>) -> Option<Vec<String>> {
        let lines = self.load(path)?;
        if path.extension().is_some_and(|e| e == "base") {
            let mut out = vec!["```base".to_string()];
            out.extend(fragment.map(|view| format!("# view: {view}")));
            out.extend(lines.iter().cloned());
            out.push("```".into());
            return Some(out);
        }
        match fragment {
            Some(part) => mdedit::embed::part(&lines, part),
            None => Some(lines.to_vec()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn a_link_resolves_to_the_notes_own_spelling() {
        // On a file system that ignores case (Windows, macOS) `crane.md`
        // is found too: the note is still `Crane.md`.
        let (dir, r) = setup("resolver-case");
        let root = mdedit::platform::canonical(&dir).unwrap();
        assert_eq!(r.resolve(None, "dune").unwrap(), root.join("Books/Dune.md"));
        assert_eq!(r.resolve(None, "home").unwrap(), root.join("Home.md"));
        assert_eq!(
            r.resolve(None, "journal/2026-08-09").unwrap(),
            root.join("Journal/2026-08-09.md")
        );
    }

    fn setup(name: &str) -> (PathBuf, VaultResolver) {
        let dir = scratch(name);
        write(
            &dir,
            &[
                ("Home.md", ""),
                ("Journal/2026-08-09.md", ""),
                ("Journal/Old/Home.md", ""),
                ("Books/Dune.md", ""),
                ("Books/Home.md", ""),
                ("assets/Sunset.png", ""),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        (vault.root.clone(), VaultResolver::new(&vault))
    }

    #[test]
    fn a_name_is_found_in_any_folder() {
        let (root, r) = setup("resolve-name");
        let from = root.join("Journal/2026-08-09.md");
        assert_eq!(
            r.resolve(Some(&from), "Dune"),
            Ok(root.join("Books/Dune.md"))
        );
        assert_eq!(
            r.resolve(Some(&from), "dune"),
            Ok(root.join("Books/Dune.md"))
        );
        assert_eq!(
            r.resolve(Some(&from), "sunset.png"),
            Ok(root.join("assets/Sunset.png"))
        );
        assert!(r.resolve(Some(&from), "Missing").is_err());
        assert!(!r.exists(Some(&from), "une"), "whole names only");
    }

    #[test]
    fn nearby_and_shorter_paths_win() {
        let (root, r) = setup("resolve-order");
        let book = root.join("Books/Dune.md");
        assert_eq!(
            r.resolve(Some(&book), "Home"),
            Ok(root.join("Books/Home.md"))
        );
        let journal = root.join("Journal/2026-08-09.md");
        assert_eq!(r.resolve(Some(&journal), "Home"), Ok(root.join("Home.md")));
        assert_eq!(
            r.resolve(Some(&journal), "Old/Home"),
            Ok(root.join("Journal/Old/Home.md"))
        );
        assert_eq!(
            r.resolve(None, "Journal/2026-08-09"),
            Ok(root.join("Journal/2026-08-09.md"))
        );
    }

    #[test]
    fn whether_a_link_points_somewhere_is_quick_to_tell() {
        let (root, r) = setup("resolver-exists");
        let from = Some(root.join("Home.md"));
        let from = from.as_deref();
        for found in [
            "Dune",
            "dune",
            "Books/Dune",
            "Sunset.png",
            "./Home",
            "Old/Home",
        ] {
            assert!(r.exists(from, found), "{found}");
        }
        for missing in ["Nowhere", "Dune.png", "Films/Dune", "assets/Sunset"] {
            assert!(!r.exists(from, missing), "{missing}");
        }
    }
}
