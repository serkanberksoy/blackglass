//! Tag and property suggestions while typing (W-24): after `#` the vault's
//! tags; in the frontmatter, its property names (on a line's key), the
//! values a property has in other notes (after `key: `) and tags (in a
//! `tags:` list). Shown in the same popup as a plugin's suggestions
//! ([`crate::link_suggest::PluginSuggest`]); Enter or Tab puts one in
//! place of what's typed.

use crate::plugins::Suggestions;
use crate::vault::Vault;

/// The most suggestions shown.
pub const MOST: usize = 10;

/// The suggestions for the cursor at char `col` of line `row`, if it's
/// where a tag or a property is typed.
pub fn suggestions(vault: &Vault, lines: &[String], row: usize, col: usize) -> Option<Suggestions> {
    let line = lines.get(row)?;
    let before: String = line.chars().take(col).collect();
    if in_frontmatter(lines, row) {
        return frontmatter(vault, lines, row, &before);
    }
    // `#ta`: at the line start or after a space, not in code.
    let hash = before.rfind('#')?;
    let typed = &before[hash + 1..];
    let after_space = before[..hash]
        .chars()
        .next_back()
        .is_none_or(char::is_whitespace);
    let in_code = before[..hash].matches('`').count() % 2 == 1;
    if !after_space || in_code || typed.is_empty() || !crate::vault::tags::is_tag_name(typed) {
        return None;
    }
    let start = before[..hash + 1].chars().count();
    let items = matching(tags(vault), typed);
    (!items.is_empty()).then(|| Suggestions {
        start,
        items: items.into_iter().map(|t| (format!("#{t}"), t)).collect(),
        ..Default::default()
    })
}

/// Whether line `row` is inside the note's frontmatter (not its `---`).
fn in_frontmatter(lines: &[String], row: usize) -> bool {
    if row == 0 || lines.first().map(|l| l.trim_end()) != Some("---") {
        return false;
    }
    let end = lines[1..]
        .iter()
        .position(|l| matches!(l.trim_end(), "---" | "..."))
        .map(|i| i + 1);
    end.is_none_or(|end| row < end)
}

/// In the frontmatter: a property's name, its value, or a tag in a list.
fn frontmatter(vault: &Vault, lines: &[String], row: usize, before: &str) -> Option<Suggestions> {
    let chars = |s: &str| s.chars().count();
    // `  - ta` under `tags:` (or `aliases` … any list: its values).
    if let Some(item) = before.trim_start().strip_prefix("- ") {
        let key = lines[1..row]
            .iter()
            .rev()
            .find(|l| !l.starts_with([' ', '\t', '-']))
            .and_then(|l| l.split_once(':'))
            .map(|(k, _)| k.trim().to_string())?;
        let start = chars(before) - chars(item);
        return values(vault, &key, item, start);
    }
    match before.split_once(':') {
        // `ke`: the vault's property names.
        None if !before.starts_with([' ', '\t']) && !before.is_empty() => {
            let mut keys: Vec<(String, usize)> = Vec::new();
            for note in &vault.notes {
                for (k, _) in &note.properties {
                    match keys.iter_mut().find(|(x, _)| x == k) {
                        Some((_, n)) => *n += 1,
                        None => keys.push((k.clone(), 1)),
                    }
                }
            }
            let items: Vec<(String, String)> = matching(keys, before)
                .into_iter()
                .filter(|k| k != before)
                .map(|k| (k.clone(), format!("{k}: ")))
                .collect();
            (!items.is_empty()).then_some(Suggestions {
                start: 0,
                items,
                ..Default::default()
            })
        }
        // `key: va` (or `tags: [a, b`): its values in other notes.
        Some((key, value)) => {
            let typed = value
                .rsplit([',', '['])
                .next()
                .unwrap_or_default()
                .trim_start();
            let start = chars(before) - chars(typed);
            values(vault, key.trim(), typed, start)
        }
        None => None,
    }
}

