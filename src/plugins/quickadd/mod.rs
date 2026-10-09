//! QuickAdd (W-146), after Christian B. B. Houmann's plugin. Its
//! **choices**, each a command of its own:
//!
//! - **Captures:** a format ([`format`]) whose questions are asked one
//!   after the other, and whose text then goes into a note: today's (or a
//!   chosen day's) daily note, a note by path (made if missing, from a
//!   template if one is set), a note chosen from a folder or a tag, or the
//!   active note (at the cursor, a new line below or above it); at the top,
//!   the bottom or under a heading. As a task, once per line typed, with a
//!   link to it and the note opened after, if asked.
//! - **Templates:** a new note from a template, its name and folder from
//!   formats; a name that's taken gets a number, or the note is opened or
//!   added to.
//!
//! Choices can be grouped (a **multi**: "Run QuickAdd" shows the group,
//! then its choices). **Global variables** are snippets put in by
//! `{{GLOBAL_VAR:name}}`. Everything is in the plugin's settings, a
//! section a choice:
//!
//! ```toml
//! [Expense]
//! format = "- [[{{FILE:Budget/Envelopes}}]] {{VALUE:amount}} {{VALUE:what}}"
//! heading = "Money"
//!
//! [Global variables]
//! sig = "— {{DATE}}"
//! ```

pub mod format;

use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDateTime;

use super::settings::{Kind, Setting, Values};
use super::{
    Answer, Context, Effect, Manifest, Place, Plugin, PluginCommand, Question, intern, slug,
};
use crate::vault::Vault;
use format::Part;

const ID: &str = "quickadd";

/// The settings section of the global variables.
const GLOBALS: &str = "Global variables";

/// Where a capture's text goes in its note.
const PLACES: [&str; 6] = [
    "under heading",
    "top",
    "bottom",
    "cursor",
    "line below cursor",
    "line above cursor",
];

/// What a template choice does when its note is there already.
const EXISTS: [&str; 3] = ["increment", "open", "append"];

/// A choice's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sort {
    Capture,
    Template,
}

/// A choice, as its settings say.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Choice {
    name: String,
    sort: Sort,
    /// The multi it's in (`""`: none).
    group: String,
    // A capture's.
    format: String,
    /// The note (a path in the format syntax, a folder ending in `/` or
    /// `#tag` to choose from); empty: the daily note.
    note: String,
    active: bool,
    place: String,
    heading: String,
    first: bool,
    make_at_top: bool,
    create: bool,
    /// A new note's template (a path in the vault).
    template: String,
    task: bool,
    per_line: bool,
    ask_day: bool,
    link: bool,
    open: bool,
    // A template choice's (its template is `template`).
    file_name: String,
    folder: String,
    exists: String,
}

impl Choice {
    /// The choice in section `name`, if it is one (it has a format or a
    /// type).
    fn read(values: &Values, name: &str) -> Option<Choice> {
        let get =
            |key: &str, default: &str| values.get(name, key).unwrap_or(default).trim().to_string();
        let on = |key: &str, default: bool| match values.get(name, key) {
            Some(v) => v.trim() == "true",
            None => default,
        };
        let sort = match values.get(name, "type").map(str::trim) {
            Some("template") => Sort::Template,
            Some(_) => Sort::Capture,
            None => {
                values.get(name, "format")?;
                Sort::Capture
            }
        };
        Some(Choice {
            name: name.to_string(),
            sort,
            group: get("group", ""),
            format: values
                .get(name, "format")
                .map(String::from)
                .filter(|f| !f.trim().is_empty())
                .unwrap_or_else(|| "{{VALUE}}".into()),
            note: get("note", ""),
            active: on("active", false),
            place: get("place", PLACES[0]),
            heading: get("heading", "")
                .trim_start_matches('#')
                .trim()
                .to_string(),
            first: on("first", false),
            make_at_top: get("heading_missing", "bottom") == "top",
            create: on("create", true),
            template: get("template", ""),
            task: on("task", false),
            per_line: on("per_line", false),
            ask_day: get("day", "today") == "ask",
            link: on("link", false),
            open: on("open", sort == Sort::Template),
            file_name: get("file_name", ""),
            folder: get("folder", ""),
            exists: get("exists", EXISTS[0]),
        })
    }

