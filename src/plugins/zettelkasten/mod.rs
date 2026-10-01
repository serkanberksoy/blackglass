//! Zettelkasten: what the slip-box method needs, after the original's
//! core plugins and the community plugins its users rely on
//! (`requirements/zettelkasten_requirements.md`).
//!
//! - [`links`]: heading and block links (suggested, ids added), copied
//!   links, a new unique note linked from here (stage 1).
//! - [`sequence`]: Folgezettel IDs, sequel and branch notes, the sequence
//!   in a sidebar tab (stage 2); [`hierarchy`]: `up` / `down` / `next` /
//!   `prev` fields, breadcrumbs in the status bar; titles shown instead of
//!   names ([`Plugin::display_name`]).
//! - [`composer`]: the selection or a heading to a note, a note split at
//!   its headings, a note merged into another (stage 3).
//! - [`graph`]: orphans, dead ends, unresolved links, the local tree of
//!   links, link counts beside links; quick capture to an inbox (stage 4).
//!
//! Settings in `.blackglass/plugins/zettelkasten/settings.toml`.

pub mod composer;
pub mod graph;
pub mod hierarchy;
pub mod links;
pub mod sequence;
pub mod text;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::settings::{Kind, Setting, Values};
use super::{
    Answer, Context, Effect, Manifest, Plugin, PluginCommand, Question, SidebarRow, Suggestions,
};
use crate::vault::Vault;

/// The plugin's id.
pub const ID: &str = "zettelkasten";

/// The plugin's settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Settings {
    /// Where new unique notes go (in the vault; `""`: the selected folder).
    unique_folder: String,
    /// Their template note (`""`: none).
    unique_template: String,
    /// Show notes' titles instead of their names.
    show_titles: bool,
    /// The property a title is in.
    title_property: String,
    /// An extract's text (`{{content}}` …).
    extract_template: String,
    /// What an extract leaves behind.
    extract_leaves: composer::Leave,
    /// Text added to a note goes at its start (else its end).
    at_start: bool,
    /// A merge asks first.
    confirm_merge: bool,
    /// Show how many notes link to a link's note, beside it.
    link_counts: bool,
    /// The inbox quick capture adds to (in the vault).
    inbox: String,
    /// A captured line's text (`{{content}}` …).
    capture_template: String,
    /// How deep the local tree of links goes.
    depth: usize,
}

/// What a command waits for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    /// A new note's title: its ID and folder.
    Title { id: String, dir: PathBuf },
    /// Which note down to go to.
    Down(Vec<PathBuf>),
    /// Where the selection goes: a new note (item 0) or one of these.
    ExtractTo {
        content: String,
        notes: Vec<PathBuf>,
    },
    /// The new note's name for the selection.
    ExtractName { content: String },
    /// Which heading level to split at.
    SplitAt(Vec<usize>),
    /// Which note to merge the active one into …
    MergeInto(Vec<PathBuf>),
    /// … and whether to.
    MergeSure { to: PathBuf },
    /// What to capture.
    Capture,
}

/// What the sidebar tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Sequence,
    /// The active note's links, out and in.
    Links,
}

#[derive(Default)]
pub struct Zettelkasten {
    settings: Settings,
    /// Notes' titles, by path.
    titles: HashMap<PathBuf, String>,
    /// The note last visited, and its breadcrumbs.
    here: Option<PathBuf>,
    trail: Option<String>,
    /// The Folgezettel notes, in order (the Sequence tab's rows).
    sequence: Vec<(sequence::Id, PathBuf)>,
    pending: Option<Pending>,
    tab: Tab,
    /// The vault's links, worked out when needed (or at once when link
    /// counts are shown).
    graph: RefCell<Option<Rc<graph::Graph>>>,
}

impl Zettelkasten {
    pub fn new() -> Self {
        Zettelkasten {
            settings: Settings {
                title_property: "title".into(),
                ..Settings::default()
            },
            ..Zettelkasten::default()
        }
    }

    /// A note's name as shown (its title if titles are shown).
    fn name_of(&self, path: &Path) -> String {
        self.display_name(path).unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
    }