/// The values property `key` has in the vault (tags for `tags`) that
/// match `typed`, replacing from char `start`.
fn values(vault: &Vault, key: &str, typed: &str, start: usize) -> Option<Suggestions> {
    if typed.is_empty() {
        return None;
    }
    let found = if key.eq_ignore_ascii_case("tags") {
        tags(vault)
    } else {
        let mut all: Vec<(String, usize)> = Vec::new();
        for note in &vault.notes {
            for (_, v) in note.properties.iter().filter(|(k, _)| k == key) {
                let v = v.trim();
                let items: Vec<&str> = match v.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    Some(list) => list.split(',').collect(),
                    None => vec![v],
                };
                for item in items {
                    let item = item.trim().trim_matches(['"', '\'']);
                    if item.is_empty() {
                        continue;
                    }
                    match all.iter_mut().find(|(x, _)| x == item) {
                        Some((_, n)) => *n += 1,
                        None => all.push((item.to_string(), 1)),
                    }
                }
            }
        }
        all
    };
    let items: Vec<(String, String)> = matching(found, typed)
        .into_iter()
        .filter(|v| v != typed)
        .map(|v| (v.clone(), v))
        .collect();
    (!items.is_empty()).then_some(Suggestions {
        start,
        items,
        ..Default::default()
    })
}

/// The vault's tags with how many notes have each.
fn tags(vault: &Vault) -> Vec<(String, usize)> {
    vault
        .tags
        .iter()
        .map(|t| (t.name.clone(), t.notes))
        .collect()
}

/// `all` that start with `typed` (any case), then those containing it;
/// each group by how often it's used, then by name; at most [`MOST`].
fn matching(mut all: Vec<(String, usize)>, typed: &str) -> Vec<String> {
    let typed = typed.to_lowercase();
    all.retain(|(name, _)| name.to_lowercase().contains(&typed));
    all.sort_by(|(a, n), (b, m)| {
        let starts = |s: &str| !s.to_lowercase().starts_with(&typed);
        (starts(a), std::cmp::Reverse(*n), a.to_lowercase()).cmp(&(
            starts(b),
            std::cmp::Reverse(*m),
            b.to_lowercase(),
        ))
    });
    all.into_iter().take(MOST).map(|(name, _)| name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    /// A vault of its own for each test (they run at once).
    fn vault(name: &str) -> Vault {
        let dir = scratch(name);
        write(
            &dir,
            &[
                (
                    "A.md",
                    "---\nstatus: done\ntags: [project]\n---\n#project/alpha #reading",
                ),
                (
                    "B.md",
                    "---\nstatus: doing\npriority: high\n---\n#project #proto",
                ),
                ("C.md", "---\nstatus: done\n---\n#project"),
            ],
        );
        Vault::open(&dir).unwrap()
    }

    fn texts(s: Option<Suggestions>) -> (usize, Vec<String>) {
        let s = s.expect("suggestions");
        (s.start, s.items.into_iter().map(|(_, t)| t).collect())
    }

    #[test]
    fn tags_after_a_hash() {
        let v = vault("tag-suggest-tags");
        let lines = vec!["see #pro".to_string()];
        assert_eq!(
            texts(suggestions(&v, &lines, 0, 8)),
            (
                5,
                vec!["project".into(), "project/alpha".into(), "proto".into()]
            ),
            "the most used first"
        );
        let lines = vec!["x #al".to_string()];
        assert_eq!(
            texts(suggestions(&v, &lines, 0, 5)).1,
            ["project/alpha"],
            "containing it"
        );
        for (line, col) in [("a#pro", 5), ("`#pro", 5), ("#", 1), ("# Heading", 9)] {
            let lines = vec![line.to_string()];
            assert!(suggestions(&v, &lines, 0, col).is_none(), "{line}");
        }
    }

    #[test]
    fn properties_and_their_values_in_the_frontmatter() {
        let v = vault("tag-suggest-properties");
        let lines: Vec<String> = [
            "---",
            "st",
            "status: do",
            "tags: [re",
            "tags:",
            "  - pro",
            "---",
            "pri",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            texts(suggestions(&v, &lines, 1, 2)),
            (0, vec!["status: ".into()])
        );
        assert_eq!(
            texts(suggestions(&v, &lines, 2, 10)),
            (8, vec!["done".into(), "doing".into()]),
            "used twice first"
        );
        assert_eq!(
            texts(suggestions(&v, &lines, 3, 9)),
            (7, vec!["reading".into()])
        );
        assert_eq!(
            texts(suggestions(&v, &lines, 5, 7)).1[0],
            "project",
            "a list under tags:"
        );
        assert!(
            suggestions(&v, &lines, 7, 3).is_none(),
            "not in the frontmatter"
        );
    }
}
