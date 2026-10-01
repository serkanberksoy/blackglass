//! Reading a note's structure for the Zettelkasten plugin: its headings,
//! its blocks (paragraphs, list items) with their `^id`s, the heading a
//! line is under, a note's name for links, new block ids.

use std::path::Path;

use crate::vault::{Note, Vault};

/// A heading: its line, level and text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub line: usize,
    pub level: usize,
    pub text: String,
}

/// A block: its line, its text (without the `^id`) and its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub line: usize,
    pub text: String,
    pub id: Option<String>,
}

/// The lines that are the note's text: not in its frontmatter or in a
/// fenced code block, with their indexes.
fn body(lines: &[String]) -> impl Iterator<Item = (usize, &String)> {
    let front_end = (lines.first().map(|l| l.trim_end()) == Some("---"))
        .then(|| {
            lines[1..]
                .iter()
                .position(|l| matches!(l.trim_end(), "---" | "..."))
                .map(|i| i + 1)
        })
        .flatten();
    let mut fence = false;
    lines.iter().enumerate().filter(move |(i, line)| {
        if front_end.is_some_and(|end| *i <= end) {
            return false;
        }
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            return false;
        }
        !fence
    })
}

/// A heading line's level and text.
pub fn heading(line: &str) -> Option<(usize, String)> {
    let t = line.trim_start();
    let level = t.chars().take_while(|&c| c == '#').count();
    ((1..=6).contains(&level) && t[level..].starts_with(' '))
        .then(|| (level, t[level..].trim().to_string()))
}

/// The note's headings, in order.
pub fn headings(lines: &[String]) -> Vec<Heading> {
    body(lines)
        .filter_map(|(i, l)| {
            heading(l).map(|(level, text)| Heading {
                line: i,
                level,
                text,
            })
        })
        .collect()
}

/// A line's `^id` at its end, and the text before it.
pub fn split_id(line: &str) -> (&str, Option<&str>) {
    let trimmed = line.trim_end();
    if let Some(at) = trimmed.rfind(" ^") {
        let id = &trimmed[at + 2..];
        if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return (&trimmed[..at], Some(id));
        }
    }
    (trimmed, None)
}

/// The note's blocks: lines of text (not headings, rules, table borders
/// or empty lines).
pub fn blocks(lines: &[String]) -> Vec<Block> {
    body(lines)
        .filter(|(_, l)| {
            let t = l.trim();
            !t.is_empty()
                && heading(l).is_none()
                && !matches!(t, "---" | "***" | "___")
                && !t.starts_with("|-")
                && !t.starts_with("| -")
        })
        .map(|(i, l)| {
            let (text, id) = split_id(l);
            Block {
                line: i,
                text: text.trim().to_string(),
                id: id.map(String::from),
            }
        })
        .collect()
}

/// The heading line `row` is under (its text), if any.
pub fn heading_above(lines: &[String], row: usize) -> Option<String> {
    headings(lines)
        .into_iter()
        .take_while(|h| h.line <= row)
        .last()
        .map(|h| h.text)
}

/// A note by the name a link uses (`Dune`, `Books/Dune`), any case.
pub fn note_named<'a>(vault: &'a Vault, name: &str) -> Option<&'a Note> {
    let name = name.trim().trim_end_matches(".md");
    vault
        .notes
        .iter()
        .filter(|n| {
            let rel = n.rel_name().replace('\\', "/");
            rel.eq_ignore_ascii_case(name) || n.name().eq_ignore_ascii_case(name)
        })
        .min_by_key(|n| n.rel_name().len())
}

/// The name a link to the note at `path` uses: its name, or its path in
/// the vault when another note has the same name.
pub fn link_name(vault: &Vault, path: &Path) -> String {
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let same = vault
        .notes
        .iter()
        .filter(|n| n.name().eq_ignore_ascii_case(&name))
        .count();
    if same > 1 {
        let rel = path.strip_prefix(&vault.root).unwrap_or(path);
        let rel = rel.to_string_lossy().replace('\\', "/");
        rel.strip_suffix(".md").unwrap_or(&rel).to_string()
    } else {
        name
    }
}

/// A new block id: six lowercase letters and digits, as the original
/// makes them; `seed` makes ids made at the same moment differ.
pub fn new_id(seed: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default()
        .hash(&mut hasher);
    let mut x = hasher.finish() | 1;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    (0..6)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            CHARS[(x % CHARS.len() as u64) as usize] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn headings_and_blocks_of_a_note() {
        let note = lines(
            "---\ntitle: x\n---\n# Dune\ntext here\n```\n# not one\n```\n## Spice\n- item ^abc\n\n---\n| a |\n|---|",
        );
        let h: Vec<(usize, usize, String)> = headings(&note)
            .into_iter()
            .map(|h| (h.line, h.level, h.text))
            .collect();
        assert_eq!(h, [(3, 1, "Dune".into()), (8, 2, "Spice".into())]);
        let b: Vec<(usize, String, Option<String>)> = blocks(&note)
            .into_iter()
            .map(|b| (b.line, b.text, b.id))
            .collect();
        assert_eq!(
            b,
            [
                (4, "text here".into(), None),
                (9, "- item".into(), Some("abc".into())),
                (12, "| a |".into(), None),
            ]
        );
        assert_eq!(heading_above(&note, 4).as_deref(), Some("Dune"));
        assert_eq!(heading_above(&note, 9).as_deref(), Some("Spice"));
        assert_eq!(heading_above(&note, 1), None);
        let id = new_id("x");
        assert!(id.len() == 6 && id.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_eq!(split_id("a ^b-1"), ("a", Some("b-1")));
        assert_eq!(split_id("2 ^ 3"), ("2 ^ 3", None));
    }
}
