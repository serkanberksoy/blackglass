//! A note as the archiver sees it: its frontmatter, then sections (a
//! heading and what's under it, sections nested by level), each with
//! blocks (lines; a list item has the lines indented under it as its
//! children). Lines keep their own indentation, so a note that isn't
//! changed prints back as it was; a block that moves is re-indented
//! ([`Block::rebase`]).

/// A line and the lines under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The line as written (with its indentation).
    pub line: String,
    /// A list item (`- `, `* `, `+ `, `1. `), outside code blocks.
    pub list: bool,
    pub children: Vec<Block>,
}

/// A heading and what's under it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    /// 1 … 6; 0 for the note's start (before any heading).
    pub level: usize,
    /// The heading's text (trimmed); `""` for the note's start.
    pub title: String,
    /// The heading's line as written.
    pub line: String,
    pub blocks: Vec<Block>,
    pub children: Vec<Section>,
}

/// A note: frontmatter lines (with their `---`), then its sections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Doc {
    pub front: Vec<String>,
    pub root: Section,
}

/// The leading whitespace of `line`.
pub fn indent(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// How far `line` is indented (a tab counts four).
fn width(line: &str) -> usize {
    indent(line)
        .chars()
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Whether `line` is a list item: `- `, `* `, `+ ` or `1. ` / `1) `
/// after its indentation.
pub fn is_list(line: &str) -> bool {
    let t = line.trim_start_matches([' ', '\t']);
    if let Some(rest) = t.strip_prefix(['-', '*', '+']) {
        return rest.starts_with([' ', '\t']);
    }
    let digits = t.len() - t.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    digits > 0 && t[digits..].starts_with(['.', ')']) && t[digits + 1..].starts_with([' ', '\t'])
}

/// A heading line's level and text.
pub fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    let rest = &line[hashes..];
    ((1..=6).contains(&hashes) && (rest.is_empty() || rest.starts_with([' ', '\t'])))
        .then(|| (hashes, rest.trim()))
}

/// Whether `line` opens or closes a code block.
fn fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// For each line, whether it's a heading (not in a code block).
pub fn headings(lines: &[String]) -> Vec<Option<(usize, String)>> {
    let mut inside = false;
    lines
        .iter()
        .map(|l| {
            if fence(l) {
                inside = !inside;
                return None;
            }
            if inside {
                None
            } else {
                heading(l).map(|(level, t)| (level, t.to_string()))
            }
        })
        .collect()
}

impl Block {
    pub fn text(line: &str) -> Block {
        Block {
            line: line.to_string(),
            list: false,
            children: Vec::new(),
        }
    }

    /// A blank line.
    pub fn is_blank(&self) -> bool {
        !self.list && self.line.trim().is_empty()
    }

    /// Its lines, children after it.
    pub fn lines(&self, out: &mut Vec<String>) {
        out.push(self.line.clone());
        for c in &self.children {
            c.lines(out);
        }
    }

    /// Every line of it, through `f`.
    pub fn map_lines(&mut self, f: &mut impl FnMut(&str) -> String) {
        self.line = f(&self.line);
        for c in &mut self.children {
            c.map_lines(f);
        }
    }

    /// Moves it to indentation `to`: each of its lines loses its own
    /// indentation's start and gets `to` instead.
    pub fn rebase(&mut self, to: &str) {
        let from = indent(&self.line).to_string();
        self.map_lines(&mut |line| match line.strip_prefix(from.as_str()) {
            Some(rest) => format!("{to}{rest}"),
            None => format!("{to}{}", line.trim_start()),
        });
    }

    /// The first block (this one or under it) that `found` takes, as the
    /// indices down to it.
    pub fn find(blocks: &[Block], found: &impl Fn(&Block) -> bool) -> Option<Vec<usize>> {
        for (i, b) in blocks.iter().enumerate() {
            if found(b) {
                return Some(vec![i]);
            }
            if let Some(mut path) = Block::find(&b.children, found) {
                path.insert(0, i);
                return Some(path);
            }
        }
        None
    }
}

/// The blocks in `lines` (a section's body): list items take the lines
/// indented more than them; other lines that aren't indented end lists.
pub fn blocks(lines: &[String]) -> Vec<Block> {
    // Each line's parent (an earlier line), then the tree from that.
    let mut parents: Vec<Option<usize>> = Vec::with_capacity(lines.len());
    let mut list = Vec::with_capacity(lines.len());
    let mut open: Vec<(usize, usize)> = Vec::new();
    let mut inside = false;
    for (i, line) in lines.iter().enumerate() {
        let is_fence = fence(line);
        let item = !inside && !is_fence && is_list(line);
        if is_fence {
            inside = !inside;
        }
        let w = width(line);
        if !item && (w == 0 || line.trim().is_empty()) {
            open.clear();
            parents.push(None);
        } else {
            while open.last().is_some_and(|&(ow, _)| ow >= w) {
                open.pop();
            }
            parents.push(open.last().map(|&(_, p)| p));
            if item {
                open.push((w, i));
            }
        }
        list.push(item);
    }
    fn build(i: usize, lines: &[String], list: &[bool], kids: &[Vec<usize>]) -> Block {
        Block {
            line: lines[i].clone(),
            list: list[i],
            children: kids[i]
                .iter()
                .map(|&k| build(k, lines, list, kids))
                .collect(),
        }
    }
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); lines.len()];
    let mut tops = Vec::new();
    for (i, p) in parents.iter().enumerate() {
        match p {
            Some(p) => kids[*p].push(i),
            None => tops.push(i),
        }
    }
    tops.into_iter()
        .map(|i| build(i, lines, &list, &kids))
        .collect()
}

