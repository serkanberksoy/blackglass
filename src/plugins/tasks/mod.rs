//! Tasks: the Tasks community plugin's features on top of blackglass's own
//! checkboxes (`requirements/tasks_requirements.md`). Checkboxes, task
//! states, Ctrl+T / Ctrl+L and Dataview's `TASK` queries stay the
//! editor's and Dataview's; this plugin adds:
//!
//! - [`task`]: dates, priorities, recurrence and dependencies read from a
//!   task's line (emoji or Dataview format), statuses, and toggling with
//!   done dates and the next occurrence.
//! - [`query`]: ```` ```tasks ```` blocks' filters, sorting and grouping.
//! - [`results`]: drawing the results; a click or Enter toggles a task.
//! - Commands: toggle done, create or edit a task (a window of every
//!   field, Alt+T), postpone.

pub mod query;
pub mod results;
pub mod task;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{Days, Months, NaiveDate};
use ratatui::text::Line;

use super::settings::{Kind, Setting, Values};
use super::{Answer, Context, Effect, FormField, Manifest, Plugin, PluginCommand, Question};
use crate::vault::Vault;
use task::{DateField, Priority, Statuses, Task, Toggle, Type};

/// The plugin's id.
const ID: &str = "tasks";

/// Rendered blocks by (query, note, width, day); cleared when the vault
/// changes.
type Cache = HashMap<(String, Option<PathBuf>, usize, NaiveDate), Vec<results::Row>>;

#[derive(Default)]
pub struct Tasks {
    /// Every task in the vault.
    tasks: Vec<Task>,
    root: PathBuf,
    statuses: Statuses,
    toggle: Toggle,
    /// Only lines with it are tasks (`#task`); empty: every checkbox.
    global_filter: String,
    remove_global_filter: bool,
    /// Instructions put before every query, separated by `;`.
    global_query: String,
    /// New fields in the Dataview format (`[due:: …]`).
    dataview: bool,
    cache: RefCell<Cache>,
    /// The task being created or edited: where, and the line as it was
    /// (`None` for a new one).
    editing: Option<(Spot, Option<String>)>,
    /// The task being postponed: where, and its line.
    postponing: Option<(Spot, Task)>,
    /// The priorities and statuses in the order the questions list them.
    priority_items: Vec<Priority>,
    status_items: Vec<char>,
}

/// Where a task being changed is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Spot {
    /// A line of the active note.
    Here(usize),
    /// A line of another note (a query result the cursor is on).
    There(PathBuf, usize),
}

/// Today (the local date).
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

impl Tasks {
    pub fn new() -> Self {
        Tasks::default()
    }

    fn index(&mut self, vault: &Vault) {
        self.tasks.clear();
        for note in &vault.notes {
            self.index_note(note);
        }
    }

