//! Dataview, blackglass's first plugin: an implementation of Obsidian's
//! [Dataview](https://blacksmithgu.github.io/obsidian-dataview/). A
//! ```` ```dataview ```` block holds a query (`LIST`, `TABLE`, `TASK`,
//! `CALENDAR` with `FROM`, `WHERE`, `SORT`, `LIMIT`, `GROUP BY`,
//! `FLATTEN`), and the editor shows its result in place of the code;
//! `` `= expr` `` and `` `$= js` `` show one value in the text. A task in
//! results is checked off by a click or Enter (a setting).
//!
//! - [`index`]: pages with fields, tags, links and tasks.
//! - [`query`]: the query language.
//! - [`eval`]: running a query.
//! - [`render`]: drawing the results.

pub mod eval;
pub mod index;
pub mod js;
pub mod query;
pub mod render;
pub mod value;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ratatui::text::Line;

use super::settings::{Kind, Setting, Values};
use super::{Answer, Context, Effect, Manifest, Plugin, PluginCommand, Question};
use crate::vault::Vault;
use index::Index;

/// The query "Dataview: insert query" adds; the cursor lands on its `FROM`.
const TEMPLATE: &str =
    "```dataview\nTABLE file.mtime AS Modified\nFROM \"\"\nSORT file.mtime DESC\n```";

/// Rendered blocks, by query, note and width; cleared when the vault
/// changes.
type Cache = HashMap<(String, Option<PathBuf>, usize), Vec<mdedit::processor::CellRow>>;

#[derive(Default)]
pub struct Dataview {
    /// Shared with DataviewJS scripts while they run.
    index: Rc<Index>,
    cache: RefCell<Cache>,
    display: render::Display,
    /// Whether ```` ```dataviewjs ```` blocks run (a setting).
    javascript: bool,
    /// Checking a task off adds ✅ and today's date (a setting).
    completion_tracking: bool,
    /// The vault's folder (for DataviewJS's `dv.io` and `dv.view`).
    root: PathBuf,
    /// The note last visited (`this` for inline queries), and their values
    /// there, by code.
    here: Option<PathBuf>,
    inline: RefCell<HashMap<InlineKey, Option<String>>>,
    /// A calendar day's notes, while "which one?" is asked.
    picking: Vec<String>,
}

/// An inline query's code and the note it's shown in.
type InlineKey = (String, Option<PathBuf>);

impl Dataview {
    pub fn new() -> Self {
        Dataview::default()
    }
}

