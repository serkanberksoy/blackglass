//! Linking (stage 1, W-119): `[[Note#` suggests the note's headings,
//! `[[Note#^` its blocks (one without an id gets one when chosen, in its
//! note), `[[##` and `[[^^` headings and blocks anywhere in the vault;
//! links to this note, heading or block copied; a new unique note linked
//! from here.

use std::path::Path;

use super::text::{self, Block};
use crate::plugins::{Context, Effect, Suggestions};
use crate::vault::{Note, Vault};

/// The most suggestions shown.
const MOST: usize = 10;

/// What's suggested for the link being typed at `col` of `line`.
pub fn suggestions(line: &str, col: usize, vault: &Vault) -> Option<Suggestions> {
    let before: String = line.chars().take(col).collect();
    let open = before.rfind("[[")?;
    let inner = &before[open + 2..];
    if inner.contains("]]") || inner.contains('|') {
        return None;
    }
    let base = before[..open + 2].chars().count();
    let chars = |s: &str| s.chars().count();
    let matches = |text: &str, q: &str| text.to_lowercase().contains(&q.to_lowercase());
    let mut out = Suggestions {
        start: base,
        ..Suggestions::default()
    };
    if let Some(q) = inner.strip_prefix("##") {
        // Headings anywhere (until the list is full).
        for note in &vault.notes {
            if out.items.len() >= MOST {
                break;
            }
            let name = text::link_name(vault, &note.path);
            for h in text::headings(&note.lines) {
                if out.items.len() < MOST && matches(&h.text, q) {
                    out.items
                        .push((format!("{name} › {}", h.text), format!("{name}#{}", h.text)));
                }
            }
        }
    } else if let Some(q) = inner.strip_prefix("^^") {
        // Blocks anywhere (something typed: there are many).
        if q.trim().is_empty() {
            return None;
        }
        for note in &vault.notes {
            if out.items.len() >= MOST {
                break;
            }
            let name = text::link_name(vault, &note.path);
            for b in text::blocks(&note.lines) {
                if out.items.len() < MOST && matches(&b.text, q) {
                    add_block(&mut out, note, &b, &name, &format!("{name}#^"));
                }
            }
        }
    } else if let Some((name, rest)) = inner.split_once('#') {
        let note = text::note_named(vault, name)?;
        if let Some(q) = rest.strip_prefix('^') {
            out.start = base + chars(name) + 2;
            for b in text::blocks(&note.lines) {
                if out.items.len() < MOST && matches(&b.text, q) {
                    add_block(&mut out, note, &b, &b.text.clone(), "");
                }
            }
        } else {
            out.start = base + chars(name) + 1;
            for h in text::headings(&note.lines) {
                if out.items.len() < MOST && matches(&h.text, rest) {
                    let indent = "  ".repeat(h.level.saturating_sub(1));
                    out.items.push((format!("{indent}{}", h.text), h.text));
                }
            }
        }
    } else {
        return None;
    }
    (!out.items.is_empty()).then_some(out)
}

/// Suggests block `b` of `note` (shown after `label`'s name): `prefix` and
/// its id; a block without one gets one when chosen.
fn add_block(out: &mut Suggestions, note: &Note, b: &Block, name: &str, prefix: &str) {
    let shown: String = b.text.chars().take(60).collect();
    let label = if name == b.text {
        shown
    } else {
        format!("{name}: {shown}")
    };
    let id = match &b.id {
        Some(id) => id.clone(),
        None => {
            let id = text::new_id(&format!("{}{}", note.path.display(), b.line));
            out.effects.push((
                out.items.len(),
                add_id(&note.path, &note.lines[b.line], b.line, &id),
            ));
            id
        }
    };
    out.items.push((label, format!("{prefix}{id}")));
}

/// Adds ` ^id` to line `i` (`old`) of the note at `path`.
fn add_id(path: &Path, old: &str, i: usize, id: &str) -> Effect {
    Effect::EditNote {
        path: path.to_path_buf(),
        from: i,
        to: i + 1,
        lines: vec![format!("{} ^{id}", old.trim_end())],
        expect: vec![old.to_string()],
    }
}

/// The active note: its path, name for links, lines and cursor.
struct Active {
    name: String,
    lines: Vec<String>,
    row: usize,
    col: usize,
}

fn active(ctx: &Context) -> Option<Active> {
    let note = ctx.note?;
    let path = note.path?;
    Some(Active {
        name: text::link_name(ctx.vault, path),
        lines: crate::vault::split_lines(note.text),
        row: note.row,
        col: note.col,
    })
}

const OPEN_A_NOTE: &str = "Zettelkasten: open a note first";

/// "Copy link to this note".
pub fn copy_note_link(ctx: &Context) -> Effect {
    match active(ctx) {
        Some(a) => Effect::CopyText(format!("[[{}]]", a.name)),
        None => Effect::Message(OPEN_A_NOTE.into()),
    }
}