    /// The tasks of `note`, added.
    fn index_note(&mut self, note: &crate::vault::Note) {
        {
            let mut heading = None;
            let mut fence = false;
            // The list items above, by indentation: (indent, line).
            let mut items: Vec<(usize, usize)> = Vec::new();
            for (i, line) in note.lines.iter().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    fence = !fence;
                    continue;
                }
                if fence {
                    continue;
                }
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
                    heading = Some(trimmed[level..].trim().to_string());
                    continue;
                }
                let indent = line.len() - trimmed.len();
                let parent = if is_list_item(trimmed) {
                    while items.last().is_some_and(|&(d, _)| d >= indent) {
                        items.pop();
                    }
                    let parent = items.last().map(|&(_, l)| l);
                    items.push((indent, i));
                    parent
                } else {
                    if !trimmed.is_empty() && indent == 0 {
                        items.clear();
                    }
                    None
                };
                if !self.global_filter.is_empty() && !line.contains(&self.global_filter) {
                    continue;
                }
                if let Some(mut t) = Task::parse(line) {
                    t.path = note.rel.clone();
                    t.line = i;
                    t.heading = heading.clone();
                    t.parent = parent;
                    self.tasks.push(t);
                }
            }
        }
    }

    /// The task on the active note's cursor line.
    /// The task a command works on: the query result the cursor is on (in
    /// view mode), or the task on the cursor's line.
    fn target(&self, ctx: &Context) -> Option<(Spot, Task)> {
        if let Some(payload) = ctx
            .note
            .and_then(|n| n.action)
            .and_then(|a| a.strip_prefix("plugin:tasks:toggle:"))
        {
            let (line, rel) = payload.split_once(':')?;
            let line: usize = line.parse().ok()?;
            let t = self
                .tasks
                .iter()
                .find(|t| t.line == line && t.path == Path::new(rel))?;
            return Some((Spot::There(self.root.join(rel), line), t.clone()));
        }
        Tasks::task_at_cursor(ctx).map(|(row, t)| (Spot::Here(row), t))
    }

    /// Puts `line` where a task is (`before`: the line it replaces).
    fn put(spot: Spot, before: Option<String>, line: String, ctx: &Context) -> Effect {
        match spot {
            Spot::Here(row) => {
                let col = ctx.note.map_or(0, |n| n.col).min(line.chars().count());
                Effect::ReplaceLines {
                    from: row,
                    to: row + 1,
                    lines: vec![line],
                    cursor: (row, col),
                }
            }
            Spot::There(path, at) => Effect::EditNote {
                path,
                from: at,
                to: at + 1,
                lines: vec![line],
                expect: before.into_iter().collect(),
            },
        }
    }

    fn task_at_cursor(ctx: &Context) -> Option<(usize, Task)> {
        let note = ctx.note?;
        let line = note.text.lines().nth(note.row)?;
        Task::parse(line).map(|t| (note.row, t))
    }

    fn render(&self, source: &[String], from: Option<&Path>, width: usize) -> Vec<results::Row> {
        let mut text = source.join("\n");
        if let Some(rel) = from.and_then(|p| p.strip_prefix(&self.root).ok()) {
            text = placeholders(&text, rel);
        }
        let today = today();
        let query = match query::parse(&text, &self.global_query, today) {
            Ok(q) => q,
            Err(e) => {
                return vec![(
                    Line::styled(
                        format!("Tasks query: {e}"),
                        ratatui::style::Style::new().fg(ratatui::style::Color::LightRed),
                    ),
                    None,
                )];
            }
        };
        let (open_ids, depended_on) = results::dependencies(&self.tasks, &self.statuses);
        let env = query::Env {
            today,
            statuses: &self.statuses,
            open_ids: &open_ids,
            depended_on: &depended_on,
        };
        let remove = (self.remove_global_filter && !self.global_filter.is_empty())
            .then_some(self.global_filter.as_str());
        results::run(&query, &self.tasks, &env, remove, width)
    }

    /// The line a toggle writes for the cursor's line: a task toggled, or
    /// a line made a task.
    fn toggle_line(&self, ctx: &Context) -> Effect {
        let Some(note) = ctx.note else {
            return Effect::Message("Open a note first".into());
        };
        let row = note.row;
        let Some(line) = note.text.lines().nth(row).or(Some("")) else {
            return Effect::None;
        };
        let Some(task) = Task::parse(line) else {
            // Not a task yet: make it one.
            let trimmed = line.trim_start();
            let indent = &line[..line.len() - trimmed.len()];
            let text = trimmed
                .strip_prefix(['-', '*', '+'])
                .map_or(trimmed, str::trim_start);
            let new = format!("{indent}- [ ] {text}");
            let col = new.chars().count();
            return Effect::ReplaceLines {
                from: row,
                to: row + 1,
                lines: vec![new],
                cursor: (row, col),
            };
        };
        let lines = task::toggle(&task, &self.statuses, today(), &self.toggle);
        // The cursor stays on the task it toggled.
        let at = if self.toggle.below || lines.len() < 2 {
            0
        } else {
            lines.len() - 1
        };
        Effect::ReplaceLines {
            from: row,
            to: row + 1,
            cursor: (row + at.min(lines.len().saturating_sub(1)), note.col),
            lines,
        }
    }

    /// The create-or-edit window (TK-20): every field of the task at the
    /// cursor (or of a new one), each with its value or default.
    fn ask_edit(&mut self, ctx: &Context) -> Effect {
        let Some(note) = ctx.note else {
            return Effect::Message("Open a note first".into());
        };
        // A query result the cursor is on, or the cursor's line.
        let (spot, task, line) = match self.target(ctx) {
            Some((Spot::There(path, at), t)) => {
                let raw = t.raw.clone();
                (Spot::There(path, at), Some(t), raw)
            }
            _ => {
                let line = note.text.lines().nth(note.row).unwrap_or_default();
                (Spot::Here(note.row), Task::parse(line), line.to_string())
            }
        };
        let line = line.as_str();
        self.editing = Some((spot, task.as_ref().map(|t| t.raw.clone())));
        let description = match &task {
            // Tags written after the fields too, so they stay.
            Some(t) => {
                let mut d = t.description.clone();
                for tag in &t.tags {
                    if !d.contains(tag.as_str()) {
                        d.push(' ');
                        d.push_str(tag);
                    }
                }
                d
            }
            None => line
                .trim_start()
                .trim_start_matches(['-', '*', '+'])
                .trim()
                .to_string(),
        };
        // Priorities as the original lists them (highest first, none in
        // the middle).
        self.priority_items = Priority::ALL.iter().map(|(p, _, _)| *p).collect();
        let priority = task.as_ref().map_or(Priority::None, |t| t.priority);
        // The core statuses first, then the others.
        let mut symbols = vec![' ', '/', 'x', '-'];
        for st in self.statuses.all() {
            if !symbols.contains(&st.symbol) {
                symbols.push(st.symbol);
            }
        }
        let status = task.as_ref().map_or(' ', |t| t.status);
        if !symbols.contains(&status) {
            symbols.push(status);
        }
        self.status_items = symbols;
        let date = |f: DateField| {
            task.as_ref()
                .and_then(|t| t.date(f))
                .map(|d| d.to_string())
                .unwrap_or_default()
        };
        let named = |items: &[String], at: usize| (items.to_vec(), at);
        let statuses: Vec<String> = self
            .status_items
            .iter()
            .map(|c| format!("{} [{c}]", self.statuses.get(*c).name))
            .collect();
        let priorities: Vec<String> = self
            .priority_items
            .iter()
            .map(|p| {
                let mut name = p.name().to_string();
                name[..1].make_ascii_uppercase();
                match p.emoji() {
                    "" => name,
                    e => format!("{name} {e}"),
                }
            })
            .collect();
        let completion = ["(nothing)", "keep", "delete"].map(String::from).to_vec();
        let on_completion = task
            .as_ref()
            .and_then(|t| t.on_completion.as_deref())
            .and_then(|c| completion.iter().position(|x| x == c))
            .unwrap_or(0);
        let (s_items, s_at) = named(
            &statuses,
            self.status_items
                .iter()
                .position(|c| *c == status)
                .unwrap_or(0),
        );
        let (p_items, p_at) = named(
            &priorities,
            self.priority_items
                .iter()
                .position(|p| *p == priority)
                .unwrap_or(3),
        );
        let when = "A date: 2026-10-20, today, tomorrow, fri, next week, in 3 days (empty: none)";
        Effect::Ask(vec![Question::Form {
            title: if task.is_some() {
                "Edit task"
            } else {
                "New task"
            }
            .into(),
            fields: vec![
                FormField::text(
                    "Description",
                    "What's to be done (tags too: #work)",
                    &description,
                ),
                FormField::choice("Status", "←→ the task's status", s_items, s_at),
                FormField::choice(
                    "Priority",
                    "←→ highest, high, medium, none, low, lowest",
                    p_items,
                    p_at,
                ),
                FormField::date("Due", when, &date(DateField::Due)),
                FormField::date("Scheduled", when, &date(DateField::Scheduled)),
                FormField::date("Start", when, &date(DateField::Start)),
                FormField::text(
                    "Recurs",
                    "every day, every week on Monday, every month on the 1st … (empty: once)",
                    task.as_ref()
                        .and_then(|t| t.recurrence.as_deref())
                        .unwrap_or_default(),
                ),
                FormField::date("Created", when, &date(DateField::Created)),
                FormField::date("Done", when, &date(DateField::Done)),
                FormField::date("Cancelled", when, &date(DateField::Cancelled)),
                FormField::text(
                    "ID",
                    "A name other tasks can wait on (letters, digits, - and _)",
                    task.as_ref()
                        .and_then(|t| t.id.as_deref())
                        .unwrap_or_default(),
                ),
                FormField::text(
                    "Depends on",
                    "The IDs of the tasks this one waits for, separated by commas",
                    &task
                        .as_ref()
                        .map(|t| t.depends.join(","))
                        .unwrap_or_default(),
                ),
                FormField::choice(
                    "On completion",
                    "←→ what happens to the task once it's done: keep it, or delete it",
                    completion,
                    on_completion,
                ),
            ],
        }])
    }

    /// Writes the task from the window's fields.
    fn answer_edit(&mut self, answers: &[Answer], ctx: &Context) -> Effect {
        let Some((spot, before)) = self.editing.take() else {
            return Effect::None;
        };
        let [Answer::Fields(fields)] = answers else {
            return Effect::None;
        };
        let text = |i: usize| match fields.get(i) {
            Some(Answer::Text(t)) => t.trim().to_string(),
            _ => String::new(),
        };
        let choice = |i: usize| match fields.get(i) {
            Some(Answer::Choice(c)) => *c,
            _ => 0,
        };
        let today = today();
        let now = chrono::Local::now().naive_local();
        let mut dates = Vec::new();
        for (i, field) in [
            (3, DateField::Due),
            (4, DateField::Scheduled),
            (5, DateField::Start),
            (7, DateField::Created),
            (8, DateField::Done),
            (9, DateField::Cancelled),
        ] {
            let typed = text(i);
            if typed.is_empty() {
                dates.push((field, None));
                continue;
            }
            match query::date(&typed, today)
                .or_else(|| crate::nldates::parse_date(&typed, now, chrono::Weekday::Mon))
            {
                Some(d) => dates.push((field, Some(d))),
                None => return Effect::Message(format!("Tasks: can't read the date {typed:?}")),
            }
        }
        let rule = text(6);
        if !rule.is_empty()
            && let Err(e) = task::Recurrence::parse(&rule)
        {
            return Effect::Message(format!("Tasks: {e}"));
        }
        let old = before.as_deref().and_then(Task::parse);
        let mut t = old
            .clone()
            .unwrap_or_else(|| Task::parse("- [ ] x").expect("a task line is a task"));
        t.description = text(0);
        t.priority = self
            .priority_items
            .get(choice(2))
            .copied()
            .unwrap_or_default();
        for (field, date) in dates {
            t.dates[field as usize] = date;
        }
        t.recurrence = Some(rule).filter(|r| !r.is_empty());
        t.id = Some(text(10)).filter(|i| !i.is_empty());
        t.depends = text(11)
            .split([',', ' '])
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(String::from)
            .collect();
        t.on_completion = match choice(12) {
            1 => Some("keep".into()),
            2 => Some("delete".into()),
            _ => None,
        };
        let symbol = self.status_items.get(choice(1)).copied().unwrap_or(' ');
        let was = self.statuses.get(t.status).kind;
        let now_kind = self.statuses.get(symbol).kind;
        // Done (or cancelled) now: its date, unless one was typed.
        if self.toggle.dates {
            for (field, kind) in [
                (DateField::Done, Type::Done),
                (DateField::Cancelled, Type::Cancelled),
            ] {
                if now_kind == kind && was != kind && t.dates[field as usize].is_none() {
                    t.dates[field as usize] = Some(today);
                }
            }
        }
        t.status = symbol;
        let dataview = self.dataview || before.as_deref().is_some_and(|b| b.contains("::"));
        let line = write(&t, dataview);
        Tasks::put(spot, before, line, ctx)
    }

    /// Postpones the cursor's task by the chosen time (TK-22).
    fn postpone(&mut self, choice: usize, ctx: &Context) -> Effect {
        let Some((spot, t)) = self.postponing.take() else {
            return Effect::Message("Tasks: the cursor is not on a task".into());
        };
        let Some(&(_, days, months)) = POSTPONE.get(choice) else {
            return Effect::None;
        };
        let field = [DateField::Due, DateField::Scheduled, DateField::Start]
            .into_iter()
            .find(|f| t.date(*f).is_some())
            .unwrap_or(DateField::Due);
        let base = t.date(field).unwrap_or_default().max(today());
        let Some(date) = base
            .checked_add_days(Days::new(days))
            .and_then(|d| d.checked_add_months(Months::new(months)))
        else {
            return Effect::None;
        };
        let line = Task::with_date(&t.raw, field, Some(date));
        Tasks::put(spot, Some(t.raw.clone()), line, ctx)
    }
}

