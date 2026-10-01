//! Git, a blackglass plugin: backs the vault up to a Git repository and
//! keeps it in sync, by running the `git` program (the user's own setup:
//! config, SSH keys, credential helpers) in the background. A Git tab in
//! the sidebar lists the changed files (stage, discard, commit); diffs and
//! the history show in read-only tabs. See
//! `requirements/git_requirements.md`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDateTime};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Color, Style};
use ratatui::text::Line;

use super::settings::{Kind, Setting, Values};
use super::{Answer, Context, Effect, Manifest, Plugin, PluginCommand, Question, SidebarRow};
use crate::vault::Vault;

/// The plugin's id.
const ID: &str = "git";

/// The default commit message.
pub const DEFAULT_MESSAGE: &str = "vault backup: {{date}}";

/// How often the status is checked while nothing else runs.
const STATUS_EVERY: Duration = Duration::from_secs(30);

/// The plugin's settings, as read from its `settings.toml`.
#[derive(Debug, Clone)]
struct Settings {
    message: String,
    /// For automatic commits (empty: `message`).
    auto_message: String,
    date_format: String,
    /// The changed files listed in the message's body.
    list_files: bool,
    /// A shell command whose output is the message (empty: off).
    script: String,
    /// Minutes between automatic commit-and-syncs (0: off).
    interval: f64,
    /// … counted from the last change instead.
    after_change: bool,
    /// Minutes between automatic pushes / pulls of their own (0: with the
    /// commit-and-sync).
    push_interval: f64,
    pull_interval: f64,
    /// Automatic commits commit only what's staged.
    staged_only: bool,
    pull_on_start: bool,
    /// Commit-and-sync pulls before it pushes.
    pull_before_push: bool,
    /// How a pull brings changes in: merge, rebase or reset.
    method: String,
    /// `-X ours` / `-X theirs` for merges (empty: git's default).
    strategy: String,
    push: bool,
    /// Submodules updated after a pull.
    submodules: bool,
    /// Leave "nothing to commit" and the like out of messages.
    quiet: bool,
    /// When the last commit was, in the status bar.
    last_commit: bool,
    /// How often the status is checked.
    refresh: Duration,
    /// What line authoring shows: "author and date", "author" or "date".
    blame: String,
    /// The Git tab shows the changes under their folders.
    tree: bool,
    /// The commits waiting are squashed into one before a push.
    squash: bool,
    /// Diffs side by side (a Before / After table).
    split: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            message: DEFAULT_MESSAGE.into(),
            auto_message: String::new(),
            date_format: "YYYY-MM-DD HH:mm:ss".into(),
            list_files: false,
            script: String::new(),
            interval: 0.0,
            after_change: false,
            push_interval: 0.0,
            pull_interval: 0.0,
            staged_only: false,
            pull_on_start: false,
            pull_before_push: true,
            method: "merge".into(),
            strategy: String::new(),
            push: true,
            submodules: false,
            quiet: false,
            last_commit: true,
            refresh: STATUS_EVERY,
            blame: "author and date".into(),
            tree: false,
            squash: false,
            split: false,
        }
    }
}

/// What a background job ends with.
#[derive(Default)]
struct Done {
    /// For the status bar (none for a status check).
    message: Option<String>,
    /// Files in the vault changed (a pull brought something, a discard).
    changed: bool,
    /// The status bar line after the job.
    status: Option<String>,
    /// The changed files after the job.
    changes: Vec<Change>,
    /// A page to show (a diff, a commit): (title, Markdown).
    page: Option<(String, String)>,
    /// The log to choose a commit from: (hash, shown).
    log: Option<Vec<(String, String)>>,
    /// Branches or remotes to choose from.
    list: Option<(ListFor, Vec<String>)>,
    /// A note's margin.
    margin: Option<Margin>,
    /// A folder to open as the vault (a clone).
    open_vault: Option<PathBuf>,
    /// A web page to open (the file on GitHub).
    url: Option<String>,
    /// The job failed.
    failed: bool,
}

/// A changed file, from `git status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Relative to the repository, `/` separators.
    pub path: String,
    /// "modified", "new", "deleted", "renamed" or "conflict".
    pub state: &'static str,
    /// Its changes are staged (for the next commit).
    pub staged: bool,
}

/// A job for the background thread.
#[derive(Debug, Clone)]
enum Job {
    /// A command by its id (commit-and-sync, pull, status …).
    Command(String),
    Stage(String),
    Unstage(String),
    /// A file's changes thrown away (`true`: a new file, deleted).
    Discard(String, bool),
    CommitStaged(String),
    Diff(String),
    Log,
    Show(String),
    /// `git args…`, then say `done` (`changed`: files may have changed).
    Run {
        args: Vec<String>,
        done: String,
        changed: bool,
    },
    /// Something to choose from, for `what`.
    List(ListFor),
    /// A remote set to a URL (added, or changed if it's there).
    SetRemote(String, String),
    /// `git …` as typed; its output in a page.
    Raw(String),
    /// A repository cloned into a folder (then opened as the vault).
    Clone(String, PathBuf),
    /// A commit: of what, with this message (else the template), amending
    /// the last commit.
    Commit {
        scope: Scope,
        message: Option<String>,
        amend: bool,
    },
    /// The note at `rel` on the remote's web site (its history).
    Web {
        rel: String,
        history: bool,
    },
    /// The change on `row` of the note at `rel`: previewed, staged or
    /// reset.
    Hunk {
        rel: String,
        row: usize,
        action: HunkAction,
    },
    /// The margin beside a note: its changed lines, or who wrote each line
    /// (`blame`); `show` false clears it.
    Marks {
        path: PathBuf,
        blame: bool,
        show: bool,
    },
}

/// A note's margin: the note, the margin's width, a mark per line, and the
/// lines where changes start (for "Go to next change").
type Margin = (PathBuf, u16, Vec<(usize, Line<'static>)>, Vec<usize>);

/// What a commit takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    All,
    Staged,
    /// What's staged, or everything when nothing is.
    StagedOrAll,
}

/// What to do with one change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HunkAction {
    Preview,
    Stage,
    Reset,
}

/// What a list to choose from is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListFor {
    SwitchBranch,
    RemoteBranch,
    DeleteBranch,
    RemoveRemote,
}

/// What the question being asked is for.
#[derive(Debug, Clone)]
enum Pending {
    Discard(String, bool),
    DiscardAll,
    DeleteRepo,
    Init,
    Message(Scope),
    Commit,
    Log(Vec<String>),
    Choose(ListFor, Vec<String>),
    NewBranch,
    Remote,
    Raw,
    Clone,
}

pub struct Git {
    root: PathBuf,
    settings: Settings,
    /// The job running in the background, if any.
    job: Option<Receiver<Done>>,
    /// When commit-and-sync last ran (for the automatic one).
    last_sync: Instant,
    /// Automatic routines paused (a command).
    paused: bool,
    /// Whether the pull on start has been started.
    started: bool,
    /// The status bar line (branch, changes, commits to push).
    status: Option<String>,
    /// When the status was last checked.
    last_status: Option<Instant>,
    /// The changed files, for the Git tab.
    changes: Vec<Change>,
    /// What the question being asked is for.
    pending: Option<Pending>,
    /// The note the margin was worked out for, and whether it's out of
    /// date (the note was saved).
    margin_for: Option<PathBuf>,
    margin_stale: bool,
    /// Where the open note's changes start (its lines).
    hunks: Vec<usize>,
    /// The margin shows who wrote each line, not the changes.
    blame: bool,
    /// The changes are shown in the margin (a setting).
    show_marks: bool,
    /// When a note was last saved (for commits after the last change).
    last_change: Option<Instant>,
    /// When the own-timer pull and push last ran.
    last_pull: Instant,
    last_push: Instant,
    /// Quit when the running job is done ("Commit-and-sync and then close").
    quit_after: bool,
}

impl Default for Git {
    fn default() -> Self {
        Self::new()
    }
}

impl Git {
    pub fn new() -> Self {
        Git {
            root: PathBuf::new(),
            settings: Settings::default(),
            job: None,
            last_sync: Instant::now(),
            paused: false,
            started: false,
            status: None,
            last_status: None,
            changes: Vec::new(),
            pending: None,
            margin_for: None,
            margin_stale: false,
            hunks: Vec::new(),
            blame: false,
            show_marks: true,
            last_change: None,
            last_pull: Instant::now(),
            last_push: Instant::now(),
            quit_after: false,
        }
    }

    /// Starts `command` in the background; `false` if a job is running.
    fn start(&mut self, command: &str) -> bool {
        self.start_job(Job::Command(command.to_string()))
    }

