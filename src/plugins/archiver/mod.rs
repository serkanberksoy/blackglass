//! The Task Archiver plugin (W-127), after the community plugin of that
//! name: done tasks move under an "Archived" heading (in the note, another
//! note or today's daily note), in a tree of headings and list items made
//! from placeholders (a date tree); deleting done tasks; archiving the
//! heading under the cursor; sorting a list (items, open tasks, done
//! tasks); checking a task off and archiving it; rules for tasks by
//! status, path and text. The work is in [`archive`] (on lines), the
//! note's shape in [`tree`].

pub mod archive;
pub mod tree;

use std::path::{Component, Path, PathBuf};

use regex::Regex;

use super::settings::{Kind, Setting, Values};
use super::{Context, Effect, Manifest, Plugin, PluginCommand};
use crate::vault::{Vault, split_lines};
use archive::{Archived, Config, Env, Rule, To};

const ID: &str = "archiver";

/// The settings' choices for where tasks go.
const PLACES: [&str; 3] = ["this note", "custom note", "daily note"];

pub struct Archiver {
    cfg: Config,
    /// The rules' sections, in order (`rule.1` …).
    rules: Vec<String>,
    /// A setting that isn't right (a regular expression), said when a
    /// command runs.
    problem: Option<String>,
}

impl Archiver {
    pub fn new() -> Self {
        Archiver {
            cfg: Config::default(),
            rules: Vec::new(),
            problem: None,
        }
    }

    /// Reads the settings file.
    fn load(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        let schema = self.general_settings();
        let get = |key: &str| {
            let setting = schema
                .iter()
                .find(|s| s.key == key)
                .expect("a declared setting");
            values.value(setting)
        };
        let on = |key: &str| get(key) == "true";
        let mut problems = Vec::new();
        let mut regex = |label: &str, pattern: &str| -> Option<Regex> {
            if pattern.trim().is_empty() {
                return None;
            }
            Regex::new(pattern)
                .map_err(|e| problems.push(format!("{label}: {e}")))
                .ok()
        };
        let levels = |text: String| -> Vec<String> {
            text.split(" > ")
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        };
        let to = |text: &str| match text {
            "custom note" => To::Custom,
            "daily note" => To::Daily,
            _ => To::Here,
        };
        let mut sections: Vec<String> = Vec::new();
        for line in text.lines().map(str::trim) {
            if let Some(name) = line
                .strip_prefix("[rule.")
                .and_then(|l| l.strip_suffix(']'))
            {
                sections.push(format!("rule.{}", name.trim()));
            }
        }
        let mut rules = Vec::new();
        for section in &sections {
            let r = |key: &str| values.get(section, key).unwrap_or_default().to_string();
            rules.push(Rule {
                statuses: r("statuses"),
                path: regex(&format!("{section}: path"), &r("path")),
                text: regex(&format!("{section}: text"), &r("text")),
                delete: r("action") == "delete",
                to: match r("to").as_str() {
                    "" => To::Custom,
                    t => to(t),
                },
                file: r("file"),
                date_format: match r("date_format") {
                    f if f.trim().is_empty() => "YYYY-MM-DD".into(),
                    f => f,
                },
            });
        }
        let replace = on("replace")
            .then(|| regex("Replace", &get("replace_regex")).map(|re| (re, get("replace_with"))));
        self.cfg = Config {
            to: to(&get("archive_to")),
            file: get("archive_file"),
            under_heading: on("archive_under_heading"),
            headings: levels(get("headings")),
            depth: get("heading_level").parse().unwrap_or(1).clamp(1, 6),
            newlines: on("blank_lines"),
            under_list_items: on("under_list_items"),
            list_items: levels(get("list_items")),
            date_format: get("date_format"),
            completed_format: get("completed_date_format"),
            newest_first: get("order") == "newest first",
            alphabetical: on("sort_alphabetically"),
            only_if_done: on("only_if_subtasks_done"),
            all_checked: on("all_checked"),
            pattern: regex("Task pattern", &get("task_pattern")),
            replace: replace.flatten(),
            metadata: on("add_metadata").then(|| (get("metadata"), get("metadata_date_format"))),
            rules,
            indent: match get("indent").as_str() {
                "2 spaces" => "  ".into(),
                "4 spaces" => "    ".into(),
                _ => "\t".into(),
            },
        };
        self.rules = sections;
        self.problem = (!problems.is_empty()).then(|| problems.join("; "));
    }