/// Whether `line` (without its indent) is a list item: `- `, `* `, `+ `,
/// `1. `.
fn is_list_item(line: &str) -> bool {
    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
        return true;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && (line[digits..].starts_with(". ") || line[digits..].starts_with(") "))
}

/// The postpone choices: (name, days, months).
const POSTPONE: [(&str, u64, u32); 6] = [
    ("1 day", 1, 0),
    ("2 days", 2, 0),
    ("3 days", 3, 0),
    ("1 week", 7, 0),
    ("2 weeks", 14, 0),
    ("1 month", 0, 1),
];

/// `{{query.file.path}}` and the others, for the note the query is in.
fn placeholders(text: &str, rel: &Path) -> String {
    let path = rel.to_string_lossy().replace('\\', "/");
    let without_ext = path.strip_suffix(".md").unwrap_or(&path).to_string();
    let filename = rel
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = filename
        .strip_suffix(".md")
        .unwrap_or(&filename)
        .to_string();
    let folder = match path.rsplit_once('/') {
        Some((f, _)) => format!("{f}/"),
        None => "/".into(),
    };
    let root = match path.split_once('/') {
        Some((r, _)) => format!("{r}/"),
        None => "/".into(),
    };
    text.replace("preset this_file", "path includes {{query.file.path}}")
        .replace("{{query.file.path}}", &path)
        .replace("{{query.file.pathWithoutExtension}}", &without_ext)
        .replace("{{query.file.filename}}", &filename)
        .replace("{{query.file.filenameWithoutExtension}}", &name)
        .replace("{{query.file.folder}}", &folder)
        .replace("{{query.file.root}}", &root)
}

