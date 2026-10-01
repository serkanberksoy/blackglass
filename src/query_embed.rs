//! Search query embeds (W-36): a ```` ```query ```` block holds a vault
//! search (the search panel's syntax: words, `"phrases"`, `-`, `OR`,
//! `tag:`, `path:` …) and shows its results in the note: each note, then
//! its matching lines, leaving out the note the block is in (its own
//! query matches it). A click opens a note, or a note at a line.
//! Core, not a plugin: the code block processor asks it first.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::vault::Vault;
use crate::vault::search::{Query, run};

/// The code block language.
pub const LANGUAGE: &str = "query";

const DIM: Style = Style::new().fg(Color::DarkGray);
/// A note's name: the theme's accent.
fn note_style() -> Style {
    Style::new()
        .fg(crate::ui::theme::paint(crate::ui::theme::ACCENT))
        .add_modifier(Modifier::BOLD)
}

/// A drawn line and its action.
type Row = (Line<'static>, Option<String>);

/// Drawn blocks by (query, note, width).
type Cache = HashMap<(String, Option<PathBuf>, usize), Vec<Row>>;

/// The vault the queries search (the App's, not a copy), and the drawn
/// blocks.
#[derive(Default)]
pub struct QueryBlocks {
    vault: Weak<Vault>,
    /// Cleared when the vault changes.
    cache: RefCell<Cache>,
}

impl QueryBlocks {
    /// The vault to search; given again whenever it changes.
    pub fn set_vault(&mut self, vault: &Rc<Vault>) {
        self.vault = Rc::downgrade(vault);
        self.cache.borrow_mut().clear();
    }

    /// Forgets the drawn blocks (the theme changed).
    pub fn clear_cache(&mut self) {
        self.cache.borrow_mut().clear();
    }

    /// A block's lines in the note at `from`, `width` columns wide.
    pub fn render(&self, source: &[String], from: Option<&Path>, width: usize) -> Vec<Row> {
        let text = source
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let key = (text.clone(), from.map(Path::to_path_buf), width);
        if let Some(rows) = self.cache.borrow().get(&key) {
            return rows.clone();
        }
        let rows = self.draw(&text, from, width);
        self.cache.borrow_mut().insert(key, rows.clone());
        rows
    }

    fn draw(&self, text: &str, from: Option<&Path>, width: usize) -> Vec<Row> {
        let Some(vault) = self.vault.upgrade() else {
            return Vec::new();
        };
        let mut hits = run(&vault, &Query::parse(text));
        hits.retain(|h| Some(vault.notes[h.note].path.as_path()) != from);
        let noun = if hits.len() == 1 { "note" } else { "notes" };
        let mut out: Vec<Row> = vec![(
            Line::from(vec![
                Span::raw(text.to_string()),
                Span::styled(format!("   {} {noun}", hits.len()), DIM),
            ]),
            None,
        )];
        for h in &hits {
            let note = &vault.notes[h.note];
            let rel = note.rel.to_string_lossy().replace('\\', "/");
            out.push((
                Line::styled(fit(&note.name(), width), note_style()),
                Some(format!("open:{rel}")),
            ));
            for &(line, _) in &h.lines {
                let shown = fit(&format!("  {}", note.lines[line].trim()), width);
                out.push((Line::raw(shown), Some(format!("line:{line}:{rel}"))));
            }
            if h.total > h.lines.len() {
                let more = format!("  … {} more", h.total - h.lines.len());
                out.push((Line::styled(more, DIM), Some(format!("open:{rel}"))));
            }
        }
        out
    }
}

/// `text` cut to `width` columns.
fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    for c in text.chars() {
        if out.width() + c.to_string().width() + 1 > width {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}
