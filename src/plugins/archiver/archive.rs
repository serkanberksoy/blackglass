//! What the archiver does to a note's lines: finds the done tasks
//! (outside the archive), takes them out, and puts them under the archive
//! heading (in this note or another), in a tree of headings and list
//! items made from placeholders; deletes them; archives the heading under
//! the cursor; sorts a list (items, open tasks, done tasks); checks a task
//! off and archives it.

use std::sync::LazyLock;

use chrono::{NaiveDate, NaiveDateTime};
use regex::Regex;

use super::tree::{self, Block, Doc, Section};
use crate::plugins::moment;

/// Where archived tasks go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum To {
    /// Under the archive heading in the same note.
    Here,
    /// A note named by the file setting (placeholders allowed).
    Custom,
    /// Today's daily note.
    Daily,
}

/// A rule: tasks it takes (by status, note path, text) go to its note,
/// or are deleted.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Status symbols (`>-`); empty: any.
    pub statuses: String,
    pub path: Option<Regex>,
    pub text: Option<Regex>,
    pub delete: bool,
    pub to: To,
    pub file: String,
    pub date_format: String,
}

impl Rule {
    /// A rule that does something: deletes, or has a note to move to.
    fn on(&self) -> bool {
        self.delete || self.to == To::Daily || !self.file.trim().is_empty()
    }
}

/// The settings, with the original's defaults.
#[derive(Debug, Clone)]
pub struct Config {
    pub to: To,
    pub file: String,
    /// In a note of its own: under the headings too (else at its top).
    pub under_heading: bool,
    /// The headings' templates, outermost first.
    pub headings: Vec<String>,
    /// The first heading's level.
    pub depth: usize,
    /// Blank lines around the archived tasks.
    pub newlines: bool,
    pub under_list_items: bool,
    pub list_items: Vec<String>,
    pub date_format: String,
    pub completed_format: String,
    pub newest_first: bool,
    pub alphabetical: bool,
    /// Only tasks whose subtasks are all done.
    pub only_if_done: bool,
    /// Tasks with any status but a space (`[-]`, `[>]` …), not only `[x]`.
    pub all_checked: bool,
    /// Only lines matching this are tasks.
    pub pattern: Option<Regex>,
    /// Replace this with that (`$1`) in the task's lines before.
    pub replace: Option<(Regex, String)>,
    /// Add this (with its date format) after the task.
    pub metadata: Option<(String, String)>,
    pub rules: Vec<Rule>,
    /// The indentation step when the note has none to copy.
    pub indent: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            to: To::Here,
            file: "{{sourceFileName}} (archive)".into(),
            under_heading: true,
            headings: vec!["Archived".into()],
            depth: 1,
            newlines: true,
            under_list_items: false,
            list_items: vec!["[[{{date}}]]".into()],
            date_format: "YYYY-MM-DD".into(),
            completed_format: "YYYY-MM-DD".into(),
            newest_first: false,
            alphabetical: false,
            only_if_done: false,
            all_checked: false,
            pattern: None,
            replace: None,
            metadata: Some((
                "🔒 [[{{date}}]] 🕸️ {{headingChain}}".into(),
                "YYYY-MM-DD".into(),
            )),
            rules: Vec::new(),
            indent: "\t".into(),
        }
    }
}

/// When and where it runs.
#[derive(Debug, Clone)]
pub struct Env {
    pub now: NaiveDateTime,
    /// The note's path in the vault, without `.md`.
    pub source: String,
    /// Today's daily note's path in the vault, without `.md`.
    pub daily: String,
}

/// A task on its way to the archive: its lines and where in the archive
/// note it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub item: Item,
    pub headings: Vec<String>,
    pub list_items: Vec<String>,
}

/// What archiving did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Archived {
    /// The note's new lines.
    pub here: Vec<String>,
    /// Tasks for other notes: (path in the vault without `.md`, tasks).
    pub elsewhere: Vec<(String, Vec<Placed>)>,
    /// Tasks taken out of the note (archived or deleted by a rule).
    pub count: usize,
}

