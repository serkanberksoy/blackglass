//! Backlinks: the notes that link to a note, with the lines that do.
//! A link is a wiki link (`[[Note]]`, `[[Note|alias]]`, `[[Note#Heading]]`,
//! `![[Note]]`) or a Markdown link to a file (`[text](Note.md)`), found
//! the way the editor follows them ([`VaultResolver`]).

use std::path::{Path, PathBuf};

use mdedit::resolver::Resolver;

use crate::resolver::VaultResolver;
use crate::vault::Vault;

/// A line of another note that links to the note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlink {
    /// The note the line is in.
    pub note: PathBuf,
    /// The line's index in it.
    pub line: usize,
    pub text: String,
}

/// The link targets in a line, in order (not inside inline code): a wiki
/// link's note (without `|alias` or `#heading`), a Markdown link's file
/// (decoded, without `#heading`; web links left out).
pub fn targets(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    // The odd pieces between backticks are inline code.
    for part in line.split('`').step_by(2) {
        wiki_targets(part, &mut out);
        markdown_targets(part, &mut out);
    }
    out
}

fn wiki_targets(text: &str, out: &mut Vec<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            return;
        };
        let target = after[..end]
            .split(['|', '#'])
            .next()
            .unwrap_or_default()
            // `\|` in a table cell.
            .trim_end_matches('\\')
            .trim();
        if !target.is_empty() {
            out.push(target.to_string());
        }
        rest = &after[end + 2..];
    }
}

fn markdown_targets(text: &str, out: &mut Vec<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else {
            return;
        };
        let raw = after[..end]
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');
        let file = raw.split('#').next().unwrap_or_default();
        let web = raw.contains("://") || raw.starts_with("mailto:");
        if !web && !file.is_empty() {
            out.push(percent_decode(file));
        }
        rest = &after[end + 1..];
    }
}

/// `%20` and the like as the characters they stand for.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The lines of the vault's other notes that link to the note at `path`
/// (one per line; not in code blocks), by the notes' paths.
pub fn backlinks(vault: &Vault, path: &Path) -> Vec<Backlink> {
    let resolver = VaultResolver::new(vault);
    let mut notes: Vec<_> = vault.notes.iter().filter(|n| n.path != path).collect();
    notes.sort_by_key(|n| n.rel.to_string_lossy().to_lowercase());
    let mut found = Vec::new();
    for note in notes {
        let mut in_code = false;
        for (i, line) in note.lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_code = !in_code;
                continue;
            }
            if in_code {
                continue;
            }
            let links_here = targets(line).iter().any(|target| {
                resolver
                    .resolve(Some(&note.path), target)
                    .is_ok_and(|p| p == path)
            });
            if links_here {
                found.push(Backlink {
                    note: note.path.clone(),
                    line: i,
                    text: line.clone(),
                });
            }
        }
    }
    found
}

/// A note's text with its links to moved files pointed at their new
/// places, and how many links changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkUpdate {
    /// The note (where it is before the move).
    pub note: PathBuf,
    pub text: String,
    pub links: usize,
}

/// For files about to move (`(from, to)`, absolute), the notes whose links
/// to them need to change, with their new text (from the files on disk;
/// line endings kept; not in code). Worked out before the move, while the
/// old names still resolve.
pub fn link_updates(vault: &Vault, moves: &[(PathBuf, PathBuf)]) -> Vec<LinkUpdate> {
    let resolver = VaultResolver::new(vault);
    let mut updates = Vec::new();
    for note in &vault.notes {
        let Ok(text) = std::fs::read_to_string(&note.path) else {
            continue;
        };
        let mut links = 0;
        let mut in_code = false;
        let mut out = String::with_capacity(text.len());
        for piece in text.split_inclusive('\n') {
            let (line, end) = match piece.strip_suffix('\n') {
                Some(line) => (
                    line.strip_suffix('\r').unwrap_or(line),
                    &piece[line.trim_end_matches('\r').len()..],
                ),
                None => (piece, ""),
            };
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_code = !in_code;
            }
            if in_code || trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                out.push_str(piece);
                continue;
            }
            let new = rewrite_line(line, |target, markdown| {
                let path = resolver.resolve(Some(&note.path), target).ok()?;
                let (_, to) = moves.iter().find(|(from, _)| *from == path)?;
                links += 1;
                Some(new_target(vault, target, to, markdown))
            });
            out.push_str(&new);
            out.push_str(end);
        }
        if links > 0 {
            updates.push(LinkUpdate {
                note: note.path.clone(),
                text: out,
                links,
            });
        }
    }
    updates
}