    fn general_settings(&self) -> Vec<Setting> {
        let text = |key: &str, label: &str, help: &str, default: &str| {
            Setting::new("", key, label, help, Kind::Text, default)
        };
        let toggle = |key: &str, label: &str, help: &str, default: &str| {
            Setting::new("", key, label, help, Kind::Toggle, default)
        };
        let choice = |key: &str, label: &str, help: &str, values: &[&str]| {
            let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
            let default = values[0].clone();
            Setting::new("", key, label, help, Kind::Choice(values), &default)
        };
        vec![
            choice(
                "archive_to",
                "Archive to",
                "This note (under the archive heading), a note of its own, or today's daily note",
                &PLACES,
            ),
            text(
                "archive_file",
                "Archive note",
                "The note archived tasks go to (placeholders like {{sourceFileName}}; .md added)",
                "{{sourceFileName}} (archive)",
            ),
            toggle(
                "archive_under_heading",
                "Archive under a heading in that note",
                "In a note of its own, under the headings too (else at its top)",
                "true",
            ),
            text(
                "headings",
                "Archive headings",
                "The archive heading, and headings in it after ' > ' (placeholders allowed)",
                "Archived",
            ),
            choice(
                "heading_level",
                "Archive heading level",
                "The level of the first archive heading (#, ## …)",
                &["1", "2", "3", "4", "5", "6"],
            ),
            toggle(
                "blank_lines",
                "Blank lines around archived tasks",
                "A blank line before the archive heading and around its tasks",
                "true",
            ),
            toggle(
                "under_list_items",
                "Archive under list items",
                "Put archived tasks under these list items (a date tree), made when missing",
                "false",
            ),
            text(
                "list_items",
                "List items",
                "The list items, outermost first, after ' > ' ([[{{date:YYYY-[W]WW}}]] > [[{{date}}]])",
                "[[{{date}}]]",
            ),
            text(
                "date_format",
                "Date format",
                "The format of {{date}} (a format after a colon wins: {{date:DD.MM}})",
                "YYYY-MM-DD",
            ),
            text(
                "completed_date_format",
                "Done date format",
                "The format of {{completedDate}}, the task's ✅ date",
                "YYYY-MM-DD",
            ),
            choice(
                "order",
                "Order",
                "Where new tasks go among the archived ones",
                &["newest last", "newest first"],
            ),
            toggle(
                "sort_alphabetically",
                "Sort alphabetically",
                "Sort the archived tasks (under their heading or list item) by their text",
                "false",
            ),
            toggle(
                "only_if_subtasks_done",
                "Only when subtasks are done",
                "Leave a done task with an open subtask where it is",
                "false",
            ),
            toggle(
                "all_checked",
                "Archive all checked tasks",
                "Any status but a space ([-], [>] …), not only [x]",
                "false",
            ),
            text(
                "task_pattern",
                "Task pattern",
                "Only tasks matching this regular expression (#task); empty: all",
                "",
            ),
            toggle(
                "replace",
                "Replace text before archiving",
                "Replace the regular expression below in archived tasks",
                "false",
            ),
            text(
                "replace_regex",
                "Replace",
                "The regular expression to replace",
                "#([A-Za-z-]+)",
            ),
            text(
                "replace_with",
                "With",
                "What replaces it ($1: its first group)",
                "@$1",
            ),
            toggle(
                "add_metadata",
                "Add text after archived tasks",
                "Add the text below after each archived task (when, from where)",
                "true",
            ),
            text(
                "metadata",
                "Text after tasks",
                "Placeholders: {{date}}, {{heading}}, {{headingChain}}, {{sourceFileName}} …",
                "🔒 [[{{date}}]] 🕸️ {{headingChain}}",
            ),
            text(
                "metadata_date_format",
                "Its date format",
                "The format of {{date}} in that text",
                "YYYY-MM-DD",
            ),
            choice(
                "indent",
                "Indentation",
                "For new list levels when the note has no indented list to copy",
                &["tab", "2 spaces", "4 spaces"],
            ),
        ]
    }

    /// Where it runs: now, the note (path in the vault, without `.md`),
    /// today's daily note.
    fn env(vault: &Vault, path: Option<&Path>) -> Env {
        let now = chrono::Local::now().naive_local();
        let rel = |p: &Path| {
            p.strip_prefix(&vault.root)
                .unwrap_or(p)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/")
        };
        let periodic = std::fs::read_to_string(super::settings_file(vault, "periodic-notes"))
            .unwrap_or_default();
        let daily = &super::periodic::parse_settings(&periodic)[0];
        let name = super::moment::format(&now, &daily.format);
        let daily = Path::new(&daily.folder).join(name);
        Env {
            now,
            source: path.map_or_else(|| "Untitled".into(), rel),
            daily: daily.to_string_lossy().replace('\\', "/"),
        }
    }

