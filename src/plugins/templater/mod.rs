//! Templater, blackglass's second plugin: an implementation of Obsidian's
//! [Templater](https://silentvoid13.github.io/Templater/). Templates are
//! the notes in the templates folder (`Templates/`, or `templates_folder`
//! in `.blackglass/plugins/templater/settings.toml`); their `<% tp.* %>`
//! commands are run when a template is inserted, used for a new note,
//! applied to a note, run in place, or at startup. Folder templates
//! (`[folder_templates]`: `Books = "Book"`) and file regex templates
//! (`[file_templates]`: `"^Journal/" = "Daily"`) fill every new note they
//! match; a new note with commands in it has them run.
//!
//! A template runs as a job ([`Job`]): it may ask questions, then need
//! things from outside (web pages, user commands, notes: [`outside`]),
//! fetched on threads while the editor goes on; it's run again with each
//! answer until it has everything, then its text goes in and its file
//! actions (`create_new`, `move`, `rename`, `cursor_append`) are done.
//!
//! - [`engine`]: parsing and running a template.
//! - [`js`]: templates as JavaScript.
//! - [`outside`]: what templates fetch, run or read.

pub mod engine;
pub mod js;
pub mod outside;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDateTime};

use super::settings::{Kind, Setting, Values};
use super::{
    Answer, Context, Effect, Manifest, NOTE_CREATED, Plugin, PluginCommand, Question, intern, slug,
};
use crate::vault::Vault;
use engine::{Action, Env, More, Results, RunMode, Setup, Source, Target};

/// The folder templates are in, unless the settings say otherwise.
pub const DEFAULT_FOLDER: &str = "Templates";

/// The settings section with the folder templates (folder = template).
const FOLDER_TEMPLATES: &str = "folder_templates";
/// … with the file regex templates (regex = template).
const FILE_TEMPLATES: &str = "file_templates";
/// … with the user system commands (name = command line).
const USER_FUNCTIONS: &str = "user_functions";
/// The other sections: what new notes get, …
const NEW_NOTES: &str = "new_notes";
/// … cursors, startup and hotkeys, …
const COMMANDS: &str = "commands";
/// … `tp.web`, …
const WEB: &str = "web";
/// … scripts and system commands.
const SCRIPTS: &str = "scripts";

/// The mode a startup template runs in (its text is dropped).
const STARTUP: &str = "startup";
/// The mode a new note with commands in it runs in (they're replaced).
const OVERWRITE: &str = "overwrite";
/// How many times a template may go back for more from outside.
const MAX_ROUNDS: usize = 10;
/// Templater skips notes this big when they're created.
const SIZE_LIMIT: usize = 100_000;

/// A template being run.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Job {
    /// What's done with its text: a command's id, [`NOTE_CREATED`],
    /// [`STARTUP`] or [`OVERWRITE`].
    mode: String,
    template: String,
    /// The template note's path in the vault.
    template_file: Option<String>,
    /// The new note's name (creating one).
    name: Option<String>,
    /// Where new notes go (absolute).
    folder: PathBuf,
    /// The active note when it started: its text goes there.
    note: Option<PathBuf>,
    answers: Vec<Answer>,
    results: Results,
    rounds: usize,
}

/// What a command is doing while it waits for answers.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// Waiting for the template to be chosen from these notes (indexes
    /// into the vault's notes), for this mode.
    Choose {
        mode: String,
        notes: Vec<usize>,
    },
    /// Waiting for the new note's name.
    Name(Job),
    /// Waiting for the answers to the template's own questions (a script
    /// may ask more once it has them).
    Prompts(Job),
    /// Adding a folder template: waiting for the folder (these, as in the
    /// settings: `Books`, `/` for the top).
    Folder(Vec<String>),
    /// … then for its template (these names), in this settings section
    /// (folder or file regex templates).
    FolderTemplate {
        section: &'static str,
        key: String,
        templates: Vec<String>,
    },
    /// Adding a file regex template: waiting for the regex.
    Regex,
    /// Adding a user function: waiting for its name, then its command.
    FunctionName,
    FunctionCommand(String),
}

/// Templater's settings, read from its settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Config {
    /// (folder, template name): new notes in the folder get the template.
    folder_templates: Vec<(String, String)>,
    /// (regex, template name): new notes whose path matches get it.
    file_templates: Vec<(String, String)>,
    trigger: bool,
    /// Folders whose new notes get nothing.
    ignore: Vec<String>,
    startup: Vec<String>,
    /// Templates with their own commands.
    hotkeys: Vec<String>,
    /// The scripts folder (in the vault; `""`: none).
    scripts_folder: String,
    /// The user system commands, on or off: (name, command line).
    functions: Vec<(String, String)>,
    setup: Setup,
    timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            folder_templates: Vec::new(),
            file_templates: Vec::new(),
            trigger: true,
            ignore: Vec::new(),
            startup: Vec::new(),
            hotkeys: Vec::new(),
            scripts_folder: String::new(),
            functions: Vec::new(),
            setup: Setup::default(),
            timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Default)]
pub struct Templater {
    /// The templates folder, relative to the vault.
    folder: String,
    step: Option<Step>,
    /// The new note's name, typed before the template was chosen.
    name: Option<String>,
    config: Config,
    /// A job waiting for what it needs from outside.
    fetching: Option<(Job, outside::Fetch)>,
    /// Startup templates still to run (filled at the first tick).
    startup: VecDeque<String>,
    started: bool,
}