    /// Works out what depends on the vault: titles, the sequence, the
    /// breadcrumbs.
    fn index(&mut self, vault: &Vault) {
        let key = self.settings.title_property.clone();
        self.titles = vault
            .notes
            .iter()
            .filter_map(|n| {
                let (_, v) = n
                    .properties
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&key))?;
                let v = v.trim().trim_matches(['"', '\'']).trim();
                (!v.is_empty()).then(|| (n.path.clone(), v.to_string()))
            })
            .collect();
        self.sequence = sequence::notes(vault);
        self.update_trail(vault);
    }

    fn update_trail(&mut self, vault: &Vault) {
        self.trail = self
            .here
            .clone()
            .and_then(|p| hierarchy::trail(vault, &p, |p| self.name_of(p)));
    }

    /// The active note's path.
    fn active(ctx: &Context) -> Option<PathBuf> {
        ctx.note.and_then(|n| n.path).map(Path::to_path_buf)
    }

    /// A new sequel (`branch`: false) or branch note from the active one:
    /// asks its title.
    fn new_in_sequence(&mut self, ctx: &Context, branch: bool) -> Effect {
        let Some(path) = Self::active(ctx) else {
            return Effect::Message("Zettelkasten: open a note first".into());
        };
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(id) = sequence::Id::of(&name) else {
            return Effect::Message(
                "Zettelkasten: this note has no Folgezettel ID (1, 1a, 1a1 … at the start of its name)"
                    .into(),
            );
        };
        let all = sequence::notes(ctx.vault);
        let start = if branch { id.branch() } else { id.sequel() };
        let new = sequence::free(start, &all);
        self.pending = Some(Pending::Title {
            id: new.to_string(),
            dir: path.parent().unwrap_or(&ctx.vault.root).to_path_buf(),
        });
        Effect::Ask(vec![Question::Text {
            prompt: format!("Title of note {new} (empty: none)"),
            default: String::new(),
        }])
    }

    /// The note next to the active one in the sequence (`step` -1 or 1),
    /// or its parent (`step` 0).
    fn move_in_sequence(&self, ctx: &Context, step: isize) -> Effect {
        let Some(path) = Self::active(ctx) else {
            return Effect::Message("Zettelkasten: open a note first".into());
        };
        let all = sequence::notes(ctx.vault);
        let Some(i) = all.iter().position(|(_, p)| *p == path) else {
            return Effect::Message("Zettelkasten: this note isn't in the sequence".into());
        };
        let target = match step {
            0 => all[i]
                .0
                .parent()
                .and_then(|parent| all.iter().find(|(id, _)| *id == parent)),
            s => i.checked_add_signed(s).and_then(|j| all.get(j)),
        };
        match target {
            Some((_, p)) => Effect::Open { path: p.clone() },
            None => Effect::Message(
                match step {
                    0 => "Zettelkasten: no parent in the sequence",
                    1 => "Zettelkasten: the last note in the sequence",
                    _ => "Zettelkasten: the first note in the sequence",
                }
                .into(),
            ),
        }
    }

    /// Goes to the note the active one's `key` field links to (asks which
    /// if several).
    fn follow_field(&mut self, ctx: &Context, key: &str) -> Effect {
        let Some(note) = Self::active(ctx).and_then(|p| ctx.vault.note(&p)) else {
            return Effect::Message("Zettelkasten: open a note first".into());
        };
        let targets = hierarchy::linked(ctx.vault, note, key);
        match targets.as_slice() {
            [] => Effect::Message(format!("Zettelkasten: this note has no {key} note")),
            [one] => Effect::Open { path: one.clone() },
            many => {
                let items = many.iter().map(|p| self.name_of(p)).collect();
                self.pending = Some(Pending::Down(many.to_vec()));
                Effect::Ask(vec![Question::Choose {
                    prompt: format!("Go {key} to"),
                    items,
                }])
            }
        }
    }
}

/// Stage 4: the graph's lists and tree, quick capture.
impl Zettelkasten {
    /// The vault's links (worked out once until the vault changes).
    fn graph(&self, vault: &Vault) -> Rc<graph::Graph> {
        if let Some(g) = self.graph.borrow().as_ref() {
            return Rc::clone(g);
        }
        let g = Rc::new(graph::Graph::of(vault));
        *self.graph.borrow_mut() = Some(Rc::clone(&g));
        g
    }

