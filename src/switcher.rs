//! The quick switcher (Ctrl+O / Ctrl+P): type part of a note's name (or
//! one of its `aliases`) to find it. Matching is fuzzy: the query's
//! letters must appear in order (case-insensitive), and closer, earlier,
//! whole-word matches rank higher.

use std::collections::HashMap;

use crate::vault::Vault;

/// Results listed at most.
pub const MAX_RESULTS: usize = 50;

/// The quick switcher's state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Switcher {
    pub input: String,
    /// Indexes into [`Vault::notes`], best first.
    pub results: Vec<usize>,
    /// The highlighted row; `results.len()` is the "create" row.
    pub selected: usize,
    /// The alias a result was found by (when it matched better than the
    /// note's name), by note.
    pub aliases: HashMap<usize, String>,
}

impl Switcher {
    pub fn new(vault: &Vault) -> Self {
        let mut s = Switcher::default();
        s.update(vault);
        s
    }

    /// Ranks the notes for the current input. With no input, the most
    /// recently modified notes come first.
    pub fn update(&mut self, vault: &Vault) {
        self.selected = 0;
        self.aliases.clear();
        let query = self.input.trim();
        if query.is_empty() {
            let mut all: Vec<usize> = (0..vault.notes.len()).collect();
            all.sort_by_key(|&i| std::cmp::Reverse(vault.notes[i].modified));
            all.truncate(MAX_RESULTS);
            self.results = all;
            return;
        }
        let mut scored: Vec<(i64, usize)> = Vec::new();
        for (i, n) in vault.notes.iter().enumerate() {
            let by_name = score(&n.rel_name(), query);
            let by_alias = n
                .aliases
                .iter()
                .filter_map(|a| score(a, query).map(|s| (s, a)))
                .max_by_key(|&(s, _)| s);
            match (by_name, by_alias) {
                (Some(s), Some((a, alias))) if a > s => {
                    self.aliases.insert(i, alias.clone());
                    scored.push((a, i));
                }
                (None, Some((a, alias))) => {
                    self.aliases.insert(i, alias.clone());
                    scored.push((a, i));
                }
                (Some(s), _) => scored.push((s, i)),
                (None, None) => {}
            }
        }
        scored.sort_by_key(|&(s, i)| (std::cmp::Reverse(s), vault.notes[i].rel.clone()));
        self.results = scored
            .into_iter()
            .take(MAX_RESULTS)
            .map(|(_, i)| i)
            .collect();
    }

    /// Whether the last row offers to create a note named after the input
    /// (there is input, and no note has exactly that name).
    pub fn offers_create(&self, vault: &Vault) -> bool {
        let query = self.input.trim();
        !query.is_empty()
            && !vault.notes.iter().any(|n| {
                n.name().eq_ignore_ascii_case(query) || n.rel_name().eq_ignore_ascii_case(query)
            })
    }

    /// The number of rows, including the "create" row.
    pub fn rows(&self, vault: &Vault) -> usize {
        self.results.len() + usize::from(self.offers_create(vault))
    }
}

/// How well `query` matches `candidate` (a note's path without `.md`);
/// `None` if its letters don't all appear in order. Higher is better.
pub fn score(candidate: &str, query: &str) -> Option<i64> {
    let text: Vec<char> = candidate.chars().flat_map(char::to_lowercase).collect();
    let query: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    // The note's own name (after the last `/`) counts more than its folder.
    let name_start = text.iter().rposition(|&c| c == '/').map_or(0, |i| i + 1);
    let mut score: i64 = 0;
    let mut at = 0;
    let mut prev: Option<usize> = None;
    for q in query {
        let found = at + text[at..].iter().position(|&c| c == q)?;
        let word_start = found == 0 || matches!(text[found - 1], '/' | ' ' | '-' | '_' | '.');
        score += 1;
        if prev == Some(found.wrapping_sub(1)) {
            score += 5; // consecutive letters
        }
        if word_start {
            score += 3;
        }
        if found >= name_start {
            score += 2;
        }
        prev = Some(found);
        at = found + 1;
    }
    // Shorter paths are closer matches.
    Some(score * 100 - text.len() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn letters_must_appear_in_order() {
        assert!(score("Journal/2026-08-09", "jrn").is_some());
        assert!(score("Journal/2026-08-09", "njr").is_none());
        assert!(score("Books", "BOOKS").is_some(), "case-insensitive");
    }

    #[test]
    fn whole_words_and_names_rank_higher() {
        let s = |c| score(c, "dune").unwrap();
        assert!(s("Books/Dune") > s("Reading/dunedin notes"));
        assert!(s("Books/Dune") > s("Dune/Books about sand"));
        assert!(s("Dune") > s("Books/Dune"), "shorter path");
        assert!(s("Books/Dune") > s("d-u-n-e"));
    }

    #[test]
    fn the_switcher_ranks_notes_and_offers_to_create_a_new_one() {
        let dir = scratch("switcher");
        write(
            &dir,
            &[
                ("Books/Dune.md", ""),
                ("Dunes of Sand.md", ""),
                ("Home.md", ""),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        let mut s = Switcher::new(&vault);
        assert_eq!(s.results.len(), 3, "no input: every note");
        s.input = "dune".into();
        s.update(&vault);
        let names: Vec<_> = s.results.iter().map(|&i| vault.notes[i].name()).collect();
        assert_eq!(names, ["Dune", "Dunes of Sand"]);
        assert!(!s.offers_create(&vault), "Dune exists");
        assert_eq!(s.rows(&vault), 2);
        s.input = "Dun".into();
        s.update(&vault);
        assert!(s.offers_create(&vault));
        assert_eq!(s.rows(&vault), 3);
    }
}