    fn command(&self) -> String {
        match self.sort {
            Sort::Capture => format!("capture-{}", slug(&self.name)),
            Sort::Template => format!("template-{}", slug(&self.name)),
        }
    }
}

/// An entry of "Run QuickAdd"'s list.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    Choice(usize),
    Group(String),
}

/// What the answers coming back are for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// One of these entries.
    Pick(Vec<Entry>),
    /// The name of a new capture, template choice or global variable.
    Name(&'static str),
    /// Choice `i`'s questions: first the day (`day`), then a note to
    /// choose (its items), then the formats' (keys, with a choice's
    /// items).
    Fill {
        i: usize,
        day: bool,
        notes: Option<Vec<String>>,
        keys: Vec<(String, Option<Vec<String>>)>,
    },
}

pub struct QuickAdd {
    choices: Vec<Choice>,
    globals: HashMap<String, String>,
    /// The global variables in file order (their settings rows).
    global_names: Vec<String>,
    step: Option<Step>,
}

impl QuickAdd {
    pub fn new() -> Self {
        QuickAdd {
            choices: Vec::new(),
            globals: HashMap::new(),
            global_names: Vec::new(),
            step: None,
        }
    }

    /// A note's text by its path in the vault (`.md` optional).
    fn read(vault: &Vault, path: &str) -> Option<String> {
        let path = path.trim().trim_matches('/');
        let path = path.strip_suffix(".md").unwrap_or(path);
        vault
            .notes
            .iter()
            .find(|n| {
                let rel = n.rel_name().replace('\\', "/");
                rel.strip_suffix(".md")
                    .unwrap_or(&rel)
                    .eq_ignore_ascii_case(path)
            })
            .map(|n| n.lines.join("\n"))
    }

    /// `text` with the global variables and templates put in, as parts.
    fn parts(&self, text: &str, vault: &Vault) -> Vec<Part> {
        let read = |p: &str| Self::read(vault, p);
        format::parse(&format::include(text, &self.globals, &read))
    }

    /// Choice `i`'s formats as parts: its text (a capture's format, a
    /// template choice's template), its note's path or file name, and (a
    /// template choice) its folder.
    fn formats(&self, i: usize, vault: &Vault) -> [Vec<Part>; 3] {
        let c = &self.choices[i];
        match c.sort {
            Sort::Capture => [
                self.parts(&c.format, vault),
                self.parts(&c.note, vault),
                Vec::new(),
            ],
            Sort::Template => [
                self.parts(&Self::read(vault, &c.template).unwrap_or_default(), vault),
                self.parts(&c.file_name, vault),
                self.parts(&c.folder, vault),
            ],
        }
    }

