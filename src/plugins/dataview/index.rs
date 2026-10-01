//! Dataview's index: one page per note, with its fields (frontmatter and
//! inline `key:: value`), tags, outgoing links and tasks, and the file
//! data behind `file.*` (name, folder, size, dates, and the day in a daily
//! note's name).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDateTime};

use super::value::{Value, link_name, parse_date};
use crate::vault::{Note, Vault, tags};

/// A task (`- [ ] text`) in a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Line index in the note.
    pub line: usize,
    pub text: String,
    /// The character between the brackets (`' '`, `x`, `/` …).
    pub status: char,
}

impl Task {
    pub fn completed(&self) -> bool {
        matches!(self.status, 'x' | 'X')
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// Absolute path.
    pub path: PathBuf,
    /// Path in the vault, with `.md`, `/` separators.
    pub rel: String,
    pub name: String,
    /// The folder in the vault (`""` at the top).
    pub folder: String,
    /// Frontmatter fields, then inline fields, as written.
    pub fields: Vec<(String, Value)>,
    /// Tags with `#`.
    pub tags: Vec<String>,
    /// Link targets, as note names in lowercase (`[[Books/Dune]]` → `dune`).
    pub outlinks: Vec<String>,
    pub tasks: Vec<Task>,
    pub size: u64,
    pub mtime: Option<NaiveDateTime>,
    pub ctime: Option<NaiveDateTime>,
    /// The date in the note's name (`2026-08-09`), or its `date` field.
    pub day: Option<NaiveDateTime>,
}

impl Page {
    pub fn from_note(note: &Note) -> Page {
        let rel = note.rel.to_string_lossy().replace('\\', "/");
        let folder = rel.rsplit_once('/').map_or("", |(f, _)| f).to_string();
        let name = note.name();
        let meta = std::fs::metadata(&note.path).ok();
        let local = |t: std::time::SystemTime| DateTime::<Local>::from(t).naive_local();
        let mut page = Page {
            path: note.path.clone(),
            rel,
            folder,
            fields: Vec::new(),
            tags: tags::extract(&note.lines)
                .into_iter()
                .map(|t| format!("#{t}"))
                .collect(),
            outlinks: Vec::new(),
            tasks: Vec::new(),
            size: note.lines.iter().map(|l| l.len() as u64 + 1).sum(),
            mtime: note.modified.map(local),
            ctime: meta
                .and_then(|m| m.created().ok())
                .map(local)
                .or(note.modified.map(local)),
            day: parse_date(name.get(..10).unwrap_or("")),
            name,
        };
        scan(&note.lines, &mut page);
        if page.day.is_none()
            && let Some(Value::Date(d)) = page.field("date")
        {
            page.day = Some(*d);
        }
        page
    }

    /// A field by name: as written, else ignoring case and with spaces as
    /// dashes (Dataview's "sanitized" key: `Due Date` → `due-date`).
    pub fn field(&self, name: &str) -> Option<&Value> {
        let simple = |k: &str| k.trim().to_lowercase().replace(' ', "-");
        self.fields
            .iter()
            .find(|(k, _)| k == name)
            .or_else(|| self.fields.iter().find(|(k, _)| simple(k) == simple(name)))
            .map(|(_, v)| v)
    }

    /// Whether this page links to the note named `name` (lowercase).
    pub fn links_to(&self, name: &str) -> bool {
        self.outlinks.iter().any(|l| l == name)
    }
}

/// The pages of every note in the vault.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Index {
    pub pages: Vec<Page>,
}

impl Index {
    pub fn build(vault: &Vault) -> Index {
        Index {
            pages: crate::vault::par_map(&vault.notes, Page::from_note),
        }
    }

    /// The note at `path` changed (or went, or came): its page follows.
    pub fn update(&mut self, vault: &Vault, path: &Path) {
        let at = self.pages.iter().position(|p| p.path == path);
        match (vault.note(path), at) {
            (Some(note), Some(i)) => self.pages[i] = Page::from_note(note),
            (None, Some(i)) => {
                self.pages.remove(i);
            }
            // A new note: in the vault's order.
            (Some(_), None) => *self = Index::build(vault),
            (None, None) => {}
        }
    }

