//! The vault's links as a graph (stage 4, W-122): which notes each note
//! links to and is linked from, and the links that point nowhere. For the
//! orphan, dead-end and unresolved lists, the local tree of links and the
//! link counts beside links.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use mdedit::resolver::Resolver;

use crate::resolver::VaultResolver;
use crate::vault::Vault;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph {
    /// The notes each note links to (each once, in order).
    pub out: HashMap<PathBuf, Vec<PathBuf>>,
    /// The notes linking to each note (each once).
    pub ins: HashMap<PathBuf, Vec<PathBuf>>,
    /// Links to nothing: target (as written) → the notes with them.
    pub unresolved: BTreeMap<String, Vec<PathBuf>>,
    /// How many notes link to a note, by its name (lowercase), for the
    /// badges beside links.
    pub counts: HashMap<String, usize>,
}

impl Graph {
    pub fn of(vault: &Vault) -> Graph {
        let resolver = VaultResolver::new(vault);
        // Most links are a name (`[[Note]]`): found without the disk, the
        // shortest path first, as the resolver does.
        let mut by_name: HashMap<String, PathBuf> = HashMap::new();
        for note in &vault.notes {
            let key = note.name().to_lowercase();
            let shorter = by_name
                .get(&key)
                .is_none_or(|p| note.path.as_os_str().len() < p.as_os_str().len());
            if shorter {
                by_name.insert(key, note.path.clone());
            }
        }
        let mut g = Graph::default();
        for note in &vault.notes {
            let mut out: Vec<PathBuf> = Vec::new();
            let mut fence = false;
            for line in &note.lines {
                let t = line.trim_start();
                if t.starts_with("```") || t.starts_with("~~~") {
                    fence = !fence;
                }
                if fence {
                    continue;
                }
                for target in crate::backlinks::targets(line) {
                    if target.is_empty() {
                        continue;
                    }
                    let name = target.trim_end_matches(".md").to_lowercase();
                    let found = match by_name.get(&name) {
                        Some(p) if !target.contains('/') => Ok(p.clone()),
                        _ => resolver.resolve(Some(&note.path), &target),
                    };
                    match found {
                        Ok(path) if crate::vault::is_note(&path) => {
                            if path != note.path && !out.contains(&path) {
                                out.push(path);
                            }
                        }
                        Ok(_) => {}
                        Err(_) => {
                            let froms = g.unresolved.entry(target).or_default();
                            if !froms.contains(&note.path) {
                                froms.push(note.path.clone());
                            }
                        }
                    }
                }
            }
            for to in &out {
                g.ins.entry(to.clone()).or_default().push(note.path.clone());
            }
            g.out.insert(note.path.clone(), out);
        }
        for note in &vault.notes {
            let n = g.ins.get(&note.path).map_or(0, Vec::len);
            if n > 0 {
                *g.counts.entry(note.name().to_lowercase()).or_default() += n;
            }
        }
        g
    }

    fn outs(&self, p: &Path) -> &[PathBuf] {
        self.out.get(p).map_or(&[], Vec::as_slice)
    }

    fn ins_of(&self, p: &Path) -> &[PathBuf] {
        self.ins.get(p).map_or(&[], Vec::as_slice)
    }

    /// Notes with no links in or out.
    pub fn orphans(&self, vault: &Vault) -> Vec<PathBuf> {
        vault
            .notes
            .iter()
            .map(|n| n.path.clone())
            .filter(|p| self.outs(p).is_empty() && self.ins_of(p).is_empty())
            .collect()
    }

    /// Notes that link nowhere.
    pub fn dead_ends(&self, vault: &Vault) -> Vec<PathBuf> {
        vault
            .notes
            .iter()
            .map(|n| n.path.clone())
            .filter(|p| self.outs(p).is_empty())
            .collect()
    }

    /// The tree of links around `from`, `depth` deep: (depth, outgoing?,
    /// note), depth first; a note shows once.
    pub fn local(&self, from: &Path, depth: usize) -> Vec<(usize, bool, PathBuf)> {
        let mut rows = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::from([from.to_path_buf()]);
        self.walk(from, 0, depth, true, &mut seen, &mut rows);
        for p in self.ins_of(from) {
            if seen.insert(p.clone()) {
                rows.push((0, false, p.clone()));
                self.walk_in(p, 1, depth, &mut seen, &mut rows);
            }
        }
        rows
    }

    fn walk(
        &self,
        from: &Path,
        level: usize,
        depth: usize,
        out: bool,
        seen: &mut HashSet<PathBuf>,
        rows: &mut Vec<(usize, bool, PathBuf)>,
    ) {
        if level >= depth {
            return;
        }
        for p in self.outs(from) {
            if seen.insert(p.clone()) {
                rows.push((level, out, p.clone()));
                self.walk(p, level + 1, depth, out, seen, rows);
            }
        }
    }

    fn walk_in(
        &self,
        to: &Path,
        level: usize,
        depth: usize,
        seen: &mut HashSet<PathBuf>,
        rows: &mut Vec<(usize, bool, PathBuf)>,
    ) {
        if level >= depth {
            return;
        }
        for p in self.ins_of(to) {
            if seen.insert(p.clone()) {
                rows.push((level, false, p.clone()));
                self.walk_in(p, level + 1, depth, seen, rows);
            }
        }
    }

    /// The badge beside a link to `target`: how many notes link to it.
    pub fn count(&self, target: &str) -> Option<usize> {
        let name = target.split('#').next()?.trim();
        let name = name
            .rsplit('/')
            .next()?
            .trim_end_matches(".md")
            .to_lowercase();
        self.counts.get(&name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn links_in_and_out() {
        let dir = scratch("zk-graph");
        write(
            &dir,
            &[
                ("Hub.md", "[[A]] [[B]] [[Missing]]\n```\n[[Code]]\n```"),
                ("A.md", "[[Hub]] [[Deep]]"),
                ("B.md", ""),
                ("Deep.md", ""),
                ("Lonely.md", ""),
            ],
        );
        let vault = Vault::open(&dir).unwrap();
        let g = Graph::of(&vault);
        let p = |n: &str| vault.root.join(format!("{n}.md"));
        assert_eq!(g.orphans(&vault), [p("Lonely")]);
        assert_eq!(
            g.unresolved.keys().collect::<Vec<_>>(),
            ["Missing"],
            "not in code"
        );
        assert_eq!(g.count("Hub"), Some(1));
        assert_eq!(g.count("Hub#Part"), Some(1));
        assert_eq!(g.count("Lonely"), None);
        let local: Vec<(usize, bool, String)> = g
            .local(&p("Hub"), 2)
            .into_iter()
            .map(|(d, o, p)| (d, o, p.file_stem().unwrap().to_string_lossy().into_owned()))
            .collect();
        assert_eq!(
            local,
            [
                (0, true, "A".into()),
                (1, true, "Deep".into()),
                (0, true, "B".into()),
            ],
            "A links back to Hub: shown once, going out"
        );
    }
}