/// `- [?]` (any list marker): the status is its group 1.
static TASK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[ \t]*(?:[-*+]|\d+[.)])[ \t]+\[(.)\]").expect("a valid pattern")
});

/// The Tasks plugin's done date.
static DONE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"✅ (\d{4}-\d{2}-\d{2})").expect("a valid pattern"));

/// `{{name}}` or `{{name:format}}`.
static PLACEHOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{(\w+)(?::([^}]*))?\}\}").expect("a valid pattern"));

/// A task's status symbol, whatever the settings.
fn status(line: &str) -> Option<char> {
    TASK.captures(line)?[1].chars().next()
}

fn done_date(line: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&DONE.captures(line)?[1], "%Y-%m-%d").ok()
}

/// Whether any of `blocks` (or under them) is an open task.
fn has_open(blocks: &[Block]) -> bool {
    blocks
        .iter()
        .any(|b| status(&b.line) == Some(' ') || has_open(&b.children))
}

impl Config {
    /// A task's status, if the line is a task (and matches the pattern).
    fn status(&self, line: &str) -> Option<char> {
        let s = status(line)?;
        self.pattern
            .as_ref()
            .is_none_or(|p| p.is_match(line))
            .then_some(s)
    }

    fn needs_archiving(&self, b: &Block) -> bool {
        let Some(s) = self.status(&b.line) else {
            return false;
        };
        if self.only_if_done && has_open(&b.children) {
            return false;
        }
        if self.rules.iter().any(|r| r.on() && r.statuses.contains(s)) {
            return true;
        }
        match s {
            ' ' => false,
            'x' | 'X' => true,
            _ => self.all_checked,
        }
    }

    /// The archive heading's title in the note the tasks are in (its
    /// tasks stay there).
    fn top(&self, env: &Env) -> Option<String> {
        self.headings
            .first()
            .map(|h| resolve(h, env, None, &self.date_format, &self.completed_format))
    }
}

/// A task taken out of its note: its lines and the headings above it.
#[derive(Debug, Clone)]
pub struct Task {
    pub block: Block,
    pub chain: Vec<String>,
}

/// Takes the tasks to archive out of `section` (and the sections in it,
/// but the archive's), with the headings above each.
fn extract(
    section: &mut Section,
    cfg: &Config,
    deep: bool,
    top: Option<&str>,
    chain: &mut Vec<String>,
    out: &mut Vec<Task>,
) {
    if section.level > 0 && Some(section.title.as_str()) == top {
        return;
    }
    if section.level > 0 {
        chain.push(section.title.clone());
    }
    take(&mut section.blocks, cfg, deep, chain, out);
    for child in &mut section.children {
        extract(child, cfg, deep, top, chain, out);
    }
    if section.level > 0 {
        chain.pop();
    }
}

fn take(blocks: &mut Vec<Block>, cfg: &Config, deep: bool, chain: &[String], out: &mut Vec<Task>) {
    let (taken, kept): (Vec<Block>, Vec<Block>) = std::mem::take(blocks)
        .into_iter()
        .partition(|b| cfg.needs_archiving(b));
    *blocks = kept;
    out.extend(taken.into_iter().map(|block| Task {
        block,
        chain: chain.to_vec(),
    }));
    if deep {
        for b in blocks.iter_mut() {
            take(&mut b.children, cfg, deep, chain, out);
        }
    }
}

/// "Archive tasks in this file" (`deep`: "including nested tasks").
pub fn archive_tasks(lines: &[String], cfg: &Config, env: &Env, deep: bool) -> Archived {
    let mut doc = Doc::parse(lines);
    let mut tasks = Vec::new();
    let top = cfg.top(env);
    extract(
        &mut doc.root,
        cfg,
        deep,
        top.as_deref(),
        &mut Vec::new(),
        &mut tasks,
    );
    if tasks.is_empty() {
        return Archived {
            here: lines.to_vec(),
            ..Archived::default()
        };
    }
    place(doc.lines(), tasks, cfg, env)
}

