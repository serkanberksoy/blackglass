//! Dataview's index: one page per note, with its fields (frontmatter and
//! inline `key:: value`), tags, outgoing links, list items and tasks (each
//! with its parent, section, tags, links, block id and own fields, emoji
//! dates included), and the file data behind `file.*` (name, folder,
//! size, dates, the day in a daily note's name, aliases, whether it's
//! bookmarked).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDateTime};

use super::value::{Value, link_name, parse_date};
use crate::vault::{Note, Vault, tags};

/// A list item (`- text`) or task (`- [ ] text`) in a page.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Task {
    /// Line index in the note.
    pub line: usize,
    pub text: String,
    /// The character between the brackets (`' '`, `x`, `/` …; `' '` for a
    /// plain list item).
    pub status: char,
    /// A task (with a checkbox), not a plain list item.
    pub is_task: bool,
    /// The item it's under (its line), if it's nested.
    pub parent: Option<usize>,
    /// The heading it's under.
    pub section: Option<String>,
    /// Its tags (with `#`) and links (note names, lowercase).
    pub tags: Vec<String>,
    pub outlinks: Vec<String>,
    /// Its `^id`.
    pub block_id: Option<String>,
    /// Its own fields: `[key:: value]` and emoji dates (`📅 2026-08-09`:
    /// `due`; ✅ `completion`, ➕ `created`, 🛫 `start`, ⏳ `scheduled`).
    pub fields: Vec<(String, Value)>,
    /// It has inline fields (`[key:: value]`), not only emoji dates.
    pub annotated: bool,
}

impl Task {
    pub fn completed(&self) -> bool {
        self.is_task && matches!(self.status, 'x' | 'X')
    }
}