    /// The sidebar tab's rows: (label, note).
    fn tab_rows(&self, ctx: &Context) -> Vec<(String, PathBuf)> {
        match self.tab {
            Tab::Sequence => self
                .sequence
                .iter()
                .map(|(id, path)| {
                    let label = format!("{}{}", "  ".repeat(id.depth()), self.name_of(path));
                    (label, path.clone())
                })
                .collect(),
            Tab::Links => {
                let Some(here) = Self::active(ctx) else {
                    return Vec::new();
                };
                let g = self.graph(ctx.vault);
                let mut rows = vec![(self.name_of(&here), here.clone())];
                for (depth, out, path) in g.local(&here, self.settings.depth) {
                    let arrow = if out { "→" } else { "←" };
                    let label =
                        format!("{}{arrow} {}", "  ".repeat(depth + 1), self.name_of(&path));
                    rows.push((label, path));
                }
                rows
            }
        }
    }

    /// A page listing orphan notes, dead ends or unresolved links.
    fn list_page(&self, id: &str, ctx: &Context) -> Effect {
        let g = self.graph(ctx.vault);
        let link = |p: &Path| format!("[[{}]]", text::link_name(ctx.vault, p));
        let (title, intro, items): (&str, &str, Vec<String>) = match id {
            "show-orphans" => (
                "Orphan notes",
                "Notes with no links to or from other notes.",
                g.orphans(ctx.vault).iter().map(|p| link(p)).collect(),
            ),
            "show-dead-ends" => (
                "Dead-end notes",
                "Notes that link to no other note.",
                g.dead_ends(ctx.vault).iter().map(|p| link(p)).collect(),
            ),
            _ => (
                "Unresolved links",
                "Links to notes that don't exist yet, and the notes with them.",
                g.unresolved
                    .iter()
                    .map(|(target, froms)| {
                        let froms: Vec<String> = froms.iter().map(|p| link(p)).collect();
                        format!("**{target}**: in {}", froms.join(", "))
                    })
                    .collect(),
            ),
        };
        let list = if items.is_empty() {
            "None.".to_string()
        } else {
            items
                .iter()
                .map(|i| format!("- {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        Effect::ShowText {
            title: title.into(),
            text: format!("# {title}\n\n{intro} {} in all.\n\n{list}\n", items.len()),
        }
    }

    /// Adds `content` to the inbox (made if needed), staying here.
    fn capture(&self, ctx: &Context, content: &str) -> Effect {
        if content.trim().is_empty() {
            return Effect::None;
        }
        let inbox = self.settings.inbox.trim_end_matches(".md");
        let path = ctx.vault.root.join(format!("{inbox}.md"));
        let line = composer::fill(
            &self.settings.capture_template,
            content.trim(),
            &Self::active_name(ctx),
            inbox,
        );
        let add = match ctx.vault.note(&path) {
            Some(note) => composer::add_to(note, &line, false),
            None => Effect::WriteFile { path, text: line },
        };
        Effect::Many(vec![add, Effect::Message(format!("Captured to {inbox}"))])
    }
}

/// Stage 3: the composer's commands.
impl Zettelkasten {
    /// The active note's name for links.
    fn active_name(ctx: &Context) -> String {
        Self::active(ctx)
            .map(|p| text::link_name(ctx.vault, &p))
            .unwrap_or_default()
    }

    /// "Extract selection to a note": asks where to (a new note first).
    fn extract_selection(&mut self, ctx: &Context) -> Effect {
        let Some(content) = ctx.note.and_then(|n| n.selection).filter(|s| !s.is_empty()) else {
            return Effect::Message("Zettelkasten: select the text to extract first".into());
        };
        let here = Self::active(ctx);
        let notes = composer::other_notes(ctx.vault, here.as_deref());
        let mut items = vec!["New note…".to_string()];
        items.extend(notes.iter().map(|p| text::link_name(ctx.vault, p)));
        self.pending = Some(Pending::ExtractTo {
            content: content.to_string(),
            notes,
        });
        Effect::Ask(vec![Question::Choose {
            prompt: "Extract the selection to".into(),
            items,
        }])
    }

    /// "Extract this heading": the cursor's section to a note named by its
    /// heading; a link (or what the settings say) in its place.
    fn extract_heading(&mut self, ctx: &Context) -> Effect {
        let Some(note) = ctx.note else {
            return Effect::Message("Zettelkasten: open a note first".into());
        };
        let lines = crate::vault::split_lines(note.text);
        let Some((start, _, end)) = composer::section(&lines, note.row) else {
            return Effect::Message("Zettelkasten: no heading above the cursor".into());
        };
        let (_, title) = text::heading(&lines[start]).expect("a section starts at a heading");
        let dir = Self::active(ctx)
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| ctx.folder.to_path_buf());
        let path = composer::free_path(&dir, &title);
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut body = lines[start + 1..end].to_vec();
        while body.last().is_some_and(|l| l.trim().is_empty()) {
            body.pop();
        }
        let left = self.settings.extract_leaves.text(&name);
        Effect::Many(vec![
            Effect::WriteFile {
                path,
                text: body.join("\n"),
            },
            Effect::ReplaceLines {
                from: start,
                to: end,
                lines: if left.is_empty() {
                    Vec::new()
                } else {
                    vec![left]
                },
                cursor: (start, 0),
            },
        ])
    }

    /// "Split note at headings": asks the level (those it has).
    fn split(&mut self, ctx: &Context) -> Effect {
        let Some(note) = ctx.note else {
            return Effect::Message("Zettelkasten: open a note first".into());
        };
        let lines = crate::vault::split_lines(note.text);
        let mut levels: Vec<usize> = text::headings(&lines).iter().map(|h| h.level).collect();
        levels.sort_unstable();
        levels.dedup();
        if levels.is_empty() {
            return Effect::Message("Zettelkasten: this note has no headings".into());
        }
        let items = levels
            .iter()
            .map(|&l| {
                let n = composer::sections_at(&lines, l).len();
                format!("Heading {l}: {n} note{}", if n == 1 { "" } else { "s" })
            })
            .collect();
        self.pending = Some(Pending::SplitAt(levels));
        Effect::Ask(vec![Question::Choose {
            prompt: "Split at headings of level".into(),
            items,
        }])
    }

    /// Each section at heading `level` to a note named by it; the note
    /// keeps what comes before them and a list of links.
    fn split_at(&mut self, ctx: &Context, level: usize) -> Effect {
        let Some(note) = ctx.note else {
            return Effect::None;
        };
        let lines = crate::vault::split_lines(note.text);
        let sections = composer::sections_at(&lines, level);
        let Some(&(_, first, _)) = sections.first() else {
            return Effect::None;
        };
        let dir = Self::active(ctx)
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| ctx.folder.to_path_buf());
        let mut effects = Vec::new();
        let mut kept = lines[..first].to_vec();
        let mut made: Vec<PathBuf> = Vec::new();
        for (title, start, end) in &sections {
            let mut path = composer::free_path(&dir, title);
            // Two sections with one title.
            let mut n = 1;
            while made.contains(&path) {
                path = composer::free_path(&dir, &format!("{title} {n}"));
                n += 1;
            }
            made.push(path.clone());
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut body = lines[start + 1..*end].to_vec();
            while body.last().is_some_and(|l| l.trim().is_empty()) {
                body.pop();
            }
            effects.push(Effect::WriteFile {
                path,
                text: body.join("\n"),
            });
            kept.push(format!("- [[{name}]]"));
        }
        // What came after the last section of a higher level stays.
        let last_end = sections.last().map_or(lines.len(), |s| s.2);
        kept.extend(lines[last_end..].iter().cloned());
        while kept.last().is_some_and(|l| l.is_empty()) {
            kept.pop();
        }
        effects.push(Effect::ReplaceNote {
            text: kept.join("\n"),
            cursor: None,
        });
        Effect::Many(effects)
    }