/// "Delete tasks in this file": the note without them, and how many.
pub fn delete_tasks(lines: &[String], cfg: &Config) -> (Vec<String>, usize) {
    let mut doc = Doc::parse(lines);
    let mut tasks = Vec::new();
    let top = cfg.headings.first().map(String::as_str);
    extract(&mut doc.root, cfg, false, top, &mut Vec::new(), &mut tasks);
    if tasks.is_empty() {
        return (lines.to_vec(), 0);
    }
    (doc.lines(), tasks.len())
}

/// JavaScript's `$1` / `$&` in a replacement, as the regex crate writes
/// them.
fn replacement(with: &str) -> String {
    let mut out = String::new();
    let mut chars = with.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('$', Some('&')) => {
                chars.next();
                out.push_str("${0}");
            }
            ('$', Some(d)) if d.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                    n.push(*d);
                    chars.next();
                }
                out.push_str(&format!("${{{n}}}"));
            }
            ('$', Some('$')) => {
                chars.next();
                out.push_str("$$");
            }
            ('$', _) => out.push_str("$$"),
            _ => out.push(c),
        }
    }
    out
}

/// Sends `tasks` (taken out of a note, now `rest`) where they go: this
/// note, other notes, or nowhere (a rule deletes them).
fn place(rest: Vec<String>, mut tasks: Vec<Task>, cfg: &Config, env: &Env) -> Archived {
    let count = tasks.len();
    let today = env.now.date();
    // Oldest first: each is added after (or, newest first, before) the
    // ones before it.
    tasks.sort_by_key(|t| done_date(&t.block.line).unwrap_or(today));
    let mut here: Vec<Placed> = Vec::new();
    let mut elsewhere: Vec<(String, Vec<Placed>)> = Vec::new();
    for mut t in tasks {
        if let Some((re, with)) = &cfg.replace {
            let with = replacement(with);
            t.block
                .map_lines(&mut |l| re.replace_all(l, with.as_str()).into_owned());
        }
        let rule = cfg.rules.iter().find(|r| {
            r.on()
                && r.path.as_ref().is_none_or(|p| p.is_match(&env.source))
                && r.text.as_ref().is_none_or(|p| p.is_match(&t.block.line))
                && (r.statuses.is_empty()
                    || status(&t.block.line).is_some_and(|s| r.statuses.contains(s)))
        });
        if rule.is_some_and(|r| r.delete) {
            continue;
        }
        let date_format = rule.map_or(cfg.date_format.as_str(), |r| r.date_format.as_str());
        if let Some((meta, format)) = &cfg.metadata {
            let format = rule.map_or(format.as_str(), |r| r.date_format.as_str());
            let m = resolve(meta, env, Some(&t), format, &cfg.completed_format);
            t.block.line = format!("{} {m}", t.block.line.trim_end());
        }
        let (to, file) = rule.map_or((cfg.to, cfg.file.as_str()), |r| (r.to, r.file.as_str()));
        let dest = match to {
            To::Here => None,
            To::Daily => Some(env.daily.clone()),
            To::Custom => Some(resolve(
                file,
                env,
                Some(&t),
                date_format,
                &cfg.completed_format,
            )),
        };
        let resolved = |templates: &[String]| {
            templates
                .iter()
                .map(|h| resolve(h, env, Some(&t), &cfg.date_format, &cfg.completed_format))
                .collect()
        };
        let placed = Placed {
            headings: resolved(&cfg.headings),
            list_items: resolved(&cfg.list_items),
            item: Item::Task(t.block),
        };
        match dest {
            Some(path) if path != env.source => {
                match elsewhere.iter_mut().find(|(p, _)| *p == path) {
                    Some((_, list)) => list.push(placed),
                    None => elsewhere.push((path, vec![placed])),
                }
            }
            _ => here.push(placed),
        }
    }
    Archived {
        here: if here.is_empty() {
            rest
        } else {
            merge(&rest, &here, cfg, false)
        },
        elsewhere,
        count,
    }
}

