//! Finds a note's tags: `tags:` in the frontmatter (a `[list]`, a comma
//! list or `- item` lines) and `#tags` in the text. A tag is what mdedit
//! styles as one (P-05): `#` after a space or the line start, then letters,
//! digits, `_`, `-` or `/`, and not only digits. Code is skipped.

/// The tags in `lines`, without `#`, each once (case-insensitive), in order
/// of appearance.
pub fn extract(lines: &[String]) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let mut add = |tag: &str| {
        let tag = tag.trim().trim_matches(['"', '\'']).trim_start_matches('#');
        if is_tag_name(tag) && !tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
            tags.push(tag.to_string());
        }
    };
    let body = match frontmatter_end(lines) {
        Some(end) => {
            frontmatter_tags(&lines[1..end], &mut add);
            end + 1
        }
        None => 0,
    };
    let mut fence: Option<&str> = None;
    for line in &lines[body.min(lines.len())..] {
        let trimmed = line.trim_start();
        if let Some(open) = fence {
            if trimmed.starts_with(open) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f)) {
            fence = Some(open);
            continue;
        }
        inline_tags(line, &mut add);
    }
    tags
}

/// Whether `name` (without `#`) is a valid tag.
pub fn is_tag_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_tag_char) && !name.chars().all(|c| c.is_ascii_digit())
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || "_-/".contains(c)
}

/// The index of the frontmatter's closing `---` (or `...`), if the note
/// starts with frontmatter.
fn frontmatter_end(lines: &[String]) -> Option<usize> {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return None;
    }
    (1..lines.len()).find(|&i| matches!(lines[i].trim_end(), "---" | "..."))
}

/// `tags:` (or `tag:`) in frontmatter lines.
fn frontmatter_tags(lines: &[String], add: &mut impl FnMut(&str)) {
    let mut in_list = false;
    for line in lines {
        if in_list {
            if let Some(item) = line.trim_start().strip_prefix("- ") {
                add(item);
                continue;
            }
            in_list = false;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !matches!(key.trim(), "tags" | "tag") || line.starts_with([' ', '\t']) {
            continue;
        }
        let value = value.trim().trim_start_matches('[').trim_end_matches(']');
        if value.is_empty() {
            in_list = true;
        }
        for tag in value.split([',', ' ']).filter(|t| !t.is_empty()) {
            add(tag);
        }
    }
}

/// `#tags` in one line of text, outside `code spans`.
fn inline_tags(line: &str, add: &mut impl FnMut(&str)) {
    let mut in_code = false;
    let mut prev: Option<char> = None;
    for (i, c) in line.char_indices() {
        if c == '`' {
            in_code = !in_code;
        } else if c == '#' && !in_code && prev.is_none_or(char::is_whitespace) {
            let rest = &line[i + 1..];
            let len = rest.find(|c: char| !is_tag_char(c)).unwrap_or(rest.len());
            add(&rest[..len]);
        }
        prev = Some(c);
    }
}

/// A row of the tags as a tree (`#project/alpha` under `#project`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRow {
    /// The whole tag (`project/alpha`), as first written.
    pub name: String,
    /// Its last part (`alpha`).
    pub label: String,
    pub depth: usize,
    /// The notes with it or a tag nested in it, each once.
    pub notes: usize,
    pub nested: bool,
    pub open: bool,
}

