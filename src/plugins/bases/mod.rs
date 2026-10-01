//! Bases: database views of the vault's notes, after Obsidian's core Bases
//! plugin (`requirements/bases_requirements.md`). A base is YAML, in a
//! ```` ```base ```` block or a `.base` file (opened rendered in a tab of
//! its own): filters, formulas and views (table, list, cards, kanban).
//!
//! - [`syntax`]: the YAML.
//! - [`expr`]: the expression language.
//! - [`views`]: running and drawing a view.
//! - [`edit`]: changing a view without writing YAML.
//! - Commands: switch, sort, group, limit, filter and search a view, choose
//!   its properties, add formulas and views, export CSV, copy, a new note
//!   from the view, edit a row's property (a row's menu too), create a
//!   base file, insert a base block, edit a base file's YAML.

pub mod edit;
pub mod expr;
pub mod syntax;
pub mod views;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ratatui::text::Line;

use super::dataview::index::Index;
use super::{Answer, Context, Effect, Manifest, Plugin, PluginCommand, Question};
use crate::vault::Vault;

/// The plugin's id.
pub const ID: &str = "bases";

/// The first line of a `.base` file's rendered page: which file it is.
pub fn page_marker(rel: &str) -> String {
    format!("%% base: {rel} %%")
}

/// A new base file's YAML.
const NEW_BASE: &str = "# A base: every note is a row. See the Bases help for filters and formulas.\nviews:\n  - type: table\n    name: Table\n    order:\n      - file.name\n      - file.folder\n      - file.mtime\n";

/// The block "Insert new base" adds.
const NEW_BLOCK: &str = "```base\nviews:\n  - type: table\n    name: Table\n    order:\n      - file.name\n      - file.mtime\n```\n";

/// Drawn blocks by (source, note, width, view, search).
type Cache = HashMap<(String, Option<PathBuf>, usize, usize, String), Vec<views::Row>>;

#[derive(Default)]
pub struct Bases {
    /// The vault's pages (shared with Dataview).
    index: Rc<Index>,
    root: PathBuf,
    /// The view shown, by base source (the first one otherwise).
    chosen: HashMap<String, usize>,
    cache: RefCell<Cache>,
    /// The base whose view is being switched.
    switching: Option<String>,
    /// "Edit a property": the rows' pages and the properties offered.
    editing: Option<(Vec<usize>, Vec<String>)>,
    /// A row's menu: its page and the properties offered; then the one
    /// whose value is asked.
    cell: Option<(usize, Vec<String>)>,
    cell_key: Option<(usize, String)>,
    /// A view's search (BA-53), by base source.
    search: HashMap<String, String>,
    /// The choices a view question offers, by command.
    choices: Vec<String>,
}

/// A base found in the active note: its source, and the `.base` file if
/// the note is a base file's page.
struct Found {
    source: String,
    file: Option<String>,
    /// The block's lines in the note (its fences).
    block: (usize, usize),
    /// The cursor's line.
    row: usize,
}

/// The base at the cursor (or the note's first one).
fn base_at_cursor(ctx: &Context) -> Option<Found> {
    let note = ctx.note?;
    let lines: Vec<&str> = note.text.lines().collect();
    let file = lines.first().and_then(|l| {
        l.trim()
            .strip_prefix("%% base: ")?
            .strip_suffix("%%")
            .map(|r| r.trim().to_string())
    });
    let mut blocks = Vec::new();
    let mut open = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        match open {
            None if t == "```base" => open = Some(i),
            Some(start) if t == "```" => {
                blocks.push((start, i));
                open = None;
            }
            _ => {}
        }
    }
    let (start, end) = blocks
        .iter()
        .find(|(s, e)| (*s..=*e).contains(&note.row))
        .or(blocks.first())
        .copied()?;
    Some(Found {
        source: lines[start + 1..end].join("\n"),
        file,
        block: (start, end),
        row: note.row,
    })
}