    /// Starts `job` in the background; `false` if one is running. Every job
    /// ends with the status and the changed files worked out again.
    fn start_job(&mut self, job: Job) -> bool {
        if self.job.is_some() {
            return false;
        }
        if matches!(&job, Job::Command(c) if c.contains("sync")) {
            self.last_sync = Instant::now();
        }
        let (tx, rx) = channel();
        let (root, settings) = (self.root.clone(), self.settings.clone());
        std::thread::spawn(move || {
            keep_session_out(&root);
            let auto = matches!(&job, Job::Command(c) if c.starts_with("auto-"));
            let result = match &job {
                Job::Command(command) => run(&root, &settings, command),
                other => run_job(&root, &settings, other),
            };
            let mut done = result.unwrap_or_else(|e| Done {
                // An automatic routine's error says it's automatic.
                message: Some(if auto {
                    format!("Git (automatic): {e}")
                } else {
                    format!("Git: {e}")
                }),
                failed: true,
                ..Done::default()
            });
            // A repository the job just made, too.
            keep_session_out(&root);
            done.status = status_line(&root, &settings);
            done.changes = changes(&root);
            let _ = tx.send(done);
        });
        self.job = Some(rx);
        self.last_status = Some(Instant::now());
        true
    }

    /// Starts `job`, or says the last one still runs.
    fn start_or_say(&mut self, job: Job) -> Effect {
        if self.start_job(job) {
            Effect::None
        } else {
            Effect::Message("Git: still working on the last command".into())
        }
    }
}

/// `git args…` in `dir`: its output, or what it said went wrong.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("cannot run git ({e}); is it installed?"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("failed");
        Err(format!("git {}: {}", args[0], first.trim()))
    }
}

/// `git args…` in `dir` with `input` on its standard input (a patch).
fn git_with_input(dir: &Path, args: &[&str], input: &str) -> Result<String, String> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run git ({e}); is it installed?"))?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .map_err(|e| format!("git {}: {e}", args[0]))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git {}: {e}", args[0]))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("failed");
        Err(format!("git {}: {}", args[0], first.trim()))
    }
}

/// A diff's header (the lines before its first `@@`) and its changes (each
/// `@@` line with the lines after it).
fn split_hunks(diff: &str) -> (String, Vec<String>) {
    let mut header = String::new();
    let mut hunks: Vec<String> = Vec::new();
    for line in diff.split_inclusive('\n') {
        if line.starts_with("@@") {
            hunks.push(line.to_string());
        } else if let Some(last) = hunks.last_mut() {
            last.push_str(line);
        } else {
            header.push_str(line);
        }
    }
    (header, hunks)
}

/// Whether the change `hunk` (a `@@ -a,b +c,d @@` header first) is on
/// line `row` (from 0) of the new text; lines deleted are on the line
/// before them.
fn hunk_covers(hunk: &str, row: usize) -> bool {
    let Some(new) = hunk.split_whitespace().nth(2) else {
        return false;
    };
    let new = new.trim_start_matches('+');
    let (start, count) = new.split_once(',').unwrap_or((new, "1"));
    let (start, count): (usize, usize) = (start.parse().unwrap_or(0), count.parse().unwrap_or(1));
    if count == 0 {
        row == start.saturating_sub(1)
    } else {
        (start - 1..start - 1 + count).contains(&row)
    }
}

/// The commit message: `{{date}}` (in `date_format`, moment.js tokens),
/// `{{hostname}}`, `{{numFiles}}`, `{{files}}`.
pub fn message(
    template: &str,
    date_format: &str,
    now: NaiveDateTime,
    host: &str,
    files: &[String],
) -> String {
    let format = if date_format.trim().is_empty() {
        "YYYY-MM-DD HH:mm:ss"
    } else {
        date_format
    };
    template
        .replace("{{date}}", &super::moment::format(&now, format))
        .replace("{{hostname}}", host)
        .replace("{{numFiles}}", &files.len().to_string())
        .replace("{{files}}", &files.join(", "))
}

/// This computer's name, for `{{hostname}}`.
fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "this computer".into())
}

/// Keeps this computer's session (the open tabs: `.blackglass/
/// workspace.json`) out of the repository at `root` (its `.git` folder):
/// in the repository's own exclude list, which isn't committed. Nothing
/// outside a repository, or when it's listed already.
fn keep_session_out(root: &Path) {
    let git = root.join(".git");
    if !git.is_dir()
        || !root
            .join(crate::session::FILE)
            .parent()
            .is_some_and(Path::exists)
    {
        return;
    }
    let exclude = git.join("info").join("exclude");
    let pattern = format!("/{}", crate::session::FILE);
    let text = std::fs::read_to_string(&exclude).unwrap_or_default();
    if text.lines().any(|l| l.trim() == pattern) {
        return;
    }
    let mut new = text;
    if !new.is_empty() && !new.ends_with('\n') {
        new.push('\n');
    }
    new.push_str(&format!(
        "# blackglass: this computer's open tabs\n{pattern}\n"
    ));
    if let Some(dir) = exclude.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&exclude, new);
}

/// The status bar line: the branch, the changed files and the commits to
/// push (`main · 2 changed · 1 to push`); `None` outside a repository.
fn status_line(root: &Path, settings: &Settings) -> Option<String> {
    git(root, &["rev-parse", "--is-inside-work-tree"]).ok()?;
    let mut line = git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|b| b.trim().to_string())
        .unwrap_or_else(|_| "no commits yet".into());
    let changed = git(root, &["status", "--porcelain"]).map_or(0, |s| s.lines().count());
    if changed > 0 {
        line.push_str(&format!(" · {changed} changed"));
    }
    let ahead = git(root, &["rev-list", "--count", "@{u}..HEAD"])
        .ok()
        .and_then(|n| n.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if ahead > 0 {
        line.push_str(&format!(" · {ahead} to push"));
    }
    if settings.last_commit
        && let Ok(when) = git(root, &["log", "-1", "--format=%ct"])
        && let Ok(when) = when.trim().parse::<i64>()
    {
        let ago = (Local::now().timestamp() - when).max(0);
        line.push_str(&format!(" · last commit {}", ago_text(ago)));
    }
    Some(line)
}