/// A comma-separated list's items.
fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// A path in the vault, `/` separators.
fn rel(vault: &Vault, path: &Path) -> String {
    path.strip_prefix(&vault.root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

impl Templater {
    pub fn new() -> Self {
        Templater {
            folder: DEFAULT_FOLDER.into(),
            ..Templater::default()
        }
    }

    /// The template for a new note in `folder` (in the vault, `/`
    /// separators): the nearest folder's up the tree that has one.
    fn folder_template(&self, folder: &str) -> Option<&str> {
        self.config
            .folder_templates
            .iter()
            .filter(|(f, _)| {
                let f = f.trim_matches('/');
                f.is_empty() || folder == f || folder.starts_with(&format!("{f}/"))
            })
            .max_by_key(|(f, _)| f.trim_matches('/').len())
            .map(|(_, t)| t.as_str())
    }

    /// The template for a new note at `path` (in the vault): the first
    /// file regex template that matches it, else its folder's.
    fn new_note_template(&self, path: &str) -> Option<&str> {
        let by_regex = self
            .config
            .file_templates
            .iter()
            .find(|(r, _)| regex::Regex::new(r).is_ok_and(|re| re.is_match(path)));
        if let Some((_, t)) = by_regex {
            return Some(t);
        }
        let folder = path.rsplit_once('/').map_or("", |(f, _)| f);
        self.folder_template(folder)
    }

    /// Whether new notes at `path` (in the vault) are left alone: in the
    /// templates folder or an excluded one.
    fn ignored(&self, path: &str) -> bool {
        let lower = path.to_lowercase();
        let under = |f: &str| {
            let f = f.trim_matches('/').to_lowercase();
            !f.is_empty() && lower.starts_with(&format!("{f}/"))
        };
        under(&self.folder) || self.config.ignore.iter().any(|f| under(f))
    }

    /// The template called `name` (as the choice shows it): its text and
    /// its path in the vault.
    fn template_named(&self, vault: &Vault, name: &str) -> Option<(String, String)> {
        let name = name.trim().trim_end_matches(".md");
        self.templates(vault)
            .into_iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(name))
            .map(|(i, _)| {
                let note = &vault.notes[i];
                (note.lines.join("\n"), rel(vault, &note.path))
            })
    }

    /// The folders a folder template can be for (not the templates
    /// folder): as saved (`Books`, `/` for the top).
    fn folders(&self, vault: &Vault) -> Vec<String> {
        fn walk(folder: &crate::vault::Folder, root: &Path, out: &mut Vec<String>) {
            let rel = folder.path.strip_prefix(root).unwrap_or(&folder.path);
            let rel = rel.to_string_lossy().replace('\\', "/");
            out.push(if rel.is_empty() { "/".into() } else { rel });
            for sub in &folder.folders {
                walk(sub, root, out);
            }
        }
        let mut all = Vec::new();
        walk(&vault.tree, &vault.root, &mut all);
        let templates = self.folder.trim_matches('/').to_lowercase();
        all.retain(|f| {
            let f = f.to_lowercase();
            f != templates && !f.starts_with(&format!("{templates}/"))
        });
        all
    }

    /// Saves `key = value` in `section` of the settings.
    fn save_setting(
        &mut self,
        vault: &Vault,
        section: &str,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        let file = super::settings_file(vault, "templater");
        let mut values = Values::parse(&std::fs::read_to_string(&file).unwrap_or_default());
        values.set(section, key, value);
        let dir = file.parent().expect("a settings file is in a folder");
        std::fs::create_dir_all(dir).map_err(|e| format!("Templater: cannot save: {e}"))?;
        mdedit::files::write_atomic(&file, &values.to_text(&self.settings()))
            .map_err(|e| format!("Templater: cannot save: {e}"))?;
        self.on_vault_changed(vault);
        Ok(())
    }

    /// The templates, as (index into the vault's notes, name shown).
    fn templates(&self, vault: &Vault) -> Vec<(usize, String)> {
        let prefix = format!("{}/", self.folder.trim_matches('/')).to_lowercase();
        vault
            .notes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let rel = n.rel_name().replace('\\', "/");
                rel.to_lowercase()
                    .starts_with(&prefix)
                    .then(|| (i, rel[prefix.len()..].to_string()))
            })
            .collect()
    }

    /// Asks which template to use for `mode`, or says there are none.
    fn choose(&mut self, vault: &Vault, mode: &str, prompt: &str) -> Effect {
        let templates = self.templates(vault);
        if templates.is_empty() {
            return self.no_templates();
        }
        let items = templates.iter().map(|(_, name)| name.clone()).collect();
        self.step = Some(Step::Choose {
            mode: mode.into(),
            notes: templates.into_iter().map(|(i, _)| i).collect(),
        });
        Effect::Ask(vec![Question::Choose {
            prompt: prompt.into(),
            items,
        }])
    }

    fn no_templates(&self) -> Effect {
        Effect::Message(format!(
            "Templater: no templates yet: add notes to {}/",
            self.folder
        ))
    }

    /// A new job: `template` (from `template_file`) in `mode`, for the
    /// active note or a new one called `name`.
    fn job(
        &self,
        mode: &str,
        template: String,
        template_file: Option<String>,
        name: Option<String>,
        ctx: &Context,
    ) -> Job {
        Job {
            mode: mode.into(),
            template,
            template_file,
            name,
            folder: ctx.folder.to_path_buf(),
            note: ctx.note.and_then(|n| n.path).map(Path::to_path_buf),
            answers: Vec::new(),
            results: Results::new(),
            rounds: 0,
        }
    }

    /// Starts a job for the template the user chose: creating a note, it
    /// asks the note's name first unless it has it.
    fn chosen(&mut self, mode: &str, template: String, file: String, ctx: &Context) -> Effect {
        if mode == "create-from-template" {
            let preset = self.name.take();
            let job = self.job(mode, template, Some(file), preset.clone(), ctx);
            if preset.is_none() {
                self.step = Some(Step::Name(job));
                return Effect::Ask(vec![Question::Text {
                    prompt: "Name of the new note".into(),
                    default: "Untitled".into(),
                }]);
            }
            return self.run_job(job, ctx);
        }
        let job = self.job(mode, template, Some(file), None, ctx);
        self.run_job(job, ctx)
    }

    /// What the job's template sees besides its note.
    fn more(&self, job: &Job, ctx: &Context) -> More {
        let run_mode = match job.mode.as_str() {
            "create-from-template" => RunMode::CreateNewFromTemplate,
            "insert-template" => RunMode::AppendActiveFile,
            "replace-in-file" | OVERWRITE => RunMode::OverwriteActiveFile,
            STARTUP => RunMode::StartupTemplate,
            _ => RunMode::OverwriteFile,
        };
        More {
            results: job.results.clone(),
            run_mode,
            template_file: job.template_file.clone(),
            active_file: ctx.note.and_then(|n| n.path).map(|p| rel(ctx.vault, p)),
            new_folder: rel(ctx.vault, &job.folder),
            setup: self.config.setup.clone(),
        }
    }

    /// Runs `job` as far as it goes: its questions asked, what it needs
    /// fetched (it goes on at a tick when that's done), else its text put
    /// where it goes and its actions done.
    fn run_job(&mut self, mut job: Job, ctx: &Context) -> Effect {
        loop {
            let active = ctx.note.and_then(|n| n.path).map(Path::to_path_buf);
            let for_note = !matches!(job.mode.as_str(), "create-from-template" | STARTUP);
            if for_note && job.note != active {
                return Effect::Message(
                    "Templater: another note is open now; the template wasn't used".into(),
                );
            }
            let more = self.more(&job, ctx);
            let target = match job.mode.as_str() {
                "create-from-template" => {
                    let title = job
                        .name
                        .as_deref()
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .unwrap_or("Untitled");
                    new_note_target(ctx.vault, &job.folder, title)
                }
                _ => target(ctx),
            };
            let env = Env {
                vault: ctx.vault,
                target: &target,
                now: Local::now().naive_local(),
                answers: &job.answers,
                more: &more,
            };
            let output = match engine::render(&job.template, &env) {
                Ok(o) => o,
                Err(e) => return Effect::Message(format!("Templater: {e}")),
            };
            if output.questions.len() > job.answers.len() {
                let questions = output.questions[job.answers.len()..].to_vec();
                self.step = Some(Step::Prompts(job));
                return Effect::Ask(questions);
            }
            let (created, needs) = match created_notes(&output, &job.answers, &more, ctx.vault) {
                Next::Done(created, needs) => (created, needs),
                Next::Ask(questions) => {
                    self.step = Some(Step::Prompts(job));
                    return Effect::Ask(questions);
                }
                Next::Failed(e) => return Effect::Message(format!("Templater: {e}")),
            };
            let needs: Vec<String> = needs
                .into_iter()
                .filter(|k| !job.results.contains_key(k))
                .collect();
            if needs.is_empty() {
                return self.finish(&job, output, &target, created, ctx);
            }
            job.rounds += 1;
            if job.rounds > MAX_ROUNDS {
                return Effect::Message(format!("Templater: {TOO_MANY_ROUNDS}"));
            }
            let slow = sort_needs(needs, &mut job.results, &self.config.setup, &ctx.vault.root);
            if slow.is_empty() {
                continue;
            }
            let what = slow.len();
            let fetch = outside::start(slow, &ctx.vault.root, self.config.timeout);
            self.fetching = Some((job, fetch));
            return Effect::Message(format!(
                "Templater: getting {what} thing{} from outside (web, commands)…",
                if what == 1 { "" } else { "s" }
            ));
        }
    }

    /// A finished job's effects: the notes it creates, its text where its
    /// mode puts it, text at the cursor, moves and renames (with every
    /// link), the created notes it opens.
    fn finish(
        &mut self,
        job: &Job,
        output: engine::Output,
        target: &Target,
        created: Vec<(PathBuf, String, bool)>,
        ctx: &Context,
    ) -> Effect {
        let text = output.text;
        let main = match job.mode.as_str() {
            STARTUP => Effect::None,
            "apply-template" | NOTE_CREATED => {
                // The editor's text ends with the file's line break.
                let note = ctx.note.map_or("", |n| n.text);
                let note = note.strip_suffix('\n').unwrap_or(note);
                let (text, cursor) = apply(note, &text, output.cursor);
                Effect::ReplaceNote { text, cursor }
            }
            "insert-template" => Effect::Insert {
                back: output.cursor.map_or(0, |c| text.chars().count() - c),
                text,
            },
            "replace-in-file" | OVERWRITE => Effect::ReplaceNote {
                text,
                cursor: output.cursor,
            },
            _ => Effect::CreateNote {
                folder: job.folder.clone(),
                name: target.title.clone(),
                text,
                cursor: output.cursor,
            },
        };
        file_effects(main, target, output.actions, created, &ctx.vault.root)
    }

    /// "Jump to next cursor location": the `tp.file.cursor` markers with
    /// the lowest order go, and the cursor goes to the first.
    fn jump_to_cursor(text: &str) -> Effect {
        static MARKER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let re = MARKER.get_or_init(|| {
            regex::Regex::new(r"<%[-_]?\s*tp\.file\.cursor\(\s*(\d*)\s*\)\s*[-_]?%>")
                .expect("a valid regex")
        });
        let order = |c: &regex::Captures| c[1].parse::<i64>().unwrap_or(0);
        let Some(lowest) = re.captures_iter(text).map(|c| order(&c)).min() else {
            return Effect::Message("Templater: no cursor location left in this note".into());
        };
        let mut out = String::with_capacity(text.len());
        let mut cursor = None;
        let mut last = 0;
        for c in re.captures_iter(text) {
            let m = c.get(0).expect("the whole match");
            out.push_str(&text[last..m.start()]);
            if order(&c) == lowest {
                cursor.get_or_insert(out.chars().count());
            } else {
                out.push_str(m.as_str());
            }
            last = m.end();
        }
        out.push_str(&text[last..]);
        let text = out.strip_suffix('\n').map(String::from).unwrap_or(out);
        Effect::ReplaceNote { text, cursor }
    }
}

