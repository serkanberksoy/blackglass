//! QuickAdd (W-146), after Christian B. B. Houmann's plugin: **captures**,
//! each a format ([`format`]) whose questions are asked one after the other
//! and whose line then goes into today's daily note or a note by path,
//! under a heading. Every capture is a command of its own, so it can have
//! keys; "Run QuickAdd" lists them. They're kept in the plugin's settings,
//! a section a capture:
//!
//! ```toml
//! [Expense]
//! format = "- [[{{NOTE:Budget/Envelopes}}]] {{VALUE:amount}} {{VALUE:what}}"
//! note = ""
//! heading = "Money"
//! ```

pub mod format;

use std::collections::HashMap;

use super::settings::{Kind, Setting, Values};
use super::{Answer, Context, Effect, Manifest, Plugin, PluginCommand, Question, intern, slug};
use crate::vault::Vault;

const ID: &str = "quickadd";

/// A capture choice.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Capture {
    name: String,
    format: String,
    /// The note's path, in the format syntax; empty: today's daily note.
    note: String,
    /// The heading it goes under (made if missing); empty: the note's end.
    heading: String,
}

/// What the answers coming back are for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// Which capture to run.
    Pick,
    /// The name of a new capture.
    Name,
    /// Capture `i`'s questions: their keys and the choices' items.
    Fill {
        i: usize,
        keys: Vec<(String, Option<Vec<String>>)>,
    },
}

pub struct QuickAdd {
    captures: Vec<Capture>,
    step: Option<Step>,
}

impl QuickAdd {
    pub fn new() -> Self {
        QuickAdd {
            captures: Vec::new(),
            step: None,
        }
    }

    /// Asks capture `i`'s questions, or adds its line at once without any.
    fn start(&mut self, i: usize, ctx: &Context) -> Effect {
        let capture = &self.captures[i];
        let (line, note) = (format::parse(&capture.format), format::parse(&capture.note));
        let asked = format::questions(&[&line, &note], ctx.vault);
        if let Some((_, Question::Choose { prompt, .. })) = asked
            .iter()
            .find(|(_, q)| matches!(q, Question::Choose { items, .. } if items.is_empty()))
        {
            return Effect::Message(format!("QuickAdd: nothing to choose for \"{prompt}\""));
        }
        if asked.is_empty() {
            return self.finish(i, &HashMap::new(), ctx);
        }
        let keys = asked
            .iter()
            .map(|(k, q)| {
                let items = match q {
                    Question::Choose { items, .. } => Some(items.clone()),
                    _ => None,
                };
                (k.clone(), items)
            })
            .collect();
        self.step = Some(Step::Fill { i, keys });
        Effect::Ask(asked.into_iter().map(|(_, q)| q).collect())
    }

    /// Capture `i`'s line, with `answers`, into its note.
    fn finish(&self, i: usize, answers: &HashMap<String, String>, ctx: &Context) -> Effect {
        let capture = &self.captures[i];
        let env = format::Env {
            now: chrono::Local::now().naive_local(),
            current: ctx
                .note
                .and_then(|n| n.path)
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned()),
            selected: ctx.note.and_then(|n| n.selection).map(String::from),
        };
        let line = format::fill(&format::parse(&capture.format), answers, &env);
        let note = format::fill(&format::parse(&capture.note), answers, &env);
        let heading = capture
            .heading
            .trim()
            .trim_start_matches('#')
            .trim()
            .to_string();
        if note.trim().is_empty() {
            return Effect::AddToDaily { heading, line };
        }
        let mut rel = note.trim().trim_matches('/').to_string();
        if !rel.ends_with(".md") {
            rel.push_str(".md");
        }
        Effect::AddToNote {
            path: ctx.vault.root.join(rel),
            heading,
            line,
        }
    }

    /// Adds a capture called `name` to the settings.
    fn add(&mut self, vault: &Vault, name: &str) -> Effect {
        let name = name.trim();
        if name.is_empty() || name.contains(['[', ']', '"']) {
            return Effect::Message("QuickAdd: a capture needs a name (without [ ] \")".into());
        }
        if self
            .captures
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(name))
        {
            return Effect::Message(format!("QuickAdd: there's a capture {name} already"));
        }
        let file = super::settings_file(vault, ID);
        let mut values = Values::parse(&std::fs::read_to_string(&file).unwrap_or_default());
        values.set(name, "format", "- {{VALUE}}");
        values.set(name, "note", "");
        values.set(name, "heading", "");
        self.captures.push(Capture {
            name: name.into(),
            format: "- {{VALUE}}".into(),
            note: String::new(),
            heading: String::new(),
        });
        let saved = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| mdedit::files::write_atomic(&file, &values.to_text(&self.settings())));
        match saved {
            Ok(()) => Effect::Message(format!(
                "QuickAdd: added {name}; set its format in Settings → QuickAdd"
            )),
            Err(e) => Effect::Message(format!("QuickAdd: cannot save: {e}")),
        }
    }
}

impl Default for QuickAdd {
    fn default() -> Self {
        Self::new()
    }
}

/// The captures in a settings file: every section with a `format`.
fn captures(values: &Values, text: &str) -> Vec<Capture> {
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
        .into_iter()
        .filter_map(|name| {
            let get = |key: &str| values.get(&name, key).unwrap_or_default().to_string();
            values.get(&name, "format")?;
            Some(Capture {
                format: get("format"),
                note: get("note"),
                heading: get("heading"),
                name,
            })
        })
        .collect()
}

