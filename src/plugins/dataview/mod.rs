//! Dataview, blackglass's first plugin: an implementation of Obsidian's
//! [Dataview](https://blacksmithgu.github.io/obsidian-dataview/). A
//! ```` ```dataview ```` block holds a query (`LIST`, `TABLE`, `TASK` with
//! `FROM`, `WHERE`, `SORT`, `LIMIT`), and the editor shows its result in
//! place of the code.
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
use super::{Context, Effect, Manifest, Plugin, PluginCommand};
use crate::vault::Vault;
use index::Index;

/// The query "Dataview: insert query" adds; the cursor lands on its `FROM`.
const TEMPLATE: &str =
    "```dataview\nTABLE file.mtime AS Modified\nFROM \"\"\nSORT file.mtime DESC\n```";

/// Rendered blocks, by query, note and width; cleared when the vault
/// changes.
type Cache = HashMap<(String, Option<PathBuf>, usize), Vec<(Line<'static>, Option<String>)>>;

#[derive(Default)]
pub struct Dataview {
    /// Shared with DataviewJS scripts while they run.
    index: Rc<Index>,
    cache: RefCell<Cache>,
    display: render::Display,
    /// Whether ```` ```dataviewjs ```` blocks run (a setting).
    javascript: bool,
}

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
    }

    fn on_theme_changed(&mut self) {
        self.cache.borrow_mut().clear();
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        self.cache.borrow_mut().clear();
        let text =
            std::fs::read_to_string(super::settings_file(vault, "dataview")).unwrap_or_default();
        let values = Values::parse(&text);
        let [date, datetime, no_results, id, javascript] = &self.settings()[..] else {
            unreachable!("Dataview has five settings");
        };
        self.javascript = values.value(javascript) == "true";
        self.display = render::Display {
            date_format: values.value(date),
            datetime_format: values.value(datetime),
            no_results: values.value(no_results),
            id_header: values.value(id),
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
            if self.javascript {
                js::render(&text, &self.index, this, width, &self.display)
            } else {
                no_actions(render::error("JavaScript queries are off in the settings"))
            }
        } else {
            match query::parse(&text).and_then(|q| eval::run(&q, &self.index, this)) {
                Ok(results) => render::rows(&results, width, &self.display),
                Err(e) => no_actions(render::error(&e)),
            }
        };
        self.cache.borrow_mut().insert(key, lines.clone());
        lines
    }
}

/// Lines without actions (messages).
fn no_actions(lines: Vec<Line<'static>>) -> Vec<(Line<'static>, Option<String>)> {
    lines.into_iter().map(|l| (l, None)).collect()
}