/// Fills in the placeholders: `{{date}}` (today), `{{sourceFileName}}`,
/// `{{sourceFilePath}}`, `{{heading}}`, `{{headingChain}}`,
/// `{{completedDate}}` (the task's ✅ date, else today; the original's
/// name for it works too);
/// a date takes a format after a colon (`{{date:YYYY-[W]WW}}`).
pub fn resolve(
    text: &str,
    env: &Env,
    task: Option<&Task>,
    date_format: &str,
    done_format: &str,
) -> String {
    let name = env.source.rsplit('/').next().unwrap_or_default();
    PLACEHOLDER
        .replace_all(text, |c: &regex::Captures| {
            let format = c.get(2).map(|f| f.as_str());
            match &c[1] {
                "date" => moment::format(&env.now, format.unwrap_or(date_format)),
                // The original's name too (`{{…TasksCompletedDate}}`).
                name if name == "completedDate"
                    || name.strip_prefix("obsidian") == Some("TasksCompletedDate") =>
                {
                    let day = task
                        .and_then(|t| done_date(&t.block.line))
                        .unwrap_or(env.now.date());
                    moment::format(
                        &day.and_time(chrono::NaiveTime::MIN),
                        format.unwrap_or(done_format),
                    )
                }
                "sourceFileName" => name.to_string(),
                "sourceFilePath" => env.source.clone(),
                "heading" => task
                    .and_then(|t| t.chain.last().cloned())
                    .unwrap_or_else(|| name.to_string()),
                "headingChain" => task
                    .map(|t| t.chain.join(" > "))
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| name.to_string()),
                _ => c[0].to_string(),
            }
        })
        .into_owned()
}

/// What's put in the archive: a task or a heading's section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Task(Block),
    Heading(Section),
}

/// Puts `placed` in the note `lines` (`separate`: a note of its own, not
/// the one they came from).
pub fn merge(lines: &[String], placed: &[Placed], cfg: &Config, separate: bool) -> Vec<String> {
    let mut all = lines.to_vec();
    for p in placed {
        match &p.item {
            Item::Task(b) => b.lines(&mut all),
            Item::Heading(s) => s.lines(&mut all),
        }
    }
    let step = tree::step(&all, &cfg.indent);
    let mut doc = Doc::parse(lines);
    for p in placed {
        let path = archive_section(&mut doc, &p.headings, cfg, separate);
        let target = doc.root.at_mut(&path);
        match &p.item {
            Item::Task(b) => merge_block(&mut target.blocks, b.clone(), &p.list_items, cfg, &step),
            Item::Heading(s) => {
                if cfg.newlines
                    && target.level > 0
                    && target.blocks.last().is_none_or(|b| !b.is_blank())
                {
                    target.blocks.push(Block::text(""));
                }
                let mut s = s.clone();
                s.relevel(target.level + 1);
                target.children.push(s);
            }
        }
    }
    doc.lines()
}

/// The section archived things go in (found, or made at the end): the
/// indices down to it from the note's start.
fn archive_section(doc: &mut Doc, headings: &[String], cfg: &Config, separate: bool) -> Vec<usize> {
    if (separate && !cfg.under_heading) || headings.is_empty() {
        return Vec::new();
    }
    if cfg.newlines {
        blank_at_end(&mut doc.root);
    }
    let mut path = Vec::new();
    for (i, h) in headings.iter().enumerate() {
        let context = doc.root.at_mut(&path);
        match context.find(h) {
            Some(rel) => path.extend(rel),
            None => {
                context.children.push(Section::new(i + cfg.depth, h));
                path.push(context.children.len() - 1);
            }
        }
    }
    path
}

/// A blank line at the note's end (in its last section), if it has none.
fn blank_at_end(section: &mut Section) {
    if let Some(last) = section.children.last_mut() {
        return blank_at_end(last);
    }
    let mut last = section.blocks.last();
    while let Some(b) = last.and_then(|b| b.children.last()) {
        last = Some(b);
    }
    let ends_blank = match last {
        Some(b) => b.line.trim().is_empty(),
        None => section.level == 0,
    };
    if !ends_blank {
        section.blocks.push(Block::text(""));
    }
}

/// The blocks at `path` (a list of block indices down from `blocks`).
fn children_at<'a>(blocks: &'a mut Vec<Block>, path: &[usize]) -> &'a mut Vec<Block> {
    path.iter().fold(blocks, |list, &i| &mut list[i].children)
}