impl Section {
    pub fn new(level: usize, title: &str) -> Section {
        Section {
            level,
            title: title.to_string(),
            line: format!("{} {title}", "#".repeat(level)),
            ..Section::default()
        }
    }

    /// Its lines: the heading, its blocks, the sections in it.
    pub fn lines(&self, out: &mut Vec<String>) {
        if self.level > 0 {
            out.push(self.line.clone());
        }
        for b in &self.blocks {
            b.lines(out);
        }
        for s in &self.children {
            s.lines(out);
        }
    }

    /// Gives it level `level`, and the sections in it the levels below.
    pub fn relevel(&mut self, level: usize) {
        self.level = level.min(6);
        self.line = format!("{} {}", "#".repeat(self.level), self.title);
        for c in &mut self.children {
            c.relevel(level + 1);
        }
    }

    /// The first section (this one or in it) titled `title`, as the
    /// indices down to it (`[]`: this one).
    pub fn find(&self, title: &str) -> Option<Vec<usize>> {
        if self.title == title {
            return Some(Vec::new());
        }
        self.children.iter().enumerate().find_map(|(i, c)| {
            c.find(title).map(|mut path| {
                path.insert(0, i);
                path
            })
        })
    }

    pub fn at_mut(&mut self, path: &[usize]) -> &mut Section {
        path.iter().fold(self, |s, &i| &mut s.children[i])
    }
}

impl Doc {
    pub fn parse(lines: &[String]) -> Doc {
        let front_end = (lines.first().is_some_and(|l| l.trim_end() == "---"))
            .then(|| lines.iter().skip(1).position(|l| l.trim_end() == "---"))
            .flatten()
            .map_or(0, |i| i + 2);
        let body = &lines[front_end..];
        // Flat sections (level, title, line, body lines), then nested.
        let mut flat: Vec<(usize, String, String, Vec<String>)> =
            vec![(0, String::new(), String::new(), Vec::new())];
        for (line, h) in body.iter().zip(headings(body)) {
            match h {
                Some((level, title)) => flat.push((level, title, line.clone(), Vec::new())),
                None => flat.last_mut().expect("the start").3.push(line.clone()),
            }
        }
        let mut sections: Vec<Section> = flat
            .into_iter()
            .map(|(level, title, line, body)| Section {
                level,
                title,
                line,
                blocks: blocks(&body),
                children: Vec::new(),
            })
            .collect();
        // Each section goes in the nearest one before it of a lower level.
        let rest = sections.split_off(1);
        let mut root = sections.pop().expect("the start");
        let mut stack: Vec<Section> = Vec::new();
        let close = |stack: &mut Vec<Section>, root: &mut Section| {
            let s = stack.pop().expect("one open");
            match stack.last_mut() {
                Some(parent) => parent.children.push(s),
                None => root.children.push(s),
            }
        };
        for s in rest {
            while stack.last().is_some_and(|top| top.level >= s.level) {
                close(&mut stack, &mut root);
            }
            stack.push(s);
        }
        while !stack.is_empty() {
            close(&mut stack, &mut root);
        }
        Doc {
            front: lines[..front_end].to_vec(),
            root,
        }
    }

    pub fn lines(&self) -> Vec<String> {
        let mut out = self.front.clone();
        self.root.lines(&mut out);
        out
    }
}

/// The note's indentation step: the smallest indentation of a list item
/// (a tab if it's tabs); `fallback` if no list item is indented.
pub fn step(lines: &[String], fallback: &str) -> String {
    lines
        .iter()
        .filter(|l| is_list(l))
        .map(|l| indent(l))
        .filter(|i| !i.is_empty())
        .min_by_key(|i| width(i))
        .map_or_else(
            || fallback.to_string(),
            |i| {
                if i.starts_with('\t') {
                    "\t".into()
                } else {
                    i.to_string()
                }
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn a_note_prints_back_as_it_was() {
        let text = "---\ntags: [a]\n---\nintro\n- [ ] a\n    - detail\n  more\n\n# One\n\
                    text\n```\n# not a heading\n- not an item\n```\n## Two\n1. x\n\t- y\n# Three";
        let doc = Doc::parse(&lines(text));
        assert_eq!(doc.lines(), lines(text));
        assert_eq!(doc.front.len(), 3);
        let one = &doc.root.children[0];
        assert_eq!(
            (one.title.as_str(), one.children[0].title.as_str()),
            ("One", "Two")
        );
        assert_eq!(doc.root.children[1].title, "Three");
        let a = &doc.root.blocks[1];
        assert_eq!(a.children.len(), 2, "its detail and its more: {a:?}");
        assert!(!one.blocks.iter().any(|b| b.list), "not in the code block");
        assert_eq!(one.children[0].blocks[0].children[0].line, "\t- y");
    }

    #[test]
    fn blocks_move_with_their_lines() {
        let mut b = blocks(&lines("\t- [x] a\n\t\t- b\n\t\t  c"))[0].clone();
        b.rebase("    ");
        let mut out = Vec::new();
        b.lines(&mut out);
        assert_eq!(out, ["    - [x] a", "    \t- b", "    \t  c"]);
        assert_eq!(step(&lines("- a\n  - b\n    - c"), "\t"), "  ");
        assert_eq!(step(&lines("- a"), "\t"), "\t");
        assert!(is_list("1) x") && is_list("  * y") && !is_list("-x") && !is_list("text"));
    }
}