/// "Copy link to this heading": the heading the cursor is under.
pub fn copy_heading_link(ctx: &Context) -> Effect {
    let Some(a) = active(ctx) else {
        return Effect::Message(OPEN_A_NOTE.into());
    };
    match text::heading_above(&a.lines, a.row) {
        Some(h) => Effect::CopyText(format!("[[{}#{h}]]", a.name)),
        None => Effect::Message("Zettelkasten: no heading above the cursor".into()),
    }
}

/// "Copy link to this block": the cursor's line, given an id if it has
/// none.
pub fn copy_block_link(ctx: &Context) -> Effect {
    let Some(a) = active(ctx) else {
        return Effect::Message(OPEN_A_NOTE.into());
    };
    let line = a.lines.get(a.row).cloned().unwrap_or_default();
    if line.trim().is_empty() || text::heading(&line).is_some() {
        return Effect::Message(
            "Zettelkasten: put the cursor on a paragraph or a list item".into(),
        );
    }
    match text::split_id(&line) {
        (_, Some(id)) => Effect::CopyText(format!("[[{}#^{id}]]", a.name)),
        (_, None) => {
            let id = text::new_id(&format!("{}{}", a.name, a.row));
            Effect::Many(vec![
                Effect::ReplaceLines {
                    from: a.row,
                    to: a.row + 1,
                    lines: vec![format!("{} ^{id}", line.trim_end())],
                    cursor: (a.row, a.col),
                },
                Effect::CopyText(format!("[[{}#^{id}]]", a.name)),
            ])
        }
    }
}

/// "New unique note linked from here": a note named by an ID (the
/// vault's ID format) in `folder` (in the vault; `""`: the selected
/// folder), from the `template` note if given, and a link to it at the
/// cursor (the selection becomes its alias).
pub fn new_unique_linked(ctx: &Context, folder: &str, template: &str) -> Effect {
    if ctx.note.is_none() {
        return Effect::Message(OPEN_A_NOTE.into());
    }
    let vault = ctx.vault;
    let dir = match folder.trim().trim_matches('/') {
        "" => ctx.folder.to_path_buf(),
        f => vault.root.join(f),
    };
    let ids = crate::note_ids::NoteIds::load(vault);
    let now = chrono::Local::now().naive_local();
    let id = ids.id(now, |id| {
        crate::note_ids::NoteIds::taken(vault, id) || dir.join(format!("{id}.md")).exists()
    });
    let text = match template.trim() {
        "" => String::new(),
        t => {
            let rel = if t.ends_with(".md") {
                t.to_string()
            } else {
                format!("{t}.md")
            };
            match std::fs::read_to_string(vault.root.join(&rel)) {
                Ok(text) => text,
                Err(_) => {
                    return Effect::Message(format!("Zettelkasten: no template {rel}"));
                }
            }
        }
    };
    let alias = ctx
        .note
        .and_then(|n| n.selection)
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty());
    let link = match alias {
        Some(alias) => format!("[[{id}|{alias}]]"),
        None => format!("[[{id}]]"),
    };
    Effect::Many(vec![
        Effect::Insert {
            text: link,
            back: 0,
        },
        Effect::CreateFile {
            path: dir.join(format!("{id}.md")),
            text,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    fn vault() -> Vault {
        let dir = scratch("zk-link-suggest");
        write(
            &dir,
            &[
                ("Dune.md", "# Dune\n## Spice\nmelange\n- item ^old"),
                ("Arrakis.md", "# Arrakis\n## Desert\nsand"),
            ],
        );
        Vault::open(&dir).unwrap()
    }

    fn texts(s: Option<Suggestions>) -> (usize, Vec<String>) {
        let s = s.expect("suggestions");
        (s.start, s.items.into_iter().map(|(_, t)| t).collect())
    }

    #[test]
    fn headings_and_blocks_are_suggested_in_links() {
        let v = vault();
        assert_eq!(
            texts(suggestions("see [[Dune#sp", 13, &v)),
            (11, vec!["Spice".into()])
        );
        let s = suggestions("[[Dune#^ite", 11, &v).unwrap();
        assert_eq!((s.start, s.items[0].1.as_str()), (8, "old"));
        assert!(s.effects.is_empty(), "it has an id");
        let s = suggestions("[[Dune#^mel", 11, &v).unwrap();
        assert_eq!(s.effects.len(), 1, "an id for it");
        assert_eq!(
            texts(suggestions("[[##des", 7, &v)),
            (2, vec!["Arrakis#Desert".into()])
        );
        let (start, items) = texts(suggestions("[[^^san", 7, &v));
        assert!(start == 2 && items[0].starts_with("Arrakis#^"), "{items:?}");
        for (line, col) in [
            ("[[Dune", 6),
            ("[[Dune]] #x", 11),
            ("[[Nope#", 7),
            ("[[^^", 4),
        ] {
            assert!(suggestions(line, col, &v).is_none(), "{line}");
        }
    }
}