impl Plugin for QuickAdd {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "QuickAdd",
            version: "0.1.0",
            author: "blackglass, after Christian B. B. Houmann's QuickAdd",
            description: "Captures: a line put together from a few questions ({{VALUE}}, a \
                          choice, a date, a note in a folder) and added to today's daily note \
                          or another note, under a heading. Each capture is a command.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        self.captures = captures(&Values::parse(&text), &text);
    }

    fn commands(&self) -> Vec<PluginCommand> {
        let mut all = vec![
            PluginCommand::new("run", "Run QuickAdd"),
            PluginCommand::new("add-capture", "Add a capture"),
        ];
        for c in &self.captures {
            all.push(PluginCommand::new(
                intern(format!("capture-{}", slug(&c.name))),
                intern(c.name.clone()),
            ));
        }
        all
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        self.step = None;
        match id {
            "run" if self.captures.is_empty() => {
                Effect::Message("QuickAdd: no captures yet; add one in Settings → QuickAdd".into())
            }
            "run" => {
                self.step = Some(Step::Pick);
                Effect::Ask(vec![Question::Choose {
                    prompt: "QuickAdd".into(),
                    items: self.captures.iter().map(|c| c.name.clone()).collect(),
                }])
            }
            "add-capture" => {
                self.step = Some(Step::Name);
                Effect::Ask(vec![Question::Text {
                    prompt: "Name of the capture".into(),
                    default: String::new(),
                }])
            }
            _ => {
                let wanted = id.strip_prefix("capture-").unwrap_or(id);
                match self.captures.iter().position(|c| slug(&c.name) == wanted) {
                    Some(i) => self.start(i, ctx),
                    None => Effect::None,
                }
            }
        }
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        match (self.step.take(), answers) {
            (Some(Step::Pick), [Answer::Choice(i)]) if *i < self.captures.len() => {
                self.start(*i, ctx)
            }
            (Some(Step::Name), [Answer::Text(name)]) => self.add(ctx.vault, name),
            (Some(Step::Fill { i, keys }), answers) if answers.len() == keys.len() => {
                let filled = keys
                    .iter()
                    .zip(answers)
                    .map(|((key, items), answer)| {
                        let text = match (answer, items) {
                            (Answer::Choice(n), Some(items)) => {
                                items.get(*n).cloned().unwrap_or_default()
                            }
                            (Answer::Text(t), _) => t.clone(),
                            _ => String::new(),
                        };
                        (key.clone(), text)
                    })
                    .collect();
                self.finish(i, &filled, ctx)
            }
            _ => Effect::None,
        }
    }

    fn settings(&self) -> Vec<Setting> {
        let mut all = Vec::new();
        for c in &self.captures {
            let name = &c.name;
            all.extend([
                Setting::new(
                    name,
                    "format",
                    &format!("{name}: format"),
                    "The line: {{VALUE}}, {{VALUE:name}}, {{VALUE:a,b}}, {{DATE}}, {{VDATE:name, format}}, {{NOTE:folder}}, {{LINKCURRENT}}",
                    Kind::Text,
                    "",
                ),
                Setting::new(
                    name,
                    "note",
                    &format!("{name}: note"),
                    "The note it goes in (a path; {{DATE:YYYY}} and the rest work); empty: today's daily note",
                    Kind::Text,
                    "",
                ),
                Setting::new(
                    name,
                    "heading",
                    &format!("{name}: under the heading"),
                    "It goes at the end of the list under this heading (made if missing); empty: the note's end",
                    Kind::Text,
                    "",
                ),
            ]);
        }
        all.push(Setting::new(
            "",
            "add",
            "Add a capture",
            "A new capture: a name, then its format, note and heading here",
            Kind::Action("add-capture"),
            "",
        ));
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn captures_come_from_the_settings_sections() {
        let dir = scratch("quickadd-settings");
        write(
            &dir,
            &[(
                ".blackglass/plugins/quickadd/settings.toml",
                "[Expense]\nformat = \"- {{VALUE}}\"\nheading = \"Money\"\n\n[Daily log]\nformat = \"- {{DATE:HH:mm}} {{VALUE}}\"\nnote = \"Logs/log\"\n",
            )],
        );
        let vault = Vault::open(&dir).unwrap();
        let mut plugin = QuickAdd::new();
        plugin.on_load(&vault);
        let names: Vec<_> = plugin.commands().iter().map(|c| (c.id, c.name)).collect();
        assert_eq!(
            names,
            [
                ("run", "Run QuickAdd"),
                ("add-capture", "Add a capture"),
                ("capture-expense", "Expense"),
                ("capture-daily-log", "Daily log"),
            ]
        );
        let ctx = Context {
            vault: &vault,
            note: None,
            folder: &vault.root,
            name: None,
            query: "",
        };
        assert!(matches!(plugin.run("capture-daily-log", &ctx), Effect::Ask(q) if q.len() == 1));
        let done = plugin.answer("capture-daily-log", &[Answer::Text("walk".into())], &ctx);
        let Effect::AddToNote { path, line, .. } = done else {
            panic!("{done:?}")
        };
        assert_eq!(path, vault.root.join("Logs/log.md"));
        assert!(line.ends_with(" walk"), "{line}");
        // A new capture is saved and becomes a command.
        plugin.run("add-capture", &ctx);
        plugin.answer("add-capture", &[Answer::Text("Habit".into())], &ctx);
        plugin.on_vault_changed(&vault);
        assert!(plugin.commands().iter().any(|c| c.name == "Habit"));
    }
}