/// `seconds` ago in words: just now, 5 min ago, 3 h ago, 2 days ago.
fn ago_text(seconds: i64) -> String {
    match seconds {
        s if s < 60 => "just now".into(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        s => {
            let days = s / 86_400;
            format!("{days} day{} ago", if days == 1 { "" } else { "s" })
        }
    }
}

/// The web page of `rel` on a GitHub remote (`git@github.com:u/r.git` or
/// `https://github.com/u/r`), on `branch`; its history with `history`.
pub fn github_url(remote: &str, branch: &str, rel: &str, history: bool) -> Option<String> {
    let remote = remote.trim().trim_end_matches('/').trim_end_matches(".git");
    let repo = remote
        .strip_prefix("git@github.com:")
        .or_else(|| remote.strip_prefix("https://github.com/"))
        .or_else(|| remote.strip_prefix("ssh://git@github.com/"))?;
    let kind = if history { "commits" } else { "blob" };
    Some(format!("https://github.com/{repo}/{kind}/{branch}/{rel}"))
}

/// The changed files (`git status --porcelain`), by path; empty outside a
/// repository.
fn changes(root: &Path) -> Vec<Change> {
    let Ok(out) = git(root, &["status", "--porcelain", "-uall"]) else {
        return Vec::new();
    };
    let mut all: Vec<Change> = out
        .lines()
        .filter(|l| l.len() > 3)
        .map(|l| {
            let (x, y) = (l.as_bytes()[0] as char, l.as_bytes()[1] as char);
            let path = l[3..]
                .rsplit(" -> ")
                .next()
                .unwrap_or_default()
                .trim_matches('"');
            let state = match (x, y) {
                ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => "conflict",
                ('?', _) | ('A', _) => "new",
                ('D', _) | (_, 'D') => "deleted",
                ('R', _) => "renamed",
                _ => "modified",
            };
            Change {
                path: path.to_string(),
                state,
                staged: x != ' ' && x != '?',
            }
        })
        .collect();
    all.sort_by_key(|c| c.path.to_lowercase());
    all
}

/// A note's changes against the last commit as marks: `+` added, `~`
/// changed, `-` lines deleted below; and where each change starts. A new
/// file is all added.
fn change_marks(
    root: &Path,
    path: &Path,
    rel: &str,
) -> (u16, Vec<(usize, Line<'static>)>, Vec<usize>) {
    let mark = |c: &'static str, color: Color| Line::styled(c, Style::default().fg(color));
    let tracked = git(root, &["ls-files", "--error-unmatch", "--", rel]).is_ok();
    if !tracked {
        let lines = std::fs::read_to_string(path).map_or(0, |t| t.lines().count());
        let marks = (0..lines).map(|l| (l, mark("+", Color::Green))).collect();
        return (2, marks, if lines > 0 { vec![0] } else { Vec::new() });
    }
    let Ok(diff) = git(root, &["diff", "-U0", "HEAD", "--", rel]) else {
        return (2, Vec::new(), Vec::new());
    };
    let mut marks = Vec::new();
    let mut hunks = Vec::new();
    for header in diff.lines().filter(|l| l.starts_with("@@")) {
        // @@ -a[,b] +c[,d] @@
        let mut parts = header.split_whitespace().skip(1);
        let (Some(old), Some(new)) = (parts.next(), parts.next()) else {
            continue;
        };
        let count = |range: &str| -> (usize, usize) {
            let range = &range[1..];
            let (start, n) = range.split_once(',').unwrap_or((range, "1"));
            (start.parse().unwrap_or(0), n.parse().unwrap_or(1))
        };
        let ((_, removed), (start, added)) = (count(old), count(new));
        if added == 0 {
            // Deleted lines: marked on the line before them.
            let at = start.saturating_sub(1);
            hunks.push(at);
            marks.push((at, mark("-", Color::Red)));
            continue;
        }
        hunks.push(start - 1);
        let (sign, color) = if removed == 0 {
            ("+", Color::Green)
        } else {
            ("~", Color::Yellow)
        };
        marks.extend((start - 1..start - 1 + added).map(|l| (l, mark(sign, color))));
    }
    (2, marks, hunks)
}

/// Who wrote each line and when (`git blame`); lines not committed yet say
/// so.
fn blame_marks(
    root: &Path,
    rel: &str,
    format: &str,
) -> (u16, Vec<(usize, Line<'static>)>, Vec<usize>) {
    let Ok(out) = git(root, &["blame", "--line-porcelain", "--", rel]) else {
        return (0, Vec::new(), Vec::new());
    };
    let mut marks = Vec::new();
    let (mut author, mut time) = (String::new(), 0i64);
    for line in out.lines() {
        if let Some(name) = line.strip_prefix("author ") {
            author = name.to_string();
        } else if let Some(t) = line.strip_prefix("author-time ") {
            time = t.parse().unwrap_or(0);
        } else if line.starts_with('\t') {
            let (text, color) = if author == "Not Committed Yet" {
                ("not committed".to_string(), Color::Green)
            } else {
                let date = chrono::DateTime::from_timestamp(time, 0)
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                let name: String = author.chars().take(14).collect();
                let text = match format {
                    "author" => name,
                    "date" => date,
                    _ => format!("{name} {date}"),
                };
                let age = (Local::now().timestamp() - time).max(0) / 86_400;
                (text, age_color(age))
            };
            marks.push((marks.len(), Line::styled(text, Style::default().fg(color))));
        }
    }
    let width = marks
        .iter()
        .map(|(_, l)| l.width() as u16)
        .max()
        .unwrap_or(0)
        + 1;
    (width.min(28), marks, Vec::new())
}

/// A line author's color by the line's age in days: newer is brighter.
pub fn age_color(days: i64) -> Color {
    match days {
        d if d < 7 => Color::LightGreen,
        d if d < 30 => Color::Green,
        d if d < 365 => Color::Yellow,
        _ => Color::DarkGray,
    }
}

/// A diff side by side: per change, a table of the lines before and after
/// (paired in order; a line only on one side leaves the other empty).
pub fn split_page(diff: &str) -> String {
    let cell = |s: &str| s.replace('|', "\\|");
    let mut out = String::new();
    let (_, hunks) = split_hunks(diff);
    for hunk in hunks {
        let mut lines = hunk.lines();
        let header = lines.next().unwrap_or_default();
        out.push_str(&format!(
            "`{}`\n\n| Before | After |\n| --- | --- |\n",
            header.trim()
        ));
        let (mut old, mut new): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
        let flush = |out: &mut String, old: &mut Vec<&str>, new: &mut Vec<&str>| {
            for i in 0..old.len().max(new.len()) {
                let (a, b) = (
                    old.get(i).copied().unwrap_or(""),
                    new.get(i).copied().unwrap_or(""),
                );
                out.push_str(&format!("| {} | {} |\n", cell(a), cell(b)));
            }
            old.clear();
            new.clear();
        };
        for line in lines {
            if let Some(l) = line.strip_prefix('-') {
                old.push(l);
            } else if let Some(l) = line.strip_prefix('+') {
                new.push(l);
            } else {
                flush(&mut out, &mut old, &mut new);
                let same = line.strip_prefix(' ').unwrap_or(line);
                out.push_str(&format!("| {} | {} |\n", cell(same), cell(same)));
            }
        }
        flush(&mut out, &mut old, &mut new);
        out.push('\n');
    }
    out
}

/// A Markdown page with `text` in a `diff` code block.
fn diff_page(text: &str) -> String {
    format!("```diff\n{}\n```\n", text.trim_end())
}

/// Runs a job other than a command (in the background thread).
fn run_job(root: &Path, settings: &Settings, job: &Job) -> Result<Done, String> {
    let said = |m: String| Done {
        message: Some(m),
        ..Done::default()
    };
    Ok(match job {
        Job::Command(_) => unreachable!("commands run through `run`"),
        Job::Stage(path) => {
            git(root, &["add", "--", path])?;
            said(format!("Git: staged {path}"))
        }
        Job::Unstage(path) => {
            git(root, &["restore", "--staged", "--", path])?;
            said(format!("Git: unstaged {path}"))
        }
        Job::Discard(path, new) => {
            if *new {
                std::fs::remove_file(root.join(path))
                    .map_err(|e| format!("cannot delete {path}: {e}"))?;
            } else {
                git(root, &["restore", "--staged", "--worktree", "--", path])?;
            }
            Done {
                message: Some(format!("Git: discarded the changes to {path}")),
                changed: true,
                ..Done::default()
            }
        }
        Job::CommitStaged(text) => {
            git(root, &["commit", "-m", text])?;
            said("Git: committed the staged files".into())
        }
        Job::Diff(path) => {
            let diff = git(root, &["diff", "HEAD", "--", path])?;
            let name = path.rsplit('/').next().unwrap_or(path);
            if diff.trim().is_empty() {
                said(format!("Git: no changes in {name}"))
            } else {
                let page = if settings.split {
                    split_page(&diff)
                } else {
                    diff_page(&diff)
                };
                Done {
                    message: Some(format!("Git: the diff of {name}")),
                    page: Some((format!("Diff: {name}"), page)),
                    ..Done::default()
                }
            }
        }
        Job::Log => {
            let out = git(
                root,
                &[
                    "log",
                    "-n",
                    "100",
                    "--date=short",
                    "--pretty=format:%h%x1f%s%x1f%an%x1f%ad",
                ],
            )?;
            let log = out
                .lines()
                .filter_map(|l| {
                    let mut f = l.split('\u{1f}');
                    let (hash, subject) = (f.next()?, f.next()?);
                    let (author, date) = (f.next()?, f.next()?);
                    Some((
                        hash.to_string(),
                        format!("{hash}  {subject}  ({author}, {date})"),
                    ))
                })
                .collect();
            Done {
                log: Some(log),
                ..Done::default()
            }
        }
        Job::Show(hash) => {
            let out = git(root, &["show", "--stat", "--patch", hash])?;
            Done {
                message: Some(format!("Git: commit {hash}")),
                page: Some((format!("Commit {hash}"), diff_page(&out))),
                ..Done::default()
            }
        }
        Job::Run {
            args,
            done,
            changed,
        } => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            git(root, &args)?;
            Done {
                message: Some(done.clone()),
                changed: *changed,
                ..Done::default()
            }
        }
        Job::List(what) => {
            let items: Vec<String> = match what {
                ListFor::SwitchBranch | ListFor::DeleteBranch => {
                    let current = git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    git(root, &["branch", "--format=%(refname:short)"])?
                        .lines()
                        .map(str::trim)
                        .filter(|b| !b.is_empty() && *b != current)
                        .map(String::from)
                        .collect()
                }
                ListFor::RemoteBranch => git(root, &["branch", "-r", "--format=%(refname:short)"])?
                    .lines()
                    .map(str::trim)
                    .filter(|b| !b.is_empty() && !b.ends_with("/HEAD") && b.contains('/'))
                    .map(String::from)
                    .collect(),
                ListFor::RemoveRemote => git(root, &["remote"])?
                    .lines()
                    .map(|r| r.trim().to_string())
                    .filter(|r| !r.is_empty())
                    .collect(),
            };
            Done {
                list: Some((*what, items)),
                ..Done::default()
            }
        }
        Job::SetRemote(name, url) => {
            let exists = git(root, &["remote"])?.lines().any(|r| r.trim() == name);
            let verb = if exists { "set-url" } else { "add" };
            git(root, &["remote", verb, name, url])?;
            said(format!("Git: the remote {name} is {url}"))
        }
        Job::Marks { path, blame, show } => {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let margin = if *blame {
                blame_marks(root, &rel, &settings.blame)
            } else if *show {
                change_marks(root, path, &rel)
            } else {
                (0, Vec::new(), Vec::new())
            };
            Done {
                margin: Some((path.clone(), margin.0, margin.1, margin.2)),
                ..Done::default()
            }
        }
        Job::Clone(url, folder) => {
            let parent = folder.parent().unwrap_or(root);
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot make {}: {e}", parent.display()))?;
            git(parent, &["clone", url, &folder.to_string_lossy()])?;
            Done {
                message: Some(format!("Git: cloned into {}", folder.display())),
                open_vault: Some(folder.clone()),
                ..Done::default()
            }
        }
        Job::Hunk { rel, row, action } => {
            let diff = git(root, &["diff", "-U0", "--", rel])?;
            let (header, hunks) = split_hunks(&diff);
            let line = row + 1;
            let Some(hunk) = hunks.iter().find(|h| hunk_covers(h, *row)) else {
                return Ok(said(format!("Git: no change on line {line}")));
            };
            let patch = format!("{header}{hunk}");
            match action {
                HunkAction::Preview => Done {
                    page: Some((format!("Change at line {line}"), diff_page(&patch))),
                    ..Done::default()
                },
                HunkAction::Stage => {
                    git_with_input(root, &["apply", "--cached", "--unidiff-zero", "-"], &patch)?;
                    said(format!("Git: staged the change at line {line}"))
                }
                HunkAction::Reset => {
                    git_with_input(root, &["apply", "-R", "--unidiff-zero", "-"], &patch)?;
                    Done {
                        message: Some(format!("Git: reset the change at line {line}")),
                        changed: true,
                        ..Done::default()
                    }
                }
            }
        }
        Job::Commit {
            scope,
            message,
            amend,
        } => {
            let said = commit(root, settings, *scope, message.as_deref(), *amend, false)?;
            Done {
                message: Some(format!("Git: {said}")),
                ..Done::default()
            }
        }
        Job::Web { rel, history } => {
            let remote = git(root, &["remote"])?
                .lines()
                .next()
                .map(str::to_string)
                .ok_or("no remote")?;
            let url = git(root, &["remote", "get-url", &remote])?;
            let branch = git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
            let page = github_url(&url, branch.trim(), rel, *history)
                .ok_or_else(|| format!("{} isn't on GitHub", url.trim()))?;
            Done {
                url: Some(page),
                ..Done::default()
            }
        }
        Job::Raw(command) => {
            let args: Vec<&str> = command
                .split_whitespace()
                .map(|a| a.trim_matches('"'))
                .collect();
            let out = match git(root, &args) {
                Ok(out) => out,
                Err(e) => e,
            };
            Done {
                message: Some(format!("Git: ran git {command}")),
                page: Some((
                    format!("git {command}"),
                    format!("```\n{}\n```\n", out.trim_end()),
                )),
                changed: true,
                ..Done::default()
            }
        }
    })
}

/// Commits `scope` with `text` (else the message script, else the
/// template: the automatic one for `auto`), or amends the last commit.
/// Returns what it did.
fn commit(
    root: &Path,
    settings: &Settings,
    scope: Scope,
    text: Option<&str>,
    amend: bool,
    auto: bool,
) -> Result<String, String> {
    let staged = |root: &Path| -> Result<Vec<String>, String> {
        Ok(git(root, &["diff", "--cached", "--name-only"])?
            .lines()
            .map(str::to_string)
            .collect())
    };
    let all = match scope {
        Scope::All => true,
        Scope::Staged => false,
        Scope::StagedOrAll => staged(root)?.is_empty(),
    };
    if all {
        git(root, &["add", "-A"])?;
    }
    let conflicted = git(root, &["diff", "--name-only", "--diff-filter=U"])?;
    if !conflicted.trim().is_empty() {
        return Err(format!(
            "conflicts in {} still: fix them, then commit-and-sync (or Git: Abort merge)",
            conflicted.split_whitespace().collect::<Vec<_>>().join(", ")
        ));
    }
    if root.join(".git").join("MERGE_HEAD").exists() {
        // Resolved: the merge's own commit.
        git(root, &["commit", "--no-edit"])?;
        return Ok("finished the merge".into());
    }
    let files = staged(root)?;
    if amend {
        let mut args = vec!["commit", "--amend"];
        match text {
            Some(t) => args.extend(["-m", t]),
            None => args.push("--no-edit"),
        }
        git(root, &args)?;
        return Ok("amended the last commit".into());
    }
    if files.is_empty() {
        return Ok("nothing to commit".into());
    }
    let mut full = match text {
        Some(t) => t.to_string(),
        None if !settings.script.trim().is_empty() => {
            let out = Command::new("sh")
                .args(["-c", &settings.script])
                .current_dir(root)
                .output()
                .map_err(|e| format!("the message script: {e}"))?;
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !out.status.success() || text.is_empty() {
                return Err("the commit message script gave no message".into());
            }
            text
        }
        None => {
            let template = if auto && !settings.auto_message.trim().is_empty() {
                &settings.auto_message
            } else {
                &settings.message
            };
            message(
                template,
                &settings.date_format,
                Local::now().naive_local(),
                &hostname(),
                &files,
            )
        }
    };
    if settings.list_files {
        full.push_str("\n\n");
        full.push_str(&files.join("\n"));
    }
    git(root, &["commit", "-m", &full])?;
    let n = files.len();
    Ok(format!(
        "committed {n} file{}",
        if n == 1 { "" } else { "s" }
    ))
}

/// Runs a command (in the background thread). `auto-…` commands are the
/// timers' (commit-and-sync, pull, push).
fn run(root: &Path, settings: &Settings, command: &str) -> Result<Done, String> {
    let auto = command.starts_with("auto-");
    let command = command.strip_prefix("auto-").unwrap_or(command);
    if command == "status" {
        return Ok(Done::default());
    }
    if command == "init" || command == "init-ignore" {
        git(root, &["init"])?;
        if command == "init-ignore" {
            let path = root.join(".gitignore");
            let mut text = std::fs::read_to_string(&path).unwrap_or_default();
            if !text.lines().any(|l| l.trim() == ".blackglass/") {
                text.push_str("# blackglass's plugin settings and state\n.blackglass/\n");
                std::fs::write(&path, text).map_err(|e| format!("cannot write .gitignore: {e}"))?;
            }
        }
        return Ok(Done {
            message: Some(format!("Git: made a repository in {}", root.display())),
            ..Done::default()
        });
    }
    if git(root, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        return Err("the vault isn't in a repository yet (Git: Initialize a new repo)".into());
    }
    let mut said: Vec<String> = Vec::new();
    let mut changed = false;
    let simple: Option<(&[&str], &str, bool)> = match command {
        "fetch" => Some((&["fetch", "--all", "--prune"], "fetched", false)),
        "discard-all" => Some((
            &["restore", "--staged", "--worktree", "--", "."],
            "discarded all changes",
            true,
        )),
        _ => None,
    };
    if let Some((args, done, changed)) = simple {
        git(root, args)?;
        return Ok(Done {
            message: Some(format!("Git: {done}")),
            changed,
            ..Done::default()
        });
    }
    if command == "delete-repo" {
        std::fs::remove_dir_all(root.join(".git"))
            .map_err(|e| format!("cannot delete .git: {e}"))?;
        return Ok(Done {
            message: Some("Git: deleted the repository (.git)".into()),
            ..Done::default()
        });
    }
    if command == "abort" {
        let git_dir = root.join(".git");
        let args: &[&str] = if git_dir.join("MERGE_HEAD").exists() {
            &["merge", "--abort"]
        } else if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
            &["rebase", "--abort"]
        } else {
            return Ok(Done {
                message: Some("Git: no merge to abort".into()),
                ..Done::default()
            });
        };
        git(root, args)?;
        return Ok(Done {
            message: Some("Git: aborted the merge".into()),
            changed: true,
            ..Done::default()
        });
    }
    if command == "submodules" {
        git(root, &["submodule", "update", "--init", "--recursive"])?;
        return Ok(Done {
            message: Some("Git: updated the submodules".into()),
            changed: true,
            ..Done::default()
        });
    }
    let sync = matches!(command, "commit-and-sync" | "sync-and-close");
    let scope = match command {
        "commit-staged" | "amend" => Some(Scope::Staged),
        "commit-staged-or-all" => Some(Scope::StagedOrAll),
        "commit" => Some(Scope::All),
        _ if sync && auto && settings.staged_only => Some(Scope::Staged),
        _ if sync => Some(Scope::All),
        _ => None,
    };
    if let Some(scope) = scope {
        said.push(commit(
            root,
            settings,
            scope,
            None,
            command == "amend",
            auto,
        )?);
    }
    let remote = git(root, &["remote"])?.lines().next().map(str::to_string);
    let upstream = git(root, &["rev-parse", "--abbrev-ref", "@{u}"]).is_ok();
    // An automatic commit-and-sync leaves pulling and pushing to their own
    // timers when they have them.
    let pull = command == "pull"
        || (sync && settings.pull_before_push && !(auto && settings.pull_interval > 0.0));
    let push =
        command == "push" || (sync && settings.push && !(auto && settings.push_interval > 0.0));
    if pull {
        match (&remote, upstream) {
            (None, _) => said.push("no remote to sync with".into()),
            (Some(_), false) if command == "pull" => {
                said.push("nothing to pull from yet (push first)".into())
            }
            (Some(_), false) => {}
            (Some(_), true) => {
                let before = git(root, &["rev-parse", "HEAD"]).unwrap_or_default();
                let mut args: Vec<&str> = match settings.method.as_str() {
                    "rebase" => vec!["pull", "--rebase", "--autostash"],
                    _ => vec!["pull", "--no-edit", "--no-rebase"],
                };
                if !settings.strategy.is_empty() && settings.method == "merge" {
                    args.extend(["-X", &settings.strategy]);
                }
                let pulled = if settings.method == "reset" {
                    // The remote's state, whatever is here.
                    git(root, &["fetch"]).and_then(|_| git(root, &["reset", "--hard", "@{u}"]))
                } else {
                    git(root, &args)
                };
                if let Err(e) = pulled {
                    let conflicted =
                        git(root, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
                    if conflicted.trim().is_empty() {
                        return Err(format!("{e} (resolve it, then commit-and-sync again)"));
                    }
                    let files = conflicted.split_whitespace().collect::<Vec<_>>().join(", ");
                    // The notes show the conflict markers: open tabs reload.
                    return Ok(Done {
                        message: Some(format!(
                            "Git: conflict in {files}: fix it, then commit-and-sync (or Git: Abort merge)"
                        )),
                        changed: true,
                        ..Done::default()
                    });
                }
                if settings.submodules {
                    git(root, &["submodule", "update", "--init", "--recursive"])?;
                }
                let after = git(root, &["rev-parse", "HEAD"]).unwrap_or_default();
                changed = before != after;
                said.push(if changed {
                    "pulled".into()
                } else {
                    "pulled, nothing new".into()
                });
            }
        }
    }
    if push {
        match (&remote, upstream) {
            (None, _) => {
                if command == "push" {
                    said.push("no remote to push to".into());
                }
            }
            (Some(_), true) => {
                let ahead = git(root, &["rev-list", "--count", "@{u}..HEAD"])
                    .ok()
                    .and_then(|n| n.trim().parse::<usize>().ok())
                    .unwrap_or(1);
                if ahead == 0 {
                    said.push("nothing to push".into());
                } else {
                    if settings.squash && ahead > 1 {
                        // The commits waiting become one.
                        let text = message(
                            &settings.message,
                            &settings.date_format,
                            Local::now().naive_local(),
                            &hostname(),
                            &[],
                        );
                        git(root, &["reset", "--soft", "@{u}"])?;
                        git(root, &["commit", "-m", &text])?;
                        said.push(format!("squashed {ahead} commits"));
                    }
                    git(root, &["push"])?;
                    said.push("pushed".into());
                }
            }
            (Some(remote), false) => {
                git(root, &["push", "-u", remote, "HEAD"])?;
                said.push("pushed".into());
            }
        }
    }
    if settings.quiet {
        said.retain(|s| {
            !matches!(
                s.as_str(),
                "nothing to commit" | "nothing to push" | "pulled, nothing new"
            )
        });
    }
    Ok(Done {
        message: (!said.is_empty()).then(|| format!("Git: {}", said.join(", "))),
        changed,
        ..Done::default()
    })
}

impl Git {
    /// The Git tab as a tree: a row per folder (with `▾`), its changes
    /// indented under it; each with the change it is, if a file.
    fn tree_rows(&self) -> Vec<(SidebarRow, Option<usize>)> {
        let mut order: Vec<usize> = (0..self.changes.len()).collect();
        order.sort_by_key(|&i| {
            let p = &self.changes[i].path;
            // Folders' files after the top's, by folder.
            (p.contains('/'), p.to_lowercase())
        });
        let mut rows = Vec::new();
        let mut folder: Option<&str> = None;
        for i in order {
            let c = &self.changes[i];
            let (dir, name) = match c.path.rsplit_once('/') {
                Some((d, n)) => (Some(d), n),
                None => (None, c.path.as_str()),
            };
            if dir.is_some() && dir != folder {
                rows.push((
                    SidebarRow {
                        label: format!("▾ {}/", dir.unwrap_or_default()),
                        detail: String::new(),
                    },
                    None,
                ));
            }
            folder = dir;
            let indent = if dir.is_some() { "  " } else { "" };
            rows.push((
                SidebarRow {
                    label: format!("{indent}{}", name.trim_end_matches(".md")),
                    detail: if c.staged {
                        format!("staged · {}", c.state)
                    } else {
                        c.state.to_string()
                    },
                },
                Some(i),
            ));
        }
        rows
    }
}

impl Plugin for Git {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Git",
            version: "0.1.0",
            author: "blackglass, after Vinzent03's Git",
            description: "Backs the vault up to a Git repository and keeps it in sync: commit-and-sync \
                          (commit, pull, push), on a timer too, and a pull on start. A Git tab lists the \
                          changes (stage, discard, commit); diffs and the history open in tabs. Runs \
                          your own git program in the background.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        let get = |key: &str| values.get("", key).map(str::to_string);
        // The repository: the vault, or a folder in it (`base_path`). A
        // custom `.git` folder comes from `GIT_DIR` in the environment.
        self.root = match get("base_path").map(|b| b.trim().trim_matches('/').to_string()) {
            Some(base) if !base.is_empty() => vault.root.join(base),
            _ => vault.root.clone(),
        };
        let on = |key: &str| get(key).is_some_and(|v| v == "true");
        let minutes = |key: &str| {
            get(key)
                .and_then(|v| v.trim().parse::<f64>().ok())
                .filter(|m| *m > 0.0)
                .unwrap_or(0.0)
        };
        let defaults = Settings::default();
        self.settings = Settings {
            message: get("commit_message")
                .filter(|m| !m.trim().is_empty())
                .unwrap_or(defaults.message),
            auto_message: get("auto_commit_message").unwrap_or_default(),
            date_format: get("date_format").unwrap_or(defaults.date_format),
            list_files: on("list_changed_files"),
            script: get("commit_message_script").unwrap_or_default(),
            interval: minutes("interval"),
            after_change: on("auto_after_change"),
            push_interval: minutes("push_interval"),
            pull_interval: minutes("pull_interval"),
            staged_only: on("auto_staged_only"),
            pull_on_start: on("pull_on_start"),
            pull_before_push: get("pull_before_push").is_none_or(|v| v == "true"),
            method: get("sync_method")
                .filter(|m| ["merge", "rebase", "reset"].contains(&m.as_str()))
                .unwrap_or(defaults.method),
            strategy: get("merge_strategy")
                .filter(|m| m == "ours" || m == "theirs")
                .unwrap_or_default(),
            push: get("push").is_none_or(|v| v == "true"),
            submodules: on("submodules"),
            quiet: on("quiet"),
            last_commit: get("show_last_commit").is_none_or(|v| v == "true"),
            refresh: get("refresh_seconds")
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|s| *s > 0)
                .map_or(defaults.refresh, Duration::from_secs),
            blame: get("blame").unwrap_or(defaults.blame),
            tree: on("tree_view"),
            squash: on("squash_before_push"),
            split: get("diff_style").is_some_and(|v| v == "split"),
        };
        self.show_marks = get("margin").is_none_or(|v| v == "true");
        // Notes changed (a save): the Git tab, status and margin follow soon.
        self.last_status = None;
        self.margin_stale = true;
    }

    fn settings(&self) -> Vec<Setting> {
        vec![
            Setting::new(
                "",
                "commit_message",
                "Commit message",
                "{{date}}, {{hostname}}, {{numFiles}} and {{files}} are filled in",
                Kind::Text,
                DEFAULT_MESSAGE,
            ),
            Setting::new(
                "",
                "auto_commit_message",
                "Automatic commit message",
                "For automatic commits (empty: the commit message above)",
                Kind::Text,
                "",
            ),
            Setting::new(
                "",
                "list_changed_files",
                "List the files in the message",
                "The changed files in the commit message's body",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "commit_message_script",
                "Commit message script",
                "A shell command whose output is the message (empty: off)",
                Kind::Text,
                "",
            ),
            Setting::new(
                "",
                "date_format",
                "Date format",
                "How {{date}} is written (moment.js tokens)",
                Kind::Text,
                "YYYY-MM-DD HH:mm:ss",
            ),
            Setting::new(
                "",
                "interval",
                "Automatic commit-and-sync",
                "Every this many minutes (0: off)",
                Kind::Text,
                "0",
            ),
            Setting::new(
                "",
                "auto_after_change",
                "… after the last change",
                "Count the minutes from the last save instead of the last commit",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "auto_staged_only",
                "Automatic commits: staged only",
                "Automatic commits commit only the staged files",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "pull_interval",
                "Automatic pull",
                "Pull every this many minutes, on its own (0: with commit-and-sync)",
                Kind::Text,
                "0",
            ),
            Setting::new(
                "",
                "push_interval",
                "Automatic push",
                "Push every this many minutes, on its own (0: with commit-and-sync)",
                Kind::Text,
                "0",
            ),
            Setting::new(
                "",
                "pull_on_start",
                "Pull on start",
                "Pull the vault's changes when blackglass opens it",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "sync_method",
                "Pull method",
                "Merge the changes, rebase yours on them, or reset to the remote's",
                Kind::Choice(vec!["merge".into(), "rebase".into(), "reset".into()]),
                "merge",
            ),
            Setting::new(
                "",
                "merge_strategy",
                "Merge conflicts",
                "When a merge conflicts: stop, or take ours or theirs",
                Kind::Choice(vec!["stop".into(), "ours".into(), "theirs".into()]),
                "stop",
            ),
            Setting::new(
                "",
                "pull_before_push",
                "Pull before push",
                "Commit-and-sync pulls before it pushes",
                Kind::Toggle,
                "true",
            ),
            Setting::new(
                "",
                "push",
                "Push",
                "Commit-and-sync pushes too (off: commit and pull only)",
                Kind::Toggle,
                "true",
            ),
            Setting::new(
                "",
                "submodules",
                "Update submodules",
                "After a pull, update the submodules (recursively)",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "margin",
                "Changes in the margin",
                "Mark the open note's changed lines: + added, ~ changed, - deleted",
                Kind::Toggle,
                "true",
            ),
            Setting::new(
                "",
                "blame",
                "Line authors show",
                "What the line authoring margin shows (colored by age)",
                Kind::Choice(vec![
                    "author and date".into(),
                    "author".into(),
                    "date".into(),
                ]),
                "author and date",
            ),
            Setting::new(
                "",
                "tree_view",
                "Changes as a tree",
                "The Git tab shows the changed files under their folders",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "squash_before_push",
                "Squash before push",
                "The commits waiting to be pushed become one (the commit message template's)",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "diff_style",
                "Diffs",
                "Unified (one column), or split (before and after side by side)",
                Kind::Choice(vec!["unified".into(), "split".into()]),
                "unified",
            ),
            Setting::new(
                "",
                "quiet",
                "Quiet",
                "Don't say \"nothing to commit\", \"nothing to push\" and the like",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "",
                "show_last_commit",
                "Last commit in the status bar",
                "Say when the last commit was, after the branch",
                Kind::Toggle,
                "true",
            ),
            Setting::new(
                "",
                "refresh_seconds",
                "Refresh every",
                "Seconds between status checks",
                Kind::Text,
                "30",
            ),
            Setting::new(
                "",
                "base_path",
                "Repository folder",
                "The repository's folder in the vault (empty: the vault)",
                Kind::Text,
                "",
            ),
        ]
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("commit-and-sync", "Commit-and-sync"),
            PluginCommand::new("commit", "Commit all changes"),
            PluginCommand::new(
                "commit-all-message",
                "Commit all changes with specific message",
            ),
            PluginCommand::new("commit-staged-or-all", "Commit (staged or all)"),
            PluginCommand::new("commit-message", "Commit with specific message"),
            PluginCommand::new("commit-staged", "Commit staged"),
            PluginCommand::new(
                "commit-staged-message",
                "Commit staged with specific message",
            ),
            PluginCommand::new("amend", "Amend staged"),
            PluginCommand::new("sync-and-close", "Commit-and-sync and then close"),
            PluginCommand::new("stage-file", "Stage current file"),
            PluginCommand::new("unstage-file", "Unstage current file"),
            PluginCommand::new("discard-all", "Discard all changes"),
            PluginCommand::new("fetch", "Fetch"),
            PluginCommand::new("switch-remote-branch", "Switch to remote branch"),
            PluginCommand::new("delete-repo", "Delete repository"),
            PluginCommand::new("open-on-github", "Open file on GitHub"),
            PluginCommand::new("history-on-github", "Open file history on GitHub"),
            PluginCommand::new("pull", "Pull"),
            PluginCommand::new("push", "Push"),
            PluginCommand::new("init", "Initialize a new repo"),
            PluginCommand::new("pause", "Pause / resume automatic routines"),
            PluginCommand::new("diff", "Show the diff of this note"),
            PluginCommand::new("history", "History"),
            PluginCommand::new("changes", "Open the Git tab"),
            PluginCommand::new("switch-branch", "Switch branch"),
            PluginCommand::new("new-branch", "Create new branch"),
            PluginCommand::new("delete-branch", "Delete branch"),
            PluginCommand::new("edit-remotes", "Edit remotes"),
            PluginCommand::new("remove-remote", "Remove remote"),
            PluginCommand::new("edit-gitignore", "Edit .gitignore"),
            PluginCommand::new("ignore-note", "Add file to .gitignore"),
            PluginCommand::new("raw", "Raw command"),
            PluginCommand::new("clone", "Clone an existing remote repo"),
            PluginCommand::new("abort", "Abort merge"),
            PluginCommand::new("submodules", "Update submodules"),
            PluginCommand::new("preview-change", "Preview change"),
            PluginCommand::new("stage-change", "Stage change"),
            PluginCommand::new("reset-change", "Reset change"),
            PluginCommand::new("next-change", "Go to next change"),
            PluginCommand::new("previous-change", "Go to previous change"),
            PluginCommand::new("blame", "Toggle line author information"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        let job = match id {
            "pause" => {
                self.paused = !self.paused;
                return Effect::Message(if self.paused {
                    "Git: automatic routines paused".into()
                } else {
                    "Git: automatic routines resumed".into()
                });
            }
            "changes" => return Effect::ShowSidebarTab,
            "next-change" | "previous-change" => {
                let row = ctx.note.map_or(0, |n| n.row);
                let to = if id == "next-change" {
                    self.hunks.iter().copied().find(|&l| l > row)
                } else {
                    self.hunks.iter().copied().rev().find(|&l| l < row)
                };
                return match to {
                    Some(line) => Effect::GoToLine(line),
                    None => Effect::Message("Git: no more changes that way".into()),
                };
            }
            "blame" => {
                self.blame = !self.blame;
                self.margin_stale = true;
                return Effect::Message(if self.blame {
                    "Git: line authors in the margin".into()
                } else {
                    "Git: changes in the margin".into()
                });
            }
            "commit-message" | "commit-all-message" | "commit-staged-message" => {
                let scope = match id {
                    "commit-all-message" => Scope::All,
                    "commit-staged-message" => Scope::Staged,
                    _ => Scope::StagedOrAll,
                };
                self.pending = Some(Pending::Message(scope));
                return Effect::Ask(vec![Question::Text {
                    prompt: "The commit message".into(),
                    default: String::new(),
                }]);
            }
            "sync-and-close" => {
                self.quit_after = true;
                Job::Command(id.to_string())
            }
            "init" => {
                self.pending = Some(Pending::Init);
                return Effect::Ask(vec![Question::Choose {
                    prompt: "A new repository in the vault".into(),
                    items: vec![
                        "Commit everything".into(),
                        "Leave out .blackglass/ (plugin settings and state)".into(),
                    ],
                }]);
            }
            "discard-all" => {
                self.pending = Some(Pending::DiscardAll);
                return Effect::Ask(vec![Question::Choose {
                    prompt: "Discard every change since the last commit?".into(),
                    items: vec!["Discard them".into(), "Keep them".into()],
                }]);
            }
            "delete-repo" => {
                self.pending = Some(Pending::DeleteRepo);
                let yes_no = |prompt: &str, yes: &str| Question::Choose {
                    prompt: prompt.into(),
                    items: vec![yes.into(), "No".into()],
                };
                return Effect::Ask(vec![
                    yes_no("Delete the repository (.git) and its history?", "Yes"),
                    yes_no("Really? It can't be undone.", "Yes, delete it"),
                ]);
            }
            "switch-remote-branch" => Job::List(ListFor::RemoteBranch),
            "stage-file" | "unstage-file" | "open-on-github" | "history-on-github" => {
                let rel = ctx
                    .note
                    .and_then(|n| n.path)
                    .and_then(|p| p.strip_prefix(&self.root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"));
                let Some(rel) = rel else {
                    return Effect::Message("Git: open a note in the repository first".into());
                };
                match id {
                    "stage-file" => Job::Stage(rel),
                    "unstage-file" => Job::Unstage(rel),
                    _ => Job::Web {
                        rel,
                        history: id == "history-on-github",
                    },
                }
            }
            "diff" => {
                let rel = ctx
                    .note
                    .and_then(|n| n.path)
                    .and_then(|p| p.strip_prefix(&self.root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"));
                match rel {
                    Some(rel) => Job::Diff(rel),
                    None => return Effect::Message("Git: open a note first".into()),
                }
            }
            "history" => Job::Log,
            "preview-change" | "stage-change" | "reset-change" => {
                let rel = ctx
                    .note
                    .and_then(|n| n.path)
                    .and_then(|p| p.strip_prefix(&self.root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"));
                let Some(rel) = rel else {
                    return Effect::Message("Git: open a note first".into());
                };
                let action = match id {
                    "preview-change" => HunkAction::Preview,
                    "stage-change" => HunkAction::Stage,
                    _ => HunkAction::Reset,
                };
                Job::Hunk {
                    rel,
                    row: ctx.note.map_or(0, |n| n.row),
                    action,
                }
            }
            "switch-branch" => Job::List(ListFor::SwitchBranch),
            "delete-branch" => Job::List(ListFor::DeleteBranch),
            "remove-remote" => Job::List(ListFor::RemoveRemote),
            "new-branch" => {
                self.pending = Some(Pending::NewBranch);
                return Effect::Ask(vec![Question::Text {
                    prompt: "The new branch's name".into(),
                    default: String::new(),
                }]);
            }
            "edit-remotes" => {
                self.pending = Some(Pending::Remote);
                return Effect::Ask(vec![
                    Question::Text {
                        prompt: "Remote (a new name adds one)".into(),
                        default: "origin".into(),
                    },
                    Question::Text {
                        prompt: "Its URL".into(),
                        default: String::new(),
                    },
                ]);
            }
            "clone" => {
                self.pending = Some(Pending::Clone);
                return Effect::Ask(vec![
                    Question::Text {
                        prompt: "The repository's URL".into(),
                        default: String::new(),
                    },
                    Question::Text {
                        prompt: "Into the folder (~ for home)".into(),
                        default: String::new(),
                    },
                ]);
            }
            "raw" => {
                self.pending = Some(Pending::Raw);
                return Effect::Ask(vec![Question::Text {
                    prompt: "git …".into(),
                    default: "status".into(),
                }]);
            }
            "edit-gitignore" => {
                let path = self.root.join(".gitignore");
                if !path.exists()
                    && let Err(e) = std::fs::write(&path, "")
                {
                    return Effect::Message(format!("Git: cannot make .gitignore: {e}"));
                }
                return Effect::Open { path };
            }
            "ignore-note" => {
                let rel = ctx
                    .note
                    .and_then(|n| n.path)
                    .and_then(|p| p.strip_prefix(&self.root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"));
                let Some(rel) = rel else {
                    return Effect::Message("Git: open a note first".into());
                };
                let path = self.root.join(".gitignore");
                let mut text = std::fs::read_to_string(&path).unwrap_or_default();
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&format!("{rel}\n"));
                return match mdedit::files::write_atomic(&path, &text) {
                    Ok(()) => Effect::Message(format!("Git: {rel} is in .gitignore")),
                    Err(e) => Effect::Message(format!("Git: cannot write .gitignore: {e}")),
                };
            }
            other => Job::Command(other.to_string()),
        };
        match self.start_or_say(job) {
            Effect::None => Effect::Message(format!("Git: {id} …")),
            said => said,
        }
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], _ctx: &Context) -> Effect {
        let job = match (self.pending.take(), answers) {
            (Some(Pending::Discard(path, new)), [Answer::Choice(0)]) => Job::Discard(path, new),
            (Some(Pending::DiscardAll), [Answer::Choice(0)]) => Job::Command("discard-all".into()),
            (Some(Pending::DeleteRepo), [Answer::Choice(0), Answer::Choice(0)]) => {
                Job::Command("delete-repo".into())
            }
            (Some(Pending::Init), [Answer::Choice(i)]) => {
                Job::Command(if *i == 1 { "init-ignore" } else { "init" }.into())
            }
            (Some(Pending::Message(scope)), [Answer::Text(text)]) if !text.trim().is_empty() => {
                Job::Commit {
                    scope,
                    message: Some(text.trim().to_string()),
                    amend: false,
                }
            }
            (Some(Pending::Commit), [Answer::Text(text)]) if !text.trim().is_empty() => {
                Job::CommitStaged(text.trim().to_string())
            }
            (Some(Pending::Log(hashes)), [Answer::Choice(i)]) => match hashes.get(*i) {
                Some(hash) => Job::Show(hash.clone()),
                None => return Effect::None,
            },
            (Some(Pending::Choose(what, items)), [Answer::Choice(i)]) => {
                let Some(name) = items.get(*i).cloned() else {
                    return Effect::None;
                };
                let (args, done, changed) = match what {
                    ListFor::SwitchBranch => (
                        vec!["switch".into(), name.clone()],
                        format!("Git: on {name}"),
                        true,
                    ),
                    ListFor::RemoteBranch => {
                        // A local branch following it (git's guess).
                        let local = name.split_once('/').map_or(name.as_str(), |(_, b)| b);
                        (
                            vec!["switch".into(), local.to_string()],
                            format!("Git: on {local}"),
                            true,
                        )
                    }
                    ListFor::DeleteBranch => (
                        vec!["branch".into(), "-d".into(), name.clone()],
                        format!("Git: deleted the branch {name}"),
                        false,
                    ),
                    ListFor::RemoveRemote => (
                        vec!["remote".into(), "remove".into(), name.clone()],
                        format!("Git: removed the remote {name}"),
                        false,
                    ),
                };
                Job::Run {
                    args,
                    done,
                    changed,
                }
            }
            (Some(Pending::NewBranch), [Answer::Text(name)]) if !name.trim().is_empty() => {
                let name = name.trim().to_string();
                Job::Run {
                    args: vec!["switch".into(), "-c".into(), name.clone()],
                    done: format!("Git: on {name} (a new branch)"),
                    changed: false,
                }
            }
            (Some(Pending::Remote), [Answer::Text(name), Answer::Text(url)])
                if !name.trim().is_empty() && !url.trim().is_empty() =>
            {
                Job::SetRemote(name.trim().into(), url.trim().into())
            }
            (Some(Pending::Clone), [Answer::Text(url), Answer::Text(folder)])
                if !url.trim().is_empty() && !folder.trim().is_empty() =>
            {
                let folder = folder.trim();
                let folder = match folder.strip_prefix('~') {
                    Some(rest) => crate::config::home().join(rest.trim_start_matches('/')),
                    None => PathBuf::from(folder),
                };
                Job::Clone(url.trim().to_string(), folder)
            }
            (Some(Pending::Raw), [Answer::Text(command)]) if !command.trim().is_empty() => {
                Job::Raw(command.trim().trim_start_matches("git ").to_string())
            }
            _ => return Effect::None,
        };
        self.start_or_say(job)
    }

    fn sidebar_tab(&self) -> Option<&'static str> {
        Some("Git")
    }

    fn sidebar_hint(&self) -> &'static str {
        "⏎ open  s stage  d discard  c commit  p pull  P push"
    }

    fn sidebar_rows(&self, _ctx: &Context) -> Vec<SidebarRow> {
        if self.settings.tree {
            return self.tree_rows().into_iter().map(|(row, _)| row).collect();
        }
        self.changes
            .iter()
            .map(|c| SidebarRow {
                label: c
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&c.path)
                    .trim_end_matches(".md")
                    .to_string(),
                detail: if c.staged {
                    format!("staged · {}", c.state)
                } else {
                    c.state.to_string()
                },
            })
            .collect()
    }

    /// Keys in the Git tab: Enter opens the file, `s` stages or unstages
    /// it, `d` discards its changes (after asking), `c` commits the staged
    /// files (asks the message).
    fn sidebar_key(&mut self, row: usize, key: KeyEvent, _ctx: &Context) -> Effect {
        match key.code {
            KeyCode::Char('p') => return self.start_or_say(Job::Command("pull".into())),
            KeyCode::Char('P') => return self.start_or_say(Job::Command("push".into())),
            _ => {}
        }
        // In the tree, a row is a folder (nothing to do) or a change.
        let row = if self.settings.tree {
            match self.tree_rows().get(row).and_then(|(_, c)| *c) {
                Some(i) => i,
                None => return Effect::None,
            }
        } else {
            row
        };
        let Some(change) = self.changes.get(row).cloned() else {
            return Effect::None;
        };
        let job = match key.code {
            KeyCode::Enter => {
                let path = self.root.join(&change.path);
                return if path.is_file() {
                    Effect::Open { path }
                } else {
                    Effect::Message(format!("Git: {} was deleted", change.path))
                };
            }
            KeyCode::Char('s') if change.staged => Job::Unstage(change.path),
            KeyCode::Char('s') => Job::Stage(change.path),
            KeyCode::Char('d') => {
                let new = change.state == "new" && !change.staged;
                self.pending = Some(Pending::Discard(change.path.clone(), new));
                return Effect::Ask(vec![Question::Choose {
                    prompt: format!("Discard the changes to {}?", change.path),
                    items: vec!["Discard them".into(), "Keep them".into()],
                }]);
            }
            KeyCode::Char('c') => {
                self.pending = Some(Pending::Commit);
                return Effect::Ask(vec![Question::Text {
                    prompt: "Commit the staged files: the message".into(),
                    default: String::new(),
                }]);
            }
            _ => return Effect::None,
        };
        self.start_or_say(job)
    }

    fn tick(&mut self, ctx: &Context) -> Effect {
        if let Some(job) = &self.job {
            match job.try_recv() {
                Ok(done) => {
                    self.job = None;
                    self.status = done.status;
                    self.changes = done.changes;
                    if std::mem::take(&mut self.quit_after) && !done.failed {
                        return Effect::Quit;
                    }
                    if let Some(url) = done.url {
                        return Effect::OpenUrl(url);
                    }
                    if let Some((path, width, lines, hunks)) = done.margin {
                        self.hunks = hunks;
                        return Effect::Margin { path, width, lines };
                    }
                    if let Some(folder) = done.open_vault {
                        return Effect::OpenVault(folder);
                    }
                    if let Some((title, text)) = done.page {
                        return Effect::ShowText { title, text };
                    }
                    if let Some((what, items)) = done.list {
                        if items.is_empty() {
                            return Effect::Message(match what {
                                ListFor::RemoveRemote => "Git: no remotes".into(),
                                _ => "Git: no other branches".into(),
                            });
                        }
                        let prompt = match what {
                            ListFor::SwitchBranch => "Switch to the branch",
                            ListFor::RemoteBranch => "Switch to the remote branch",
                            ListFor::DeleteBranch => "Delete the branch",
                            ListFor::RemoveRemote => "Remove the remote",
                        };
                        self.pending = Some(Pending::Choose(what, items.clone()));
                        return Effect::Ask(vec![Question::Choose {
                            prompt: prompt.into(),
                            items,
                        }]);
                    }
                    if let Some(log) = done.log {
                        if log.is_empty() {
                            return Effect::Message("Git: no commits yet".into());
                        }
                        let n = log.len();
                        let items = log.iter().map(|(_, shown)| shown.clone()).collect();
                        self.pending =
                            Some(Pending::Log(log.into_iter().map(|(h, _)| h).collect()));
                        return Effect::Ask(vec![Question::Choose {
                            prompt: format!("History ({n} commits)"),
                            items,
                        }]);
                    }
                    return match (done.message, done.changed) {
                        (Some(m), true) => Effect::FilesChanged(m),
                        (Some(m), false) => Effect::Message(m),
                        (None, _) => Effect::Redraw,
                    };
                }
                Err(TryRecvError::Empty) => return Effect::None,
                Err(TryRecvError::Disconnected) => {
                    self.job = None;
                    return Effect::Message("Git: the command stopped".into());
                }
            }
        }
        // The open note's margin, when it changed or was saved.
        let open = ctx.note.and_then(|n| n.path).map(Path::to_path_buf);
        if let Some(path) = open.filter(|p| p.starts_with(&self.root))
            && (self.margin_stale || self.margin_for.as_ref() != Some(&path))
            && self.root.join(".git").exists()
        {
            self.margin_for = Some(path.clone());
            self.margin_stale = false;
            let job = Job::Marks {
                path,
                blame: self.blame,
                show: self.show_marks,
            };
            self.start_job(job);
            return Effect::None;
        }
        if !self.started {
            self.started = true;
            if self.settings.pull_on_start && self.root.join(".git").exists() {
                self.start("pull");
                return Effect::None;
            }
        }
        let every = |minutes: f64| Duration::from_secs_f64(minutes * 60.0);
        let s = self.settings.clone();
        if !self.paused && s.interval > 0.0 {
            let due = if s.after_change {
                self.last_change
                    .is_some_and(|t| t.elapsed() >= every(s.interval))
            } else {
                self.last_sync.elapsed() >= every(s.interval)
            };
            if due && self.start("auto-commit-and-sync") {
                self.last_change = None;
                return Effect::None;
            }
        }
        if !self.paused
            && s.pull_interval > 0.0
            && self.last_pull.elapsed() >= every(s.pull_interval)
        {
            if self.start("auto-pull") {
                self.last_pull = Instant::now();
            }
            return Effect::None;
        }
        if !self.paused
            && s.push_interval > 0.0
            && self.last_push.elapsed() >= every(s.push_interval)
        {
            if self.start("auto-push") {
                self.last_push = Instant::now();
            }
            return Effect::None;
        }
        if self
            .last_status
            .is_none_or(|t| t.elapsed() >= self.settings.refresh)
        {
            self.start("status");
        }
        Effect::None
    }

    fn status(&self) -> Option<String> {
        self.status.clone()
    }

    fn on_note_saved(&mut self, path: &Path, _vault: &Vault) {
        if path.starts_with(&self.root) {
            self.last_change = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_pages_ages_and_colors() {
        assert_eq!(
            github_url("git@github.com:me/notes.git", "main", "A/B.md", false).as_deref(),
            Some("https://github.com/me/notes/blob/main/A/B.md")
        );
        assert_eq!(
            github_url("https://github.com/me/notes", "dev", "B.md", true).as_deref(),
            Some("https://github.com/me/notes/commits/dev/B.md")
        );
        assert_eq!(
            github_url("git@gitlab.com:me/notes.git", "main", "B.md", false),
            None
        );
        assert_eq!(ago_text(5), "just now");
        assert_eq!(ago_text(600), "10 min ago");
        assert_eq!(ago_text(7200), "2 h ago");
        assert_eq!(ago_text(86_400), "1 day ago");
        assert_eq!(age_color(1), Color::LightGreen);
        assert_eq!(age_color(400), Color::DarkGray);
    }

    #[test]
    fn the_commit_message_template() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 9, 30)
            .unwrap()
            .and_hms_opt(15, 4, 5)
            .unwrap();
        let files = ["a.md".to_string(), "b.md".to_string()];
        assert_eq!(
            message(
                "vault backup: {{date}}",
                "YYYY-MM-DD HH:mm:ss",
                now,
                "box",
                &files
            ),
            "vault backup: 2026-09-30 15:04:05"
        );
        assert_eq!(
            message(
                "{{numFiles}} files on {{hostname}}: {{files}}",
                "",
                now,
                "box",
                &files
            ),
            "2 files on box: a.md, b.md"
        );
    }

    #[test]
    fn the_session_is_kept_out_of_the_repository_once() {
        let dir = crate::vault::tests::scratch("git-session-exclude");
        std::fs::create_dir_all(dir.join(".blackglass")).unwrap();
        keep_session_out(&dir);
        assert!(!dir.join(".git").exists(), "not a repository: nothing");
        std::fs::create_dir_all(dir.join(".git/info")).unwrap();
        std::fs::write(dir.join(".git/info/exclude"), "*.tmp").unwrap();
        keep_session_out(&dir);
        keep_session_out(&dir);
        let text = std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap();
        assert!(text.starts_with("*.tmp\n"), "{text}");
        assert_eq!(
            text.matches("/.blackglass/workspace.json").count(),
            1,
            "{text}"
        );
    }
}