    /// "Merge this note into another": asks which.
    fn merge(&mut self, ctx: &Context) -> Effect {
        let Some(here) = Self::active(ctx) else {
            return Effect::Message("Zettelkasten: open a note first".into());
        };
        let notes = composer::other_notes(ctx.vault, Some(&here));
        let items = notes
            .iter()
            .map(|p| text::link_name(ctx.vault, p))
            .collect();
        self.pending = Some(Pending::MergeInto(notes));
        Effect::Ask(vec![Question::Choose {
            prompt: format!("Merge {} into", Self::active_name(ctx)),
            items,
        }])
    }

    /// The active note's text added to `to`, its links pointed at `to`,
    /// itself to the trash; `to` opened.
    fn merge_into(&self, ctx: &Context, to: &Path) -> Effect {
        let (Some(from), Some(target)) = (Self::active(ctx), ctx.vault.note(to)) else {
            return Effect::None;
        };
        // The text as it is in the editor (unsaved changes too).
        let text = ctx.note.map_or(String::new(), |n| n.text.to_string());
        let merged = crate::vault::Note {
            lines: crate::vault::split_lines(&text),
            ..target.clone()
        };
        let body = composer::body(&merged);
        Effect::Many(vec![
            composer::add_to(target, &body, self.settings.at_start),
            Effect::Retarget {
                from: from.clone(),
                to: to.to_path_buf(),
            },
            Effect::DeleteNote(from),
            Effect::Open {
                path: to.to_path_buf(),
            },
        ])
    }
}