/// A task's line from its fields, in upstream's order: description, id,
/// dependencies, priority, recurrence, on completion, then the dates.
fn write(t: &Task, dataview: bool) -> String {
    let mut out = format!("{}[{}] {}", t.prefix, t.status, t.description);
    let mut field = |emoji: &str, key: &str, value: &str| {
        if dataview {
            out.push_str(&format!(" [{key}:: {value}]"));
        } else {
            out.push_str(&format!(" {emoji} {value}"));
        }
    };
    if let Some(id) = &t.id {
        field("🆔", "id", id);
    }
    if !t.depends.is_empty() {
        field("⛔", "dependsOn", &t.depends.join(","));
    }
    if t.priority != Priority::None {
        if dataview {
            field("", "priority", t.priority.name());
        } else {
            out.push_str(&format!(" {}", t.priority.emoji()));
        }
    }
    let mut field = |emoji: &str, key: &str, value: &str| {
        if dataview {
            out.push_str(&format!(" [{key}:: {value}]"));
        } else {
            out.push_str(&format!(" {emoji} {value}"));
        }
    };
    if let Some(r) = &t.recurrence {
        field("🔁", "repeat", r);
    }
    if let Some(c) = &t.on_completion {
        field("🏁", "onCompletion", c);
    }
    for f in [
        DateField::Created,
        DateField::Start,
        DateField::Scheduled,
        DateField::Due,
        DateField::Cancelled,
        DateField::Done,
    ] {
        if let Some(d) = t.date(f) {
            let key = DateField::ALL[f as usize].3;
            field(f.emoji(), key, &d.to_string());
        }
    }
    out
}

