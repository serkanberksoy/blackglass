//! Renders a Templater template: `<% command %>` tags are replaced by their
//! value. Commands are Templater's `tp.*` functions and properties, text in
//! quotes, numbers and lists, joined with `+`. `<%-` / `-%>` remove one
//! line break before / after a tag, `<%_` / `_%>` all whitespace.
//! A template with a JavaScript command (`<%* … %>`) runs as JavaScript
//! instead ([`super::js`]).
//!
//! Questions (`tp.system.prompt`, `tp.system.suggester`) are collected by
//! a first run without answers, then the template is run with them. What
//! a template needs from outside it (a web page, a user command's output,
//! a note's text) is collected the same way ([`Output::needs`]): the
//! plugin gets it, off the UI thread when it's slow, and runs the template
//! again with the [`More::results`]. What it does to files
//! (`tp.file.create_new`, `move`, `rename`, `cursor_append`) comes back as
//! [`Action`]s for the plugin to carry out once the text is done.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use chrono::{Duration, NaiveDateTime};

use crate::plugins::moment;
use crate::plugins::{Answer, Question};
use crate::vault::Vault;

/// The note a template is rendered for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Target {
    /// The note's name, without `.md`.
    pub title: String,
    /// Its folder in the vault (`""` at the top), `/` separators.
    pub folder: String,
    /// Its path in the vault, with `.md`.
    pub rel_path: String,
    pub abs_path: PathBuf,
    pub created: Option<NaiveDateTime>,
    pub modified: Option<NaiveDateTime>,
    /// Its text before the template runs.
    pub content: String,
    /// The text selected in it.
    pub selection: String,
}

/// What a template can use while it runs.
pub struct Env<'a> {
    pub vault: &'a Vault,
    pub target: &'a Target,
    pub now: NaiveDateTime,
    /// Answers to the template's questions, in order.
    pub answers: &'a [Answer],
    /// Everything else ([`More::none`] for nothing).
    pub more: &'a More,
}

/// What a template got from outside it, by its request (a JSON object:
/// `{"get": url}`, `{"run": name, "cmd": …, "env": {…}}`, `{"read":
/// path}`): the text, or why there's none.
pub type Results = BTreeMap<String, Result<String, String>>;

/// How a template was started: Templater's `RunMode`
/// (`tp.config.run_mode`), its numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunMode {
    #[default]
    CreateNewFromTemplate = 0,
    AppendActiveFile = 1,
    OverwriteFile = 2,
    OverwriteActiveFile = 3,
    DynamicProcessor = 4,
    StartupTemplate = 5,
}

/// The plugin's settings a template runs with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    /// `tp.web` may go to the network.
    pub web: bool,
    /// Where `tp.web.daily_quote()` gets its quotes (English).
    pub quotes: String,
    /// User scripts: (function name, CommonJS source).
    pub scripts: Vec<(String, String)>,
    /// User system commands: (function name, command line); empty when
    /// they're off.
    pub commands: Vec<(String, String)>,
    /// Jump to the first cursor and remove its marker (else every
    /// `tp.file.cursor` stays in the text, for "Jump to next cursor").
    pub auto_jump: bool,
}

/// The English quotes `tp.web.daily_quote()` picks from (Templater's).
pub const QUOTES_EN: &str =
    "https://raw.githubusercontent.com/Zachatoo/quotes-database/refs/heads/main/quotes.json";
/// … and the Spanish ones.
pub const QUOTES_ES: &str =
    "https://raw.githubusercontent.com/wulflo/frases-inventario/refs/heads/main/quotes.json";

impl Default for Setup {
    fn default() -> Self {
        Setup {
            web: true,
            quotes: QUOTES_EN.into(),
            scripts: Vec::new(),
            commands: Vec::new(),
            auto_jump: true,
        }
    }
}

/// The rest of what a template sees: what it asked for from outside, how
/// it was started (`tp.config`), the settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct More {
    pub results: Results,
    pub run_mode: RunMode,
    /// The template's path in the vault, if it's a note.
    pub template_file: Option<String>,
    /// The active note's path in the vault, if one is open.
    pub active_file: Option<String>,
    /// Where `tp.file.create_new` puts notes without a folder (in the
    /// vault, `""` for the top).
    pub new_folder: String,
    pub setup: Setup,
}

impl More {
    /// Nothing more: no results, the default settings.
    pub fn none() -> &'static More {
        static NONE: OnceLock<More> = OnceLock::new();
        NONE.get_or_init(More::default)
    }
}