    /// The note `rel` (without `.md`) in the vault, if it's in it.
    fn note_path(vault: &Vault, rel: &str) -> Option<PathBuf> {
        let rel = Path::new(rel.trim_start_matches('/'));
        rel.components()
            .all(|c| matches!(c, Component::Normal(_)))
            .then(|| vault.root.join(format!("{}.md", rel.display())))
    }

    /// The effects of an archiving: other notes changed (or made), this
    /// note's lines replaced, a message.
    fn apply(
        &self,
        ctx: &Context,
        lines: &[String],
        archived: Archived,
        cursor: (usize, usize),
        message: String,
    ) -> Effect {
        let mut effects = Vec::new();
        for (rel, placed) in archived.elsewhere {
            let Some(path) = Self::note_path(ctx.vault, &rel) else {
                return Effect::Message(format!("Task Archiver: {rel} is outside the vault"));
            };
            let old = ctx
                .vault
                .notes
                .iter()
                .find(|n| n.path == path)
                .map(|n| n.lines.clone())
                .or_else(|| std::fs::read_to_string(&path).ok().map(|t| split_lines(&t)));
            effects.push(match old {
                Some(old) => Effect::EditNote {
                    path,
                    from: 0,
                    to: old.len(),
                    lines: archive::merge(&old, &placed, &self.cfg, true),
                    expect: old,
                },
                None => Effect::WriteFile {
                    path,
                    text: archive::merge(&[], &placed, &self.cfg, true).join("\n"),
                },
            });
        }
        if archived.here != lines {
            let row = cursor.0.min(archived.here.len().saturating_sub(1));
            effects.push(Effect::ReplaceLines {
                from: 0,
                to: lines.len(),
                lines: archived.here,
                cursor: (row, cursor.1),
            });
        }
        effects.push(Effect::Message(message));
        Effect::Many(effects)
    }

    /// Adds a rule (off until it has a note, or deletes) to the settings.
    fn add_rule(&mut self, vault: &Vault) -> Effect {
        let file = super::settings_file(vault, ID);
        let mut values = Values::parse(&std::fs::read_to_string(&file).unwrap_or_default());
        let n = (1..)
            .find(|n| !self.rules.contains(&format!("rule.{n}")))
            .expect("a free number");
        let section = format!("rule.{n}");
        for (key, value) in [
            ("statuses", ""),
            ("path", ""),
            ("text", ""),
            ("action", "move"),
            ("to", "custom note"),
            ("file", ""),
            ("date_format", "YYYY-MM-DD"),
        ] {
            values.set(&section, key, value);
        }
        let dir = file.parent().expect("a settings file is in a folder");
        let written = std::fs::create_dir_all(dir)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                mdedit::files::write_atomic(&file, &values.to_text(&self.settings()))
                    .map_err(|e| e.to_string())
            });
        match written {
            Ok(()) => {
                self.load(vault);
                Effect::Message(format!(
                    "Task Archiver: rule {n} added; give it a note (or Delete) in its settings"
                ))
            }
            Err(e) => Effect::Message(format!("Task Archiver: cannot save: {e}")),
        }
    }
}

impl Default for Archiver {
    fn default() -> Self {
        Self::new()
    }
}

/// `n task` / `n tasks`.
fn tasks(n: usize) -> String {
    if n == 1 {
        "1 task".into()
    } else {
        format!("{n} tasks")
    }
}

