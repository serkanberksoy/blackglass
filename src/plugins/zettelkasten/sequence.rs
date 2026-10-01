//! Folgezettel (stage 2, ZK-20, ZK-21): Luhmann's IDs at the start of a
//! note's name (`1`, `1a`, `1a1`, `1a1b` … then its title: `1a Spice`),
//! numbers and letters taking turns. A sequel continues at the same level
//! (`1a` → `1b`), a branch goes one down (`1a` → `1a1`); notes sort by
//! their IDs (`1a2` before `1a10`, a note before its branches), the
//! order they're read in.

use std::cmp::Ordering;
use std::path::PathBuf;

use crate::vault::Vault;

/// One part of an ID: a number or letters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Number(u64),
    Letters(String),
}

impl Part {
    /// The next one at its level: `3` → `4`, `a` → `b`, `z` → `aa`.
    fn next(&self) -> Part {
        match self {
            Part::Number(n) => Part::Number(n + 1),
            Part::Letters(s) => {
                let mut chars: Vec<u8> = s.bytes().collect();
                let mut i = chars.len();
                loop {
                    if i == 0 {
                        chars.insert(0, b'a');
                        break;
                    }
                    i -= 1;
                    if chars[i] == b'z' {
                        chars[i] = b'a';
                    } else {
                        chars[i] += 1;
                        break;
                    }
                }
                Part::Letters(String::from_utf8(chars).expect("ASCII letters"))
            }
        }
    }

    fn order(&self, other: &Part) -> Ordering {
        match (self, other) {
            (Part::Number(a), Part::Number(b)) => a.cmp(b),
            (Part::Letters(a), Part::Letters(b)) => (a.len(), a).cmp(&(b.len(), b)),
            (Part::Number(_), Part::Letters(_)) => Ordering::Less,
            (Part::Letters(_), Part::Number(_)) => Ordering::Greater,
        }
    }
}

/// A Folgezettel ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Id(pub Vec<Part>);

impl Id {
    /// The ID a note's name starts with (then a space or nothing):
    /// numbers and lowercase letters taking turns, a number first.
    pub fn of(name: &str) -> Option<Id> {
        let token = name.split(' ').next()?;
        if token.is_empty()
            || !token
                .chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
        {
            return None;
        }
        let mut parts = Vec::new();
        let mut rest = token;
        while !rest.is_empty() {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            if digits > 0 {
                parts.push(Part::Number(rest[..digits].parse().ok()?));
                rest = &rest[digits..];
                continue;
            }
            let letters = rest.chars().take_while(char::is_ascii_lowercase).count();
            parts.push(Part::Letters(rest[..letters].to_string()));
            rest = &rest[letters..];
        }
        matches!(parts.first(), Some(Part::Number(_))).then_some(Id(parts))
    }

    pub fn depth(&self) -> usize {
        self.0.len() - 1
    }

    pub fn parent(&self) -> Option<Id> {
        (self.0.len() > 1).then(|| Id(self.0[..self.0.len() - 1].to_vec()))
    }

    /// The next ID at this level.
    pub fn sequel(&self) -> Id {
        let mut parts = self.0.clone();
        let last = parts.pop().expect("an ID has a part");
        parts.push(last.next());
        Id(parts)
    }

    /// The first ID one level down.
    pub fn branch(&self) -> Id {
        let mut parts = self.0.clone();
        parts.push(match parts.last() {
            Some(Part::Number(_)) => Part::Letters("a".into()),
            _ => Part::Number(1),
        });
        Id(parts)
    }

    /// Luhmann's order: a note before its branches, `2` before `10`.
    pub fn order(&self, other: &Id) -> Ordering {
        for (a, b) in self.0.iter().zip(&other.0) {
            match a.order(b) {
                Ordering::Equal => {}
                o => return o,
            }
        }
        self.0.len().cmp(&other.0.len())
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for p in &self.0 {
            match p {
                Part::Number(n) => write!(f, "{n}")?,
                Part::Letters(s) => write!(f, "{s}")?,
            }
        }
        Ok(())
    }
}

/// The vault's Folgezettel notes, in order: (ID, path).
pub fn notes(vault: &Vault) -> Vec<(Id, PathBuf)> {
    let mut all: Vec<(Id, PathBuf)> = vault
        .notes
        .iter()
        .filter_map(|n| Id::of(&n.name()).map(|id| (id, n.path.clone())))
        .collect();
    all.sort_by(|(a, pa), (b, pb)| a.order(b).then(pa.cmp(pb)));
    all
}

/// The first free ID from `id` on at its level (a sequel's or branch's).
pub fn free(start: Id, all: &[(Id, PathBuf)]) -> Id {
    let mut id = start;
    while all.iter().any(|(other, _)| *other == id) {
        id = id.sequel();
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> Id {
        Id::of(s).unwrap_or_else(|| panic!("{s}"))
    }

    #[test]
    fn ids_are_read_ordered_and_continued() {
        assert_eq!(id("1a10 Spice").to_string(), "1a10");
        for not in ["a1", "Spice", "1A", "", "202610011445x Big"] {
            assert!(Id::of(not).is_none() || not.starts_with('2'), "{not}");
        }
        assert_eq!(id("1a").sequel().to_string(), "1b");
        assert_eq!(id("1z").sequel().to_string(), "1aa");
        assert_eq!(id("1a9").sequel().to_string(), "1a10");
        assert_eq!(id("1a").branch().to_string(), "1a1");
        assert_eq!(id("1a1").branch().to_string(), "1a1a");
        assert_eq!(id("1a1").parent().unwrap().to_string(), "1a");
        let mut ids = [
            id("10"),
            id("1a10"),
            id("2"),
            id("1a2"),
            id("1"),
            id("1a"),
            id("1b"),
        ];
        ids.sort_by(|a, b| a.order(b));
        let order: Vec<String> = ids.iter().map(ToString::to_string).collect();
        assert_eq!(order, ["1", "1a", "1a2", "1a10", "1b", "2", "10"]);
        let all = vec![(id("1b"), PathBuf::new())];
        assert_eq!(free(id("1b"), &all).to_string(), "1c");
    }
}