/// Where `tp.file.create_new` gets the new note's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A template note (its path in the vault).
    File(String),
    /// This text (its commands run too).
    Text(String),
}

/// What a template does to files, once its text is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `tp.file.create_new`: a note `name` in `folder` (in the vault;
    /// `None`: [`More::new_folder`]) from `template`, opened or not.
    Create {
        template: Source,
        name: String,
        open: bool,
        folder: Option<String>,
    },
    /// `tp.file.move`: the note at `file` (the target if `None`) to `to`
    /// (in the vault, without `.md`).
    Move { file: Option<String>, to: String },
    /// `tp.file.rename`: the target, in its folder.
    Rename(String),
    /// `tp.file.cursor_append`: this text at the cursor.
    Append(String),
}

/// A rendered template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub text: String,
    /// Where `tp.file.cursor()` was (the lowest order), in chars.
    pub cursor: Option<usize>,
    /// The questions the template asks, in order.
    pub questions: Vec<Question>,
    /// What it needs from outside that [`More::results`] hasn't got (its
    /// text is unfinished until it has).
    pub needs: Vec<String>,
    pub actions: Vec<Action>,
}

/// Marks where `tp.file.cursor()` was, until the output is finished.
pub(super) const CURSOR: char = '\u{E000}';
/// How deep `tp.file.include` may nest (a note including itself stops).
pub(super) const MAX_INCLUDE_DEPTH: usize = 10;

/// Starts the simple engine's errors for what only JavaScript does: the
/// template runs as JavaScript instead.
const JS: &str = "\u{0}js:";

/// Renders `template`. `Err` explains the first command that failed. A
/// template with a JavaScript command (`<%* … %>`), or a command only
/// JavaScript runs (`tp.web`, `await`, `moment()` …), runs as JavaScript
/// ([`super::js`]); any other runs here.
pub fn render(template: &str, env: &Env) -> Result<Output, String> {
    let pieces = parse(template)?;
    let exec = pieces
        .iter()
        .any(|p| matches!(p, Piece::Command { exec: true, .. }));
    if !exec {
        let mut run = Run {
            env,
            asked: Vec::new(),
            cursors: Vec::new(),
        };
        match run.template(template, 0) {
            Ok(text) => {
                let questions = run.asked.into_iter().map(|(_, q)| q).collect();
                return Ok(finish(&text, &run.cursors, questions, env.more));
            }
            Err(e) if !e.contains(JS) => return Err(e),
            Err(_) => {}
        }
    }
    let js = super::js::render(&pieces, env)?;
    let mut out = finish(&js.text, &js.cursors, js.questions, env.more);
    out.needs = js.needs;
    out.actions = js.actions;
    Ok(out)
}

/// A cursor marker as Templater leaves it in the text.
pub fn cursor_tag(order: i64) -> String {
    if order == 0 {
        "<% tp.file.cursor() %>".into()
    } else {
        format!("<% tp.file.cursor({order}) %>")
    }
}

/// The output with the cursor at the first marker with the lowest order
/// (`cursors`: each marker's order, in text order), the markers with that
/// order gone and the others left as `<% tp.file.cursor(n) %>` for "Jump
/// to next cursor"; without [`Setup::auto_jump`], every marker left.
fn finish(text: &str, cursors: &[i64], questions: Vec<Question>, more: &More) -> Output {
    let lowest = cursors
        .iter()
        .min()
        .copied()
        .filter(|_| more.setup.auto_jump);
    let mut cursor = None;
    let mut out = String::with_capacity(text.len());
    let mut seen = 0;
    for c in text.chars() {
        if c == CURSOR {
            let order = cursors.get(seen).copied().unwrap_or(0);
            seen += 1;
            if Some(order) == lowest {
                cursor.get_or_insert(out.chars().count());
            } else {
                out.push_str(&cursor_tag(order));
            }
        } else {
            out.push(c);
        }
    }
    Output {
        text: out,
        cursor,
        questions,
        needs: Vec::new(),
        actions: Vec::new(),
    }
}

/// How a tag trims the text next to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trim {
    None,
    /// `-`: one line break.
    Newline,
    /// `_`: all whitespace.
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Piece {
    Text(String),
    Command { code: String, exec: bool },
}