/// The indentation of the children of the block at `path`.
fn child_indent(blocks: &[Block], path: &[usize], step: &str) -> String {
    let Some((&first, rest)) = path.split_first() else {
        return String::new();
    };
    let block = rest.iter().fold(&blocks[first], |b, &i| &b.children[i]);
    format!("{}{step}", tree::indent(&block.line))
}

/// Puts `block` in an archive section's `blocks`: under the list items
/// (found, or made) if the settings say so, first or last.
fn merge_block(
    blocks: &mut Vec<Block>,
    mut block: Block,
    list_items: &[String],
    cfg: &Config,
    step: &str,
) {
    while blocks.first().is_some_and(Block::is_blank) {
        blocks.remove(0);
    }
    while blocks.last().is_some_and(Block::is_blank) {
        blocks.pop();
    }
    let add = |list: &mut Vec<Block>, b: Block| {
        if cfg.newest_first {
            list.insert(0, b);
            0
        } else {
            list.push(b);
            list.len() - 1
        }
    };
    let mut path: Vec<usize> = Vec::new();
    if cfg.under_list_items {
        for item in list_items {
            let base = child_indent(blocks, &path, step);
            let list = children_at(blocks, &path);
            match Block::find(list, &|b| b.list && b.line.contains(item.as_str())) {
                Some(rel) => path.extend(rel),
                None => {
                    let new = Block {
                        line: format!("{base}- {item}"),
                        list: true,
                        children: Vec::new(),
                    };
                    path.push(add(list, new));
                }
            }
        }
    }
    block.rebase(&child_indent(blocks, &path, step));
    let list = children_at(blocks, &path);
    add(list, block);
    if cfg.alphabetical {
        list.sort_by(|a, b| a.line.trim().cmp(b.line.trim()));
    }
    if cfg.newlines {
        blocks.insert(0, Block::text(""));
        blocks.push(Block::text(""));
    }
}

/// "Archive heading under cursor": the heading at or above line `row`,
/// with everything under it.
pub fn archive_heading(
    lines: &[String],
    row: usize,
    cfg: &Config,
    env: &Env,
) -> Result<Archived, String> {
    let hs = tree::headings(lines);
    let start = (0..=row.min(lines.len().saturating_sub(1)))
        .rev()
        .find(|&i| hs.get(i).is_some_and(Option::is_some))
        .ok_or("put the cursor under a heading")?;
    let (level, title) = hs[start].clone().expect("a heading");
    if cfg.top(env).as_deref() == Some(title.as_str()) {
        return Err("that's the archive".into());
    }
    let end = (start + 1..lines.len())
        .find(|&i| hs[i].as_ref().is_some_and(|(l, _)| *l <= level))
        .unwrap_or(lines.len());
    let section = Doc::parse(&lines[start..end]).root.children.remove(0);
    let rest: Vec<String> = lines[..start]
        .iter()
        .chain(&lines[end..])
        .cloned()
        .collect();
    let headings: Vec<String> = cfg
        .headings
        .iter()
        .map(|h| resolve(h, env, None, &cfg.date_format, &cfg.completed_format))
        .collect();
    let placed = vec![Placed {
        item: Item::Heading(section),
        headings,
        list_items: Vec::new(),
    }];
    let dest = match cfg.to {
        To::Here => None,
        To::Daily => Some(env.daily.clone()),
        To::Custom => Some(resolve(
            &cfg.file,
            env,
            None,
            &cfg.date_format,
            &cfg.completed_format,
        )),
    };
    Ok(match dest {
        Some(path) if path != env.source => Archived {
            here: rest,
            elsewhere: vec![(path, placed)],
            count: 1,
        },
        _ => Archived {
            here: merge(&rest, &placed, cfg, false),
            elsewhere: Vec::new(),
            count: 1,
        },
    })
}

/// A list item or a line indented under one.
fn in_list(line: &str) -> bool {
    tree::is_list(line) || (!line.trim().is_empty() && line.starts_with([' ', '\t']))
}