/// Says the template keeps asking for more.
pub const TOO_MANY_ROUNDS: &str = "the template keeps asking for more (web pages, commands, notes)";

/// What comes of rendering the notes a template creates.
pub enum Next {
    /// The notes (path, text, opened or not) and what they all need.
    Done(Vec<(PathBuf, String, bool)>, Vec<String>),
    /// Questions still to answer.
    Ask(Vec<Question>),
    Failed(String),
}

/// Renders the notes `output`'s `tp.file.create_new` calls create, each
/// with its own template (their questions come after the template's own:
/// `answers` has them all). Gathers what they and `output` need.
pub fn created_notes(
    output: &engine::Output,
    answers: &[Answer],
    more: &More,
    vault: &Vault,
) -> Next {
    let mut asked = output.questions.len();
    let mut needs = output.needs.clone();
    let mut created = Vec::new();
    for action in &output.actions {
        let Action::Create {
            template,
            name,
            open,
            folder,
        } = action
        else {
            continue;
        };
        let (text, file) = match template {
            Source::File(path) => {
                let text = vault
                    .notes
                    .iter()
                    .find(|n| rel(vault, &n.path) == *path)
                    .map(|n| n.lines.join("\n"))
                    .unwrap_or_default();
                (text, Some(path.clone()))
            }
            Source::Text(text) => (text.clone(), None),
        };
        let dir = vault
            .root
            .join(folder.as_deref().unwrap_or(&more.new_folder));
        let new = new_note_target(vault, &dir, name);
        let nested = More {
            run_mode: RunMode::CreateNewFromTemplate,
            template_file: file,
            ..more.clone()
        };
        let given = answers.get(asked..).unwrap_or_default();
        let env = Env {
            vault,
            target: &new,
            now: Local::now().naive_local(),
            answers: given,
            more: &nested,
        };
        let o = match engine::render(&text, &env) {
            Ok(o) => o,
            Err(e) => return Next::Failed(format!("{name}: {e}")),
        };
        if o.questions.len() > given.len() {
            return Next::Ask(o.questions[given.len()..].to_vec());
        }
        asked += o.questions.len();
        needs.extend(o.needs);
        created.push((new.abs_path, o.text, *open));
    }
    needs.sort();
    needs.dedup();
    Next::Done(created, needs)
}