/// Splits a template into text and commands, applying whitespace control.
pub(super) fn parse(template: &str) -> Result<Vec<Piece>, String> {
    let mut pieces = Vec::new();
    let mut rest = template;
    let mut trim_next = Trim::None;
    while let Some(open) = rest.find("<%") {
        let mut text = rest[..open].to_string();
        trim_start(&mut text, trim_next);
        let mut inner = &rest[open + 2..];
        let trim_before = match inner.chars().next() {
            Some('-') => Trim::Newline,
            Some('_') => Trim::All,
            _ => Trim::None,
        };
        if trim_before != Trim::None {
            inner = &inner[1..];
        }
        trim_end(&mut text, trim_before);
        pieces.push(Piece::Text(text));
        let exec = inner.starts_with('*');
        if exec || inner.starts_with('+') {
            inner = &inner[1..];
        }
        let close = inner.find("%>").ok_or("a <% tag isn't closed with %>")?;
        let mut code = inner[..close].trim_end();
        trim_next = match code.chars().last() {
            Some('-') => Trim::Newline,
            Some('_') => Trim::All,
            _ => Trim::None,
        };
        if trim_next != Trim::None {
            code = &code[..code.len() - 1];
        }
        pieces.push(Piece::Command {
            code: code.trim().to_string(),
            exec,
        });
        rest = &inner[close + 2..];
    }
    let mut text = rest.to_string();
    trim_start(&mut text, trim_next);
    pieces.push(Piece::Text(text));
    pieces.retain(|p| !matches!(p, Piece::Text(t) if t.is_empty()));
    Ok(pieces)
}

fn trim_start(text: &mut String, trim: Trim) {
    let cut = match trim {
        Trim::None => 0,
        Trim::Newline if text.starts_with("\r\n") => 2,
        Trim::Newline if text.starts_with('\n') => 1,
        Trim::Newline => 0,
        Trim::All => text.len() - text.trim_start().len(),
    };
    text.drain(..cut);
}

fn trim_end(text: &mut String, trim: Trim) {
    match trim {
        Trim::None => {}
        Trim::Newline => {
            if text.ends_with('\n') {
                text.pop();
                if text.ends_with('\r') {
                    text.pop();
                }
            }
        }
        Trim::All => text.truncate(text.trim_end().len()),
    }
}

/// A value in a command.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<Value>),
}

impl Value {
    /// As Templater writes it into the note.
    fn show(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
            Value::Number(n) => n.to_string(),
            Value::Text(t) => t.clone(),
            Value::List(l) => l.iter().map(Value::show).collect::<Vec<_>>().join(","),
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }
}

struct Run<'a> {
    env: &'a Env<'a>,
    /// Questions asked so far, by their call's source (asked once each).
    asked: Vec<(String, Question)>,
    /// The order given to each `tp.file.cursor()` marker, in the order the
    /// markers appear.
    cursors: Vec<i64>,
}

