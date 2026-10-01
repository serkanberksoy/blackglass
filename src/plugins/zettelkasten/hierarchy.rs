//! Hierarchy fields (stage 2, ZK-22, after Breadcrumbs): a note's `up`,
//! `down`, `next` and `prev` properties link it into a structure; its
//! breadcrumbs follow `up` (`Home › Project › Note`), and commands go up,
//! down, to the next or the previous note.

use std::path::{Path, PathBuf};

use super::text;
use crate::vault::{Note, Vault};

/// The notes a note's property `key` links to (`"[[Note]]"`, `Note`, or a
/// list of them).
pub fn linked(vault: &Vault, note: &Note, key: &str) -> Vec<PathBuf> {
    let Some((_, value)) = note
        .properties
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
    else {
        return Vec::new();
    };
    // The index keeps a list as `a, b` (quotes gone), and an unquoted
    // `[[Note]]` as YAML reads it, `[Note]`.
    value
        .split(',')
        .filter_map(|item| {
            let item = item.trim().trim_matches(['"', '\'', '[', ']']);
            let name = item.split(['|', '#']).next()?.trim();
            text::note_named(vault, name).map(|n| n.path.clone())
        })
        .collect()
}

/// The note at `path`'s breadcrumbs: its `up` chain from the top, then
/// itself (the notes' shown names via `name`).
pub fn trail(vault: &Vault, path: &Path, name: impl Fn(&Path) -> String) -> Option<String> {
    let mut chain = vec![path.to_path_buf()];
    let mut at = vault.note(path)?;
    while let Some(up) = linked(vault, at, "up").into_iter().next() {
        if chain.contains(&up) || chain.len() > 20 {
            break;
        }
        chain.push(up.clone());
        let Some(next) = vault.note(&up) else { break };
        at = next;
    }
    (chain.len() > 1).then(|| {
        chain
            .iter()
            .rev()
            .map(|p| name(p))
            .collect::<Vec<_>>()
            .join(" › ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn fields_link_to_notes_in_any_spelling() {
        let dir = scratch("zk-hierarchy");
        write(
            &dir,
            &[
                (
                    "A.md",
                    "---\nup: \"[[Home|home]]\"\ndown: [\"[[B]]\", C]\nnext: [[B]]\n---\n",
                ),
                ("Home.md", ""),
                ("B.md", "---\nup: \"[[A]]\"\n---\n"),
                ("C.md", ""),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        let a = vault.note(&vault.root.join("A.md")).unwrap();
        let names = |key: &str| -> Vec<String> {
            linked(&vault, a, key)
                .iter()
                .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(names("up"), ["Home"]);
        assert_eq!(names("down"), ["B", "C"]);
        assert_eq!(names("next"), ["B"], "unquoted");
        let shown = |p: &Path| p.file_stem().unwrap().to_string_lossy().into_owned();
        assert_eq!(
            trail(&vault, &vault.root.join("B.md"), shown).as_deref(),
            Some("Home › A › B")
        );
        assert_eq!(trail(&vault, &vault.root.join("C.md"), shown), None);
    }
}