/// Settles the `needs` that are quick (notes to read, needs the settings
/// refuse) into `results`; returns the slow ones (web pages, commands),
/// for [`outside::start`].
pub fn sort_needs(
    needs: Vec<String>,
    results: &mut Results,
    setup: &Setup,
    root: &Path,
) -> Vec<String> {
    let mut slow = Vec::new();
    for key in needs {
        let result = match outside::Need::parse(&key) {
            None => Some(Err(format!("can't tell what {key} needs"))),
            Some(need) => match outside::allowed(&need, setup) {
                Err(e) => Some(Err(e)),
                Ok(()) => outside::read(root, &key),
            },
        };
        match result {
            Some(r) => {
                results.insert(key, r);
            }
            None => slow.push(key),
        }
    }
    slow
}

/// A finished template's effects: the notes it creates, `main` (its text
/// where it goes), text at the cursor, moves and renames (with every link
/// to the note), the created notes it opens.
pub fn file_effects(
    mut main: Effect,
    target: &Target,
    actions: Vec<Action>,
    created: Vec<(PathBuf, String, bool)>,
    root: &Path,
) -> Effect {
    let mut appends = Vec::new();
    let mut moves = Vec::new();
    for action in actions {
        let (from, to) = match action {
            Action::Append(text) => {
                appends.push(Effect::Insert { text, back: 0 });
                continue;
            }
            Action::Create { .. } => continue,
            Action::Rename(title) => {
                let dir = target.abs_path.parent().unwrap_or(root);
                (target.abs_path.clone(), dir.join(format!("{title}.md")))
            }
            Action::Move { file, to } => (
                file.map_or_else(|| target.abs_path.clone(), |f| root.join(f)),
                root.join(format!("{}.md", to.trim_matches('/'))),
            ),
        };
        // The note being created is created there instead.
        if let Effect::CreateNote { folder, name, .. } = &mut main
            && from == target.abs_path
        {
            let rel_to = to.strip_prefix(root).unwrap_or(&to);
            *folder = root.join(rel_to.parent().unwrap_or(Path::new("")));
            *name = to
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            continue;
        }
        moves.push(Effect::MoveNote { from, to });
    }
    let mut all: Vec<Effect> = created
        .iter()
        .map(|(path, text, _)| Effect::WriteFile {
            path: path.clone(),
            text: text.clone(),
        })
        .collect();
    if matches!(main, Effect::CreateNote { .. }) {
        // At the cursor of the note that was open.
        all.append(&mut appends);
    }
    all.push(main);
    all.append(&mut appends);
    all.append(&mut moves);
    all.extend(
        created
            .into_iter()
            .filter(|(_, _, open)| *open)
            .map(|(path, _, _)| Effect::Open { path }),
    );
    all.retain(|e| *e != Effect::None);
    match all.len() {
        0 => Effect::None,
        1 => all.pop().expect("one effect"),
        _ => Effect::Many(all),
    }
}