impl Plugin for Dataview {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "dataview",
            name: "Dataview",
            version: "0.1.0",
            author: "blackglass, after Michael Brenan's Dataview",
            description: "Treat your vault as a database: ```dataview blocks run LIST, TABLE and TASK \
                          queries over frontmatter, key:: value fields, tags, links and tasks.",
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
        // The shared index is updated; the drawn results are out of date.
        self.cache.borrow_mut().clear();
        self.inline.borrow_mut().clear();
    }

    fn on_note_opened(&mut self, path: &Path, _vault: &Vault) {
        self.here = Some(path.to_path_buf());
    }

    fn rendered_spans(&self) -> Vec<(&'static str, &'static str)> {
        vec![("`=", "`"), ("`$=", "`")]
    }

    fn render_span(&self, open: &str, inner: &str) -> Option<String> {
        // `==` (a highlight's marks, written as code) and a lone `=` aren't
        // queries.
        if open == "`=" && (inner.starts_with('=') || inner.trim().is_empty()) {
            return None;
        }
        let key = (format!("{open}{inner}"), self.here.clone());
        if let Some(v) = self.inline.borrow().get(&key) {
            return v.clone();
        }
        let this = self.here.as_deref().and_then(|p| self.index.page(p));
        let shown = match open {
            "`=" => match query::parse_expr(inner.trim()) {
                Ok(e) => {
                    let scope = match this {
                        Some(page) => eval::Scope::of_page(&self.index, page, Some(page)),
                        // Not a note in the vault: no fields, `this` nothing.
                        None => return None,
                    };
                    Some(match eval::eval(&e, scope) {
                        Ok(v) => render::inline(&v, &self.display),
                        Err(e) => format!("⚠ {e}"),
                    })
                }
                Err(e) => Some(format!("⚠ {e}")),
            },
            "`$=" if self.javascript => Some(
                match js::inline(inner.trim(), &self.index, this, &self.root) {
                    Ok(v) => v,
                    Err(e) => format!("⚠ {e}"),
                },
            ),
            _ => None,
        };
        self.inline.borrow_mut().insert(key, shown.clone());
        shown
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], _ctx: &Context) -> Effect {
        let picking = std::mem::take(&mut self.picking);
        match answers {
            [Answer::Choice(i)] => picking.get(*i).map_or(Effect::None, |rel| Effect::Open {
                path: self.root.join(rel),
            }),
            _ => Effect::None,
        }
    }

    fn row_action(&mut self, payload: &str, _ctx: &Context) -> Effect {
        // `pick:<path>\t<path>…`: a calendar day's notes; which one?
        if let Some(rels) = payload.strip_prefix("pick:") {
            self.picking = rels.split('\t').map(String::from).collect();
            let items = self
                .picking
                .iter()
                .map(|rel| {
                    let name = rel.rsplit('/').next().unwrap_or(rel);
                    name.strip_suffix(".md").unwrap_or(name).to_string()
                })
                .collect();
            return Effect::Ask(vec![Question::Choose {
                prompt: "Open which note?".into(),
                items,
            }]);
        }
        // `toggle:<line>:<path in the vault>`: check a task off, or back.
        let Some((line, rel)) = payload
            .strip_prefix("toggle:")
            .and_then(|r| r.split_once(':'))
        else {
            return Effect::None;
        };
        let (Ok(line), Some(page)) = (
            line.parse::<usize>(),
            self.index.pages.iter().find(|p| p.rel == rel),
        ) else {
            return Effect::Message("Dataview: that task isn't there any more".into());
        };
        let text = std::fs::read_to_string(&page.path).unwrap_or_default();
        let Some(old) = text
            .split('\n')
            .nth(line)
            .map(|l| l.trim_end_matches('\r').to_string())
        else {
            return Effect::None;
        };
        match toggled(&old, self.completion_tracking) {
            Some(new) => Effect::EditNote {
                path: page.path.clone(),
                from: line,
                to: line + 1,
                lines: vec![new],
                expect: vec![old],
            },
            None => Effect::Message("Dataview: that line isn't a task any more".into()),
        }
    }

    fn on_theme_changed(&mut self) {
        self.cache.borrow_mut().clear();
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        self.cache.borrow_mut().clear();
        self.inline.borrow_mut().clear();
        self.root.clone_from(&vault.root);
        let text =
            std::fs::read_to_string(super::settings_file(vault, "dataview")).unwrap_or_default();
        let values = Values::parse(&text);
        let [
            date,
            datetime,
            no_results,
            id,
            javascript,
            task_click,
            tracking,
        ] = &self.settings()[..]
        else {
            unreachable!("Dataview has seven settings");
        };
        self.javascript = values.value(javascript) == "true";
        self.completion_tracking = values.value(tracking) == "true";
        self.display = render::Display {
            date_format: values.value(date),
            datetime_format: values.value(datetime),
            no_results: values.value(no_results),
            id_header: values.value(id),
            check_tasks: values.value(task_click) != "open",
        };
    }

    fn settings(&self) -> Vec<Setting> {
        let d = render::Display::default();
        let javascript = Setting::new(
            "",
            "enable_javascript",
            "Enable JavaScript queries",
            "Run ```dataviewjs blocks (JavaScript in a sandbox: no files, no network)",
            Kind::Toggle,
            "true",
        );
        let mut all = vec![
            Setting::new(
                "",
                "date_format",
                "Date format",
                "How dates show in results (moment.js tokens)",
                Kind::Text,
                &d.date_format,
            ),
            Setting::new(
                "",
                "datetime_format",
                "Date and time format",
                "How dates with a time show in results",
                Kind::Text,
                &d.datetime_format,
            ),
            Setting::new(
                "",
                "no_results",
                "No results text",
                "Shown when a query finds nothing",
                Kind::Text,
                &d.no_results,
            ),
            Setting::new(
                "",
                "id_header",
                "Page column header",
                "The first column's header in TABLE results",
                Kind::Text,
                &d.id_header,
            ),
        ];
        all.push(javascript);
        all.push(Setting::new(
            "",
            "task_click",
            "A click or Enter on a task",
            "In TASK results: check it off (or back), or open its note at it",
            Kind::Choice(vec!["check".into(), "open".into()]),
            "check",
        ));
        all.push(Setting::new(
            "",
            "completion_tracking",
            "Automatic task completion tracking",
            "Checking a task off in results adds ✅ and today's date (unchecking takes it off)",
            Kind::Toggle,
            "false",
        ));
        all
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("insert-query", "Insert query"),
            PluginCommand::new("refresh", "Refresh views"),
        ]
    }

    fn run(&mut self, command: &str, ctx: &Context) -> Effect {
        match command {
            "insert-query" => Effect::Insert {
                text: TEMPLATE.into(),
                // Back to between the quotes after FROM.
                back: "\"\nSORT file.mtime DESC\n```".chars().count(),
            },
            "refresh" => {
                self.on_vault_changed(ctx.vault);
                Effect::Message(format!(
                    "Dataview: {} pages indexed",
                    self.index.pages.len()
                ))
            }
            _ => Effect::None,
        }
    }

    fn block_languages(&self) -> &[&'static str] {
        &["dataview", "dataviewjs"]
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
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<(Line<'static>, Option<String>)> {
        // A row's whole-row action (a calendar's days have their own).
        self.render_block_cells(lang, source, from, width)
            .into_iter()
            .map(|(line, parts)| {
                let whole = parts
                    .into_iter()
                    .find(|&(_, to, _)| to == usize::MAX)
                    .map(|(.., a)| a);
                (line, whole)
            })
            .collect()
    }

    fn render_block_cells(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<mdedit::processor::CellRow> {
        let text = source.join("\n");
        let key = (
            format!("{lang}\n{text}"),
            from.map(Path::to_path_buf),
            width,
        );
        if let Some(lines) = self.cache.borrow().get(&key) {
            return lines.clone();
        }
        let this = from.and_then(|p| self.index.page(p));
        let lines = if lang == "dataviewjs" {
            let rows = if self.javascript {
                js::render(&text, &self.index, this, &self.root, width, &self.display)
            } else {
                no_actions(render::error("JavaScript queries are off in the settings"))
            };
            render::whole_rows(rows)
        } else {
            match query::parse(&text).and_then(|q| eval::run(&q, &self.index, this)) {
                Ok(results) => render::cells(&results, width, &self.display),
                Err(e) => render::whole_rows(no_actions(render::error(&e))),
            }
        };
        self.cache.borrow_mut().insert(key, lines.clone());
        lines
    }
}

/// A task's line checked off (or back): `[ ]` ↔ `[x]`, with ✅ and today's
/// date added or taken off if `dated`; `None` if it isn't a task.
fn toggled(line: &str, dated: bool) -> Option<String> {
    let at = line.find("[")?;
    let before = line[..at].trim_start();
    let marker = before.trim_end();
    let list = matches!(marker, "-" | "*" | "+")
        || (marker.ends_with(['.', ')'])
            && marker[..marker.len() - 1]
                .chars()
                .all(|c| c.is_ascii_digit())
            && marker.len() > 1);
    if !list || line.get(at + 2..at + 3) != Some("]") {
        return None;
    }
    let state = line[at + 1..].chars().next()?;
    let done = matches!(state, 'x' | 'X');
    let mut out = format!(
        "{}[{}]{}",
        &line[..at],
        if done { ' ' } else { 'x' },
        &line[at + 3..]
    );
    if dated {
        if done {
            // Its completion date goes with it.
            if let Some(i) = out.find(" ✅ ") {
                let end = (i + " ✅ ".len() + 10).min(out.len());
                out.replace_range(i..end, "");
            }
        } else {
            let today = chrono::Local::now().format("%Y-%m-%d");
            out = format!("{} ✅ {today}", out.trim_end());
        }
    }
    Some(out)
}

/// Lines without actions (messages).
fn no_actions(lines: Vec<Line<'static>>) -> Vec<(Line<'static>, Option<String>)> {
    lines.into_iter().map(|l| (l, None)).collect()
}