/// What a filter needs of a note to pass: `prop == "value"` (into
/// `wants`) and `file.inFolder("F")` (into `folder`), from statements that
/// must all hold.
fn required(f: &syntax::Filter, wants: &mut Vec<(String, String)>, folder: &mut Option<String>) {
    use expr::{Expr, Value};
    match f {
        syntax::Filter::And(all) => {
            for f in all {
                required(f, wants, folder);
            }
        }
        syntax::Filter::Expr(Expr::Binary(op, a, b)) if op == "==" => {
            let name = match a.as_ref() {
                Expr::Name(n) => Some(n.clone()),
                Expr::Member(m, n) if matches!(m.as_ref(), Expr::Name(r) if r == "note") => {
                    Some(n.clone())
                }
                _ => None,
            };
            let value = match b.as_ref() {
                Expr::Literal(Value::Text(t)) => Some(t.clone()),
                Expr::Literal(Value::Number(n)) => Some(expr::number(*n)),
                Expr::Literal(Value::Bool(v)) => Some(v.to_string()),
                _ => None,
            };
            if let (Some(n), Some(v)) = (name, value) {
                wants.push((n, v));
            }
        }
        syntax::Filter::Expr(Expr::Method(target, m, args))
            if m == "inFolder" && matches!(target.as_ref(), Expr::Name(f) if f == "file") =>
        {
            if let Some(Expr::Literal(Value::Text(f))) = args.first() {
                *folder = Some(f.clone());
            }
        }
        _ => {}
    }
}

impl Bases {
    pub fn new() -> Self {
        Bases::default()
    }

    fn page_index(&self, path: Option<&Path>) -> Option<usize> {
        let path = path?;
        self.index.pages.iter().position(|p| p.path == path)
    }

