//! Citations, after the Citations and Zotero Integration plugins: a
//! bibliography (BibTeX or CSL JSON, as Zotero exports it) in the vault or
//! anywhere; insert a citation (`[@key]`), shown in the live preview as
//! its author and year; open a reference's literature note (made from a
//! template the first time) or link to it.
//!
//! - [`bib`]: reading the bibliography.
//!
//! Settings in `.blackglass/plugins/citations/settings.toml`.

pub mod bib;

use std::path::PathBuf;

use super::settings::{Kind, Setting, Values};
use super::{Answer, Context, Effect, Manifest, Plugin, PluginCommand, Question};
use crate::vault::Vault;
use bib::Reference;

/// The plugin's id.
pub const ID: &str = "citations";

/// A literature note's text unless the settings name a template.
const DEFAULT_TEMPLATE: &str = "# {{title}}\n\n{{authors}} ({{year}})\n\n{{abstract}}\n\n{{url}}\n";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Settings {
    /// The bibliography (in the vault, or an absolute path).
    bibliography: String,
    /// Where literature notes go (in the vault).
    folder: String,
    /// A literature note's name (`@{{citekey}}`).
    title: String,
    /// Its template note (`""`: the default).
    template: String,
    /// A citation as inserted (`[@{{citekey}}]`).
    cite: String,
}

/// What a command waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Cite,
    Open,
    Link,
}

#[derive(Default)]
pub struct Citations {
    settings: Settings,
    references: Vec<Reference>,
    /// Why there are none (the bibliography can't be read).
    problem: Option<String>,
    pending: Option<Pending>,
}

/// `template` with a reference's fields: `{{citekey}}`, `{{title}}`,
/// `{{authors}}`, `{{author}}` (short), `{{year}}`, `{{abstract}}`, `{{url}}`.
fn fill(template: &str, r: &Reference) -> String {
    template
        .replace("{{citekey}}", &r.key)
        .replace("{{title}}", &r.title)
        .replace("{{authors}}", &r.authors_full())
        .replace("{{author}}", &r.author_short())
        .replace("{{year}}", &r.year)
        .replace("{{abstract}}", &r.abstract_)
        .replace("{{url}}", &r.url)
}

/// The citation (`[@key]`, `[@a; @b, p. 3]`) the char column `col` of
/// `line` is in: its keys.
fn cited_at(line: &str, col: usize) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let text: String = chars.iter().collect();
    let mut start = 0;
    while let Some(open) = text[start..].find("[@").map(|i| start + i) {
        let Some(close) = text[open..].find(']').map(|i| open + i) else {
            break;
        };
        let (a, b) = (text[..open].chars().count(), text[..=close].chars().count());
        if (a..=b).contains(&col) {
            return keys(&text[open + 1..close]);
        }
        start = close + 1;
    }
    Vec::new()
}

/// The keys in a citation's text (`@a; @b, p. 3`).
fn keys(inner: &str) -> Vec<String> {
    inner
        .split(';')
        .filter_map(|part| {
            let part = part.trim().strip_prefix('@')?;
            let key = part.split([',', ' ']).next()?.trim();
            (!key.is_empty()).then(|| key.to_string())
        })
        .collect()
}

impl Citations {
    pub fn new() -> Self {
        Citations::default()
    }

    fn find(&self, key: &str) -> Option<&Reference> {
        self.references.iter().find(|r| r.key == key)
    }

    /// Asks which reference (for `pending`).
    fn choose(&mut self, pending: Pending, prompt: &str) -> Effect {
        if self.references.is_empty() {
            return Effect::Message(self.problem.clone().unwrap_or_else(|| {
                "Citations: no references (set the bibliography in the settings)".into()
            }));
        }
        self.pending = Some(pending);
        Effect::Ask(vec![Question::Choose {
            prompt: prompt.into(),
            items: self
                .references
                .iter()
                .map(|r| format!("{}: {}", r.short(), r.title))
                .collect(),
        }])
    }

    /// The literature note for `r`: its path.
    fn note_path(&self, vault: &Vault, r: &Reference) -> PathBuf {
        let folder = self.settings.folder.trim().trim_matches('/');
        let name = fill(&self.settings.title, r);
        vault.root.join(folder).join(format!("{name}.md"))
    }

    /// Opens `r`'s literature note, made from the template if it's new.
    fn open_note(&self, ctx: &Context, r: &Reference) -> Effect {
        let path = self.note_path(ctx.vault, r);
        if path.exists() {
            return Effect::Open { path };
        }
        let template = match self.settings.template.trim() {
            "" => DEFAULT_TEMPLATE.to_string(),
            t => {
                let rel = if t.ends_with(".md") {
                    t.to_string()
                } else {
                    format!("{t}.md")
                };
                match std::fs::read_to_string(ctx.vault.root.join(&rel)) {
                    Ok(text) => text,
                    Err(_) => return Effect::Message(format!("Citations: no template {rel}")),
                }
            }
        };
        Effect::CreateFile {
            path,
            text: fill(&template, r),
        }
    }
}