/// The settings templates run with (Templater's, for other plugins that
/// run templates: Periodic Notes), and how long a fetch may take.
pub fn setup(vault: &Vault) -> (Setup, Duration) {
    let mut t = Templater::new();
    t.on_vault_changed(vault);
    (t.config.setup, t.config.timeout)
}

/// The target for a new note `name` in `folder` (absolute), not yet
/// written; `name` may have folders of its own (`2026/08/2026-08-09`).
pub fn new_note_target(vault: &Vault, folder: &Path, name: &str) -> Target {
    let now = Local::now().naive_local();
    let base = folder
        .strip_prefix(&vault.root)
        .unwrap_or(folder)
        .to_string_lossy()
        .replace('\\', "/");
    let rel_path = [base.as_str(), name]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("/")
        + ".md";
    let (folder, title) = match rel_path.trim_end_matches(".md").rsplit_once('/') {
        Some((f, t)) => (f.to_string(), t.to_string()),
        None => (String::new(), rel_path.trim_end_matches(".md").to_string()),
    };
    Target {
        title,
        abs_path: vault.root.join(&rel_path),
        folder,
        rel_path,
        created: Some(now),
        modified: Some(now),
        content: String::new(),
        selection: String::new(),
    }
}

/// Puts a rendered template on top of a note's text: the template's
/// properties (frontmatter) join the note's (the note's own values stay),
/// then come the template's text and the note's. Returns the text and
/// where the template's cursor is in it.
pub fn apply(note: &str, template: &str, cursor: Option<usize>) -> (String, Option<usize>) {
    let (note_props, note_body) = split_frontmatter(note);
    let (template_props, template_body) = split_frontmatter(template);
    let props = match (note_props, template_props) {
        (None, None) => None,
        (Some(p), None) | (None, Some(p)) => Some(p.to_string()),
        (Some(mine), Some(theirs)) => Some(join_properties(mine, theirs)),
    };
    let mut out = String::new();
    if let Some(props) = props {
        out.push_str("---\n");
        out.push_str(&props);
        if !props.is_empty() && !props.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("---\n");
    }
    let body_at = out.chars().count();
    out.push_str(template_body);
    if !note_body.is_empty() && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(note_body);
    let template_body_at = template.chars().count() - template_body.chars().count();
    let cursor = cursor
        .filter(|&c| c >= template_body_at)
        .map(|c| body_at + c - template_body_at);
    (out, cursor)
}

/// A note's properties (the lines between its `---` lines) and the text
/// after them; `None` without properties.
fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return (None, text);
    };
    let mut at = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return (Some(&rest[..at]), &rest[at + line.len()..]);
        }
        at += line.len();
    }
    (None, text)
}

