//! Bases: database views of the vault's notes, after Obsidian's core Bases
//! plugin (`requirements/bases_requirements.md`). A base is YAML, in a
//! ```` ```base ```` block or a `.base` file (opened rendered in a tab of
//! its own): filters, formulas and views (table, list, cards, kanban).
//!
//! - [`syntax`]: the YAML.
//! - [`expr`]: the expression language.
//! - [`views`]: running and drawing a view.
//! - [`edit`]: changing a view without writing YAML.
//! - [`board`]: a kanban view's columns; on a `.base` page the board is
//!   driven by keys (←→↑↓ choose a card, Shift+←→ move it to another
//!   column, Alt+Shift+←→ move the column, Enter open it, `n` a new note in the
//!   column, Space collapse it), each also a command.
//! - [`pane`]: the filters as rows in a pane above a `.base` page's
//!   results (Alt+F shows or hides it, Alt+M sizes it), results following
//!   every key.
//! - Commands: switch, sort, group, limit, filter and search a view, choose
//!   its properties, add formulas and views, export CSV, copy, a new note
//!   from the view, edit a row's property (a row's menu too), create a
//!   base file, insert a base block, edit a base file's YAML.

pub mod board;
pub mod edit;
pub mod expr;
pub mod filters;
pub mod pane;
pub mod syntax;
pub mod views;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    /// A board's selected card, by base source: its column and place,
    /// and its note (which it follows when it moves).
    selected: HashMap<String, Selection>,
    /// A board's collapsed columns (their labels), by base source.
    collapsed: HashMap<String, Vec<String>>,
    /// "New note in this column": the grouped property and the column's
    /// group.
    new_in: Option<(String, Option<String>)>,
    /// The filter pane.
    pane: pane::FilterPane,
    /// Each base's tag in actions (a hash of its YAML) → its YAML.
    tags: RefCell<HashMap<String, String>>,
    /// A board card's menu: its note, the property the board groups by,
    /// its column's group and the groups it can move to.
    card_menu: Option<CardMenu>,
    /// The Group menu's command waiting for a choice (its groups, all of
    /// them, and the one being moved).
    group_menu: Option<GroupMenu>,
}

/// A board card's menu: its note (page), the property the board groups
/// by, its column's group and the groups it can move to.
type CardMenu = (usize, String, Option<String>, Vec<Option<String>>);

/// What a Group menu command asked about.
#[derive(Debug, Clone)]
struct GroupMenu {
    /// Every group the view has (shown or not), in the board's order.
    all: Vec<Option<String>>,
    /// The groups shown, in order (the `groupOrder` to write).
    shown: Vec<Option<String>>,
    /// "Reorder groups": the group chosen to move.
    moving: Option<Option<String>>,
}

/// A base's tag in actions: a short hash of its YAML.
fn tag_of(source: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    format!("{:x}", h.finish())
}

/// A board's selected card.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Selection {
    column: usize,
    card: usize,
    /// The card's note (its path in the vault).
    note: Option<String>,
}