    fn input<'a>(
        &'a self,
        base: &'a syntax::Base,
        source: &str,
        this: Option<usize>,
    ) -> views::Input<'a> {
        let only = base
            .only
            .as_ref()
            .and_then(|name| base.views.iter().position(|v| v.name == *name));
        let view = only
            .or_else(|| self.chosen.get(source).copied())
            .unwrap_or(0)
            .min(base.views.len().saturating_sub(1));
        views::Input {
            base,
            view,
            pages: &self.index.pages,
            this,
            now: chrono::Local::now().naive_local(),
            search: self.search.get(source).map(String::as_str),
        }
    }

    /// The view shown for `source`.
    fn view_of(&self, base: &syntax::Base, source: &str) -> usize {
        self.input(base, source, None).view
    }

    /// Makes `change` to the base at the cursor and writes it back (its
    /// block, or its `.base` file).
    fn rewrite(&mut self, ctx: &Context, change: &edit::Change) -> Effect {
        let Some(found) = base_at_cursor(ctx) else {
            return Effect::Message("Bases: no base here".into());
        };
        let view = match syntax::parse(&found.source) {
            Ok(base) => self.view_of(&base, &found.source),
            Err(_) => 0,
        };
        let (yaml, shown) = match edit::apply(&found.source, view, change) {
            Ok(done) => done,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let lines: Vec<String> = yaml.lines().map(String::from).collect();
        self.chosen.insert(lines.join("\n"), shown);
        if let Some(rel) = &found.file {
            let path = self.root.join(rel);
            let old = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => return Effect::Message(format!("Bases: cannot read {rel}: {e}")),
            };
            return Effect::EditNote {
                path,
                from: 0,
                to: old.lines().count(),
                lines,
                expect: old.lines().map(String::from).collect(),
            };
        }
        let (start, end) = found.block;
        let mut block = vec!["```base".to_string()];
        block.extend(lines);
        block.push("```".into());
        let row = if found.row > end {
            (found.row + block.len()).saturating_sub(end + 1 - start)
        } else {
            found.row.min(start)
        };
        Effect::ReplaceLines {
            from: start,
            to: end + 1,
            lines: block,
            cursor: (row, 0),
        }
    }

    /// What a view can sort, group or show by: its properties, its
    /// formulas, the vault's note properties and the file's.
    fn properties(&self, base: &syntax::Base, view: usize) -> Vec<String> {
        let v = &base.views[view];
        let mut all: Vec<String> = v.order.clone();
        all.extend(v.group_by.iter().map(|(p, _)| p.clone()));
        all.extend(base.formulas.iter().map(|(n, _)| format!("formula.{n}")));
        let mut fields: Vec<String> = self
            .index
            .pages
            .iter()
            .flat_map(|p| p.fields.iter().map(|(k, _)| k.clone()))
            .collect();
        fields.sort_by_key(|f| f.to_lowercase());
        all.extend(fields);
        all.extend(
            [
                "file.name",
                "file.folder",
                "file.mtime",
                "file.ctime",
                "file.size",
                "file.tags",
            ]
            .map(String::from),
        );
        let mut seen = std::collections::HashSet::new();
        all.retain(|p| seen.insert(p.clone()));
        all
    }

    /// Starts a view question: what's asked, and the choices to keep.
    fn ask_view(&mut self, ctx: &Context, id: &str) -> Effect {
        let Some(found) = base_at_cursor(ctx) else {
            return Effect::Message("Bases: no base here".into());
        };
        let base = match syntax::parse(&found.source) {
            Ok(b) => b,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let view = self.view_of(&base, &found.source);
        let v = &base.views[view];
        let text = |prompt: &str, default: String| Question::Text {
            prompt: prompt.into(),
            default,
        };
        let choose = |prompt: &str, items: Vec<String>| Question::Choose {
            prompt: prompt.into(),
            items,
        };
        let questions = match id {
            "sort-view" => {
                self.choices = self.properties(&base, view);
                vec![
                    choose("Bases: sort by", self.choices.clone()),
                    choose(
                        "Bases: direction",
                        vec!["Ascending".into(), "Descending".into()],
                    ),
                ]
            }
            "group-view" => {
                self.choices = std::iter::once("(none)".to_string())
                    .chain(self.properties(&base, view))
                    .collect();
                vec![choose("Bases: group by", self.choices.clone())]
            }
            "set-limit" => vec![text(
                "Bases: at most this many rows (empty: all)",
                v.limit.map(|l| l.to_string()).unwrap_or_default(),
            )],
            "view-properties" => vec![text(
                "Bases: the properties shown, in order (file.name, rating, formula.x …)",
                v.order.join(", "),
            )],
            "add-filter" => vec![text(
                "Bases: a filter (rating > 3, file.hasTag(\"x\") …)",
                String::new(),
            )],
            "add-formula" => vec![
                text("Bases: the formula's name", String::new()),
                text("Bases: its expression (price * 2 …)", String::new()),
            ],
            "add-view" => {
                self.choices = ["table", "list", "cards", "kanban"]
                    .map(String::from)
                    .to_vec();
                vec![
                    choose("Bases: the view's type", self.choices.clone()),
                    text("Bases: its name", String::new()),
                ]
            }
            // An empty answer is every row again (no default to fall back on).
            "search-view" => vec![text(
                &match self.search.get(&found.source) {
                    Some(q) => format!("Bases: show rows containing (now {q:?}; empty: all)"),
                    None => "Bases: show rows containing".to_string(),
                },
                String::new(),
            )],
            "new-note" => vec![text("Bases: the new note's name", String::new())],
            _ => return Effect::None,
        };
        Effect::Ask(questions)
    }

    /// The answers to a view question.
    fn answer_view(&mut self, id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        let pick = |i: usize| self.choices.get(i).cloned();
        let change = match (id, answers) {
            ("sort-view", [Answer::Choice(p), Answer::Choice(d)]) => {
                pick(*p).map(|p| edit::Change::Sort(p, *d == 1))
            }
            ("group-view", [Answer::Choice(0)]) => Some(edit::Change::Group(None)),
            ("group-view", [Answer::Choice(i)]) => pick(*i).map(|p| edit::Change::Group(Some(p))),
            ("set-limit", [Answer::Text(n)]) => match n.trim() {
                "" => Some(edit::Change::Limit(None)),
                n => match n.parse() {
                    Ok(n) => Some(edit::Change::Limit(Some(n))),
                    Err(_) => return Effect::Message(format!("Bases: {n:?} isn't a number")),
                },
            },
            ("view-properties", [Answer::Text(list)]) => Some(edit::Change::Order(
                list.split(',')
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(String::from)
                    .collect(),
            )),
            ("add-filter", [Answer::Text(e)]) if !e.trim().is_empty() => {
                Some(edit::Change::Filter(e.trim().to_string()))
            }
            ("add-formula", [Answer::Text(name), Answer::Text(e)])
                if !name.trim().is_empty() && !e.trim().is_empty() =>
            {
                Some(edit::Change::Formula(name.trim().into(), e.trim().into()))
            }
            ("add-view", [Answer::Choice(k), Answer::Text(name)]) => pick(*k).map(|kind| {
                let name = if name.trim().is_empty() {
                    kind.clone()
                } else {
                    name.trim().to_string()
                };
                edit::Change::View(kind, name)
            }),
            ("search-view", [Answer::Text(q)]) => {
                if let Some(found) = base_at_cursor(ctx) {
                    match q.trim() {
                        "" => self.search.remove(&found.source),
                        q => self.search.insert(found.source, q.to_string()),
                    };
                }
                self.cache.borrow_mut().clear();
                return Effect::Redraw;
            }
            ("new-note", [Answer::Text(name)]) if !name.trim().is_empty() => {
                return self.new_note(ctx, name.trim());
            }
            _ => None,
        };
        match change {
            Some(change) => self.rewrite(ctx, &change),
            None => Effect::None,
        }
    }

    /// The view's rows as a Markdown table, on the clipboard (BA-55).
    fn copy(&self, ctx: &Context) -> Effect {
        let Some(found) = base_at_cursor(ctx) else {
            return Effect::Message("Bases: no base here".into());
        };
        let base = match syntax::parse(&found.source) {
            Ok(b) => b,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let this = self.page_index(ctx.note.and_then(|n| n.path));
        let results = match views::run(&self.input(&base, &found.source, this)) {
            Ok(r) => r,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let cell = |s: String| s.replace('|', "\\|");
        let line = |cells: Vec<String>| format!("| {} |\n", cells.join(" | "));
        let mut out = line(
            results
                .columns
                .iter()
                .map(|(_, h)| cell(h.clone()))
                .collect(),
        );
        out.push_str(&line(vec!["---".to_string(); results.columns.len()]));
        for row in &results.cells {
            out.push_str(&line(
                row.iter()
                    .map(|v| cell(v.display(&self.index.pages)))
                    .collect(),
            ));
        }
        Effect::CopyText(out)
    }

    /// A new note from the view (BA-54): in the folder its filters name
    /// (`file.inFolder`), else beside the base, with the properties its
    /// filters ask for (`type == "book"`).
    fn new_note(&self, ctx: &Context, name: &str) -> Effect {
        let Some(found) = base_at_cursor(ctx) else {
            return Effect::Message("Bases: no base here".into());
        };
        let base = match syntax::parse(&found.source) {
            Ok(b) => b,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let view = &base.views[self.view_of(&base, &found.source)];
        let mut wants = Vec::new();
        let mut folder = None;
        for f in base.filters.iter().chain(view.filters.iter()) {
            required(f, &mut wants, &mut folder);
        }
        let beside = match (&found.file, ctx.note.and_then(|n| n.path)) {
            (Some(rel), _) => self.root.join(rel).parent().map(Path::to_path_buf),
            (None, Some(path)) => path.parent().map(Path::to_path_buf),
            _ => None,
        };
        let folder = folder
            .map(|f| self.root.join(f))
            .or(beside)
            .unwrap_or_else(|| self.root.clone());
        let mut text = String::new();
        if !wants.is_empty() {
            text.push_str("---\n");
            for (k, v) in &wants {
                text.push_str(&format!("{k}: {v}\n"));
            }
            text.push_str("---\n");
        }
        Effect::CreateNote {
            folder,
            name: name.to_string(),
            cursor: Some(text.chars().count()),
            text,
        }
    }

    fn switch_view(&mut self, ctx: &Context) -> Effect {
        let Some(found) = base_at_cursor(ctx) else {
            return Effect::Message("Bases: no base here".into());
        };
        let base = match syntax::parse(&found.source) {
            Ok(b) => b,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let items = base.views.iter().map(|v| v.name.clone()).collect();
        self.switching = Some(found.source);
        Effect::Ask(vec![Question::Choose {
            prompt: "Bases: view".into(),
            items,
        }])
    }

    fn export(&self, ctx: &Context) -> Effect {
        let Some(found) = base_at_cursor(ctx) else {
            return Effect::Message("Bases: no base here".into());
        };
        let base = match syntax::parse(&found.source) {
            Ok(b) => b,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let this = self.page_index(ctx.note.and_then(|n| n.path));
        let csv = match views::csv(&self.input(&base, &found.source, this)) {
            Ok(c) => c,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let file = match (&found.file, ctx.note.and_then(|n| n.path)) {
            (Some(rel), _) => self.root.join(rel).with_extension("csv"),
            (None, Some(path)) => path.with_extension("csv"),
            (None, None) => ctx.folder.join("base.csv"),
        };
        let name = file
            .strip_prefix(&self.root)
            .unwrap_or(&file)
            .to_string_lossy()
            .into_owned();
        match mdedit::files::write_atomic(&file, &csv) {
            Ok(()) => Effect::FilesChanged(format!("Bases: the view is in {name}")),
            Err(e) => Effect::Message(format!("Bases: cannot write {name}: {e}")),
        }
    }

    fn ask_edit(&mut self, ctx: &Context) -> Effect {
        let Some(found) = base_at_cursor(ctx) else {
            return Effect::Message("Bases: no base here".into());
        };
        let base = match syntax::parse(&found.source) {
            Ok(b) => b,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        let this = self.page_index(ctx.note.and_then(|n| n.path));
        let input = self.input(&base, &found.source, this);
        let results = match views::run(&input) {
            Ok(r) => r,
            Err(e) => return Effect::Message(format!("Bases: {e}")),
        };
        if results.rows.is_empty() {
            return Effect::Message("Bases: the view has no notes".into());
        }
        let props = views::editable(&base.views[input.view]);
        if props.is_empty() {
            return Effect::Message("Bases: the view shows no note properties".into());
        }
        let names = results
            .rows
            .iter()
            .map(|&i| self.index.pages[i].name.clone())
            .collect();
        self.editing = Some((results.rows, props.clone()));
        Effect::Ask(vec![
            Question::Choose {
                prompt: "Bases: note".into(),
                items: names,
            },
            Question::Choose {
                prompt: "Bases: property".into(),
                items: props,
            },
            Question::Text {
                prompt: "Bases: new value".into(),
                default: String::new(),
            },
        ])
    }

    /// Writes the property into the note's frontmatter.
    fn answer_edit(&mut self, answers: &[Answer]) -> Effect {
        let Some((rows, props)) = self.editing.take() else {
            return Effect::None;
        };
        let [
            Answer::Choice(row),
            Answer::Choice(prop),
            Answer::Text(value),
        ] = answers
        else {
            return Effect::None;
        };
        let (Some(&page), Some(key)) = (rows.get(*row), props.get(*prop)) else {
            return Effect::None;
        };
        self.write_property(page, key, value)
    }

    /// Writes `key: value` (its type guessed) into page `page`'s
    /// frontmatter (made if there's none).
    fn write_property(&self, page: usize, key: &str, value: &str) -> Effect {
        let path = self.index.pages[page].path.clone();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return Effect::Message(format!("Bases: cannot read the note: {e}")),
        };
        let (mut properties, body) = crate::properties::parse(&text);
        let value = crate::properties::Value::guess(value.trim());
        match properties.iter_mut().find(|p| p.key == *key) {
            Some(p) => p.value = value,
            None => properties.push(crate::properties::Property {
                key: key.to_string(),
                value,
            }),
        }
        let new = crate::properties::write(&properties, body);
        Effect::EditNote {
            path,
            from: 0,
            to: text.lines().count(),
            lines: new.lines().map(String::from).collect(),
            expect: text.lines().map(String::from).collect(),
        }
    }

    /// "Create new base": `Untitled.base` (or the next free name) in the
    /// selected folder.
    fn create(&self, ctx: &Context) -> Effect {
        let path = (0..)
            .map(|n| {
                let name = if n == 0 {
                    "Untitled.base".to_string()
                } else {
                    format!("Untitled {n}.base")
                };
                ctx.folder.join(name)
            })
            .find(|p| !p.exists())
            .expect("a free name");
        Effect::CreateFile {
            path,
            text: NEW_BASE.into(),
        }
    }
}

impl Plugin for Bases {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Bases",
            version: "0.1.0",
            author: "blackglass, after the Bases core plugin",
            description: "Database views of your notes: ```base blocks and .base files filter \
                          notes by their properties, compute formulas, and show them as a table \
                          (grouped, with summaries), a list, cards or a kanban board.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_unload(&mut self) {
        self.index = Rc::default();
        self.cache.borrow_mut().clear();
    }

    fn uses_index(&self) -> bool {
        true
    }

    fn set_index(&mut self, index: Option<Rc<Index>>) {
        self.index = index.unwrap_or_default();
        self.cache.borrow_mut().clear();
    }

    fn on_note_changed(&mut self, _path: &Path, _vault: &Vault) {
        self.cache.borrow_mut().clear();
    }

    fn on_theme_changed(&mut self) {
        self.cache.borrow_mut().clear();
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        self.root = vault.root.clone();
        self.cache.borrow_mut().clear();
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("create-base", "Create new base"),
            PluginCommand::new("insert-base", "Insert new base"),
            PluginCommand::new("switch-view", "Switch view"),
            PluginCommand::new("edit-property", "Edit a property"),
            PluginCommand::new("export-csv", "Export view as CSV"),
            PluginCommand::new("edit-source", "Edit base source"),
            PluginCommand::new("sort-view", "Sort view"),
            PluginCommand::new("group-view", "Group view"),
            PluginCommand::new("set-limit", "Set view limit"),
            PluginCommand::new("view-properties", "Choose view properties"),
            PluginCommand::new("add-filter", "Add view filter"),
            PluginCommand::new("add-formula", "Add formula"),
            PluginCommand::new("add-view", "Add view"),
            PluginCommand::new("search-view", "Search view"),
            PluginCommand::new("copy-view", "Copy view"),
            PluginCommand::new("new-note", "New note from view"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        match id {
            "create-base" => self.create(ctx),
            "insert-base" => {
                let Some(note) = ctx.note else {
                    return Effect::Message("Open a note first".into());
                };
                let lead = if note.col > 0 { "\n" } else { "" };
                Effect::Insert {
                    text: format!("{lead}{NEW_BLOCK}"),
                    back: 0,
                }
            }
            "switch-view" => self.switch_view(ctx),
            "edit-property" => self.ask_edit(ctx),
            "export-csv" => self.export(ctx),
            "copy-view" => self.copy(ctx),
            "sort-view" | "group-view" | "set-limit" | "view-properties" | "add-filter"
            | "add-formula" | "add-view" | "search-view" | "new-note" => self.ask_view(ctx, id),
            "edit-source" => match base_at_cursor(ctx).and_then(|f| f.file) {
                Some(rel) => Effect::OpenSource(self.root.join(rel)),
                None => Effect::Message("Bases: this isn't a .base file's page".into()),
            },
            _ => Effect::None,
        }
    }

    /// A row's menu (a click or Enter): open the note, or change one of
    /// the view's properties (a checkbox at once, others asked).
    fn row_action(&mut self, payload: &str, _ctx: &Context) -> Effect {
        let Some((props, rel)) = payload.strip_prefix("row:").and_then(|r| r.split_once(':'))
        else {
            return Effect::None;
        };
        let Some(page) = self.index.pages.iter().position(|p| p.rel == rel) else {
            return Effect::Message(format!("Bases: {rel} isn't there any more"));
        };
        let props: Vec<String> = props
            .split(',')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect();
        let p = &self.index.pages[page];
        let mut items = vec![format!("Open {}", p.name)];
        for key in &props {
            let value = p.field(key).map(expr::Value::from_dv);
            items.push(match value {
                Some(expr::Value::Bool(b)) => {
                    format!(
                        "{key}: {} → {}",
                        if b { "☑" } else { "☐" },
                        if b { "☐" } else { "☑" }
                    )
                }
                Some(v) => format!("{key}: {}", v.display(&self.index.pages)),
                None => format!("{key}: (empty)"),
            });
        }
        self.cell = Some((page, props));
        Effect::Ask(vec![Question::Choose {
            prompt: format!("Bases: {}", p.name),
            items,
        }])
    }

    fn answer(&mut self, id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        match (id, answers) {
            (
                "sort-view" | "group-view" | "set-limit" | "view-properties" | "add-filter"
                | "add-formula" | "add-view" | "search-view" | "new-note",
                _,
            ) => self.answer_view(id, answers, ctx),
            ("row-action", [Answer::Choice(0)]) => match self.cell.take() {
                Some((page, _)) => Effect::Open {
                    path: self.index.pages[page].path.clone(),
                },
                None => Effect::None,
            },
            ("row-action", [Answer::Choice(i)]) => {
                let Some((page, props)) = self.cell.take() else {
                    return Effect::None;
                };
                let Some(key) = props.get(i - 1).cloned() else {
                    return Effect::None;
                };
                let now = self.index.pages[page].field(&key).map(expr::Value::from_dv);
                if let Some(expr::Value::Bool(b)) = now {
                    return self.write_property(page, &key, if b { "false" } else { "true" });
                }
                let default = now.map_or(String::new(), |v| v.display(&self.index.pages));
                self.cell_key = Some((page, key.clone()));
                Effect::Ask(vec![Question::Text {
                    prompt: format!("Bases: {key}"),
                    default,
                }])
            }
            ("row-action", [Answer::Text(value)]) => match self.cell_key.take() {
                Some((page, key)) => self.write_property(page, &key, value),
                None => Effect::None,
            },
            ("switch-view", [Answer::Choice(i)]) => {
                if let Some(source) = self.switching.take() {
                    self.chosen.insert(source, *i);
                }
                Effect::Redraw
            }
            ("edit-property", _) => self.answer_edit(answers),
            _ => Effect::None,
        }
    }

    fn block_languages(&self) -> &[&'static str] {
        &["base"]
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
        let text = source.join("\n");
        let view = self.chosen.get(&text).copied().unwrap_or(0);
        let search = self.search.get(&text).cloned().unwrap_or_default();
        let key = (
            text.clone(),
            from.map(Path::to_path_buf),
            width,
            view,
            search,
        );
        if let Some(rows) = self.cache.borrow().get(&key) {
            return rows.clone();
        }
        let rows = match syntax::parse(&text) {
            Ok(base) => {
                let input = self.input(&base, &text, self.page_index(from));
                views::draw(&input, width)
            }
            Err(e) => views::error(&e),
        };
        self.cache.borrow_mut().insert(key, rows.clone());
        rows
    }
}