/// The tags of `notes` (each note's tags) as a tree, parents before their
/// nested tags (which only show under an `open` parent, by lowercase
/// name); a parent no note has itself is there too.
pub fn tree<'a>(
    notes: impl IntoIterator<Item = &'a [String]>,
    open: &std::collections::HashSet<String>,
) -> Vec<TagRow> {
    use std::collections::{BTreeMap, BTreeSet};
    // By their parts, lowercased: (spelling, notes).
    let mut all: BTreeMap<Vec<String>, (String, usize)> = BTreeMap::new();
    for tags in notes {
        let mut prefixes: BTreeSet<Vec<String>> = BTreeSet::new();
        for tag in tags {
            let parts: Vec<&str> = tag.split('/').filter(|p| !p.is_empty()).collect();
            for n in 1..=parts.len() {
                let key: Vec<String> = parts[..n].iter().map(|p| p.to_lowercase()).collect();
                all.entry(key.clone())
                    .or_insert_with(|| (parts[..n].join("/"), 0));
                prefixes.insert(key);
            }
        }
        for key in prefixes {
            all.get_mut(&key).expect("added above").1 += 1;
        }
    }
    let keys: Vec<&Vec<String>> = all.keys().collect();
    let mut rows = Vec::new();
    for (i, key) in keys.iter().enumerate() {
        let shown = (1..key.len()).all(|n| open.contains(&key[..n].join("/")));
        if !shown {
            continue;
        }
        let (name, notes) = &all[*key];
        let nested = keys
            .get(i + 1)
            .is_some_and(|next| next.len() > key.len() && next.starts_with(key));
        rows.push(TagRow {
            label: name.rsplit('/').next().unwrap_or(name).to_string(),
            name: name.clone(),
            depth: key.len() - 1,
            notes: *notes,
            nested,
            open: open.contains(&key.join("/")),
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(text: &str) -> Vec<String> {
        extract(&crate::vault::split_lines(text))
    }

    #[test]
    fn inline_tags_follow_mdedits_rules() {
        assert_eq!(
            tags("# Heading\nsome #idea and #work/project, #2026 #y2026\nurl.com/#anchor a#b"),
            ["idea", "work/project", "y2026"]
        );
    }

    #[test]
    fn tags_are_listed_once_ignoring_case() {
        assert_eq!(tags("#Idea #idea\n#IDEA #other"), ["Idea", "other"]);
    }

    #[test]
    fn code_is_not_searched_for_tags() {
        assert_eq!(
            tags("`#not` #yes\n```\n#code\n```\n~~~\n#tilde\n~~~\n#after"),
            ["yes", "after"]
        );
    }

    #[test]
    fn frontmatter_tags_in_every_form() {
        assert_eq!(
            tags("---\ntags: [a, \"#b\"]\n---\n#c"),
            ["a", "b", "c"],
            "flow list"
        );
        assert_eq!(tags("---\ntags: a, b\n---"), ["a", "b"], "comma list");
        assert_eq!(
            tags("---\ntitle: x\ntags:\n  - one\n  - two\nother: y\n---"),
            ["one", "two"],
            "block list"
        );
        assert_eq!(tags("---\ntag: solo\n---"), ["solo"], "tag:");
        assert_eq!(
            tags("---\ntitle: #notatag\n---"),
            Vec::<String>::new(),
            "other keys"
        );
    }

    #[test]
    fn frontmatter_needs_to_start_the_note() {
        assert_eq!(tags("text\n---\ntags: a\n---"), Vec::<String>::new());
        assert_eq!(tags("---\ntags: a"), Vec::<String>::new(), "never closed");
    }
    #[test]
    fn nested_tags_make_a_tree() {
        let notes: Vec<Vec<String>> = vec![
            vec!["project/alpha".into(), "solo".into()],
            vec!["project/beta".into(), "Project".into()],
            vec!["project/alpha/deep".into()],
            vec!["project-x".into()],
        ];
        let rows = |open: &[&str]| -> Vec<(String, usize, usize, bool)> {
            let open = open.iter().map(|s| s.to_string()).collect();
            tree(notes.iter().map(Vec::as_slice), &open)
                .into_iter()
                .map(|r| (r.label, r.depth, r.notes, r.nested))
                .collect()
        };
        let folded = rows(&[]);
        assert_eq!(
            folded,
            [
                ("project".into(), 0, 3, true),
                ("project-x".into(), 0, 1, false),
                ("solo".into(), 0, 1, false),
            ],
            "a parent counts each note once"
        );
        assert_eq!(
            rows(&["project"])[1..3],
            [("alpha".into(), 1, 2, true), ("beta".into(), 1, 1, false)]
        );
        assert_eq!(
            rows(&["project", "project/alpha"])[2],
            ("deep".into(), 2, 1, false)
        );
    }
}