impl Plugin for Tasks {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Tasks",
            version: "0.1.0",
            author: "blackglass, after Clare Macrae's and Ilyas Landikov's Tasks",
            description: "Due, scheduled and start dates, priorities and recurrence on tasks; \
                          ```tasks blocks filter, sort and group the vault's tasks (a click checks \
                          one off); toggling writes done dates and the next occurrence.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_unload(&mut self) {
        self.tasks.clear();
        self.cache.borrow_mut().clear();
    }

    fn on_note_changed(&mut self, path: &Path, vault: &Vault) {
        let Ok(rel) = path.strip_prefix(&vault.root) else {
            return;
        };
        self.tasks.retain(|t| t.path != rel);
        if let Some(note) = vault.note(path) {
            self.index_note(note);
            // Grouped by note, in the vault's order, as a full scan has them.
            self.tasks.sort_by(|a, b| {
                let key = |p: &Path| p.to_string_lossy().to_lowercase();
                key(&a.path).cmp(&key(&b.path)).then(a.line.cmp(&b.line))
            });
        }
        self.cache.borrow_mut().clear();
    }

    fn on_theme_changed(&mut self) {
        self.cache.borrow_mut().clear();
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        let get = |key: &str| values.get("", key).unwrap_or_default().to_string();
        let on = |key: &str, default: bool| match values.get("", key) {
            Some(v) => v == "true",
            None => default,
        };
        self.global_filter = get("global_filter").trim().to_string();
        self.remove_global_filter = on("remove_global_filter", false);
        self.global_query = get("global_query");
        self.toggle = Toggle {
            dates: on("set_done_date", true),
            below: on("recurrence_below", false),
            created: on("set_created_date", false),
        };
        self.dataview = get("format") == "dataview";
        self.statuses = Statuses::with_custom(&get("statuses")).0;
        self.root = vault.root.clone();
        self.index(vault);
        self.cache.borrow_mut().clear();
    }

    fn settings(&self) -> Vec<Setting> {
        let toggle = |key: &str, label: &str, help: &str, default: &str| {
            Setting::new("", key, label, help, Kind::Toggle, default)
        };
        let text =
            |key: &str, label: &str, help: &str| Setting::new("", key, label, help, Kind::Text, "");
        vec![
            text(
                "global_filter",
                "Global filter",
                "Only lines with this (e.g. #task) are tasks; empty: every checkbox",
            ),
            toggle(
                "remove_global_filter",
                "Hide the global filter",
                "Leave the global filter's text out of query results",
                "false",
            ),
            text(
                "global_query",
                "Global query",
                "Instructions put before every query, separated by ;",
            ),
            toggle(
                "set_done_date",
                "Set done dates",
                "Checking a task off writes ✅ today (❌ when cancelled)",
                "true",
            ),
            toggle(
                "set_created_date",
                "Set created dates",
                "A recurring task's next occurrence gets ➕ today",
                "false",
            ),
            toggle(
                "recurrence_below",
                "Next occurrence below",
                "A recurring task's next occurrence goes below the done one",
                "false",
            ),
            text(
                "statuses",
                "Custom statuses",
                "symbol|name|next|TYPE, separated by ; (e.g. ~|Waiting|x|ON_HOLD)",
            ),
            Setting::new(
                "",
                "format",
                "Task format",
                "How new dates and priorities are written",
                Kind::Choice(vec!["emoji".into(), "dataview".into()]),
                "emoji",
            ),
        ]
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("toggle-done", "Toggle task done"),
            PluginCommand::new("create-or-edit", "Create or edit task").keys("Alt+T"),
            PluginCommand::new("postpone", "Postpone task"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        match id {
            "toggle-done" => self.toggle_line(ctx),
            "create-or-edit" => self.ask_edit(ctx),
            "postpone" => {
                self.postponing = self.target(ctx);
                if self.postponing.is_none() {
                    return Effect::Message("Tasks: the cursor is not on a task".into());
                }
                Effect::Ask(vec![Question::Choose {
                    prompt: "Postpone by".into(),
                    items: POSTPONE.iter().map(|(n, _, _)| n.to_string()).collect(),
                }])
            }
            _ => Effect::None,
        }
    }

    fn answer(&mut self, id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        match (id, answers) {
            ("create-or-edit", _) => self.answer_edit(answers, ctx),
            ("postpone", [Answer::Choice(i)]) => self.postpone(*i, ctx),
            _ => Effect::None,
        }
    }

    /// On a task line (TK-21): the fields as a word is typed (`du` → 📅
    /// due date), dates after a date's emoji, rules after 🔁.
    fn suggestions(&self, line: &str, col: usize, _vault: &Vault) -> Option<super::Suggestions> {
        task::checkbox(line)?;
        let chars: Vec<char> = line.chars().collect();
        let col = col.min(chars.len());
        let before: String = chars[..col].iter().collect();
        // After a field's emoji: what's typed since.
        let after = |emoji: &str| -> Option<(usize, String)> {
            let at = before.rfind(emoji)?;
            let typed = before[at + emoji.len()..].strip_prefix(' ')?;
            let ok = typed
                .chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == ',');
            ok.then(|| {
                (
                    before[..at + emoji.len() + 1].chars().count(),
                    typed.to_string(),
                )
            })
        };
        let pick = |all: Vec<(String, String)>, typed: &str| {
            let typed = typed.to_lowercase();
            all.into_iter()
                .filter(|(label, _)| label.to_lowercase().contains(typed.trim()))
                .take(6)
                .collect::<Vec<_>>()
        };
        let dates = ["📅", "⏳", "🛫", "➕", "✅", "❌"];
        // A date being written (not one already there: then the next word).
        let writing = |(_, typed): &(usize, String)| {
            !typed.chars().take(10).any(|c| c.is_ascii_digit()) || typed.len() < 10
        };
        if let Some((start, typed)) = dates.iter().find_map(|e| after(e)).filter(writing) {
            let today = today();
            let all = [
                "today",
                "tomorrow",
                "in 2 days",
                "next monday",
                "next friday",
                "in a week",
                "in 2 weeks",
                "in a month",
            ]
            .iter()
            .filter_map(|phrase| {
                let p = phrase.replace("in a ", "in 1 ");
                let d = query::date(&p, today)?;
                Some((format!("{phrase} ({d})"), d.to_string()))
            })
            .collect();
            let items = pick(all, &typed);
            return Some(super::Suggestions {
                start,
                items,
                ..Default::default()
            });
        }
        if let Some((start, typed)) = after("🔁") {
            let all = [
                "every day",
                "every week",
                "every weekday",
                "every month",
                "every year",
                "every week on Monday",
                "every month on the 1st",
                "every day when done",
            ]
            .iter()
            .map(|r| (r.to_string(), r.to_string()))
            .collect();
            let items = pick(all, &typed);
            return Some(super::Suggestions {
                start,
                items,
                ..Default::default()
            });
        }
        // A word being typed: the fields.
        let start = before
            .rfind(' ')
            .map_or(0, |i| before[..=i].chars().count());
        let word: String = chars[start..col].iter().collect();
        if word.is_empty() || !word.chars().all(char::is_alphabetic) {
            return None;
        }
        let fields: Vec<(String, String)> = [
            ("📅 due date", "📅 "),
            ("⏳ scheduled date", "⏳ "),
            ("🛫 start date", "🛫 "),
            ("➕ created date", "➕ "),
            ("🔁 recurring (repeat)", "🔁 "),
            ("⏫ high priority", "⏫ "),
            ("🔼 medium priority", "🔼 "),
            ("🔽 low priority", "🔽 "),
            ("🔺 highest priority", "🔺 "),
            ("⏬ lowest priority", "⏬ "),
            ("🆔 id", "🆔 "),
            ("⛔ depends on", "⛔ "),
            ("🏁 on completion", "🏁 "),
        ]
        .iter()
        .map(|(l, t)| (l.to_string(), t.to_string()))
        .collect();
        let items = pick(fields, &word);
        (!items.is_empty()).then_some(super::Suggestions {
            start,
            items,
            ..Default::default()
        })
    }

    fn row_action(&mut self, payload: &str, _ctx: &Context) -> Effect {
        let Some((line, rel)) = payload
            .strip_prefix("toggle:")
            .and_then(|r| r.split_once(':'))
        else {
            return Effect::None;
        };
        let Ok(line) = line.parse::<usize>() else {
            return Effect::None;
        };
        let rel = Path::new(rel);
        let Some(t) = self.tasks.iter().find(|t| t.line == line && t.path == rel) else {
            return Effect::Message("Tasks: that task isn't there any more".into());
        };
        let lines = task::toggle(t, &self.statuses, today(), &self.toggle);
        Effect::EditNote {
            path: self.root.join(rel),
            from: line,
            to: line + 1,
            lines,
            expect: vec![t.raw.clone()],
        }
    }

    fn block_languages(&self) -> &[&'static str] {
        &["tasks"]
    }

    fn render_block(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<Line<'static>> {
        self.render_block_rows(lang, source, from, width)
            .into_iter()
            .map(|(line, _)| line)
            .collect()
    }

    fn render_block_rows(
        &self,
        _lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<(Line<'static>, Option<String>)> {
        let key = (
            source.join("\n"),
            from.map(Path::to_path_buf),
            width,
            today(),
        );
        if let Some(rows) = self.cache.borrow().get(&key) {
            return rows.clone();
        }
        let rows = self.render(source, from, width);
        self.cache.borrow_mut().insert(key, rows.clone());
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_name_the_querys_note() {
        let text = placeholders(
            "path includes {{query.file.folder}}\npreset this_file\n{{query.file.filenameWithoutExtension}}",
            Path::new("Work/Plan.md"),
        );
        assert_eq!(
            text,
            "path includes Work/\npath includes Work/Plan.md\nPlan"
        );
    }

    #[test]
    fn a_task_is_written_in_either_format() {
        let t = Task::parse("- [ ] Call 🔁 every week 📅 2026-10-01 ⏫ 🆔 c1").unwrap();
        assert_eq!(
            write(&t, false),
            "- [ ] Call 🆔 c1 ⏫ 🔁 every week 📅 2026-10-01"
        );
        assert_eq!(
            write(&t, true),
            "- [ ] Call [id:: c1] [priority:: high] [repeat:: every week] [due:: 2026-10-01]"
        );
    }
}