/// How a link that said `target` names `to`, where its file moves: a bare name stays
/// a bare name, a path becomes the new path (from the vault's top); `.md`
/// kept if it was written; Markdown links percent-encode spaces.
fn new_target(vault: &Vault, target: &str, to: &Path, markdown: bool) -> String {
    let with_md = target.to_lowercase().ends_with(".md");
    let rel = to.strip_prefix(&vault.root).unwrap_or(to);
    let mut new = if target.contains('/') {
        rel.to_string_lossy().replace('\\', "/")
    } else {
        rel.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    if !with_md && let Some(stem) = new.strip_suffix(".md") {
        new = stem.to_string();
    }
    if markdown {
        new = new.replace(' ', "%20");
    }
    new
}

/// `line` with each link's target (outside inline code) replaced by what
/// `rewrite` gives for it (`None`: kept). `rewrite` gets the target as
/// resolved (Markdown links decoded) and whether it's a Markdown link.
pub fn rewrite_line(line: &str, mut rewrite: impl FnMut(&str, bool) -> Option<String>) -> String {
    let mut out = String::with_capacity(line.len());
    for (i, part) in line.split('`').enumerate() {
        if i > 0 {
            out.push('`');
        }
        if i % 2 == 1 {
            out.push_str(part);
            continue;
        }
        let wiki = rewrite_wiki(part, &mut rewrite);
        out.push_str(&rewrite_markdown(&wiki, &mut rewrite));
    }
    out
}

fn rewrite_wiki(text: &str, rewrite: &mut impl FnMut(&str, bool) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        let inner = &after[..end];
        let cut = inner.find(['|', '#']).unwrap_or(inner.len());
        let (raw, tail) = inner.split_at(cut);
        let target = raw.trim_end_matches('\\').trim();
        out.push_str(&rest[..start + 2]);
        match (!target.is_empty())
            .then(|| rewrite(target, false))
            .flatten()
        {
            Some(new) => {
                out.push_str(&new);
                out.push_str(&raw[raw.trim_end_matches('\\').len()..]);
            }
            None => out.push_str(raw),
        }
        out.push_str(tail);
        out.push_str("]]");
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

fn rewrite_markdown(text: &str, rewrite: &mut impl FnMut(&str, bool) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else {
            break;
        };
        let inner = &after[..end];
        let cut = inner.find('#').unwrap_or(inner.len());
        let (file, tail) = inner.split_at(cut);
        let web = inner.contains("://") || inner.starts_with("mailto:");
        out.push_str(&rest[..start + 2]);
        let decoded = percent_decode(file.trim());
        match (!web && !decoded.is_empty())
            .then(|| rewrite(&decoded, true))
            .flatten()
        {
            Some(new) => out.push_str(&new),
            None => out.push_str(file),
        }
        out.push_str(tail);
        out.push(')');
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// A link from the note to another (or to nothing yet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    /// The target as written.
    pub name: String,
    /// Where it leads; `None` if no such note exists.
    pub path: Option<PathBuf>,
}

/// The links in the note's `lines` (not in code), each target once, in
/// order: resolved from `from` like the editor does.
pub fn outgoing(lines: &[String], from: &Path, vault: &Vault) -> Vec<Outgoing> {
    let resolver = VaultResolver::new(vault);
    let mut out: Vec<Outgoing> = Vec::new();
    let mut in_code = false;
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        for name in targets(line) {
            if out.iter().any(|o| o.name.eq_ignore_ascii_case(&name)) {
                continue;
            }
            let path = resolver.resolve(Some(from), &name).ok();
            out.push(Outgoing { name, path });
        }
    }
    out
}

/// Where `names` (the note's name and aliases) are written in `line` as a
/// whole word, outside links and inline code: the first (start, end) in
/// bytes.
pub fn mention_at(line: &str, names: &[String]) -> Option<(usize, usize)> {
    // Links and code blanked out, keeping the byte positions.
    let mut plain: Vec<u8> = line.as_bytes().to_vec();
    let mut blank = |from: usize, to: usize| {
        for b in &mut plain[from..to.min(line.len())] {
            *b = b' ';
        }
    };
    for (open, close) in [("[[", "]]"), ("`", "`"), ("](", ")"), ("[", "]")] {
        let mut at = 0;
        while let Some(start) = line[at..].find(open).map(|i| at + i) {
            let Some(end) = line[start + open.len()..]
                .find(close)
                .map(|i| start + open.len() + i)
            else {
                break;
            };
            if open == "[" {
                // A Markdown link's text only, not any bracket.
                if line[end..].starts_with("](") {
                    blank(start, end + 1);
                }
            } else {
                blank(start, end + close.len());
            }
            at = end + close.len();
        }
    }
    let plain = String::from_utf8_lossy(&plain).to_lowercase();
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    names
        .iter()
        .filter(|n| !n.trim().is_empty())
        .filter_map(|name| {
            let name = name.to_lowercase();
            let mut from = 0;
            while let Some(i) = plain[from..].find(&name).map(|i| from + i) {
                let end = i + name.len();
                if !word(plain[..i].chars().next_back()) && !word(plain[end..].chars().next()) {
                    return Some((i, end));
                }
                from = i + 1;
            }
            None
        })
        .min()
}

/// The lines of the vault's other notes that name the note at `path` (or
/// one of its `aliases`) without linking it: not in code or the
/// properties.
pub fn mentions(vault: &Vault, path: &Path) -> Vec<Backlink> {
    let Some(note) = vault.note(path) else {
        return Vec::new();
    };
    let mut names = vec![note.name()];
    names.extend(note.aliases.iter().cloned());
    let mut notes: Vec<_> = vault.notes.iter().filter(|n| n.path != path).collect();
    notes.sort_by_key(|n| n.rel.to_string_lossy().to_lowercase());
    let mut found = Vec::new();
    for other in notes {
        let front = crate::vault::properties::frontmatter_end(&other.lines).map_or(0, |e| e + 1);
        let mut in_code = false;
        for (i, line) in other.lines.iter().enumerate().skip(front) {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_code = !in_code;
                continue;
            }
            if !in_code && mention_at(line, &names).is_some() {
                found.push(Backlink {
                    note: other.path.clone(),
                    line: i,
                    text: line.clone(),
                });
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn link_targets_in_a_line() {
        assert_eq!(
            targets("See [[Dune]], [[Books/Emma|Emma]] and ![[Map#West]]."),
            ["Dune", "Books/Emma", "Map"]
        );
        assert_eq!(
            targets("[text](Other%20Note.md) [web](https://x.org)"),
            ["Other Note.md"]
        );
        assert_eq!(targets("`[[not a link]]` [[a]]"), ["a"], "not in code");
        assert!(targets("no links [[]]").is_empty());
    }

    #[test]
    fn links_are_rewritten_in_their_own_style() {
        let line = "[[Dune]] [[Dune|b]] [[Books/Dune#P]] `[[Dune]]` [m](Books/Dune.md#x) [w](https://a.b/Dune)";
        let out = rewrite_line(line, |t, md| {
            let name = t.trim_end_matches(".md");
            (name.ends_with("Dune")).then(|| {
                format!(
                    "{}{}",
                    name.replace("Dune", "New Name"),
                    if md { ".md" } else { "" }
                )
            })
        });
        assert_eq!(
            out,
            "[[New Name]] [[New Name|b]] [[Books/New Name#P]] `[[Dune]]` [m](Books/New Name.md#x) [w](https://a.b/Dune)"
        );
    }

    #[test]
    fn mentions_are_whole_words_outside_links() {
        let names = vec!["Crane".to_string()];
        assert_eq!(mention_at("tastes like crane.", &names), Some((12, 17)));
        assert_eq!(
            mention_at("[[Crane]] and `crane` and [crane](x)", &names),
            None
        );
        assert_eq!(
            mention_at("cranes crane", &names),
            Some((7, 12)),
            "a whole word"
        );
    }

    #[test]
    fn backlinks_are_the_lines_linking_to_a_note() {
        let dir = scratch("backlinks");
        write(
            &dir,
            &[
                ("Dune.md", "# Dune"),
                ("A.md", "top\nsee [[Dune]]\n```\n[[Dune]] in code\n```"),
                ("Sub/B.md", "[[dune|the book]] and [[Dune#Plot]]"),
                ("C.md", "[[Dune2]] is another"),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        let dune = vault.root.join("Dune.md");
        let found = backlinks(&vault, &dune);
        let lines: Vec<_> = found
            .iter()
            .map(|b| (crate::vault::slash(vault.rel(&b.note).unwrap()), b.line))
            .collect();
        assert_eq!(
            lines,
            [("A.md".into(), 1), ("Sub/B.md".into(), 0)],
            "one per line; not in code"
        );
        assert_eq!(found[0].text, "see [[Dune]]");
    }
}