/// "Sort tasks in list under cursor": the list around line `row` with
/// each level in order: plain items, open tasks, done tasks.
pub fn sort_list(lines: &[String], row: usize) -> Option<Vec<String>> {
    if !lines.get(row).is_some_and(|l| in_list(l)) {
        return None;
    }
    let mut start = row;
    while start > 0 && in_list(&lines[start - 1]) {
        start -= 1;
    }
    let mut end = row + 1;
    while end < lines.len() && in_list(&lines[end]) {
        end += 1;
    }
    fn sort(blocks: &mut [Block]) {
        blocks.sort_by_key(|b| match status(&b.line) {
            None => 0,
            Some('x' | 'X') => 2,
            Some(_) => 1,
        });
        for b in blocks {
            sort(&mut b.children);
        }
    }
    let mut blocks = tree::blocks(&lines[start..end]);
    sort(&mut blocks);
    let mut out = lines[..start].to_vec();
    for b in &blocks {
        b.lines(&mut out);
    }
    out.extend_from_slice(&lines[end..]);
    Some(out)
}

/// "Toggle task under cursor done and archive it": the archived note and
/// the line the task was on.
pub fn toggle_and_archive(
    lines: &[String],
    row: usize,
    cfg: &Config,
    env: &Env,
) -> Result<(Archived, usize), String> {
    const NOT_A_TASK: &str = "put the cursor on a task";
    let mut start = row;
    loop {
        let line = lines.get(start).ok_or(NOT_A_TASK)?;
        if tree::is_list(line) {
            break;
        }
        if !in_list(line) || start == 0 {
            return Err(NOT_A_TASK.into());
        }
        start -= 1;
    }
    let found = TASK.captures(&lines[start]).ok_or(NOT_A_TASK)?;
    let mark = found.get(1).expect("a status");
    let base = tree::indent(&lines[start]).len();
    let mut end = start + 1;
    while end < lines.len() && tree::indent(&lines[end]).len() > base {
        end += 1;
    }
    let mut block = tree::blocks(&lines[start..end]).remove(0);
    if mark.as_str() == " " {
        block.line.replace_range(mark.range(), "x");
    }
    // The headings above it.
    let mut chain: Vec<(usize, String)> = Vec::new();
    for (level, title) in tree::headings(&lines[..start]).into_iter().flatten() {
        chain.retain(|(l, _)| *l < level);
        chain.push((level, title));
    }
    let task = Task {
        block,
        chain: chain.into_iter().map(|(_, t)| t).collect(),
    };
    let rest: Vec<String> = lines[..start]
        .iter()
        .chain(&lines[end..])
        .cloned()
        .collect();
    Ok((place(rest, vec![task], cfg, env), start))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    /// The original's test settings: no metadata.
    fn cfg() -> Config {
        Config {
            metadata: None,
            ..Config::default()
        }
    }

    fn env() -> Env {
        Env {
            now: NaiveDate::from_ymd_opt(2021, 9, 16)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap(),
            source: "Projects/Plan".into(),
            daily: "Journal/2021-09-16".into(),
        }
    }

    fn here(text: &str, c: &Config, deep: bool) -> String {
        archive_tasks(&lines(text), c, &env(), deep).here.join("\n")
    }

    #[test]
    fn done_tasks_go_under_the_archive_heading() {
        let note = "- [ ] This one I haven't done yet\n- [x] Water the dog\n    - Some task details\n- [x] Feed the plants";
        let out = archive_tasks(&lines(note), &cfg(), &env(), false);
        assert_eq!(out.count, 2);
        assert_eq!(
            out.here.join("\n"),
            "- [ ] This one I haven't done yet\n\n# Archived\n\n- [x] Water the dog\n    - Some task details\n- [x] Feed the plants\n"
        );
        // Again: added to the archive, not into it again.
        let again = format!("- [x] One more\n{}", out.here.join("\n"));
        let again = here(&again, &cfg(), false);
        assert!(
            again.ends_with("- [x] Feed the plants\n- [x] One more\n"),
            "{again}"
        );
        assert_eq!(again.matches("# Archived").count(), 1);
        assert_eq!(
            archive_tasks(&lines("- [ ] a"), &cfg(), &env(), false).count,
            0
        );
    }

    #[test]
    fn a_date_tree_of_list_items() {
        let c = Config {
            under_list_items: true,
            list_items: vec!["[[{{date:YYYY-[W]WW}}]]".into(), "[[{{date}}]]".into()],
            ..cfg()
        };
        let note = "- [x] Water the dog\n    - Some task details\n- [x] Feed the plants";
        assert_eq!(
            here(note, &c, false),
            "# Archived\n\n- [[2021-W37]]\n    - [[2021-09-16]]\n        - [x] Water the dog\n            - Some task details\n        - [x] Feed the plants\n"
        );
    }

    #[test]
    fn nested_tasks_are_archived_deeply() {
        let note = "- [ ] Incomplete task\n    - [x] Completed subtask\n        - Task details\n    - [ ] Incomplete subtask";
        assert_eq!(
            here(note, &cfg(), true),
            "- [ ] Incomplete task\n    - [ ] Incomplete subtask\n\n# Archived\n\n- [x] Completed subtask\n    - Task details\n"
        );
        assert_eq!(
            archive_tasks(&lines(note), &cfg(), &env(), false).count,
            0,
            "not without deep"
        );
    }

    #[test]
    fn which_tasks_are_done() {
        let note = "- [x] a\n- [X] b\n- [-] c\n- [>] d\n- [x] e #task\n- [x] f\n    - [ ] open";
        let titles = |c: &Config| {
            let out = archive_tasks(&lines(note), c, &env(), false);
            let kept: Vec<String> = out
                .here
                .iter()
                .take_while(|l| !l.is_empty())
                .filter(|l| l.starts_with("- ["))
                .map(|l| l[6..7].to_string())
                .collect();
            kept.join("")
        };
        assert_eq!(titles(&cfg()), "cd", "x and X");
        let all = Config {
            all_checked: true,
            ..cfg()
        };
        assert_eq!(titles(&all), "", "any checked status");
        let only = Config {
            only_if_done: true,
            ..cfg()
        };
        assert_eq!(titles(&only), "cdf", "f has an open subtask");
        let pattern = Config {
            pattern: Some(Regex::new("#task").unwrap()),
            ..cfg()
        };
        assert_eq!(titles(&pattern), "abcdf");
        // Done tasks already in the archive stay there.
        let archived = "# Archived\n- [x] old";
        assert_eq!(
            archive_tasks(&lines(archived), &cfg(), &env(), false).count,
            0
        );
    }

    #[test]
    fn placeholders_metadata_replacement_and_order() {
        let c = Config {
            metadata: Some((
                "🔒 [[{{date}}]] 🕸️ {{headingChain}}".into(),
                "YYYY-MM-DD".into(),
            )),
            replace: Some((Regex::new("#([A-Za-z-]+)").unwrap(), "@$1".into())),
            newest_first: true,
            headings: vec!["Archived".into(), "{{sourceFileName}}".into()],
            ..cfg()
        };
        let note = "# Project\n## Team\n- [x] older #home ✅ 2021-09-01\n- [x] newer ✅ 2021-09-10";
        assert_eq!(
            here(note, &c, false),
            "# Project\n## Team\n\n# Archived\n## Plan\n\n\
             - [x] newer ✅ 2021-09-10 🔒 [[2021-09-16]] 🕸️ Project > Team\n\
             - [x] older @home ✅ 2021-09-01 🔒 [[2021-09-16]] 🕸️ Project > Team\n"
        );
        let r = |text: &str| resolve(text, &env(), None, "YYYY-MM-DD", "YYYY-MM-DD");
        assert_eq!(
            r("{{sourceFilePath}} {{heading}} {{date:DD.MM}}"),
            "Projects/Plan Plan 16.09"
        );
    }

    #[test]
    fn rules_and_other_notes() {
        let c = Config {
            rules: vec![
                Rule {
                    statuses: ">".into(),
                    path: None,
                    text: None,
                    delete: false,
                    to: To::Custom,
                    file: "Later/{{sourceFileName}}".into(),
                    date_format: "YYYY-MM-DD".into(),
                },
                Rule {
                    statuses: "-".into(),
                    path: Some(Regex::new("^Projects/").unwrap()),
                    text: None,
                    delete: true,
                    to: To::Custom,
                    file: String::new(),
                    date_format: "YYYY-MM-DD".into(),
                },
            ],
            ..cfg()
        };
        let note = "- [>] later\n- [-] dropped\n- [x] done\n- [ ] open";
        let out = archive_tasks(&lines(note), &c, &env(), false);
        assert_eq!(out.count, 3);
        assert_eq!(
            out.here.join("\n"),
            "- [ ] open\n\n# Archived\n\n- [x] done\n"
        );
        assert_eq!(out.elsewhere.len(), 1, "{:?}", out.elsewhere);
        let (path, placed) = &out.elsewhere[0];
        assert_eq!(path, "Later/Plan");
        assert_eq!(
            merge(&[], placed, &c, true).join("\n"),
            "# Archived\n\n- [>] later\n"
        );
        // Everything to the daily note, at its top.
        let daily = Config {
            to: To::Daily,
            under_heading: false,
            ..cfg()
        };
        let out = archive_tasks(&lines("- [x] a"), &daily, &env(), false);
        assert!(out.here.is_empty());
        assert_eq!(out.elsewhere[0].0, "Journal/2021-09-16");
        let existing = lines("# Day\n- [ ] x");
        assert_eq!(
            merge(&existing, &out.elsewhere[0].1, &daily, true).join("\n"),
            "\n- [x] a\n\n# Day\n- [ ] x",
            "at its top, before its headings"
        );
    }

    #[test]
    fn deleting_and_archiving_a_heading() {
        let (left, n) = delete_tasks(&lines("- [x] a\n- [ ] b\n- [x] c"), &cfg());
        assert_eq!((left.join("\n"), n), ("- [ ] b".into(), 2));
        let note = "Some top-level text\n\n# H1 heading\n\nSome text\n\n## H2 heading\n\nMore text";
        let out = archive_heading(&lines(note), 4, &cfg(), &env()).unwrap();
        assert_eq!(
            out.here.join("\n"),
            "Some top-level text\n\n# Archived\n\n## H1 heading\n\nSome text\n\n### H2 heading\n\nMore text"
        );
        assert!(archive_heading(&lines("no heading"), 0, &cfg(), &env()).is_err());
    }

    #[test]
    fn sorting_a_list() {
        let note = "before\n\n- [x] Task\n- Item\n- [ ] Incomplete\n    - [x] Task\n    - Item More notes\n    - [ ] Incomplete\n- Item 2\n- [ ] Incomplete 2\n    - [x] Task\n    - Item\n    - [x] Task 2\n\nafter";
        assert_eq!(
            sort_list(&lines(note), 5).unwrap().join("\n"),
            "before\n\n- Item\n- Item 2\n- [ ] Incomplete\n    - Item More notes\n    - [ ] Incomplete\n    - [x] Task\n- [ ] Incomplete 2\n    - Item\n    - [x] Task\n    - [x] Task 2\n- [x] Task\n\nafter"
        );
        assert!(sort_list(&lines(note), 0).is_none());
    }

    #[test]
    fn checking_off_and_archiving_one_task() {
        let note = "# Today\n- [ ] keep\n- [ ] this one\n    its detail\n- [ ] keep too";
        let (out, row) = toggle_and_archive(&lines(note), 3, &cfg(), &env()).unwrap();
        assert_eq!(row, 2);
        assert_eq!(
            out.here.join("\n"),
            "# Today\n- [ ] keep\n- [ ] keep too\n\n# Archived\n\n- [x] this one\n    its detail\n"
        );
        assert!(toggle_and_archive(&lines(note), 0, &cfg(), &env()).is_err());
        // Into an archive that's there, the note ending with a blank line.
        let note = ["- [ ] open", "", "# Archived", "", "- [x] done", ""].map(String::from);
        let (out, _) = toggle_and_archive(&note, 0, &cfg(), &env()).unwrap();
        assert_eq!(
            out.here.join("\n"),
            "\n# Archived\n\n- [x] done\n- [x] open\n"
        );
    }
}
