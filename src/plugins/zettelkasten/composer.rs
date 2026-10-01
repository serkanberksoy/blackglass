//! Refactoring (stage 3, W-121, after Note composer and Note refactor):
//! the selection extracted to a new or existing note (a template, and a
//! link, an embed or nothing left behind), a heading's section to a note
//! named by it, a note split at its headings, a note merged into another
//! (its links follow, it goes to the trash).

use std::path::{Path, PathBuf};

use super::text;
use crate::plugins::Effect;
use crate::vault::{Note, Vault};

/// What an extract leaves where the text was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Leave {
    #[default]
    Link,
    Embed,
    Nothing,
}

impl Leave {
    pub fn parse(s: &str) -> Leave {
        match s {
            "embed" => Leave::Embed,
            "nothing" => Leave::Nothing,
            _ => Leave::Link,
        }
    }

    /// What's left for a note called `name`.
    pub fn text(self, name: &str) -> String {
        match self {
            Leave::Link => format!("[[{name}]]"),
            Leave::Embed => format!("![[{name}]]"),
            Leave::Nothing => String::new(),
        }
    }
}

/// The extract's text from the template: `{{content}}`, `{{fromTitle}}`,
/// `{{newTitle}}`, `{{date}}` or `{{date:FORMAT}}` (moment tokens).
pub fn fill(template: &str, content: &str, from: &str, new: &str) -> String {
    let now = chrono::Local::now().naive_local();
    let mut out = template
        .replace("{{content}}", content)
        .replace("{{fromTitle}}", from)
        .replace("{{newTitle}}", new)
        .replace(
            "{{date}}",
            &crate::plugins::moment::format(&now, "YYYY-MM-DD"),
        );
    while let Some(at) = out.find("{{date:") {
        let Some(end) = out[at..].find("}}") else {
            break;
        };
        let format = out[at + 7..at + end].to_string();
        out.replace_range(
            at..at + end + 2,
            &crate::plugins::moment::format(&now, &format),
        );
    }
    out
}

/// Where text goes in a note's `lines`: at its end (before a last empty
/// line, the file's line break) or its start (after its properties).
pub fn place(lines: &[String], at_start: bool) -> usize {
    if at_start {
        if lines.first().map(|l| l.trim_end()) == Some("---")
            && let Some(end) = lines[1..]
                .iter()
                .position(|l| matches!(l.trim_end(), "---" | "..."))
        {
            return end + 2;
        }
        return 0;
    }
    match lines.last() {
        Some(last) if last.is_empty() && lines.len() > 1 => lines.len() - 1,
        _ => lines.len(),
    }
}

/// Adds `text`'s lines to `note` (at its end or start).
pub fn add_to(note: &Note, text: &str, at_start: bool) -> Effect {
    let at = place(&note.lines, at_start);
    Effect::EditNote {
        path: note.path.clone(),
        from: at,
        to: at,
        lines: text.split('\n').map(String::from).collect(),
        expect: Vec::new(),
    }
}

/// A note path in `dir` for `name` that's free (`name 1`, `name 2` …).
pub fn free_path(dir: &Path, name: &str) -> PathBuf {
    let clean: String = name
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|#^[]".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = if clean.is_empty() {
        "Untitled".into()
    } else {
        clean
    };
    (0..)
        .map(|n| match n {
            0 => dir.join(format!("{clean}.md")),
            n => dir.join(format!("{clean} {n}.md")),
        })
        .find(|p| !p.exists())
        .expect("some name is free")
}

/// The section of the heading line `row` is under: its heading's line
/// and level, and the line after it (the next heading as high or the
/// end).
pub fn section(lines: &[String], row: usize) -> Option<(usize, usize, usize)> {
    let headings = text::headings(lines);
    let h = headings.iter().take_while(|h| h.line <= row).last()?;
    let end = headings
        .iter()
        .find(|o| o.line > h.line && o.level <= h.level)
        .map_or(lines.len(), |o| o.line);
    Some((h.line, h.level, end))
}

/// The note's text without its frontmatter (for a merge).
pub fn body(note: &Note) -> String {
    let start = place(&note.lines, true);
    let mut lines = note.lines[start..].to_vec();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

/// The note's sections at heading `level`: (heading text, first line,
/// end), and where the first starts.
pub fn sections_at(lines: &[String], level: usize) -> Vec<(String, usize, usize)> {
    let headings = text::headings(lines);
    headings
        .iter()
        .filter(|h| h.level == level)
        .map(|h| {
            let end = headings
                .iter()
                .find(|o| o.line > h.line && o.level <= level)
                .map_or(lines.len(), |o| o.line);
            (h.text.clone(), h.line, end)
        })
        .collect()
}

/// The notes a merge or an extract can go to: every note but `but`, by
/// path (and their names).
pub fn other_notes(vault: &Vault, but: Option<&Path>) -> Vec<PathBuf> {
    vault
        .notes
        .iter()
        .filter(|n| Some(n.path.as_path()) != but)
        .map(|n| n.path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.split('\n').map(String::from).collect()
    }

    #[test]
    fn templates_places_and_sections() {
        let t = fill(
            "{{content}} from {{fromTitle}} to {{newTitle}} {{date:YYYY}}",
            "x",
            "A",
            "B",
        );
        assert!(t.starts_with("x from A to B 20"), "{t}");
        assert_eq!(
            place(&lines("a\nb\n"), false),
            2,
            "before the last line break"
        );
        assert_eq!(place(&lines("a\nb"), false), 2);
        assert_eq!(
            place(&lines("---\nx: 1\n---\nbody"), true),
            3,
            "after the properties"
        );
        let note = lines("# A\nintro\n## B\nb text\n### C\nc\n## D\nd");
        assert_eq!(section(&note, 5), Some((4, 3, 6)));
        assert_eq!(section(&note, 3), Some((2, 2, 6)), "with its sub-headings");
        assert_eq!(
            sections_at(&note, 2),
            [("B".to_string(), 2, 6), ("D".to_string(), 6, 8)]
        );
        assert_eq!(Leave::parse("embed").text("N"), "![[N]]");
    }
}