impl Run<'_> {
    fn template(&mut self, template: &str, depth: usize) -> Result<String, String> {
        let mut out = String::new();
        for piece in parse(template)? {
            match piece {
                Piece::Text(t) => out.push_str(&t),
                Piece::Command { exec: true, .. } => {
                    return Err(
                        "a JavaScript command in an included note (run it on its own)".into(),
                    );
                }
                Piece::Command { code, .. } if code.is_empty() => {}
                Piece::Command { code, .. } => {
                    let tokens = tokenize(&code).map_err(|e| format!("{e} in <% {code} %>"))?;
                    let mut p = Parser {
                        tokens,
                        at: 0,
                        source: &code,
                    };
                    let value = p
                        .expr(self, depth)
                        .and_then(|v| p.end().map(|()| v))
                        .map_err(|e| format!("{e} in <% {code} %>"))?;
                    out.push_str(&value.show());
                }
            }
        }
        Ok(out)
    }

    /// The answer to a question, asking it (once) if it's new.
    fn ask(&mut self, key: &str, question: Question) -> Option<&Answer> {
        let i = match self.asked.iter().position(|(k, _)| k == key) {
            Some(i) => i,
            None => {
                self.asked.push((key.to_string(), question));
                self.asked.len() - 1
            }
        };
        self.env.answers.get(i)
    }

    /// The value of `tp.<path>` called with `args` (`None`: not called).
    fn tp(
        &mut self,
        path: &[String],
        args: Option<Vec<Value>>,
        key: &str,
        depth: usize,
    ) -> Result<Value, String> {
        let env = self.env;
        let target = env.target;
        let names: Vec<&str> = path.iter().map(String::as_str).collect();
        let args = args.unwrap_or_default();
        let text = |i: usize| args.get(i).and_then(Value::text);
        let format_or = |i: usize, default: &str| text(i).unwrap_or(default).to_string();
        let date = |d: NaiveDateTime, i: usize| {
            Value::Text(moment::format(&d, &format_or(i, "YYYY-MM-DD")))
        };
        Ok(match names.as_slice() {
            ["date", "now"] => {
                let mut base = env.now;
                if let Some(reference) = text(2) {
                    let format = format_or(3, "YYYY-MM-DD");
                    base = moment::parse(reference, &format)
                        .ok_or_else(|| format!("can't read \"{reference}\" as {format}"))?;
                }
                match args.get(1) {
                    Some(Value::Number(days)) => base += Duration::days(*days as i64),
                    Some(Value::Text(offset)) => {
                        base = moment::shift(base, offset)
                            .ok_or_else(|| format!("\"{offset}\" isn't an offset"))?;
                    }
                    _ => {}
                }
                date(base, 0)
            }
            ["date", "tomorrow"] => date(env.now + Duration::days(1), 0),
            ["date", "yesterday"] => date(env.now - Duration::days(1), 0),
            ["date", "weekday"] => {
                let Some(Value::Number(day)) = args.get(1) else {
                    return Err("tp.date.weekday needs a day number".into());
                };
                let base = match text(2) {
                    Some(reference) => {
                        let format = format_or(3, "YYYY-MM-DD");
                        moment::parse(reference, &format)
                            .ok_or_else(|| format!("can't read \"{reference}\" as {format}"))?
                    }
                    None => env.now,
                };
                date(moment::weekday(base, *day as i64), 0)
            }
            ["file", "title"] => Value::Text(target.title.clone()),
            ["file", "content"] => Value::Text(target.content.clone()),
            ["file", "selection"] => Value::Text(target.selection.clone()),
            ["file", "tags"] => Value::List(
                crate::vault::tags::extract(&crate::vault::split_lines(&target.content))
                    .into_iter()
                    .map(|t| Value::Text(format!("#{t}")))
                    .collect(),
            ),
            ["file", "folder"] => {
                let absolute = matches!(args.first(), Some(Value::Bool(true)));
                let name = target.folder.rsplit('/').next().unwrap_or_default();
                Value::Text(if absolute {
                    target.folder.clone()
                } else {
                    name.to_string()
                })
            }
            ["file", "path"] => {
                let relative = matches!(args.first(), Some(Value::Bool(true)));
                Value::Text(if relative {
                    target.rel_path.clone()
                } else {
                    target.abs_path.display().to_string()
                })
            }
            ["file", "creation_date"] => {
                let d = target.created.unwrap_or(env.now);
                Value::Text(moment::format(&d, &format_or(0, "YYYY-MM-DD HH:mm")))
            }
            ["file", "last_modified_date"] => {
                let d = target.modified.unwrap_or(env.now);
                Value::Text(moment::format(&d, &format_or(0, "YYYY-MM-DD HH:mm")))
            }
            ["file", "exists"] => {
                let path = text(0).ok_or("tp.file.exists needs a path")?;
                Value::Bool(env.vault.root.join(path).is_file())
            }
            ["file", "cursor"] => {
                let order = match args.first() {
                    Some(Value::Number(n)) => *n as i64,
                    _ => 0,
                };
                self.cursors.push(order);
                Value::Text(CURSOR.to_string())
            }
            ["file", "include"] => {
                let link = text(0).ok_or("tp.file.include needs a [[link]]")?;
                Value::Text(self.include(link, depth)?)
            }
            ["frontmatter", key @ ..] if !key.is_empty() => {
                frontmatter(&target.content, &key.join("."))
            }
            ["system", "prompt"] | ["system", "suggester"] if args.len() > 4 => {
                return Err(format!("{JS}tp.{}", names.join(".")));
            }
            ["system", "prompt"] if args.len() > 2 => {
                return Err(format!("{JS}tp.system.prompt"));
            }
            ["system", "prompt"] => {
                let prompt = format_or(0, "Enter a value");
                let default = format_or(1, "");
                let question = Question::Text {
                    prompt,
                    default: default.clone(),
                };
                match self.ask(key, question) {
                    Some(Answer::Text(t)) => Value::Text(t.clone()),
                    _ => Value::Text(default),
                }
            }
            ["system", "suggester"] => {
                let list = |i: usize| match args.get(i) {
                    Some(Value::List(l)) => Ok(l.clone()),
                    _ => Err(format!(
                        "{JS}tp.system.suggester needs two lists (for a function, use a <%* %> command)"
                    )),
                };
                let (shown, values) = (list(0)?, list(1)?);
                let placeholder = format_or(3, "Choose");
                let question = Question::Choose {
                    prompt: placeholder,
                    items: shown.iter().map(Value::show).collect(),
                };
                match self.ask(key, question) {
                    Some(Answer::Choice(i)) => values.get(*i).cloned().unwrap_or(Value::Null),
                    _ => Value::Null,
                }
            }
            ["system", "clipboard"] => Value::Text(clipboard()),
            // The rest of Templater runs as JavaScript.
            [module, ..]
                if ["web", "app", "obsidian", "hooks", "config", "user"].contains(module) =>
            {
                return Err(format!("{JS}tp.{module}"));
            }
            [
                "file",
                "create_new" | "move" | "rename" | "find_tfile" | "cursor_append",
            ]
            | ["system", "multi_suggester"] => {
                return Err(format!("{JS}tp.{}", names.join(".")));
            }
            _ => return Err(format!("unknown tp.{}", names.join("."))),
        })
    }

    /// `tp.file.include("[[Note]]")` or `("[[Note#Heading]]")`: the note's
    /// text (or section), with its own commands run.
    fn include(&mut self, link: &str, depth: usize) -> Result<String, String> {
        if depth >= MAX_INCLUDE_DEPTH {
            return Err("tp.file.include nests too deep (does a note include itself?)".into());
        }
        let inner = link.trim().trim_start_matches("[[").trim_end_matches("]]");
        let inner = inner.split('|').next().unwrap_or_default();
        let (name, heading) = match inner.split_once('#') {
            Some((n, h)) => (n, Some(h)),
            None => (inner, None),
        };
        let name = name.trim_end_matches(".md");
        let note = self
            .env
            .vault
            .notes
            .iter()
            .find(|n| {
                n.rel_name().eq_ignore_ascii_case(name) || n.name().eq_ignore_ascii_case(name)
            })
            .ok_or_else(|| format!("can't include [[{name}]]: no such note"))?;
        let lines = match heading {
            Some(h) => mdedit::embed::section(&note.lines, h)
                .ok_or_else(|| format!("can't include [[{inner}]]: no such heading"))?,
            None => note.lines.clone(),
        };
        self.template(&lines.join("\n"), depth + 1)
    }
}