impl Plugin for Citations {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Citations",
            version: "0.1.0",
            author: "blackglass, after the Citations and Zotero Integration plugins",
            description: "A BibTeX or CSL JSON bibliography (exported from Zotero): insert a \
                          citation ([@key], shown as its author and year), open or link a \
                          reference's literature note (made from a template).",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        let get = |key: &str, default: &str| match values.get("", key).map(str::trim) {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => default.to_string(),
        };
        self.settings = Settings {
            bibliography: get("bibliography", ""),
            folder: get("folder", "Reading notes"),
            title: get("note_title", "@{{citekey}}"),
            template: get("template", ""),
            cite: get("cite_format", "[@{{citekey}}]"),
        };
        self.references.clear();
        self.problem = None;
        let file = self.settings.bibliography.trim();
        if file.is_empty() {
            return;
        }
        let path = std::path::Path::new(file);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            vault.root.join(path)
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.references = bib::parse(&text);
                if self.references.is_empty() {
                    self.problem = Some(format!("Citations: no references in {file}"));
                }
            }
            Err(e) => self.problem = Some(format!("Citations: cannot read {file}: {e}")),
        }
    }

    fn settings(&self) -> Vec<Setting> {
        let text = |key: &str, label: &str, help: &str, default: &str| {
            Setting::new("", key, label, help, Kind::Text, default)
        };
        vec![
            text(
                "bibliography",
                "Bibliography",
                "A BibTeX (.bib) or CSL JSON (.json) file: in the vault, or an absolute path",
                "",
            ),
            text(
                "folder",
                "Literature notes folder",
                "Where literature notes go (in the vault)",
                "Reading notes",
            ),
            text(
                "note_title",
                "Literature note name",
                "A literature note's name: {{citekey}}, {{title}}, {{author}}, {{year}}",
                "@{{citekey}}",
            ),
            text(
                "template",
                "Literature note template",
                "Its template note: {{title}}, {{authors}}, {{year}}, {{citekey}}, {{abstract}}, {{url}} (empty: a simple one)",
                "",
            ),
            text(
                "cite_format",
                "Citation format",
                "What \"Insert citation\" puts in the note",
                "[@{{citekey}}]",
            ),
        ]
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("insert-citation", "Insert citation"),
            PluginCommand::new("open-note", "Open literature note"),
            PluginCommand::new("insert-link", "Insert link to literature note"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        self.pending = None;
        match id {
            "insert-citation" => self.choose(Pending::Cite, "Insert citation"),
            "open-note" => {
                // The reference cited at the cursor, else ask.
                let at = ctx
                    .note
                    .and_then(|n| {
                        let line = n.text.split('\n').nth(n.row)?;
                        cited_at(line, n.col).into_iter().next()
                    })
                    .and_then(|key| self.find(&key).cloned());
                match at {
                    Some(r) => self.open_note(ctx, &r),
                    None => self.choose(Pending::Open, "Open literature note"),
                }
            }
            "insert-link" => self.choose(Pending::Link, "Link to literature note"),
            _ => Effect::None,
        }
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        let (Some(pending), [Answer::Choice(i)]) = (self.pending.take(), answers) else {
            return Effect::None;
        };
        let Some(r) = self.references.get(*i).cloned() else {
            return Effect::None;
        };
        match pending {
            Pending::Cite => Effect::Insert {
                text: fill(&self.settings.cite, &r),
                back: 0,
            },
            Pending::Open => self.open_note(ctx, &r),
            Pending::Link => Effect::Insert {
                text: format!("[[{}]]", fill(&self.settings.title, &r)),
                back: 0,
            },
        }
    }

    fn rendered_spans(&self) -> Vec<(&'static str, &'static str)> {
        if self.references.is_empty() {
            return Vec::new();
        }
        vec![("[@", "]")]
    }

    fn render_span(&self, open: &str, inner: &str) -> Option<String> {
        if open != "[@" {
            return None;
        }
        // `[@a; @b, p. 3]`: each known, with its locator.
        let parts: Option<Vec<String>> = format!("@{inner}")
            .split(';')
            .map(|part| {
                let part = part.trim().strip_prefix('@')?;
                let (key, locator) = match part.split_once(',') {
                    Some((k, l)) => (k.trim(), Some(l.trim())),
                    None => (part.trim(), None),
                };
                let r = self.find(key)?;
                Some(match locator {
                    Some(l) if !l.is_empty() => format!("{}, {l}", r.short()),
                    _ => r.short(),
                })
            })
            .collect();
        parts.map(|p| p.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn citations_are_found_and_shown() {
        assert_eq!(cited_at("see [@a; @b, p. 3] here", 6), ["a", "b"]);
        assert!(cited_at("see [@a] here", 12).is_empty());
        let mut c = Citations::new();
        c.references = bib::parse("@book{a, author = {Doe, J}, year = 2020}");
        assert_eq!(
            c.render_span("[@", "a, p. 3").as_deref(),
            Some("Doe 2020, p. 3")
        );
        assert_eq!(c.render_span("[@", "nope"), None, "unknown: as written");
    }
}