    pub fn page(&self, path: &Path) -> Option<&Page> {
        self.pages.iter().find(|p| p.path == path)
    }

    /// The pages linking to `page`.
    pub fn inlinks<'a>(&'a self, page: &'a Page) -> impl Iterator<Item = &'a Page> {
        let name = page.name.to_lowercase();
        self.pages.iter().filter(move |p| p.links_to(&name))
    }
}

/// Reads frontmatter fields, inline fields, links and tasks from `lines`.
fn scan(lines: &[String], page: &mut Page) {
    let mut body = 0;
    if lines.first().map(|l| l.trim_end()) == Some("---")
        && let Some(end) = (1..lines.len()).find(|&i| matches!(lines[i].trim_end(), "---" | "..."))
    {
        frontmatter(&lines[1..end], &mut page.fields);
        body = end + 1;
    }
    let mut fence: Option<&str> = None;
    // The links seen (a note with thousands of links stays linear).
    let mut seen: HashSet<String> = page.outlinks.iter().cloned().collect();
    for (i, line) in lines.iter().enumerate().skip(body) {
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
        links(line, &mut page.outlinks, &mut seen);
        let text = if let Some(task) = task(trimmed) {
            let text = task.1.to_string();
            page.tasks.push(Task {
                line: i,
                text: text.clone(),
                status: task.0,
            });
            text
        } else {
            strip_marker(trimmed).to_string()
        };
        inline_fields(&text, &mut page.fields);
    }
}