/// Every frontmatter field of `content`, as JSON (for JavaScript).
pub(super) fn frontmatter_json(content: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let lines: Vec<&str> = content.lines().collect();
    if lines.first().map(|l| l.trim_end()) == Some("---") {
        for line in lines.iter().skip(1) {
            if matches!(line.trim_end(), "---" | "...") {
                break;
            }
            if line.starts_with([' ', '\t']) {
                continue;
            }
            if let Some((key, _)) = line.split_once(':') {
                let key = key.trim();
                map.insert(key.to_string(), value_json(&frontmatter(content, key)));
            }
        }
    }
    serde_json::Value::Object(map)
}

fn value_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => (*b).into(),
        Value::Number(n) => (*n).into(),
        Value::Text(t) => t.clone().into(),
        Value::List(l) => l.iter().map(value_json).collect(),
    }
}

/// A frontmatter field of `content`, as text (a list as a list).
fn frontmatter(content: &str, key: &str) -> Value {
    let lines: Vec<&str> = content.lines().collect();
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return Value::Null;
    }
    let unquote = |s: &str| s.trim().trim_matches(['"', '\'']).to_string();
    for (i, line) in lines.iter().enumerate().skip(1) {
        if matches!(line.trim_end(), "---" | "...") {
            break;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if k.trim() != key || line.starts_with([' ', '\t']) {
            continue;
        }
        let v = v.trim();
        if let Some(items) = v.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            return Value::List(items.split(',').map(|s| Value::Text(unquote(s))).collect());
        }
        if !v.is_empty() {
            return Value::Text(unquote(v));
        }
        // A `- item` list under the key.
        let items: Vec<Value> = lines[i + 1..]
            .iter()
            .map_while(|l| l.trim_start().strip_prefix("- "))
            .map(|s| Value::Text(unquote(s)))
            .collect();
        return if items.is_empty() {
            Value::Null
        } else {
            Value::List(items)
        };
    }
    Value::Null
}