    /// The notes a capture's note setting chooses from: in a folder
    /// (`Folder/`, or a folder's path), with a tag (`#tag`).
    fn note_choices(c: &Choice, vault: &Vault) -> Option<Vec<String>> {
        if c.active || c.sort == Sort::Template {
            return None;
        }
        let note = c.note.trim();
        if let Some(tag) = note.strip_prefix('#') {
            let mut found: Vec<String> = vault
                .notes
                .iter()
                .filter(|n| n.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
                .map(|n| n.rel_name().replace('\\', "/"))
                .map(|r| r.trim_end_matches(".md").to_string())
                .collect();
            found.sort_by_key(|r| r.to_lowercase());
            return Some(found);
        }
        let folder = note.trim_end_matches('/');
        let is_folder =
            note.ends_with('/') || (!note.is_empty() && vault.root.join(folder).is_dir());
        is_folder.then(|| {
            format::notes_in(vault, folder)
                .into_iter()
                .map(|n| format!("{folder}/{n}"))
                .collect()
        })
    }

    /// Asks choice `i`'s questions, or does it at once without any.
    fn start(&mut self, i: usize, ctx: &Context) -> Effect {
        let c = self.choices[i].clone();
        if c.sort == Sort::Template && c.template.is_empty() {
            return Effect::Message(format!(
                "QuickAdd: {} has no template; set it in Settings → QuickAdd",
                c.name
            ));
        }
        if c.active && ctx.note.is_none() {
            return Effect::Message("QuickAdd: open a note first".into());
        }
        // A template choice's name and folder are asked before its text.
        let [text, b, d] = self.formats(i, ctx.vault);
        let mut asked = match c.sort {
            Sort::Capture => format::questions(&[&text, &b], ctx.vault),
            Sort::Template => format::questions(&[&b, &d, &text], ctx.vault),
        };
        if c.sort == Sort::Template
            && c.file_name.trim().is_empty()
            && !asked.iter().any(|(k, _)| k == "value:")
        {
            asked.insert(
                0,
                (
                    "value:".into(),
                    Question::Text {
                        prompt: "Name of the new note".into(),
                        default: String::new(),
                    },
                ),
            );
        }
        // One entry per line: the value takes several lines.
        if c.per_line
            && let Some((_, q)) = asked.iter_mut().find(|(k, _)| k == "value:")
            && let Question::Text { prompt, default } = q
        {
            *q = Question::Lines {
                prompt: std::mem::take(prompt),
                default: std::mem::take(default),
            };
        }
        if let Some((_, Question::Choose { prompt, .. })) = asked
            .iter()
            .find(|(_, q)| matches!(q, Question::Choose { items, .. } if items.is_empty()))
        {
            return Effect::Message(format!("QuickAdd: nothing to choose for \"{prompt}\""));
        }
        let notes = Self::note_choices(&c, ctx.vault);
        if notes.as_ref().is_some_and(Vec::is_empty) {
            return Effect::Message(format!("QuickAdd: no notes in {}", c.note));
        }
        let mut questions = Vec::new();
        if c.ask_day {
            questions.push(Question::Text {
                prompt: "Which day (2026-10-20, yesterday, last friday)".into(),
                default: "today".into(),
            });
        }
        if let Some(items) = &notes {
            questions.push(Question::Choose {
                prompt: "Into which note".into(),
                items: items.clone(),
            });
        }
        let keys = asked
            .iter()
            .map(|(k, q)| {
                let items = match q {
                    Question::Choose { items, .. } | Question::Many { items, .. } => {
                        Some(items.clone())
                    }
                    _ => None,
                };
                (k.clone(), items)
            })
            .collect();
        questions.extend(asked.into_iter().map(|(_, q)| q));
        let step = Step::Fill {
            i,
            day: c.ask_day,
            notes,
            keys,
        };
        if questions.is_empty() {
            return self.answered(step, &[], ctx);
        }
        self.step = Some(step);
        Effect::Ask(questions)
    }

    /// The answers to a choice's questions came: do it.
    fn answered(&mut self, step: Step, answers: &[Answer], ctx: &Context) -> Effect {
        let Step::Fill {
            i,
            day,
            notes,
            keys,
        } = step
        else {
            return Effect::None;
        };
        let mut answers = answers.iter();
        let now = chrono::Local::now().naive_local();
        let when = if day {
            let typed = match answers.next() {
                Some(Answer::Text(t)) => t.clone(),
                _ => String::new(),
            };
            match crate::nldates::parse_date(&typed, now, chrono::Weekday::Mon) {
                Some(d) => d.and_time(now.time()),
                None => {
                    return Effect::Message(format!("QuickAdd: can't read the day {typed:?}"));
                }
            }
        } else {
            now
        };
        let picked = match &notes {
            Some(items) => match answers.next() {
                Some(Answer::Choice(n)) => items.get(*n).cloned(),
                _ => None,
            },
            None => None,
        };
        let filled: HashMap<String, String> = keys
            .iter()
            .zip(answers)
            .map(|((key, items), answer)| {
                let text = match (answer, items) {
                    (Answer::Choice(n), Some(items)) => items.get(*n).cloned().unwrap_or_default(),
                    (Answer::Choices(ns), Some(items)) => ns
                        .iter()
                        .filter_map(|n| items.get(*n).cloned())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    (Answer::Text(t), _) => t.clone(),
                    _ => String::new(),
                };
                (key.clone(), text)
            })
            .collect();
        match self.choices[i].sort {
            Sort::Capture => self.capture(i, when, picked, &filled, ctx),
            Sort::Template => self.create(i, when, &filled, ctx),
        }
    }

    /// The format's environment: the active note, the moment, the note
    /// written to (its path in the vault without `.md`).
    fn env(ctx: &Context, now: NaiveDateTime, target: &str) -> format::Env {
        let root = &ctx.vault.root;
        let rel = |p: &Path| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/")
        };
        let section = ctx.note.and_then(|n| {
            n.text
                .lines()
                .take(n.row + 1)
                .filter(|l| l.starts_with('#') && l.trim_start_matches('#').starts_with(' '))
                .last()
                .map(|l| l.trim_start_matches('#').trim().to_string())
        });
        let (folder, title) = target.rsplit_once('/').unwrap_or(("", target));
        format::Env {
            now,
            current: ctx.note.and_then(|n| n.path).map(rel),
            section,
            selected: ctx.note.and_then(|n| n.selection).map(String::from),
            title: title.to_string(),
            folder: folder.to_string(),
        }
    }