/// What can be done on a board (keys on a `.base` page, or commands).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardAction {
    /// Choose the column (−1 / +1) or card (−1 / +1) beside.
    Column(i32),
    Card(i32),
    Open,
    /// Move the card to the column beside.
    MoveCard(i32),
    /// Move the column one place.
    MoveColumn(i32),
    NewNote,
    Collapse,
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
            selected: None,
            collapsed: self.collapsed.get(source).map_or(&[], Vec::as_slice),
            tag: {
                let tag = tag_of(source);
                self.tags
                    .borrow_mut()
                    .insert(tag.clone(), source.to_string());
                tag
            },
        }
    }

    /// Where a board's selection is now (its note may have moved).
    fn locate(&self, source: &str, columns: &[board::Column], rows: &[usize]) -> (usize, usize) {
        let sel = self.selected.get(source).cloned().unwrap_or_default();
        if let Some(rel) = &sel.note {
            for (c, column) in columns.iter().enumerate() {
                if let Some(k) = column
                    .cards
                    .iter()
                    .position(|&r| self.index.pages[rows[r]].rel == *rel)
                {
                    return (c, k);
                }
            }
        }
        let c = sel.column.min(columns.len().saturating_sub(1));
        let k = sel.card.min(
            columns
                .get(c)
                .map_or(0, |col| col.cards.len().saturating_sub(1)),
        );
        (c, k)
    }

    /// Chooses card `card` of column `column` on the board of `source`.
    fn select(
        &mut self,
        source: &str,
        columns: &[board::Column],
        rows: &[usize],
        column: usize,
        card: usize,
    ) {
        let note = columns
            .get(column)
            .and_then(|c| c.cards.get(card))
            .map(|&r| self.index.pages[rows[r]].rel.clone());
        self.selected
            .insert(source.to_string(), Selection { column, card, note });
        self.cache.borrow_mut().clear();
    }

    /// Does `action` on the board of the `.base` page that's open; `None`
    /// when there's none (not a page, or not a kanban view).
    fn board(&mut self, action: BoardAction, ctx: &Context) -> Option<Effect> {
        let found = base_at_cursor(ctx)?;
        found.file.as_ref()?;
        let base = syntax::parse(&found.source).ok()?;
        let input = self.input(&base, &found.source, None);
        let view = &base.views[input.view];
        if view.kind != syntax::Kind::Kanban {
            return None;
        }
        let (property, _) = view.group_by.clone()?;
        let results = match views::run(&input) {
            Ok(r) => r,
            Err(e) => return Some(Effect::Message(format!("Bases: {e}"))),
        };
        let columns = board::columns(view, &results, &self.index.pages);
        if columns.is_empty() {
            return Some(Effect::Message("Bases: the board has no columns".into()));
        }
        let rows = results.rows.clone();
        let (c, k) = self.locate(&found.source, &columns, &rows);
        let source = found.source.clone();
        let beside = |d: i32| {
            usize::try_from(c as i64 + i64::from(d))
                .ok()
                .filter(|&t| t < columns.len())
        };
        let card = columns[c].cards.get(k).map(|&r| rows[r]);
        Some(match action {
            BoardAction::Column(d) => {
                let t = beside(d).unwrap_or(c);
                let k = k.min(columns[t].cards.len().saturating_sub(1));
                self.select(&source, &columns, &rows, t, k);
                Effect::Redraw
            }
            BoardAction::Card(d) => {
                let last = columns[c].cards.len().saturating_sub(1);
                let k = (k as i64 + i64::from(d)).clamp(0, last as i64) as usize;
                self.select(&source, &columns, &rows, c, k);
                Effect::Redraw
            }
            BoardAction::Open => match card {
                Some(page) => Effect::Open {
                    path: self.index.pages[page].path.clone(),
                },
                None => Effect::Message("Bases: the column is empty".into()),
            },
            BoardAction::MoveCard(d) => {
                let (Some(page), Some(t)) = (card, beside(d)) else {
                    return Some(Effect::Message("Bases: no card, or no column there".into()));
                };
                let effect = self.move_card(
                    page,
                    &property,
                    columns[c].group.as_deref(),
                    columns[t].group.as_deref(),
                );
                // The selection follows the card (by its note).
                self.selected.insert(
                    source,
                    Selection {
                        column: t,
                        card: 0,
                        note: Some(self.index.pages[page].rel.clone()),
                    },
                );
                self.cache.borrow_mut().clear();
                effect
            }
            BoardAction::MoveColumn(d) => {
                let Some(t) = beside(d) else {
                    return Some(Effect::Message("Bases: no column there".into()));
                };
                let mut order = board::order(&columns);
                order.swap(c, t);
                self.select(&source, &columns, &rows, c, k);
                if let Some(sel) = self.selected.get_mut(&source) {
                    sel.column = t;
                }
                self.rewrite(ctx, &edit::Change::GroupOrder(Some(order)))
            }
            BoardAction::NewNote => {
                let group = columns[c].group.clone();
                self.new_in = Some((property, group));
                Effect::Ask(vec![Question::Text {
                    prompt: format!("Bases: a new note in {}", columns[c].label()),
                    default: String::new(),
                }])
            }
            BoardAction::Collapse => {
                let label = columns[c].label().to_string();
                let list = self.collapsed.entry(source).or_default();
                match list.iter().position(|l| *l == label) {
                    Some(i) => {
                        list.remove(i);
                    }
                    None => list.push(label),
                }
                self.cache.borrow_mut().clear();
                Effect::Redraw
            }
        })
    }

    /// Moves page `page`'s card from group `from` to group `to` of the
    /// board grouped by `property`: writes the property in the note (no
    /// value for "None"; in a list, the one value swapped), or moves the
    /// file (grouped by `file.folder`).
    fn move_card(
        &self,
        page: usize,
        property: &str,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Effect {
        let p = &self.index.pages[page];
        if property == "file.folder" {
            let name = p.path.file_name().expect("a note has a name");
            let folder = to.map_or_else(|| self.root.clone(), |f| self.root.join(f));
            return Effect::MoveNote {
                from: p.path.clone(),
                to: folder.join(name),
            };
        }
        let key = property.strip_prefix("note.").unwrap_or(property);
        if key.starts_with("file.") || key.starts_with("formula.") {
            return Effect::Message(format!(
                "Bases: cards grouped by {property} can't move (it isn't a note property)"
            ));
        }
        let text = match std::fs::read_to_string(&p.path) {
            Ok(t) => t,
            Err(e) => return Effect::Message(format!("Bases: cannot read the note: {e}")),
        };
        let (mut properties, body) = crate::properties::parse(&text);
        use crate::properties::{Property, Value};
        let at = properties.iter().position(|q| q.key == key);
        match (at, to) {
            (Some(i), None) => {
                properties.remove(i);
            }
            (None, None) => {}
            (Some(i), Some(to)) => match &mut properties[i].value {
                Value::List(items, _) => {
                    match items.iter().position(|v| Some(v.as_str()) == from) {
                        Some(j) => items[j] = to.to_string(),
                        None => items.push(to.to_string()),
                    }
                }
                value => *value = Value::guess(to),
            },
            (None, Some(to)) => properties.push(Property {
                key: key.to_string(),
                value: Value::guess(to),
            }),
        }
        let new = crate::properties::write(&properties, body);
        Effect::EditNote {
            path: p.path.clone(),
            from: 0,
            to: text.lines().count(),
            lines: new.lines().map(String::from).collect(),
            expect: text.lines().map(String::from).collect(),
        }
    }

    /// The groups of the view at the cursor (its base grouped): every one
    /// it has and those shown, for the Group menu.
    fn groups_here(&self, ctx: &Context) -> Result<GroupMenu, Effect> {
        let no = |m: &str| Effect::Message(format!("Bases: {m}"));
        let found = base_at_cursor(ctx).ok_or_else(|| no("no base here"))?;
        let base = syntax::parse(&found.source).map_err(|e| no(&e))?;
        let input = self.input(&base, &found.source, None);
        let view = &base.views[input.view];
        if view.group_by.is_none() {
            return Err(no("the view isn't grouped (Bases: Group view)"));
        }
        let results = views::run(&input).map_err(|e| no(&e))?;
        let shown = board::order(&board::columns(view, &results, &self.index.pages));
        // Every group: the notes' (without the view's order and hiding),
        // and those groupOrder names.
        let mut free = view.clone();
        free.group_order = None;
        free.options.retain(|(k, _)| k != "hideEmptyColumns");
        let mut all = board::order(&board::columns(&free, &results, &self.index.pages));
        for g in view.group_order.iter().flatten() {
            if !all.contains(g) {
                all.push(g.clone());
            }
        }
        Ok(GroupMenu {
            all,
            shown,
            moving: None,
        })
    }

    /// A collapse or a card's menu, from a click on a base's group
    /// heading or board card (`group:<tag>:<label>`,
    /// `card:<tag>:<column>:<note>`).
    fn base_action(&mut self, payload: &str) -> Option<Effect> {
        if let Some(rest) = payload.strip_prefix("group:") {
            let (tag, label) = rest.split_once(':')?;
            let source = self.tags.borrow().get(tag).cloned()?;
            let list = self.collapsed.entry(source).or_default();
            match list.iter().position(|l| l == label) {
                Some(i) => {
                    list.remove(i);
                }
                None => list.push(label.to_string()),
            }
            self.cache.borrow_mut().clear();
            return Some(Effect::Redraw);
        }
        let rest = payload.strip_prefix("card:")?;
        let (tag, rest) = rest.split_once(':')?;
        let (column, rel) = rest.split_once(':')?;
        let source = self.tags.borrow().get(tag).cloned()?;
        let base = syntax::parse(&source).ok()?;
        let input = self.input(&base, &source, None);
        let view = &base.views[input.view];
        let (property, _) = view.group_by.clone()?;
        let results = views::run(&input).ok()?;
        let columns = board::columns(view, &results, &self.index.pages);
        let from = columns.get(column.parse::<usize>().ok()?)?.group.clone();
        let page = self.index.pages.iter().position(|p| p.rel == rel)?;
        let targets: Vec<Option<String>> = columns
            .iter()
            .map(|c| c.group.clone())
            .filter(|g| *g != from)
            .collect();
        let name = self.index.pages[page].name.clone();
        let mut items = vec![format!("Open {name}")];
        items.extend(
            targets
                .iter()
                .map(|g| format!("Move to {}", g.as_deref().unwrap_or(board::NONE))),
        );
        self.card_menu = Some((page, property, from, targets));
        Some(Effect::Ask(vec![Question::Choose {
            prompt: format!("Bases: {name}"),
            items,
        }]))
    }

    /// The Group menu's commands: add a group, show or hide one, reorder.
    fn group_command(&mut self, id: &str, ctx: &Context) -> Effect {
        let menu = match self.groups_here(ctx) {
            Ok(m) => m,
            Err(e) => return e,
        };
        let label = |g: &Option<String>| g.as_deref().unwrap_or(board::NONE).to_string();
        let question = match id {
            "add-group" => Question::Text {
                prompt: "Bases: the new group's name".into(),
                default: String::new(),
            },
            "show-hide-group" => Question::Choose {
                prompt: "Bases: show or hide a group".into(),
                items: menu
                    .all
                    .iter()
                    .map(|g| {
                        if menu.shown.contains(g) {
                            format!("{} (shown)", label(g))
                        } else {
                            format!("{} (hidden)", label(g))
                        }
                    })
                    .collect(),
            },
            _ => Question::Choose {
                prompt: "Bases: move which group?".into(),
                items: menu.shown.iter().map(label).collect(),
            },
        };
        self.group_menu = Some(menu);
        Effect::Ask(vec![question])
    }

    /// A Group menu's answer: the new `groupOrder` written, or the next
    /// question (where a group moves to).
    fn group_answer(&mut self, id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        let Some(mut menu) = self.group_menu.take() else {
            return Effect::None;
        };
        let label = |g: &Option<String>| g.as_deref().unwrap_or(board::NONE).to_string();
        let order = match (id, answers, menu.moving.take()) {
            ("add-group", [Answer::Text(name)], _) => {
                let name = name.trim();
                if name.is_empty() {
                    return Effect::None;
                }
                let mut order = menu.shown.clone();
                order.push(Some(name.to_string()));
                order
            }
            ("show-hide-group", [Answer::Choice(i)], _) => {
                let Some(g) = menu.all.get(*i).cloned() else {
                    return Effect::None;
                };
                let mut order = menu.shown.clone();
                match order.iter().position(|s| *s == g) {
                    Some(at) => {
                        order.remove(at);
                    }
                    None => order.push(g),
                }
                order
            }
            ("reorder-groups", [Answer::Choice(i)], None) => {
                let Some(g) = menu.shown.get(*i).cloned() else {
                    return Effect::None;
                };
                let others: Vec<Option<String>> =
                    menu.shown.iter().filter(|s| **s != g).cloned().collect();
                let mut items = vec!["First".to_string()];
                items.extend(others.iter().map(|o| format!("After {}", label(o))));
                menu.moving = Some(g.clone());
                self.group_menu = Some(menu);
                return Effect::Ask(vec![Question::Choose {
                    prompt: format!("Bases: move {} to", label(&g)),
                    items,
                }]);
            }
            ("reorder-groups", [Answer::Choice(i)], Some(g)) => {
                let mut order: Vec<Option<String>> =
                    menu.shown.iter().filter(|s| **s != g).cloned().collect();
                order.insert((*i).min(order.len()), g);
                order
            }
            _ => return Effect::None,
        };
        self.rewrite(ctx, &edit::Change::GroupOrder(Some(order)))
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
        let new_source = lines.join("\n");
        self.chosen.insert(new_source.clone(), shown);
        // A board's selection and collapsed columns stay with it.
        if let Some(sel) = self.selected.get(&found.source).cloned() {
            self.selected.insert(new_source.clone(), sel);
        }
        if let Some(c) = self.collapsed.get(&found.source).cloned() {
            self.collapsed.insert(new_source, c);
        }
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
        self.new_note_with(ctx, name, None)
    }

    /// A new note from the view, with `extra` (property, value) too.
    fn new_note_with(&self, ctx: &Context, name: &str, extra: Option<(&str, &str)>) -> Effect {
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
        if let Some((key, value)) = extra {
            let key = key.strip_prefix("note.").unwrap_or(key);
            wants.retain(|(k, _)| k != key);
            wants.push((key.to_string(), value.to_string()));
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
        let name = crate::vault::slash(file.strip_prefix(&self.root).unwrap_or(&file));
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
    fn create(&mut self, ctx: &Context) -> Effect {
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
        // Its filters open with it.
        self.pane.open();
        Effect::Many(vec![
            Effect::CreateFile {
                path,
                text: NEW_BASE.into(),
            },
            Effect::FocusPane(true),
        ])
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
            PluginCommand::new("card-next-column", "Move card to next column"),
            PluginCommand::new("card-previous-column", "Move card to previous column"),
            PluginCommand::new("column-right", "Move column right"),
            PluginCommand::new("column-left", "Move column left"),
            PluginCommand::new("new-in-column", "New note in this column"),
            PluginCommand::new("collapse-column", "Collapse or expand column"),
            PluginCommand::new("reorder-groups", "Reorder groups"),
            PluginCommand::new("show-hide-group", "Show or hide a group"),
            PluginCommand::new("add-group", "Add a group"),
            PluginCommand::new("toggle-filters", "Show or hide filters").keys("Alt+F"),
            PluginCommand::new("filters-size", "Change filters size").keys("Alt+M"),
        ]
    }

    fn pane(&self, ctx: &Context, _width: u16, height: u16) -> Option<super::Pane> {
        self.filter_pane(ctx, height)
    }

    fn pane_key(&mut self, key: KeyEvent, ctx: &Context) -> Effect {
        self.filter_key(key, ctx)
    }

    /// The board's own keys (on a `.base` page showing a kanban view).
    fn takes_key(&self, key: &KeyEvent) -> bool {
        matches!(
            key.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Enter
                | KeyCode::Char('n' | ' ')
        )
    }

    fn editor_key(&mut self, key: KeyEvent, ctx: &Context) -> Option<Effect> {
        let plain = key.modifiers.is_empty();
        let action = match key.code {
            KeyCode::Left if plain => BoardAction::Column(-1),
            KeyCode::Right if plain => BoardAction::Column(1),
            KeyCode::Up if plain => BoardAction::Card(-1),
            KeyCode::Down if plain => BoardAction::Card(1),
            KeyCode::Enter if plain => BoardAction::Open,
            KeyCode::Left if key.modifiers == KeyModifiers::SHIFT => BoardAction::MoveCard(-1),
            KeyCode::Right if key.modifiers == KeyModifiers::SHIFT => BoardAction::MoveCard(1),
            // Alt+←→ switch tabs: Alt+Shift moves a column.
            KeyCode::Left if key.modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                BoardAction::MoveColumn(-1)
            }
            KeyCode::Right if key.modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                BoardAction::MoveColumn(1)
            }
            KeyCode::Char('n') if plain => BoardAction::NewNote,
            KeyCode::Char(' ') if plain => BoardAction::Collapse,
            _ => return None,
        };
        self.board(action, ctx)
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        if matches!(id, "add-group" | "show-hide-group" | "reorder-groups") {
            return self.group_command(id, ctx);
        }
        let on_board = match id {
            "card-next-column" => Some(BoardAction::MoveCard(1)),
            "card-previous-column" => Some(BoardAction::MoveCard(-1)),
            "column-right" => Some(BoardAction::MoveColumn(1)),
            "column-left" => Some(BoardAction::MoveColumn(-1)),
            "new-in-column" => Some(BoardAction::NewNote),
            "collapse-column" => Some(BoardAction::Collapse),
            _ => None,
        };
        if let Some(action) = on_board {
            return self.board(action, ctx).unwrap_or_else(|| {
                Effect::Message("Bases: open a .base file with a kanban view first".into())
            });
        }
        match id {
            "toggle-filters" => self.toggle_filters(ctx),
            "filters-size" => self.filters_size(ctx),
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
        if payload.starts_with("group:") || payload.starts_with("card:") {
            return self
                .base_action(payload)
                .unwrap_or_else(|| Effect::Message("Bases: that base changed; try again".into()));
        }
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
        if matches!(id, "add-group" | "show-hide-group" | "reorder-groups") {
            return self.group_answer(id, answers, ctx);
        }
        // A board card's menu: open it, or move it to a column.
        if let (Some((page, property, from, targets)), [Answer::Choice(i)]) =
            (self.card_menu.take(), answers)
        {
            return match i.checked_sub(1) {
                None => Effect::Open {
                    path: self.index.pages[page].path.clone(),
                },
                Some(t) => match targets.get(t) {
                    Some(to) => {
                        self.cache.borrow_mut().clear();
                        self.move_card(page, &property, from.as_deref(), to.as_deref())
                    }
                    None => Effect::None,
                },
            };
        }
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
            (_, [Answer::Text(name)]) if self.new_in.is_some() => {
                let (property, group) = self.new_in.take().expect("checked");
                match (name.trim(), group) {
                    ("", _) => Effect::None,
                    (name, Some(group)) => self.new_note_with(ctx, name, Some((&property, &group))),
                    (name, None) => self.new_note(ctx, name),
                }
            }
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
        self.render_block_cells(lang, source, from, width)
            .into_iter()
            .map(|(line, _)| line)
            .collect()
    }

    fn render_block_rows(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<(Line<'static>, Option<String>)> {
        // A row's first action, for view mode.
        self.render_block_cells(lang, source, from, width)
            .into_iter()
            .map(|(line, parts)| (line, parts.into_iter().next().map(|(.., a)| a)))
            .collect()
    }

    fn render_block_cells(
        &self,
        _lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<mdedit::processor::CellRow> {
        let text = source.join("\n");
        let view = self.chosen.get(&text).copied().unwrap_or(0);
        let search = self.search.get(&text).cloned().unwrap_or_default();
        // Collapsed groups change what's drawn.
        let collapsed = self
            .collapsed
            .get(&text)
            .map(|c| c.join("\u{1f}"))
            .unwrap_or_default();
        let key = (
            text.clone(),
            from.map(Path::to_path_buf),
            width,
            view,
            format!("{search}\u{1e}{collapsed}"),
        );
        if let Some(rows) = self.cache.borrow().get(&key) {
            return rows.clone();
        }
        let rows = match syntax::parse(&text) {
            Ok(base) => {
                let mut input = self.input(&base, &text, self.page_index(from));
                // A board's selection, on a `.base` page (no note).
                if from.is_none() && base.views[input.view].kind == syntax::Kind::Kanban {
                    input.selected = views::run(&input).ok().map(|results| {
                        let columns =
                            board::columns(&base.views[input.view], &results, &self.index.pages);
                        self.locate(&text, &columns, &results.rows)
                    });
                }
                views::draw(&input, width)
            }
            Err(e) => views::error(&e),
        };
        self.cache.borrow_mut().insert(key, rows.clone());
        rows
    }
}