/// The clipboard's text (as Ctrl+V reads it).
fn clipboard() -> String {
    use mdedit::clipboard::Clipboard;
    mdedit::clipboard::SystemClipboard
        .read()
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Text(String),
    Number(f64),
    Symbol(char),
}

fn tokenize(code: &str) -> Result<Vec<(Token, usize)>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<(usize, char)> = code.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '\'' || c == '`' {
            let mut text = String::new();
            i += 1;
            loop {
                let Some(&(_, ch)) = chars.get(i) else {
                    return Err("a text in quotes isn't closed".into());
                };
                i += 1;
                match ch {
                    '\\' => {
                        let escaped = chars.get(i).map(|&(_, e)| e).unwrap_or('\\');
                        text.push(match escaped {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                        i += 1;
                    }
                    q if q == c => break,
                    other => text.push(other),
                }
            }
            tokens.push((Token::Text(text), at));
        } else if c.is_ascii_digit() {
            while i < chars.len() && (chars[i].1.is_ascii_digit() || chars[i].1 == '.') {
                i += 1;
            }
            let end = chars.get(i).map_or(code.len(), |&(b, _)| b);
            let n = code[at..end]
                .parse()
                .map_err(|_| format!("bad number {}", &code[at..end]))?;
            tokens.push((Token::Number(n), at));
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            while i < chars.len()
                && (chars[i].1.is_alphanumeric() || matches!(chars[i].1, '_' | '$'))
            {
                i += 1;
            }
            let end = chars.get(i).map_or(code.len(), |&(b, _)| b);
            tokens.push((Token::Name(code[at..end].to_string()), at));
        } else if "().,[]+-".contains(c) {
            tokens.push((Token::Symbol(c), at));
            i += 1;
        } else {
            return Err(format!("{JS}unexpected \"{c}\""));
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: Vec<(Token, usize)>,
    at: usize,
    source: &'a str,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|(t, _)| t)
    }

    fn eat(&mut self, symbol: char) -> bool {
        let yes = self.peek() == Some(&Token::Symbol(symbol));
        if yes {
            self.at += 1;
        }
        yes
    }

    fn end(&self) -> Result<(), String> {
        match self.peek() {
            None => Ok(()),
            Some(_) => Err(format!(
                "{JS}unexpected text after the value (for JavaScript, use a <%* %> command)"
            )),
        }
    }

    /// `term (+ term)*`: numbers add, anything else joins as text.
    fn expr(&mut self, run: &mut Run, depth: usize) -> Result<Value, String> {
        let mut value = self.term(run, depth)?;
        while self.eat('+') {
            let right = self.term(run, depth)?;
            value = match (value, right) {
                (Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                (a, b) => Value::Text(a.show() + &b.show()),
            };
        }
        Ok(value)
    }

    fn term(&mut self, run: &mut Run, depth: usize) -> Result<Value, String> {
        let start = self
            .tokens
            .get(self.at)
            .map_or(self.source.len(), |&(_, at)| at);
        let Some((token, _)) = self.tokens.get(self.at).cloned() else {
            return Err("expected a value".into());
        };
        self.at += 1;
        match token {
            Token::Text(t) => Ok(Value::Text(t)),
            Token::Number(n) => Ok(Value::Number(n)),
            Token::Symbol('-') => match self.term(run, depth)? {
                Value::Number(n) => Ok(Value::Number(-n)),
                _ => Err("- needs a number".into()),
            },
            Token::Symbol('(') => {
                let v = self.expr(run, depth)?;
                if !self.eat(')') {
                    return Err("missing )".into());
                }
                Ok(v)
            }
            Token::Symbol('[') => {
                let mut items = Vec::new();
                if !self.eat(']') {
                    loop {
                        items.push(self.expr(run, depth)?);
                        if self.eat(']') {
                            break;
                        }
                        if !self.eat(',') {
                            return Err("missing ] after a list".into());
                        }
                    }
                }
                Ok(Value::List(items))
            }
            Token::Name(n) if n == "true" || n == "false" => Ok(Value::Bool(n == "true")),
            Token::Name(n) if n == "null" || n == "undefined" => Ok(Value::Null),
            Token::Name(n) if n == "tp" => self.tp_path(run, depth, start),
            Token::Name(n) => Err(format!("{JS}unknown name {n} (only tp.* is supported)")),
            Token::Symbol(c) => Err(format!("{JS}unexpected \"{c}\"")),
        }
    }

    /// `tp.a.b`, `tp.a["b"]`, `tp.a.b(args)`.
    fn tp_path(&mut self, run: &mut Run, depth: usize, start: usize) -> Result<Value, String> {
        let mut path = Vec::new();
        loop {
            if self.eat('.') {
                match self.tokens.get(self.at).cloned() {
                    Some((Token::Name(n), _)) => {
                        self.at += 1;
                        path.push(n);
                    }
                    _ => return Err("expected a name after .".into()),
                }
            } else if self.eat('[') {
                let key = self.expr(run, depth)?;
                if !self.eat(']') {
                    return Err("missing ]".into());
                }
                path.push(key.show());
            } else {
                break;
            }
        }
        let args = if self.eat('(') {
            let mut args = Vec::new();
            if !self.eat(')') {
                loop {
                    args.push(self.expr(run, depth)?);
                    if self.eat(')') {
                        break;
                    }
                    if !self.eat(',') {
                        return Err("missing ) after the arguments".into());
                    }
                }
            }
            Some(args)
        } else {
            None
        };
        let end = self
            .tokens
            .get(self.at)
            .map_or(self.source.len(), |&(_, at)| at);
        // The call's text identifies its question, so it's asked once.
        let key = self.source[start..end].trim().to_string();
        run.tp(&path, args, &key, depth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};
    use chrono::NaiveDate;

    /// One vault for every test here (tests run in parallel).
    fn vault() -> &'static Vault {
        static VAULT: std::sync::OnceLock<Vault> = std::sync::OnceLock::new();
        VAULT.get_or_init(|| {
            let dir = scratch("templater-engine");
            write(
                &dir,
                &[
                    (
                        "Snippets/Sig.md",
                        "Regards,\n<% tp.file.title %>\n## Part\npart text",
                    ),
                    ("Loop.md", "<% tp.file.include(\"[[Loop]]\") %>"),
                ],
            );
            Vault::open(&dir).unwrap()
        })
    }

    fn target() -> Target {
        Target {
            title: "2026-08-09".into(),
            folder: "Journal/Daily".into(),
            rel_path: "Journal/Daily/2026-08-09.md".into(),
            abs_path: PathBuf::from("/vault/Journal/Daily/2026-08-09.md"),
            created: NaiveDate::from_ymd_opt(2026, 8, 1)
                .unwrap()
                .and_hms_opt(9, 30, 0),
            modified: None,
            content: "---\nmood: good\ntags: [a, b]\nlist:\n  - x\n  - y\n---\ntext #inline".into(),
            selection: "picked".into(),
        }
    }

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 8, 9)
            .unwrap()
            .and_hms_opt(14, 7, 0)
            .unwrap()
    }

    fn out(template: &str, answers: &[Answer]) -> Result<Output, String> {
        let t = target();
        render(
            template,
            &Env {
                vault: vault(),
                target: &t,
                now: now(),
                answers,
                more: More::none(),
            },
        )
    }

    fn text(template: &str) -> String {
        out(template, &[]).unwrap().text
    }

    #[test]
    fn dates() {
        assert_eq!(text("<% tp.date.now() %>"), "2026-08-09");
        assert_eq!(
            text("<% tp.date.now(\"dddd Do MMMM\") %>"),
            "Sunday 9th August"
        );
        assert_eq!(text("<% tp.date.now(\"YYYY-MM-DD\", -1) %>"), "2026-08-08");
        assert_eq!(
            text("<% tp.date.now(\"YYYY-MM-DD\", \"P1M\") %>"),
            "2026-09-09"
        );
        assert_eq!(
            text("<% tp.date.now(\"YYYY-MM-DD\", 1, tp.file.title, \"YYYY-MM-DD\") %>"),
            "2026-08-10",
            "the day after the note's date"
        );
        assert_eq!(
            text("<% tp.date.tomorrow() %> <% tp.date.yesterday(\"D\") %>"),
            "2026-08-10 8"
        );
        assert_eq!(
            text("<% tp.date.weekday(\"YYYY-MM-DD\", 0) %>"),
            "2026-08-03"
        );
    }

    #[test]
    fn file_and_frontmatter() {
        assert_eq!(text("# <% tp.file.title %>"), "# 2026-08-09");
        assert_eq!(
            text("<% tp.file.folder() %>|<% tp.file.folder(true) %>"),
            "Daily|Journal/Daily"
        );
        assert_eq!(
            text("<% tp.file.path(true) %>"),
            "Journal/Daily/2026-08-09.md"
        );
        assert_eq!(text("<% tp.file.creation_date() %>"), "2026-08-01 09:30");
        assert_eq!(
            text("<% tp.file.last_modified_date(\"HH:mm\") %>"),
            "14:07",
            "unknown: now"
        );
        assert_eq!(text("<% tp.file.tags %>"), "#a,#b,#inline");
        assert_eq!(text("<% tp.file.selection() %>"), "picked");
        assert_eq!(
            text("<% tp.frontmatter.mood %> <% tp.frontmatter[\"tags\"] %>"),
            "good a,b"
        );
        assert_eq!(
            text("<% tp.frontmatter.list %>|<% tp.frontmatter.none %>|"),
            "x,y||"
        );
        assert_eq!(
            text("<% tp.file.exists(\"Loop.md\") %> <% tp.file.exists(\"No.md\") %>"),
            "true false"
        );
        assert_eq!(text("<% \"a\" + 1 + tp.file.title %>"), "a12026-08-09");
    }

    #[test]
    fn whitespace_control() {
        assert_eq!(text("a\n<%- \"b\" -%>\nc"), "abc");
        assert_eq!(text("a  \n\n<%_ \"b\" _%>\n\n  c"), "abc");
        assert_eq!(text("a\n<% \"b\" %>\nc"), "a\nb\nc");
    }

    #[test]
    fn include_and_cursor() {
        assert_eq!(
            text("<% tp.file.include(\"[[Sig]]\") %>").lines().next(),
            Some("Regards,")
        );
        assert_eq!(
            text("<% tp.file.include(\"[[Snippets/Sig#Part]]\") %>"),
            "## Part\npart text"
        );
        let template = "ab<% tp.file.cursor(2) %>cd<% tp.file.cursor(1) %>e";
        let o = out(template, &[]).unwrap();
        let rest = "ab<% tp.file.cursor(2) %>cd";
        assert_eq!(
            (o.text.as_str(), o.cursor),
            ("ab<% tp.file.cursor(2) %>cde", Some(rest.chars().count())),
            "the lowest order wins; the others stay for \"Jump to next cursor\""
        );
        let mut more = More::default();
        more.setup.auto_jump = false;
        let o = render(
            template,
            &Env {
                vault: vault(),
                target: &target(),
                now: now(),
                answers: &[],
                more: &more,
            },
        )
        .unwrap();
        assert_eq!((o.text.as_str(), o.cursor), (template, None), "no jump");
        let e = out("<% tp.file.include(\"[[Loop]]\") %>", &[]).unwrap_err();
        assert!(e.contains("nests too deep"), "{e}");
    }

    #[test]
    fn questions_are_collected_then_answered() {
        let template = "Hi <% tp.system.prompt(\"Name\", \"you\") %>, <% tp.system.suggester([\"Red\", \"Blue\"], [\"r\", \"b\"]) %> <% tp.system.prompt(\"Name\", \"you\") %>";
        let first = out(template, &[]).unwrap();
        assert_eq!(
            first.questions,
            [
                Question::Text {
                    prompt: "Name".into(),
                    default: "you".into()
                },
                Question::Choose {
                    prompt: "Choose".into(),
                    items: vec!["Red".into(), "Blue".into()]
                },
            ],
            "the same prompt is asked once"
        );
        let answered = out(template, &[Answer::Text("Ann".into()), Answer::Choice(1)]).unwrap();
        assert_eq!(answered.text, "Hi Ann, b Ann");
    }

    #[test]
    fn errors_name_the_command() {
        assert_eq!(
            out("<%* tR += 'x' %>", &[]).unwrap().text,
            "x",
            "runs as JavaScript"
        );
        assert_eq!(
            out("<% tp.web.daily_quote() %>", &[]).unwrap().needs.len(),
            1,
            "runs as JavaScript, asking for the quotes"
        );
        assert!(
            out("<% tp.file.nope %>", &[])
                .unwrap_err()
                .contains("unknown tp.file.nope")
        );
        assert!(
            out("<% tp.file.title", &[])
                .unwrap_err()
                .contains("isn't closed")
        );
        assert_eq!(
            out("<% moment().format('YYYY') %>", &[]).unwrap().text,
            "2026",
            "runs as JavaScript"
        );
        let e = out("<% nope.x %>", &[]).unwrap_err();
        assert!(e.contains("nope"), "JavaScript's error: {e}");
    }
}