impl Plugin for Archiver {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Task Archiver",
            version: "0.1.0",
            author: "blackglass, after Ivan Lednev's Task Archiver",
            description: "Done tasks move under an Archived heading (in the note, another note or \
                          today's daily note), in a date tree if you like; delete done tasks; \
                          archive a heading; sort a list by done-ness; rules by status, path, text.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.load(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        self.load(vault);
    }

    fn on_note_changed(&mut self, _path: &Path, _vault: &Vault) {}

    fn settings(&self) -> Vec<Setting> {
        let mut all = self.general_settings();
        for section in &self.rules {
            let n = section.trim_start_matches("rule.");
            let s = |key: &str, label: &str, help: &str, kind: Kind, default: &str| {
                Setting::new(
                    section,
                    key,
                    &format!("Rule {n}: {label}"),
                    help,
                    kind,
                    default,
                )
            };
            all.extend([
                s(
                    "statuses",
                    "statuses",
                    "Tasks with these status symbols (>-; empty: any), archived even if not done",
                    Kind::Text,
                    "",
                ),
                s(
                    "path",
                    "note path",
                    "Only in notes whose path matches this regular expression (empty: any)",
                    Kind::Text,
                    "",
                ),
                s(
                    "text",
                    "task text",
                    "Only tasks whose text matches this regular expression (empty: any)",
                    Kind::Text,
                    "",
                ),
                s(
                    "action",
                    "action",
                    "Move the tasks to its note, or delete them",
                    Kind::Choice(vec!["move".into(), "delete".into()]),
                    "move",
                ),
                s(
                    "to",
                    "move to",
                    "A note of its own (below), today's daily note, or this note",
                    Kind::Choice(vec![
                        "custom note".into(),
                        "daily note".into(),
                        "this note".into(),
                    ]),
                    "custom note",
                ),
                s(
                    "file",
                    "note",
                    "Its note (placeholders allowed; empty and not Delete: the rule is off)",
                    Kind::Text,
                    "",
                ),
                s(
                    "date_format",
                    "date format",
                    "The format of {{date}} in its note's name",
                    Kind::Text,
                    "YYYY-MM-DD",
                ),
            ]);
        }
        all.push(Setting::new(
            "rules",
            "add",
            "Add a rule",
            "Tasks matching a rule (by status, note path, text) go to its note, or are deleted",
            Kind::Action("add-rule"),
            "",
        ));
        all
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("archive-tasks", "Archive tasks in this file"),
            PluginCommand::new(
                "archive-tasks-deeply",
                "Archive tasks including nested tasks in this file",
            ),
            PluginCommand::new("delete-tasks", "Delete tasks in this file"),
            PluginCommand::new(
                "archive-heading-under-cursor",
                "Archive heading under cursor",
            ),
            PluginCommand::new(
                "sort-tasks-in-list-under-cursor",
                "Sort tasks in list under cursor",
            ),
            PluginCommand::new("toggle-done-and-archive", "Toggle task done and archive it"),
            PluginCommand::new("add-rule", "Add an archiving rule"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        if id == "add-rule" {
            return self.add_rule(ctx.vault);
        }
        if let Some(problem) = &self.problem {
            return Effect::Message(format!("Task Archiver: check the settings: {problem}"));
        }
        let Some(note) = ctx.note else {
            return Effect::Message("Task Archiver: open a note first".into());
        };
        // As the editor has them, a last empty line too.
        let lines: Vec<String> = note
            .text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
            .collect();
        let env = Self::env(ctx.vault, note.path);
        let cursor = (note.row, note.col);
        let fail = |e: String| Effect::Message(format!("Task Archiver: {e}"));
        match id {
            "archive-tasks" | "archive-tasks-deeply" => {
                let out =
                    archive::archive_tasks(&lines, &self.cfg, &env, id == "archive-tasks-deeply");
                let message = match out.count {
                    0 => "Task Archiver: no tasks to archive".to_string(),
                    n => format!("Task Archiver: archived {}", tasks(n)),
                };
                self.apply(ctx, &lines, out, cursor, message)
            }
            "delete-tasks" => match archive::delete_tasks(&lines, &self.cfg) {
                (_, 0) => Effect::Message("Task Archiver: no tasks to delete".into()),
                (left, n) => {
                    let out = Archived {
                        here: left,
                        ..Archived::default()
                    };
                    self.apply(
                        ctx,
                        &lines,
                        out,
                        cursor,
                        format!("Task Archiver: deleted {}", tasks(n)),
                    )
                }
            },
            "archive-heading-under-cursor" => {
                match archive::archive_heading(&lines, note.row, &self.cfg, &env) {
                    Ok(out) => {
                        let message = "Task Archiver: archived the heading".to_string();
                        self.apply(ctx, &lines, out, cursor, message)
                    }
                    Err(e) => fail(e),
                }
            }
            "sort-tasks-in-list-under-cursor" => match archive::sort_list(&lines, note.row) {
                Some(sorted) => Effect::ReplaceLines {
                    from: 0,
                    to: lines.len(),
                    lines: sorted,
                    cursor,
                },
                None => fail("put the cursor in a list".into()),
            },
            "toggle-done-and-archive" => {
                match archive::toggle_and_archive(&lines, note.row, &self.cfg, &env) {
                    Ok((out, row)) => {
                        let message = "Task Archiver: archived the task".to_string();
                        self.apply(ctx, &lines, out, (row, 0), message)
                    }
                    Err(e) => fail(e),
                }
            }
            _ => Effect::None,
        }
    }
}