/// `key: value` lines, with `- item` lists under an empty value.
fn frontmatter(lines: &[String], fields: &mut Vec<(String, Value)>) {
    let mut list: Option<(String, Vec<Value>)> = None;
    for line in lines {
        if let Some((_, items)) = &mut list {
            if let Some(item) = line.trim_start().strip_prefix("- ") {
                items.push(Value::scalar(item));
                continue;
            }
            let (key, items) = list.take().expect("a list is open");
            fields.push((key, Value::List(items)));
        }
        if line.starts_with([' ', '\t']) {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_string();
        if value.trim().is_empty() {
            list = Some((key, Vec::new()));
        } else {
            fields.push((key, Value::parse(value)));
        }
    }
    if let Some((key, items)) = list {
        let value = if items.is_empty() {
            Value::Null
        } else {
            Value::List(items)
        };
        fields.push((key, value));
    }
}

/// `key:: value` filling a line, or `[key:: value]` / `(key:: value)`
/// inside it.
fn inline_fields(text: &str, fields: &mut Vec<(String, Value)>) {
    let is_key = |k: &str| {
        !k.is_empty()
            && k.chars().next().is_some_and(char::is_alphanumeric)
            && k.chars().all(|c| c.is_alphanumeric() || " _-/".contains(c))
    };
    let mut found = false;
    for (open, close) in [('[', ']'), ('(', ')')] {
        let mut rest = text;
        while let Some(start) = rest.find(open) {
            let after = &rest[start + 1..];
            let Some(end) = after.find(close) else { break };
            if let Some((k, v)) = after[..end].split_once("::")
                && is_key(k.trim())
            {
                fields.push((k.trim().to_string(), Value::parse(v)));
                found = true;
            }
            rest = &after[end + 1..];
        }
    }
    if !found
        && let Some((k, v)) = text.split_once("::")
        && is_key(k.trim())
    {
        fields.push((k.trim().to_string(), Value::parse(v)));
    }
}

/// `- [c] text` (any list marker): the state character and the text.
fn task(line: &str) -> Option<(char, &str)> {
    let rest = strip_list_marker(line)?;
    let mut chars = rest.chars();
    if chars.next() != Some('[') {
        return None;
    }
    let status = chars.next()?;
    let rest = chars.as_str().strip_prefix("] ")?;
    Some((status, rest.trim()))
}

/// The text after a list marker (`-`, `*`, `+`, `1.`), if there is one.
fn strip_list_marker(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix(['-', '*', '+']) {
        return rest.strip_prefix(' ');
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    (digits > 0)
        .then(|| &line[digits..])
        .and_then(|r| r.strip_prefix(['.', ')']))
        .and_then(|r| r.strip_prefix(' '))
}

fn strip_marker(line: &str) -> &str {
    strip_list_marker(line).unwrap_or(line)
}

/// Link targets in a line: `[[Note]]`, `![[Note]]`, `[text](note.md)`.
fn links(line: &str, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    let mut add = |target: &str| {
        let name = link_name(&target.replace("%20", " "));
        let name = name.rsplit('/').next().unwrap_or(&name).to_lowercase();
        if !name.is_empty() && seen.insert(name.clone()) {
            out.push(name);
        }
    };
    let mut rest = line;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        add(&after[..end]);
        rest = &after[end + 2..];
    }
    let mut rest = line;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else { break };
        let target = &after[..end];
        if !target.contains("://") && target.ends_with(".md") {
            add(target);
        }
        rest = &after[end + 1..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn one_changed_note_updates_its_page() {
        let dir = scratch("dv-update");
        write(&dir, &[("A.md", "rating:: 1"), ("B.md", "x")]);
        let mut vault = Vault::open(&dir).unwrap();
        let mut index = Index::build(&vault);
        let a = vault.root.join("A.md");
        vault.update_note(&a, "rating:: 5\n[[B]]");
        index.update(&vault, &a);
        let page = index.page(&a).unwrap();
        assert_eq!(page.field("rating"), Some(&Value::Number(5.0)));
        assert_eq!(page.outlinks, ["b"]);
        assert_eq!(index.pages.len(), 2);
        // A note that's gone leaves; a new one comes in its place.
        std::fs::remove_file(&a).unwrap();
        vault.rescan().unwrap();
        index.update(&vault, &a);
        assert_eq!(index.pages.len(), 1);
        write(&dir, &[("C.md", "")]);
        vault.rescan().unwrap();
        index.update(&vault, &dir.canonicalize().unwrap().join("C.md"));
        assert_eq!(index.pages.len(), 2);
    }

    fn page(text: &str) -> Page {
        let dir = scratch(&format!("dv-page-{}", text.len()));
        write(&dir, &[("Journal/2026-08-09.md", text)]);
        let vault = Vault::open(&dir).unwrap();
        Page::from_note(&vault.notes[0])
    }

    #[test]
    fn frontmatter_and_inline_fields() {
        let p = page(
            "---\nwaistline: 138\ncategory: Journal\ntags:\n  - daily\n  - log\n---\n\
             mood:: good\n- rating:: 4\nread [author:: Frank] and (pages:: 412)\n```\nx:: no\n```",
        );
        assert_eq!(p.field("waistline"), Some(&Value::Number(138.0)));
        assert_eq!(
            p.field("Category"),
            Some(&Value::Text("Journal".into())),
            "any case"
        );
        assert_eq!(
            p.field("tags"),
            Some(&Value::List(vec![
                Value::Text("daily".into()),
                Value::Text("log".into())
            ]))
        );
        assert_eq!(p.field("mood"), Some(&Value::Text("good".into())));
        assert_eq!(p.field("rating"), Some(&Value::Number(4.0)));
        assert_eq!(p.field("author"), Some(&Value::Text("Frank".into())));
        assert_eq!(p.field("pages"), Some(&Value::Number(412.0)));
        assert_eq!(p.field("x"), None, "not in code");
        assert_eq!(p.tags, ["#daily", "#log"]);
    }

    #[test]
    fn file_data_tasks_and_links() {
        let p = page(
            "- [ ] one [[Books/Dune|book]]\n- [x] two\n1. [/] three\n![[Sunset]] [a](Other%20note.md) [w](https://x.md)",
        );
        assert_eq!(
            (p.name.as_str(), p.folder.as_str()),
            ("2026-08-09", "Journal")
        );
        assert_eq!(p.rel, "Journal/2026-08-09.md");
        assert_eq!(
            p.day.map(|d| d.to_string()),
            Some("2026-08-09 00:00:00".into())
        );
        let tasks: Vec<_> = p
            .tasks
            .iter()
            .map(|t| (t.status, t.text.as_str(), t.completed()))
            .collect();
        assert_eq!(
            tasks,
            [
                (' ', "one [[Books/Dune|book]]", false),
                ('x', "two", true),
                ('/', "three", false)
            ]
        );
        assert_eq!(p.outlinks, ["dune", "sunset", "other note"]);
        assert!(p.mtime.is_some());
    }
}