    /// A capture's text into its note.
    fn capture(
        &self,
        i: usize,
        when: NaiveDateTime,
        picked: Option<String>,
        answers: &HashMap<String, String>,
        ctx: &Context,
    ) -> Effect {
        let c = &self.choices[i];
        let [line_parts, note_parts, _] = self.formats(i, ctx.vault);
        let root = &ctx.vault.root;
        let bare = |p: &str| {
            let p = p.trim().trim_matches('/');
            p.strip_suffix(".md").unwrap_or(p).to_string()
        };
        // Where it goes (its path in the vault without `.md`; `None`: the
        // daily note).
        let target: Option<String> = if c.active {
            ctx.note
                .and_then(|n| n.path)
                .and_then(|p| p.strip_prefix(root).ok())
                .map(|p| bare(&p.to_string_lossy().replace('\\', "/")))
        } else if let Some(p) = &picked {
            Some(bare(p))
        } else if c.note.trim().is_empty() {
            None
        } else {
            let env = Self::env(ctx, when, "");
            Some(bare(&format::fill(&note_parts, answers, &env)))
        };
        let shown = target
            .clone()
            .unwrap_or_else(|| when.format("%Y-%m-%d").to_string());
        let env = Self::env(ctx, when, &shown);
        // What goes: once per line typed, as a task.
        let typed = answers.get("value:").cloned().unwrap_or_default();
        let mut text = if c.per_line && typed.contains('\n') {
            typed
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let mut one = answers.clone();
                    one.insert("value:".into(), l.to_string());
                    format::fill(&line_parts, &one, &env)
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            format::fill(&line_parts, answers, &env)
        };
        if c.task {
            text = text
                .lines()
                .map(|l| {
                    let t = l.trim_start();
                    let indent = &l[..l.len() - t.len()];
                    if t.starts_with("- [") {
                        l.to_string()
                    } else {
                        let t = t.strip_prefix("- ").unwrap_or(t);
                        format!("{indent}- [ ] {t}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
        // At the cursor or by it, in the active note.
        if c.active && c.place.contains("cursor") {
            let note = ctx.note.expect("checked when it started");
            if c.place == "cursor" {
                let back = text
                    .split_once(format::CURSOR)
                    .map_or(0, |(_, after)| after.chars().count());
                return Effect::Insert {
                    text: text.replace(format::CURSOR, ""),
                    back,
                };
            }
            let row = if c.place == "line below cursor" {
                note.row + 1
            } else {
                note.row
            };
            let lines: Vec<String> = text
                .replace(format::CURSOR, "")
                .lines()
                .map(String::from)
                .collect();
            let col = lines.last().map_or(0, |l| l.chars().count());
            let last = row + lines.len().saturating_sub(1);
            return Effect::ReplaceLines {
                from: row,
                to: row,
                lines,
                cursor: (last, col),
            };
        }
        let text = text.replace(format::CURSOR, "");
        let path = target.map(|t| root.join(format!("{t}.md")));
        if !c.create && path.as_ref().is_some_and(|p| !p.is_file()) {
            return Effect::Message(format!("QuickAdd: {shown} isn't there (create is off)"));
        }
        let new_text = if c.template.is_empty() {
            String::new()
        } else {
            let template = Self::read(ctx.vault, &c.template).unwrap_or_default();
            format::fill(&self.parts(&template, ctx.vault), answers, &env)
                .replace(format::CURSOR, "")
        };
        let place = match c.place.as_str() {
            "top" => Place::Top,
            "bottom" => Place::Bottom,
            _ => Place::Heading {
                name: c.heading.clone(),
                first: c.first,
                make_at_top: c.make_at_top,
            },
        };
        Effect::AddToNote {
            path,
            day: when.date(),
            place,
            line: text,
            new_text,
            open: c.open,
            link: c.link,
        }
    }

    /// A template choice's new note.
    fn create(
        &self,
        i: usize,
        when: NaiveDateTime,
        answers: &HashMap<String, String>,
        ctx: &Context,
    ) -> Effect {
        let c = &self.choices[i];
        let [body, name_parts, folder_parts] = self.formats(i, ctx.vault);
        let root = &ctx.vault.root;
        let env = Self::env(ctx, when, "");
        let name = if c.file_name.trim().is_empty() {
            answers.get("value:").cloned().unwrap_or_default()
        } else {
            format::fill(&name_parts, answers, &env)
        };
        let name = name.trim().trim_end_matches(".md").to_string();
        if name.is_empty() {
            return Effect::Message("QuickAdd: the new note needs a name".into());
        }
        let folder = if c.folder.trim().is_empty() {
            ctx.folder.to_path_buf()
        } else {
            root.join(
                format::fill(&folder_parts, answers, &env)
                    .trim()
                    .trim_matches('/'),
            )
        };
        let mut path = folder.join(format!("{name}.md"));
        let link = |p: &Path| Effect::Insert {
            text: format!(
                "[[{}]]",
                p.file_stem().unwrap_or_default().to_string_lossy()
            ),
            back: 0,
        };
        let mut effects = Vec::new();
        if path.is_file() {
            match c.exists.as_str() {
                "open" => {
                    if c.link {
                        effects.push(link(&path));
                    }
                    effects.push(Effect::Open { path });
                    return Effect::Many(effects);
                }
                "append" => {
                    let rel = path.strip_prefix(root).unwrap_or(&path).with_extension("");
                    let env = Self::env(ctx, when, &rel.to_string_lossy());
                    return Effect::AddToNote {
                        path: Some(path),
                        day: when.date(),
                        place: Place::Bottom,
                        line: format::fill(&body, answers, &env).replace(format::CURSOR, ""),
                        new_text: String::new(),
                        open: c.open,
                        link: c.link,
                    };
                }
                _ => {
                    // The first free name with a number.
                    let (stem, n) = match name.rsplit_once(' ') {
                        Some((s, n)) if n.parse::<u32>().is_ok() => {
                            (s.to_string(), n.parse::<u32>().unwrap_or(1))
                        }
                        _ => (name.clone(), 0),
                    };
                    path = (n + 1..)
                        .map(|k| folder.join(format!("{stem} {k}.md")))
                        .find(|p| !p.is_file())
                        .expect("some number is free");
                }
            }
        }
        let rel = path.strip_prefix(root).unwrap_or(&path).with_extension("");
        let env = Self::env(ctx, when, &rel.to_string_lossy().replace('\\', "/"));
        let text = format::fill(&body, answers, &env).replace(format::CURSOR, "");
        if c.link {
            effects.push(link(&path));
        }
        effects.push(if c.open {
            Effect::CreateFile { path, text }
        } else {
            Effect::WriteFile { path, text }
        });
        Effect::Many(effects)
    }

    /// Adds a choice (or a global variable) called `name` to the settings.
    fn add(&mut self, vault: &Vault, what: &str, name: &str) -> Effect {
        let name = name.trim();
        if name.is_empty() || name.contains(['[', ']', '"']) || name == GLOBALS {
            return Effect::Message("QuickAdd: that needs a name (without [ ] \")".into());
        }
        let file = super::settings_file(vault, ID);
        let mut values = Values::parse(&std::fs::read_to_string(&file).unwrap_or_default());
        if what == "global" {
            if self.globals.contains_key(name) {
                return Effect::Message(format!("QuickAdd: there's a variable {name} already"));
            }
            values.set(GLOBALS, name, "");
            self.global_names.push(name.into());
            self.globals.insert(name.into(), String::new());
        } else {
            if self
                .choices
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(name))
            {
                return Effect::Message(format!("QuickAdd: there's a choice {name} already"));
            }
            if what == "template" {
                values.set(name, "type", "template");
                values.set(name, "template", "");
            } else {
                values.set(name, "format", "- {{VALUE}}");
            }
            if let Some(c) = Choice::read(&values, name) {
                self.choices.push(c);
            }
        }
        let saved = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| mdedit::files::write_atomic(&file, &values.to_text(&self.settings())));
        match saved {
            Ok(()) => Effect::Message(format!(
                "QuickAdd: added {name}; set it up in Settings → QuickAdd"
            )),
            Err(e) => Effect::Message(format!("QuickAdd: cannot save: {e}")),
        }
    }

    /// "Run QuickAdd"'s list: the choices outside groups and the groups
    /// (`group`: that group's choices).
    fn pick(&mut self, group: Option<&str>) -> Effect {
        let mut entries = Vec::new();
        let mut items = Vec::new();
        for (i, c) in self.choices.iter().enumerate() {
            match group {
                Some(g) if c.group == g => {
                    entries.push(Entry::Choice(i));
                    items.push(c.name.clone());
                }
                None if c.group.is_empty() => {
                    entries.push(Entry::Choice(i));
                    items.push(c.name.clone());
                }
                None if !entries.contains(&Entry::Group(c.group.clone())) => {
                    entries.push(Entry::Group(c.group.clone()));
                    items.push(format!("{} ▸", c.group));
                }
                _ => {}
            }
        }
        self.step = Some(Step::Pick(entries));
        Effect::Ask(vec![Question::Choose {
            prompt: group.map_or("QuickAdd".into(), |g| format!("QuickAdd: {g}")),
            items,
        }])
    }
}

impl Default for QuickAdd {
    fn default() -> Self {
        Self::new()
    }
}

/// The sections of a settings file, in order.
fn sections(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let name = name.trim().to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

impl Plugin for QuickAdd {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "QuickAdd",
            version: "0.2.0",
            author: "blackglass, after Christian B. B. Houmann's QuickAdd",
            description: "Captures: text put together from a few questions (a value, a choice, a \
                          date, a note, a property's values) added to a daily note or another \
                          note; templates: new notes named and placed by formats. Each choice is \
                          a command; choices can be grouped.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        self.choices = sections(&text)
            .iter()
            .filter(|s| *s != GLOBALS)
            .filter_map(|s| Choice::read(&values, s))
            .collect();
        self.global_names = values
            .section(GLOBALS)
            .map(|(k, _)| k.to_string())
            .collect();
        self.globals = values
            .section(GLOBALS)
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    }

    fn commands(&self) -> Vec<PluginCommand> {
        let mut all = vec![
            PluginCommand::new("run", "Run QuickAdd"),
            PluginCommand::new("add-capture", "Add a capture"),
            PluginCommand::new("add-template", "Add a template choice"),
            PluginCommand::new("add-global", "Add a global variable"),
        ];
        for c in &self.choices {
            all.push(PluginCommand::new(
                intern(c.command()),
                intern(c.name.clone()),
            ));
        }
        all
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        self.step = None;
        match id {
            "run" if self.choices.is_empty() => {
                Effect::Message("QuickAdd: no choices yet; add one in Settings → QuickAdd".into())
            }
            "run" => self.pick(None),
            "add-capture" | "add-template" | "add-global" => {
                let (what, prompt) = match id {
                    "add-capture" => ("capture", "Name of the capture"),
                    "add-template" => ("template", "Name of the template choice"),
                    _ => ("global", "Name of the variable ({{GLOBAL_VAR:name}})"),
                };
                self.step = Some(Step::Name(what));
                Effect::Ask(vec![Question::Text {
                    prompt: prompt.into(),
                    default: String::new(),
                }])
            }
            _ => match self.choices.iter().position(|c| c.command() == id) {
                Some(i) => self.start(i, ctx),
                None => Effect::None,
            },
        }
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        match (self.step.take(), answers) {
            (Some(Step::Pick(entries)), [Answer::Choice(n)]) => match entries.get(*n).cloned() {
                Some(Entry::Choice(i)) => self.start(i, ctx),
                Some(Entry::Group(g)) => self.pick(Some(&g)),
                None => Effect::None,
            },
            (Some(Step::Name(what)), [Answer::Text(name)]) => self.add(ctx.vault, what, name),
            (Some(step @ Step::Fill { .. }), answers) => self.answered(step, answers, ctx),
            _ => Effect::None,
        }
    }

    fn settings(&self) -> Vec<Setting> {
        let mut all = Vec::new();
        for c in &self.choices {
            let name = &c.name;
            let row = |key: &str, label: &str, help: &str, kind: Kind, default: &str| {
                Setting::new(name, key, &format!("{name}: {label}"), help, kind, default)
            };
            let text = |key: &str, help: &str| row(key, key, help, Kind::Text, "");
            let toggle =
                |key: &str, help: &str, default: &str| row(key, key, help, Kind::Toggle, default);
            let choice = |key: &str, help: &str, items: &[&str]| {
                let items: Vec<String> = items.iter().map(|s| s.to_string()).collect();
                let first = items[0].clone();
                row(key, key, help, Kind::Choice(items), &first)
            };
            match c.sort {
                Sort::Capture => all.extend([
                    text(
                        "format",
                        "What's added: {{VALUE}}, {{VALUE:a,b}}, {{DATE}}, {{FIELD:x}}, {{FILE:folder}} … (empty: {{VALUE}})",
                    ),
                    text(
                        "note",
                        "The note it goes in: a path (formats work), Folder/ or #tag to choose one; empty: the daily note",
                    ),
                    toggle(
                        "active",
                        "Into the note you're in instead (at the cursor, by it, or where \"place\" says)",
                        "false",
                    ),
                    choice(
                        "place",
                        "Under the heading, at the top or bottom, or (the active note) at the cursor or on a new line by it",
                        &PLACES,
                    ),
                    text(
                        "heading",
                        "The heading it goes under (empty: the note's end)",
                    ),
                    toggle(
                        "first",
                        "Right under the heading, newest first (off: at the end of its list)",
                        "false",
                    ),
                    choice(
                        "heading_missing",
                        "Where the heading is made when the note hasn't got it",
                        &["bottom", "top"],
                    ),
                    toggle(
                        "create",
                        "Make the note when it isn't there (off: say so instead)",
                        "true",
                    ),
                    text(
                        "template",
                        "A note made for the capture starts as this template (a path; empty: empty)",
                    ),
                    toggle("task", "Each line added as a task (- [ ])", "false"),
                    toggle(
                        "per_line",
                        "A {{VALUE}} of several lines adds the format once for each line",
                        "false",
                    ),
                    choice(
                        "day",
                        "The day its dates and the daily note are about: today, or ask each time",
                        &["today", "ask"],
                    ),
                    toggle(
                        "link",
                        "Put a link to the note written to at the cursor of the note you're in",
                        "false",
                    ),
                    toggle("open", "Open the note written to afterwards", "false"),
                    text(
                        "group",
                        "\"Run QuickAdd\" lists it under this group (empty: none)",
                    ),
                ]),
                Sort::Template => all.extend([
                    text(
                        "template",
                        "The template note (a path in the vault); its {{…}} are asked too",
                    ),
                    text(
                        "file_name",
                        "The new note's name, in the format syntax ({{DATE}} {{VALUE:title}}); empty: ask",
                    ),
                    text(
                        "folder",
                        "Where it goes (formats work); empty: the folder chosen in the explorer",
                    ),
                    choice(
                        "exists",
                        "When the name is taken: a number added, open that note, or add the template to it",
                        &EXISTS,
                    ),
                    toggle("open", "Open the new note", "true"),
                    toggle(
                        "link",
                        "Put a link to the new note at the cursor of the note you're in",
                        "false",
                    ),
                    text(
                        "group",
                        "\"Run QuickAdd\" lists it under this group (empty: none)",
                    ),
                ]),
            }
        }
        for name in &self.global_names {
            all.push(Setting::new(
                GLOBALS,
                name,
                &format!("{{{{GLOBAL_VAR:{name}}}}}"),
                "The text {{GLOBAL_VAR:name}} puts in (formats in it work too)",
                Kind::Text,
                "",
            ));
        }
        for (key, label, help, action) in [
            (
                "add",
                "Add a capture",
                "A new capture: a name, then its settings here",
                "add-capture",
            ),
            (
                "add-template",
                "Add a template choice",
                "A new note from a template: a name, then its settings here",
                "add-template",
            ),
            (
                "add-global",
                "Add a global variable",
                "A snippet any format puts in with {{GLOBAL_VAR:name}}",
                "add-global",
            ),
        ] {
            all.push(Setting::new("", key, label, help, Kind::Action(action), ""));
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    fn ctx(vault: &Vault) -> Context<'_> {
        Context {
            vault,
            note: None,
            folder: &vault.root,
            name: None,
            query: "",
        }
    }

    #[test]
    fn choices_groups_and_globals_come_from_the_settings() {
        let dir = scratch("quickadd-settings");
        write(
            &dir,
            &[(
                ".blackglass/plugins/quickadd/settings.toml",
                "[Expense]\nformat = \"- {{VALUE}}\"\nheading = \"Money\"\ngroup = \"Log\"\n\n[Daily log]\nformat = \"- {{DATE:HH:mm}} {{VALUE}} {{GLOBAL_VAR:sig}}\"\nnote = \"Logs/log\"\n\n[Meeting]\ntype = \"template\"\ntemplate = \"Templates/Meeting.md\"\nfile_name = \"{{DATE}} {{VALUE:topic}}\"\nfolder = \"Meetings\"\n\n[Global variables]\nsig = \"(by me)\"\n",
            )],
        );
        let vault = Vault::open(&dir).unwrap();
        let mut plugin = QuickAdd::new();
        plugin.on_load(&vault);
        let names: Vec<_> = plugin.commands().iter().map(|c| (c.id, c.name)).collect();
        assert_eq!(
            names[4..],
            [
                ("capture-expense", "Expense"),
                ("capture-daily-log", "Daily log"),
                ("template-meeting", "Meeting"),
            ]
        );
        let ctx = ctx(&vault);
        // "Run QuickAdd": the groups, then their choices.
        let Effect::Ask(q) = plugin.run("run", &ctx) else {
            panic!("a question")
        };
        assert!(
            matches!(&q[0], Question::Choose { items, .. } if items == &["Log ▸", "Daily log", "Meeting"])
        );
        let Effect::Ask(q) = plugin.answer("run", &[Answer::Choice(0)], &ctx) else {
            panic!("a question")
        };
        assert!(matches!(&q[0], Question::Choose { items, .. } if items == &["Expense"]));
        // A capture: its line, the global variable put in.
        assert!(matches!(plugin.run("capture-daily-log", &ctx), Effect::Ask(q) if q.len() == 1));
        let done = plugin.answer("capture-daily-log", &[Answer::Text("walk".into())], &ctx);
        let Effect::AddToNote { path, line, .. } = done else {
            panic!("{done:?}")
        };
        assert_eq!(path, Some(vault.root.join("Logs/log.md")));
        assert!(line.ends_with(" walk (by me)"), "{line}");
        // New choices are saved and become commands.
        plugin.run("add-capture", &ctx);
        plugin.answer("add-capture", &[Answer::Text("Habit".into())], &ctx);
        plugin.run("add-global", &ctx);
        plugin.answer("add-global", &[Answer::Text("team".into())], &ctx);
        plugin.on_vault_changed(&vault);
        assert!(plugin.commands().iter().any(|c| c.name == "Habit"));
        assert!(
            plugin
                .settings()
                .iter()
                .any(|s| s.label == "{{GLOBAL_VAR:team}}")
        );
    }
}