/// `mine`'s property lines, then `theirs`' for the keys `mine` hasn't
/// (a key's indented or list lines go with it).
fn join_properties(mine: &str, theirs: &str) -> String {
    let key = |line: &str| {
        let top = !line.starts_with([' ', '\t', '-']);
        line.split_once(':')
            .filter(|_| top)
            .map(|(k, _)| k.trim().to_string())
    };
    let have: Vec<String> = mine.lines().filter_map(key).collect();
    let mut out = mine.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    let mut taking = false;
    for line in theirs.lines() {
        if let Some(k) = key(line) {
            taking = !have.contains(&k);
        }
        if taking {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The note a template runs for: the active note.
fn target(ctx: &Context) -> Target {
    let Some(note) = ctx.note else {
        return Target::default();
    };
    let path = note.path.unwrap_or(Path::new("untitled.md"));
    let meta = std::fs::metadata(path).ok();
    let local =
        |t: std::time::SystemTime| -> NaiveDateTime { DateTime::<Local>::from(t).naive_local() };
    let rel_path = rel(ctx.vault, path);
    Target {
        title: path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        folder: rel_path.rsplit_once('/').map_or("", |(f, _)| f).to_string(),
        rel_path,
        abs_path: path.to_path_buf(),
        created: meta.as_ref().and_then(|m| m.created().ok()).map(local),
        modified: meta.as_ref().and_then(|m| m.modified().ok()).map(local),
        content: note.text.to_string(),
        selection: note.selection.unwrap_or_default().to_string(),
    }
}

impl Plugin for Templater {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "templater",
            name: "Templater",
            version: "0.2.0",
            author: "blackglass, after SilentVoid's Templater",
            description: "Templates with commands: <% tp.date.now(\"YYYY-MM-DD\") %>, <% tp.file.title %>, \
                          tp.file.cursor(), tp.file.include, tp.frontmatter, tp.system.prompt / suggester \
                          questions, tp.web (a daily quote, web requests), tp.config, tp.hooks and your own \
                          tp.user scripts and commands, with JavaScript in <%* %>. Insert a template, create a \
                          note from one, or run the commands in a note. Templates live in Templates/.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let settings =
            std::fs::read_to_string(super::settings_file(vault, "templater")).unwrap_or_default();
        let values = Values::parse(&settings);
        let get =
            |section: &str, key: &str| values.get(section, key).unwrap_or_default().to_string();
        let on = |section: &str, key: &str, default: bool| {
            values
                .get(section, key)
                .map_or(default, |v| v.trim().eq_ignore_ascii_case("true"))
        };
        self.folder = values
            .get("", "templates_folder")
            .filter(|v| !v.is_empty())
            .unwrap_or(DEFAULT_FOLDER)
            .to_string();
        let pairs = |section: &str| -> Vec<(String, String)> {
            values
                .section(section)
                .filter(|(_, v)| !v.trim().is_empty())
                .map(|(k, v)| (k.to_string(), v.trim().to_string()))
                .collect()
        };
        let scripts_folder = get(SCRIPTS, "scripts_folder")
            .trim()
            .trim_matches('/')
            .to_string();
        // The scripts: the folder's `.js` files, by their names.
        let scripts = if scripts_folder.is_empty() {
            Vec::new()
        } else {
            let prefix = format!("{}/", scripts_folder.to_lowercase());
            vault
                .files
                .iter()
                .map(|f| f.to_string_lossy().replace('\\', "/"))
                .filter(|f| f.to_lowercase().starts_with(&prefix) && f.ends_with(".js"))
                .filter_map(|f| {
                    let name = Path::new(&f).file_stem()?.to_string_lossy().into_owned();
                    let code = std::fs::read_to_string(vault.root.join(&f)).ok()?;
                    Some((name, code))
                })
                .collect()
        };
        let quotes = get(WEB, "quotes_url");
        let functions = pairs(USER_FUNCTIONS);
        self.config = Config {
            folder_templates: pairs(FOLDER_TEMPLATES),
            file_templates: pairs(FILE_TEMPLATES),
            trigger: on(NEW_NOTES, "trigger_on_file_creation", true),
            ignore: list(&get(NEW_NOTES, "ignore_folders")),
            startup: list(&get(COMMANDS, "startup_templates")),
            hotkeys: list(&get(COMMANDS, "template_hotkeys")),
            scripts_folder,
            functions: functions.clone(),
            setup: Setup {
                web: on(WEB, "web_access", true),
                quotes: if quotes.trim().is_empty() {
                    engine::QUOTES_EN.into()
                } else {
                    quotes.trim().into()
                },
                scripts,
                commands: if on(SCRIPTS, "system_commands", false) {
                    functions
                } else {
                    Vec::new()
                },
                auto_jump: on(COMMANDS, "auto_jump_to_cursor", true),
            },
            timeout: Duration::from_secs(
                get(SCRIPTS, "timeout").trim().parse().unwrap_or(5).max(1),
            ),
        };
    }

    fn on_note_created(&mut self, ctx: &Context) -> Effect {
        let Some(note) = ctx.note else {
            return Effect::None;
        };
        let Some(path) = note.path else {
            return Effect::None;
        };
        let rel_path = rel(ctx.vault, path);
        if !self.config.trigger || self.ignored(&rel_path) {
            return Effect::None;
        }
        self.step = None;
        if !note.text.trim().is_empty() {
            // A new note with commands in it: they're run.
            if note.text.contains("<%") && note.text.len() <= SIZE_LIMIT {
                let job = self.job(OVERWRITE, note.text.to_string(), None, None, ctx);
                return self.run_job(job, ctx);
            }
            return Effect::None;
        }
        let Some(name) = self.new_note_template(&rel_path).map(String::from) else {
            return Effect::None;
        };
        let Some((template, file)) = self.template_named(ctx.vault, &name) else {
            return Effect::Message(format!("Templater: no template {name} in {}/", self.folder));
        };
        let job = self.job(NOTE_CREATED, template, Some(file), None, ctx);
        self.run_job(job, ctx)
    }

    fn tick(&mut self, ctx: &Context) -> Effect {
        if !self.started {
            self.started = true;
            self.startup = self.config.startup.iter().cloned().collect();
        }
        if let Some((_, fetch)) = &mut self.fetching {
            let Some(got) = fetch.poll() else {
                return Effect::None;
            };
            let (mut job, _) = self.fetching.take().expect("fetching");
            job.results.extend(got);
            return self.run_job(job, ctx);
        }
        if self.step.is_none()
            && let Some(name) = self.startup.pop_front()
        {
            let Some((template, file)) = self.template_named(ctx.vault, &name) else {
                return Effect::Message(format!(
                    "Templater: no startup template {name} in {}/",
                    self.folder
                ));
            };
            let job = self.job(STARTUP, template, Some(file), None, ctx);
            return self.run_job(job, ctx);
        }
        Effect::None
    }

    fn settings(&self) -> Vec<Setting> {
        let text = |section: &str, key: &str, label: &str, help: &str, default: &str| {
            Setting::new(section, key, label, help, Kind::Text, default)
        };
        let toggle = |section: &str, key: &str, label: &str, help: &str, default: &str| {
            Setting::new(section, key, label, help, Kind::Toggle, default)
        };
        let mut all = vec![text(
            "",
            "templates_folder",
            "Templates folder",
            "The folder whose notes are templates (relative to the vault)",
            DEFAULT_FOLDER,
        )];
        for (folder, _) in &self.config.folder_templates {
            let shown = format!("{}/", folder.trim_end_matches('/'));
            all.push(Setting::new(
                FOLDER_TEMPLATES,
                folder,
                &shown,
                &format!("The template for new notes in {shown} (empty: none)"),
                Kind::Text,
                "",
            ));
        }
        all.push(Setting::new(
            FOLDER_TEMPLATES,
            "add",
            "Add a folder template",
            "New notes in a folder (and the folders in it) get a template",
            Kind::Action("add-folder-template"),
            "",
        ));
        for (regex, _) in &self.config.file_templates {
            all.push(Setting::new(
                FILE_TEMPLATES,
                regex,
                &format!("/{regex}/"),
                &format!("The template for new notes whose path matches /{regex}/ (empty: none)"),
                Kind::Text,
                "",
            ));
        }
        all.push(Setting::new(
            FILE_TEMPLATES,
            "add",
            "Add a file regex template",
            "New notes whose path matches a regex get a template (before folder templates)",
            Kind::Action("add-file-template"),
            "",
        ));
        all.extend([
            toggle(
                NEW_NOTES,
                "trigger_on_file_creation",
                "Trigger on new file creation",
                "New notes get their folder's (or file regex's) template; a new note with commands has them run",
                "true",
            ),
            text(
                NEW_NOTES,
                "ignore_folders",
                "Excluded folders",
                "New notes in these folders (comma-separated) get no template",
                "",
            ),
            toggle(
                COMMANDS,
                "auto_jump_to_cursor",
                "Automatic jump to cursor",
                "Jump to the first tp.file.cursor() when a template is used (off: they stay for \"Jump to next cursor location\")",
                "true",
            ),
            text(
                COMMANDS,
                "startup_templates",
                "Startup templates",
                "Templates run when blackglass starts, their text dropped (comma-separated names)",
                "",
            ),
            text(
                COMMANDS,
                "template_hotkeys",
                "Template hotkeys",
                "Templates with their own commands (\"Insert <name>\", \"Create from <name>\") to give keys to (comma-separated)",
                "",
            ),
            toggle(
                WEB,
                "web_access",
                "Web access",
                "tp.web may fetch web pages (daily quote, random picture, request)",
                "true",
            ),
            text(
                WEB,
                "quotes_url",
                "Quotes database",
                "Where tp.web.daily_quote() gets its quotes: a JSON list of {quote, author}",
                engine::QUOTES_EN,
            ),
            text(
                SCRIPTS,
                "scripts_folder",
                "Script files folder",
                "Its .js files are tp.user.<file name> (CommonJS: module.exports); empty: none",
                "",
            ),
            toggle(
                SCRIPTS,
                "system_commands",
                "System commands",
                "tp.user.<name>() runs a command line from User functions (its arguments as environment variables)",
                "false",
            ),
            text(
                SCRIPTS,
                "timeout",
                "Timeout",
                "Seconds a user command or a web page may take",
                "5",
            ),
        ]);
        for (name, _) in &self.config.functions {
            all.push(Setting::new(
                USER_FUNCTIONS,
                name,
                &format!("tp.user.{name}"),
                &format!("The command line tp.user.{name}() runs (empty: none)"),
                Kind::Text,
                "",
            ));
        }
        all.push(Setting::new(
            USER_FUNCTIONS,
            "add",
            "Add a user function",
            "A name and the command line it runs (System commands must be on)",
            Kind::Action("add-user-function"),
            "",
        ));
        all
    }

    fn commands(&self) -> Vec<PluginCommand> {
        let mut all = vec![
            PluginCommand::new("insert-template", "Insert template").keys("Alt+E"),
            PluginCommand::new("create-from-template", "Create new note from template"),
            PluginCommand::new("replace-in-file", "Replace templates in the active file"),
            PluginCommand::new("apply-template", "Apply template to this note"),
            PluginCommand::new("jump-to-cursor", "Jump to next cursor location"),
            PluginCommand::new("add-folder-template", "Add a folder template"),
            PluginCommand::new("add-file-template", "Add a file regex template"),
            PluginCommand::new("add-user-function", "Add a user function"),
        ];
        for name in &self.config.hotkeys {
            let slug = slug(name);
            all.push(PluginCommand::new(
                intern(format!("insert-{slug}")),
                intern(format!("Insert {name}")),
            ));
            all.push(PluginCommand::new(
                intern(format!("create-{slug}")),
                intern(format!("Create from {name}")),
            ));
        }
        all
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        self.step = None;
        // A new command replaces a template still waiting for the web.
        if id != "jump-to-cursor" {
            self.fetching = None;
        }
        let needs_note = matches!(
            id,
            "insert-template" | "replace-in-file" | "apply-template" | "jump-to-cursor"
        ) || (id.starts_with("insert-") && id != "insert-template");
        if needs_note && ctx.note.is_none() {
            return Effect::Message("Templater: open a note first".into());
        }
        match id {
            "insert-template" => self.choose(ctx.vault, id, "Insert template"),
            "apply-template" => self.choose(ctx.vault, id, "Apply template"),
            "jump-to-cursor" => Self::jump_to_cursor(ctx.note.map_or("", |n| n.text)),
            "add-folder-template" => {
                let folders = self.folders(ctx.vault);
                let items = folders
                    .iter()
                    .map(|f| format!("{}/", f.trim_end_matches('/')))
                    .collect();
                self.step = Some(Step::Folder(folders));
                Effect::Ask(vec![Question::Choose {
                    prompt: "Folder whose new notes get a template".into(),
                    items,
                }])
            }
            "add-file-template" => {
                self.step = Some(Step::Regex);
                Effect::Ask(vec![Question::Text {
                    prompt: "Regex for the new notes' paths (Journal/.*)".into(),
                    default: String::new(),
                }])
            }
            "add-user-function" => {
                self.step = Some(Step::FunctionName);
                Effect::Ask(vec![Question::Text {
                    prompt: "Name of the function (tp.user.<name>)".into(),
                    default: String::new(),
                }])
            }
            "create-from-template" => {
                self.name = ctx
                    .name
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .map(String::from);
                self.choose(ctx.vault, id, "Create a note from template")
            }
            "replace-in-file" => {
                let text = ctx.note.map(|n| n.text.to_string()).unwrap_or_default();
                let job = self.job(id, text, None, None, ctx);
                self.run_job(job, ctx)
            }
            _ => {
                // A template's own command.
                let (mode, slug_) = if let Some(s) = id.strip_prefix("insert-") {
                    ("insert-template", s)
                } else if let Some(s) = id.strip_prefix("create-") {
                    ("create-from-template", s)
                } else {
                    return Effect::None;
                };
                let Some(name) = self
                    .config
                    .hotkeys
                    .iter()
                    .find(|n| slug(n) == slug_)
                    .cloned()
                else {
                    return Effect::None;
                };
                let Some((template, file)) = self.template_named(ctx.vault, &name) else {
                    return Effect::Message(format!(
                        "Templater: no template {name} in {}/",
                        self.folder
                    ));
                };
                self.name = None;
                self.chosen(mode, template, file, ctx)
            }
        }
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        match (self.step.take(), answers) {
            (Some(Step::Choose { mode, notes }), [Answer::Choice(i)]) => {
                let Some(note) = notes.get(*i).and_then(|&n| ctx.vault.notes.get(n)) else {
                    return Effect::None;
                };
                let (template, file) = (note.lines.join("\n"), rel(ctx.vault, &note.path));
                self.chosen(&mode, template, file, ctx)
            }
            (Some(Step::Folder(folders)), [Answer::Choice(i)]) => {
                let Some(folder) = folders.get(*i).cloned() else {
                    return Effect::None;
                };
                let shown = format!("{}/", folder.trim_end_matches('/'));
                self.ask_template(
                    ctx,
                    FOLDER_TEMPLATES,
                    folder,
                    &format!("new notes in {shown}"),
                )
            }
            (Some(Step::Regex), [Answer::Text(regex)]) => {
                let regex = regex.trim().to_string();
                if regex.is_empty() {
                    return Effect::None;
                }
                if let Err(e) = regex::Regex::new(&regex) {
                    return Effect::Message(format!("Templater: /{regex}/ isn't a regex: {e}"));
                }
                let what = format!("new notes matching /{regex}/");
                self.ask_template(ctx, FILE_TEMPLATES, regex, &what)
            }
            (
                Some(Step::FolderTemplate {
                    section,
                    key,
                    templates,
                }),
                [Answer::Choice(i)],
            ) => {
                let Some(template) = templates.get(*i) else {
                    return Effect::None;
                };
                match self.save_setting(ctx.vault, section, &key, template) {
                    Ok(()) if section == FOLDER_TEMPLATES => Effect::Message(format!(
                        "Templater: new notes in {}/ get the {template} template",
                        key.trim_end_matches('/')
                    )),
                    Ok(()) => Effect::Message(format!(
                        "Templater: new notes matching /{key}/ get the {template} template"
                    )),
                    Err(e) => Effect::Message(e),
                }
            }
            (Some(Step::FunctionName), [Answer::Text(name)]) => {
                let name = name.trim();
                if name.is_empty() {
                    return Effect::None;
                }
                if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Effect::Message(
                        "Templater: a function's name is letters, digits and _".into(),
                    );
                }
                self.step = Some(Step::FunctionCommand(name.to_string()));
                Effect::Ask(vec![Question::Text {
                    prompt: format!("Command line tp.user.{name}() runs"),
                    default: String::new(),
                }])
            }
            (Some(Step::FunctionCommand(name)), [Answer::Text(command)]) => {
                match self.save_setting(ctx.vault, USER_FUNCTIONS, &name, command.trim()) {
                    Ok(()) if self.config.setup.commands.is_empty() => Effect::Message(format!(
                        "Templater: tp.user.{name}() added; turn System commands on to run it"
                    )),
                    Ok(()) => Effect::Message(format!("Templater: tp.user.{name}() added")),
                    Err(e) => Effect::Message(e),
                }
            }
            (Some(Step::Name(mut job)), [Answer::Text(name)]) => {
                job.name = Some(name.clone());
                self.run_job(job, ctx)
            }
            (Some(Step::Prompts(mut job)), answers) => {
                job.answers.extend_from_slice(answers);
                self.run_job(job, ctx)
            }
            _ => Effect::None,
        }
    }
}

impl Templater {
    /// Asks which template `key` of `section` gets (for `what`).
    fn ask_template(
        &mut self,
        ctx: &Context,
        section: &'static str,
        key: String,
        what: &str,
    ) -> Effect {
        let templates: Vec<String> = self
            .templates(ctx.vault)
            .into_iter()
            .map(|(_, name)| name)
            .collect();
        if templates.is_empty() {
            return self.no_templates();
        }
        self.step = Some(Step::FolderTemplate {
            section,
            key,
            templates: templates.clone(),
        });
        Effect::Ask(vec![Question::Choose {
            prompt: format!("Template for {what}"),
            items: templates,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_goes_on_top_and_properties_are_joined() {
        let template = "---\nstatus: new\ntags:\n  - book\n---\n# T\n";
        let cursor = Some(template.find("# T").unwrap() + 3);
        let (text, at) = apply("---\nstatus: read\n---\nBody", template, cursor);
        assert_eq!(text, "---\nstatus: read\ntags:\n  - book\n---\n# T\nBody");
        assert_eq!(
            at,
            Some(text.find("# T").unwrap() + 3),
            "the cursor follows"
        );
        let (text, _) = apply("Body", "# T", None);
        assert_eq!(text, "# T\nBody", "a line break between");
        let (text, _) = apply("", "---\na: 1\n---\n", None);
        assert_eq!(text, "---\na: 1\n---\n");
        let (text, at) = apply("---\nx: 1\n---\n", "# T", Some(1));
        assert_eq!((text.as_str(), at), ("---\nx: 1\n---\n# T", Some(14)));
    }

    #[test]
    fn the_nearest_folder_template_wins() {
        let mut t = Templater::new();
        t.config.folder_templates = vec![
            ("/".into(), "Note".into()),
            ("Books".into(), "Book".into()),
            ("Books/Comics".into(), "Comic".into()),
        ];
        assert_eq!(t.folder_template("Books/Comics/Old"), Some("Comic"));
        assert_eq!(t.folder_template("Books/Sci-fi"), Some("Book"));
        assert_eq!(t.folder_template("Bookshelf"), Some("Note"), "not a prefix");
        assert_eq!(t.folder_template(""), Some("Note"));
        t.config.folder_templates.remove(0);
        assert_eq!(t.folder_template("Journal"), None);
    }
}