/// The emoji dates tasks carry, and the fields they make.
const EMOJI_DATES: [(&str, &str); 6] = [
    ("✅", "completion"),
    ("📅", "due"),
    ("➕", "created"),
    ("🛫", "start"),
    ("⏳", "scheduled"),
    ("⌛", "scheduled"),
];

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
    /// Its tasks, and every list item (tasks too).
    pub tasks: Vec<Task>,
    pub lists: Vec<Task>,
    /// Its frontmatter fields alone.
    pub frontmatter: Vec<(String, Value)>,
    /// Its other names (`aliases`).
    pub aliases: Vec<String>,
    /// Bookmarked (the Bookmarks plugin).
    pub starred: bool,
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
            lists: Vec::new(),
            frontmatter: Vec::new(),
            aliases: Vec::new(),
            starred: false,
            size: note.lines.iter().map(|l| l.len() as u64 + 1).sum(),
            mtime: note.modified.map(local),
            ctime: meta
                .and_then(|m| m.created().ok())
                .map(local)
                .or(note.modified.map(local)),
            day: day_in(&name),
            name,
        };
        scan(&note.lines, &mut page);
        page.aliases = match page.field("aliases") {
            Some(Value::List(items)) => items.iter().map(Value::display).collect(),
            Some(Value::Null) | None => Vec::new(),
            Some(one) => vec![one.display()],
        };
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
        let starred = bookmarked(vault);
        let mut pages = crate::vault::par_map(&vault.notes, Page::from_note);
        for page in &mut pages {
            page.starred = starred.contains(&page.rel);
        }
        Index { pages }
    }

    /// The note at `path` changed (or went, or came): its page follows.
    pub fn update(&mut self, vault: &Vault, path: &Path) {
        let at = self.pages.iter().position(|p| p.path == path);
        match (vault.note(path), at) {
            (Some(note), Some(i)) => {
                let mut page = Page::from_note(note);
                page.starred = bookmarked(vault).contains(&page.rel);
                self.pages[i] = page;
            }
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

/// The paths (in the vault) the Bookmarks plugin has bookmarked.
fn bookmarked(vault: &Vault) -> HashSet<String> {
    let file = vault
        .root
        .join(".blackglass/plugins/bookmarks/bookmarks.json");
    let Ok(text) = std::fs::read_to_string(file) else {
        return HashSet::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return HashSet::new();
    };
    json["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|i| i["type"] == "file")
        .filter_map(|i| i["path"].as_str().map(String::from))
        .collect()
}

/// The date in a note's name: `2026-08-09` anywhere in it, or
/// `20260809`.
fn day_in(name: &str) -> Option<NaiveDateTime> {
    let chars: Vec<(usize, char)> = name.char_indices().collect();
    for (k, &(at, c)) in chars.iter().enumerate() {
        if !c.is_ascii_digit() || (k > 0 && chars[k - 1].1.is_ascii_digit()) {
            continue;
        }
        let rest = &name[at..];
        if let Some(d) = rest.get(..10).and_then(parse_date) {
            return Some(d);
        }
        if let Some(digits) = rest.get(..8)
            && digits.chars().all(|c| c.is_ascii_digit())
            && !rest[8..].starts_with(|c: char| c.is_ascii_digit())
            && let Ok(d) = chrono::NaiveDate::parse_from_str(digits, "%Y%m%d")
        {
            return Some(d.and_time(chrono::NaiveTime::MIN));
        }
    }
    None
}

/// Reads frontmatter fields, inline fields, links, list items and tasks
/// from `lines`.
fn scan(lines: &[String], page: &mut Page) {
    let mut body = 0;
    if lines.first().map(|l| l.trim_end()) == Some("---")
        && let Some(end) = (1..lines.len()).find(|&i| matches!(lines[i].trim_end(), "---" | "..."))
    {
        frontmatter(&lines[1..end], &mut page.fields);
        page.frontmatter = page.fields.clone();
        body = end + 1;
    }
    let mut fence: Option<&str> = None;
    // The links seen (a note with thousands of links stays linear).
    let mut seen: HashSet<String> = page.outlinks.iter().cloned().collect();
    let mut section: Option<String> = None;
    // The open list items: (indent, line).
    let mut open: Vec<(usize, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(body) {
        let trimmed = line.trim_start();
        if let Some(f) = fence {
            if trimmed.starts_with(f) {
                fence = None;
            }
            continue;
        }
        if let Some(f) = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f)) {
            fence = Some(f);
            continue;
        }
        links(line, &mut page.outlinks, &mut seen, false);
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            section = Some(trimmed[hashes..].trim().to_string());
            open.clear();
        }
        let indent: usize = line[..line.len() - trimmed.len()]
            .chars()
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum();
        let item = strip_list_marker(trimmed).map(|rest| (task(trimmed), rest));
        let text = match item {
            Some((checkbox, rest)) => {
                while open.last().is_some_and(|(at, _)| *at >= indent) {
                    open.pop();
                }
                let (status, text) = match checkbox {
                    Some((c, t)) => (Some(c), t),
                    None => (None, rest.trim()),
                };
                let entry = list_item(i, text, status, open.last().map(|(_, l)| *l), &section);
                open.push((indent, i));
                if entry.is_task {
                    page.tasks.push(entry.clone());
                }
                page.lists.push(entry);
                text.to_string()
            }
            None => {
                // Text that isn't indented under an item ends the list.
                if !trimmed.is_empty() && indent == 0 {
                    open.clear();
                }
                trimmed.to_string()
            }
        };
        inline_fields(&text, &mut page.fields);
    }
}

/// A list item at line `line` (`status`: its checkbox's, if it's a task).
fn list_item(
    line: usize,
    text: &str,
    status: Option<char>,
    parent: Option<usize>,
    section: &Option<String>,
) -> Task {
    let mut fields = Vec::new();
    inline_fields(text, &mut fields);
    let annotated = !fields.is_empty();
    for (emoji, key) in EMOJI_DATES {
        if let Some(at) = text.find(emoji) {
            let after = text[at + emoji.len()..].trim_start();
            if let Some(d) = after.get(..10).and_then(parse_date) {
                fields.push((key.to_string(), Value::Date(d)));
            }
        }
    }
    let line_text = vec![text.to_string()];
    let mut outlinks = Vec::new();
    links(text, &mut outlinks, &mut HashSet::new(), true);
    let trimmed = text.trim_end();
    let block_id = trimmed
        .rsplit_once(" ^")
        .map(|(_, id)| id)
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .map(String::from);
    Task {
        line,
        text: text.to_string(),
        status: status.unwrap_or(' '),
        is_task: status.is_some(),
        parent,
        section: section.clone(),
        tags: tags::extract(&line_text)
            .into_iter()
            .map(|t| format!("#{t}"))
            .collect(),
        outlinks,
        block_id,
        fields,
        annotated,
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
            // The matching bracket: a link inside (`[[Note]]`) nests.
            let mut depth = 0usize;
            let end = after.char_indices().find_map(|(i, c)| {
                if c == open {
                    depth += 1;
                } else if c == close {
                    if depth == 0 {
                        return Some(i);
                    }
                    depth -= 1;
                }
                None
            });
            let Some(end) = end else { break };
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

/// Link targets in a line: `[[Note]]`, `![[Note]]`, `[text](note.md)`, by
/// name, lowercase (for matching) unless `as_written`.
fn links(line: &str, out: &mut Vec<String>, seen: &mut HashSet<String>, as_written: bool) {
    let mut add = |target: &str| {
        let name = link_name(&target.replace("%20", " "));
        let name = name.rsplit('/').next().unwrap_or(&name);
        let lower = name.to_lowercase();
        if !lower.is_empty() && seen.insert(lower.clone()) {
            out.push(if as_written { name.to_string() } else { lower });
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
        index.update(
            &vault,
            &mdedit::platform::canonical(&dir).unwrap().join("C.md"),
        );
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
             mood:: good\n- rating:: 4\nread [author:: Frank] and (pages:: 412)\n\
             - [spent:: [[Groceries]]] [amount:: 62.4] (shop:: [[Market|the market]])\n```\nx:: no\n```",
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
        // A link inside a field's brackets: the field runs to the
        // matching bracket.
        assert_eq!(p.field("spent"), Some(&Value::Link("Groceries".into())));
        assert_eq!(p.field("amount"), Some(&Value::Number(62.4)));
        assert_eq!(p.field("shop"), Some(&Value::Link("Market".into())));
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
        // A list item's links read as written (shown in results).
        assert_eq!(p.lists[0].outlinks, ["Dune"]);
        assert!(p.mtime.is_some());
    }
}