impl Plugin for Zettelkasten {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Zettelkasten",
            version: "0.2.0",
            author: "blackglass, after the slip-box plugins",
            description: "Links to headings and blocks ([[Note#, [[Note#^, [[## and [[^^ \
                          anywhere), copied links, new unique notes linked from here; \
                          Folgezettel sequences, breadcrumbs from up / down / next / prev, \
                          titles shown instead of IDs.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        let get = |key: &str| values.get("", key).unwrap_or_default().trim().to_string();
        let title_property = get("title_property");
        self.settings = Settings {
            unique_folder: get("unique_folder"),
            unique_template: get("unique_template"),
            show_titles: get("show_titles") == "true",
            title_property: if title_property.is_empty() {
                "title".into()
            } else {
                title_property
            },
            extract_template: match values.get("", "extract_template") {
                Some(t) if !t.trim().is_empty() => t.to_string(),
                _ => "{{content}}".into(),
            },
            extract_leaves: composer::Leave::parse(&get("extract_leaves")),
            at_start: get("add_at") == "start",
            confirm_merge: get("confirm_merge") != "false",
            link_counts: get("link_counts") == "true",
            inbox: match get("inbox") {
                i if i.is_empty() => "Inbox".into(),
                i => i,
            },
            capture_template: match values.get("", "capture_template") {
                Some(t) if !t.trim().is_empty() => t.to_string(),
                _ => "- {{content}}".into(),
            },
            depth: get("depth").parse().unwrap_or(2).clamp(1, 5),
        };
        *self.graph.borrow_mut() = None;
        if self.settings.link_counts {
            self.graph(vault);
        }
        self.index(vault);
    }

    fn on_note_opened(&mut self, path: &Path, vault: &Vault) {
        self.here = Some(path.to_path_buf());
        self.update_trail(vault);
    }

    fn display_name(&self, path: &Path) -> Option<String> {
        if !self.settings.show_titles {
            return None;
        }
        self.titles.get(path).cloned()
    }

    fn status(&self) -> Option<String> {
        self.trail.clone()
    }

    fn settings(&self) -> Vec<Setting> {
        let text = |key: &str, label: &str, help: &str, default: &str| {
            Setting::new("", key, label, help, Kind::Text, default)
        };
        vec![
            text(
                "unique_folder",
                "Unique notes folder",
                "Where new unique notes go (in the vault; empty: the folder chosen in the explorer)",
                "",
            ),
            text(
                "unique_template",
                "Unique note template",
                "The note new unique notes start from (Templates/Zettel; empty: none)",
                "",
            ),
            Setting::new(
                "",
                "show_titles",
                "Show titles",
                "Show a note's title (its title property) instead of its name: explorer, tabs, lists",
                Kind::Toggle,
                "false",
            ),
            text(
                "title_property",
                "Title property",
                "The property a note's title is in",
                "title",
            ),
            text(
                "extract_template",
                "Extract template",
                "An extract's text: {{content}}, {{fromTitle}}, {{newTitle}}, {{date:YYYY-MM-DD}}",
                "{{content}}",
            ),
            Setting::new(
                "",
                "extract_leaves",
                "An extract leaves",
                "What's left where the extracted text was: a link to it, an embed, or nothing",
                Kind::Choice(vec!["link".into(), "embed".into(), "nothing".into()]),
                "link",
            ),
            Setting::new(
                "",
                "add_at",
                "Add text at",
                "Where extracted or merged text goes in a note: its end or its start",
                Kind::Choice(vec!["end".into(), "start".into()]),
                "end",
            ),
            Setting::new(
                "",
                "confirm_merge",
                "Ask before merging",
                "Merging a note into another deletes it (to the vault's trash): ask first",
                Kind::Toggle,
                "true",
            ),
            Setting::new(
                "",
                "link_counts",
                "Link counts",
                "Show beside each link how many notes link to its note",
                Kind::Toggle,
                "false",
            ),
            text(
                "inbox",
                "Inbox",
                "The note quick capture adds to (in the vault)",
                "Inbox",
            ),
            text(
                "capture_template",
                "Capture template",
                "A captured line: {{content}}, {{date:HH:mm}} …",
                "- {{content}}",
            ),
            text(
                "depth",
                "Local links depth",
                "How many links deep the Links tab goes (1 to 5)",
                "2",
            ),
        ]
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("new-unique-linked", "New unique note linked from here"),
            PluginCommand::new("copy-note-link", "Copy link to this note"),
            PluginCommand::new("copy-heading-link", "Copy link to this heading"),
            PluginCommand::new("copy-block-link", "Copy link to this block"),
            PluginCommand::new("new-sequel", "New sequel note"),
            PluginCommand::new("new-branch", "New branch note"),
            PluginCommand::new("sequence-parent", "Go to parent in sequence"),
            PluginCommand::new("sequence-next", "Go to next in sequence"),
            PluginCommand::new("sequence-previous", "Go to previous in sequence"),
            PluginCommand::new("show-sequence", "Show sequence"),
            PluginCommand::new("go-up", "Go up"),
            PluginCommand::new("go-down", "Go down"),
            PluginCommand::new("go-next", "Go to next note"),
            PluginCommand::new("go-previous", "Go to previous note"),
            PluginCommand::new("extract-selection", "Extract selection to another note"),
            PluginCommand::new("extract-heading", "Extract this heading"),
            PluginCommand::new("split-note", "Split note at headings"),
            PluginCommand::new("merge-note", "Merge this note into another"),
            PluginCommand::new("show-orphans", "Show orphan notes"),
            PluginCommand::new("show-dead-ends", "Show dead-end notes"),
            PluginCommand::new("show-unresolved", "Show unresolved links"),
            PluginCommand::new("show-local", "Show local links"),
            PluginCommand::new("quick-capture", "Quick capture"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        self.pending = None;
        match id {
            "new-unique-linked" => links::new_unique_linked(
                ctx,
                &self.settings.unique_folder,
                &self.settings.unique_template,
            ),
            "copy-note-link" => links::copy_note_link(ctx),
            "copy-heading-link" => links::copy_heading_link(ctx),
            "copy-block-link" => links::copy_block_link(ctx),
            "new-sequel" => self.new_in_sequence(ctx, false),
            "new-branch" => self.new_in_sequence(ctx, true),
            "sequence-parent" => self.move_in_sequence(ctx, 0),
            "sequence-next" => self.move_in_sequence(ctx, 1),
            "sequence-previous" => self.move_in_sequence(ctx, -1),
            "show-sequence" => {
                self.tab = Tab::Sequence;
                Effect::ShowSidebarTab
            }
            "go-up" => self.follow_field(ctx, "up"),
            "go-down" => self.follow_field(ctx, "down"),
            "go-next" => self.follow_field(ctx, "next"),
            "go-previous" => self.follow_field(ctx, "prev"),
            "extract-selection" => self.extract_selection(ctx),
            "extract-heading" => self.extract_heading(ctx),
            "split-note" => self.split(ctx),
            "merge-note" => self.merge(ctx),
            "show-orphans" | "show-dead-ends" | "show-unresolved" => self.list_page(id, ctx),
            "show-local" => {
                self.tab = Tab::Links;
                Effect::ShowSidebarTab
            }
            "quick-capture" => {
                self.pending = Some(Pending::Capture);
                Effect::Ask(vec![Question::Text {
                    prompt: format!("Capture to {}", self.settings.inbox),
                    default: String::new(),
                }])
            }
            _ => Effect::None,
        }
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        match (self.pending.take(), answers) {
            (Some(Pending::Title { id, dir }), [Answer::Text(title)]) => {
                let title = title.trim();
                let name = if title.is_empty() {
                    id
                } else {
                    format!("{id} {title}")
                };
                let path = dir.join(format!("{name}.md"));
                if path.exists() {
                    return Effect::Message(format!("Zettelkasten: {name} exists already"));
                }
                let mut effects = Vec::new();
                if ctx.note.is_some() {
                    effects.push(Effect::Insert {
                        text: format!("[[{name}]]"),
                        back: 0,
                    });
                }
                effects.push(Effect::CreateFile {
                    path,
                    text: String::new(),
                });
                Effect::Many(effects)
            }
            (Some(Pending::Down(targets)), [Answer::Choice(i)]) => match targets.get(*i) {
                Some(p) => Effect::Open { path: p.clone() },
                None => Effect::None,
            },
            (Some(Pending::ExtractTo { content, .. }), [Answer::Choice(0)]) => {
                self.pending = Some(Pending::ExtractName { content });
                Effect::Ask(vec![Question::Text {
                    prompt: "Name of the new note".into(),
                    default: String::new(),
                }])
            }
            (Some(Pending::ExtractTo { content, notes }), [Answer::Choice(i)]) => {
                match notes.get(i - 1).and_then(|p| ctx.vault.note(p)) {
                    Some(note) => {
                        let name = text::link_name(ctx.vault, &note.path);
                        let from = Self::active_name(ctx);
                        let filled =
                            composer::fill(&self.settings.extract_template, &content, &from, &name);
                        Effect::Many(vec![
                            composer::add_to(note, &filled, self.settings.at_start),
                            Effect::Insert {
                                text: self.settings.extract_leaves.text(&name),
                                back: 0,
                            },
                        ])
                    }
                    None => Effect::None,
                }
            }
            (Some(Pending::ExtractName { content }), [Answer::Text(name)]) => {
                if name.trim().is_empty() {
                    return Effect::None;
                }
                let dir = Self::active(ctx)
                    .and_then(|p| p.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| ctx.folder.to_path_buf());
                let path = composer::free_path(&dir, name.trim());
                let new = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let from = Self::active_name(ctx);
                let filled = composer::fill(&self.settings.extract_template, &content, &from, &new);
                Effect::Many(vec![
                    Effect::WriteFile { path, text: filled },
                    Effect::Insert {
                        text: self.settings.extract_leaves.text(&new),
                        back: 0,
                    },
                ])
            }
            (Some(Pending::SplitAt(levels)), [Answer::Choice(i)]) => match levels.get(*i) {
                Some(&level) => self.split_at(ctx, level),
                None => Effect::None,
            },
            (Some(Pending::MergeInto(notes)), [Answer::Choice(i)]) => match notes.get(*i) {
                Some(to) if self.settings.confirm_merge => {
                    let from = Self::active_name(ctx);
                    self.pending = Some(Pending::MergeSure { to: to.clone() });
                    Effect::Ask(vec![Question::Choose {
                        prompt: format!("Merge {from} into {}?", self.name_of(to)),
                        items: vec![format!("Merge and delete {from}"), "Cancel".into()],
                    }])
                }
                Some(to) => self.merge_into(ctx, to),
                None => Effect::None,
            },
            (Some(Pending::MergeSure { to }), [Answer::Choice(0)]) => self.merge_into(ctx, &to),
            (Some(Pending::Capture), [Answer::Text(content)]) => self.capture(ctx, content),
            _ => Effect::None,
        }
    }

    fn sidebar_tab(&self) -> Option<&'static str> {
        Some(match self.tab {
            Tab::Sequence => "Sequence",
            Tab::Links => "Links",
        })
    }

    fn sidebar_rows(&self, ctx: &Context) -> Vec<SidebarRow> {
        self.tab_rows(ctx)
            .into_iter()
            .map(|(label, _)| SidebarRow {
                label,
                detail: String::new(),
            })
            .collect()
    }

    fn sidebar_key(&mut self, row: usize, key: KeyEvent, ctx: &Context) -> Effect {
        match (key.code, self.tab_rows(ctx).get(row)) {
            (KeyCode::Enter, Some((_, path))) => Effect::Open { path: path.clone() },
            _ => Effect::None,
        }
    }

    fn link_badge(&self, target: &str) -> Option<String> {
        if !self.settings.link_counts {
            return None;
        }
        let graph = self.graph.try_borrow().ok()?;
        graph.as_ref()?.count(target).map(|n| n.to_string())
    }

    fn suggestions(&self, line: &str, col: usize, vault: &Vault) -> Option<Suggestions> {
        // Cheap: only inside a link.
        if !line.contains("[[") {
            return None;
        }
        links::suggestions(line, col, vault)
    }
}
