//! Vault search: finds notes by their text, like Obsidian's search.
//! Several words must all match (anywhere in the note); `"a phrase"` is one
//! piece; `-word` leaves notes out; `OR` gives either; parentheses group.
//! Each word is smart case (like mdedit's Ctrl+F: a capital letter makes it
//! case-sensitive). Terms: `tag:name` (nested tags too: `tag:work` finds
//! `#work/project`), `file:` (the note's name), `path:` (its path),
//! `line:(a b)` (words on one line), `task:` / `task-todo:` / `task-done:`
//! (task lines, all / open / done), `[name]` / `[name:value]` (a
//! property in the frontmatter).

use super::{Note, Vault};

/// Matches shown per note; the rest are counted.
pub const MAX_LINES_PER_NOTE: usize = 10;

/// What a search query asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// Nothing to search for.
    Empty,
    Expr(Expr),
}

/// A search expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Term(Term),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

/// One thing to look for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// A word or a phrase, in the note's name or text.
    Text(String),
    /// A tag, without `#`.
    Tag(String),
    /// In the note's name.
    File(String),
    /// In its path in the vault.
    Path(String),
    /// These words on one line.
    Line(Vec<String>),
    /// A task line with this text (any if empty): `None` any task, `Some(done)`.
    Task(Option<bool>, String),
    /// `[name]` a property, `[name:value]` with this in its value.
    Property(String, Option<String>),
}

/// A piece of the query as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    Or,
    Not,
    /// A word (or `op:value`), or a quoted phrase.
    Word(String, bool),
}

fn tokens(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    // Text up to the closing `end` (or the end of the input).
    let group = |i: &mut usize, end: char| {
        let start = *i;
        while *i < chars.len() && chars[*i] != end {
            *i += 1;
        }
        let text: String = chars[start..*i].iter().collect();
        *i += 1;
        text
    };
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '(' {
            out.push(Token::Open);
            i += 1;
        } else if c == ')' {
            out.push(Token::Close);
            i += 1;
        } else if c == '"' {
            i += 1;
            out.push(Token::Word(group(&mut i, '"'), true));
        } else if c == '[' {
            i += 1;
            out.push(Token::Word(format!("[{}]", group(&mut i, ']')), false));
        } else if c == '-' && chars.get(i + 1).is_some_and(|n| !n.is_whitespace()) {
            out.push(Token::Not);
            i += 1;
        } else {
            let mut word = String::new();
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' && chars[i] != ')'
            {
                word.push(chars[i]);
                i += 1;
                // `op:(…)` and `op:"…"` take the group as their value.
                if word.ends_with(':') && operator(&word[..word.len() - 1]) {
                    match chars.get(i) {
                        Some('(') => {
                            i += 1;
                            word.push_str(&group(&mut i, ')'));
                            break;
                        }
                        Some('"') => {
                            i += 1;
                            word.push_str(&group(&mut i, '"'));
                            break;
                        }
                        _ => {}
                    }
                }
            }
            // `tag:` with the name after a space.
            if word == "tag:" {
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                while i < chars.len() && !chars[i].is_whitespace() && chars[i] != ')' {
                    word.push(chars[i]);
                    i += 1;
                }
            }
            if word == "OR" {
                out.push(Token::Or);
            } else {
                out.push(Token::Word(word, false));
            }
        }
    }
    out
}

/// Whether `name` is a search operator (`name:value`).
fn operator(name: &str) -> bool {
    matches!(
        name,
        "tag" | "file" | "path" | "line" | "task" | "task-todo" | "task-done"
    )
}

/// The term a word stands for (`None`: nothing, e.g. `tag:` alone).
fn term(word: &str, quoted: bool) -> Option<Term> {
    if let Some(inner) = word.strip_prefix('[').and_then(|w| w.strip_suffix(']')) {
        let (name, value) = match inner.split_once(':') {
            Some((name, value)) => (name, Some(value.trim().trim_matches('"').to_string())),
            None => (inner, None),
        };
        let name = name.trim();
        return (!name.is_empty() && !quoted)
            .then(|| Term::Property(name.to_string(), value.filter(|v| !v.is_empty())));
    }
    let op = (!quoted)
        .then(|| word.split_once(':'))
        .flatten()
        .filter(|(name, _)| operator(name));
    let Some((name, value)) = op else {
        return (!word.is_empty()).then(|| Term::Text(word.to_string()));
    };
    let value = value.trim();
    Some(match name {
        "tag" => {
            let tag = value.trim_start_matches('#');
            if tag.is_empty() {
                return None;
            }
            Term::Tag(tag.to_string())
        }
        "file" if !value.is_empty() => Term::File(value.into()),
        "path" if !value.is_empty() => Term::Path(value.into()),
        "line" if !value.is_empty() => {
            Term::Line(value.split_whitespace().map(String::from).collect())
        }
        "task" => Term::Task(None, value.into()),
        "task-todo" => Term::Task(Some(false), value.into()),
        "task-done" => Term::Task(Some(true), value.into()),
        _ => return None,
    })
}

/// Parses tokens: `OR` binds less than the words side by side (AND).
struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn or(&mut self) -> Option<Expr> {
        let mut any = vec![self.and()];
        while self.tokens.get(self.at) == Some(&Token::Or) {
            self.at += 1;
            any.push(self.and());
        }
        let mut any: Vec<Expr> = any.into_iter().flatten().collect();
        match any.len() {
            0 => None,
            1 => any.pop(),
            _ => Some(Expr::Or(any)),
        }
    }

    fn and(&mut self) -> Option<Expr> {
        let mut all = Vec::new();
        while let Some(token) = self.tokens.get(self.at) {
            if matches!(token, Token::Or | Token::Close) {
                break;
            }
            if let Some(e) = self.unary() {
                all.push(e);
            }
        }
        match all.len() {
            0 => None,
            1 => all.pop(),
            _ => Some(Expr::And(all)),
        }
    }

    fn unary(&mut self) -> Option<Expr> {
        let token = self.tokens.get(self.at)?.clone();
        self.at += 1;
        match token {
            Token::Not => self.unary().map(|e| Expr::Not(Box::new(e))),
            Token::Open => {
                let inner = self.or();
                if self.tokens.get(self.at) == Some(&Token::Close) {
                    self.at += 1;
                }
                inner
            }
            Token::Word(word, quoted) => term(&word, quoted).map(Expr::Term),
            Token::Or | Token::Close => None,
        }
    }
}

impl Query {
    pub fn parse(input: &str) -> Query {
        let mut parser = Parser {
            tokens: tokens(input),
            at: 0,
        };
        let mut all = Vec::new();
        // Stray `)` are skipped.
        while parser.at < parser.tokens.len() {
            if let Some(e) = parser.or() {
                all.push(e);
            }
            if parser.tokens.get(parser.at) == Some(&Token::Close) {
                parser.at += 1;
            }
        }
        match all.len() {
            0 => Query::Empty,
            1 => Query::Expr(all.pop().expect("one")),
            _ => Query::Expr(Expr::And(all)),
        }
    }

    /// The text to highlight in matching lines: the first word (or tag)
    /// the notes must have.
    pub fn highlight(&self) -> Option<String> {
        fn first(e: &Expr) -> Option<String> {
            match e {
                Expr::Term(Term::Text(t)) => Some(t.clone()),
                Expr::Term(Term::Tag(t)) => Some(format!("#{t}")),
                Expr::Term(Term::Line(words)) => words.first().cloned(),
                Expr::Term(Term::Task(_, t)) if !t.is_empty() => Some(t.clone()),
                Expr::Term(Term::Property(_, Some(v))) => Some(v.clone()),
                Expr::Term(_) | Expr::Not(_) => None,
                Expr::And(all) | Expr::Or(all) => all.iter().find_map(first),
            }
        }
        match self {
            Query::Empty => None,
            Query::Expr(e) => first(e),
        }
    }
}

/// One note with matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteHits {
    /// Index into [`Vault::notes`].
    pub note: usize,
    /// Matches as (line, char column), at most [`MAX_LINES_PER_NOTE`], one
    /// per line.
    pub lines: Vec<(usize, usize)>,
    /// All matching lines, including those not in `lines`.
    pub total: usize,
}

/// The notes matching `query`: notes whose name matches first, then by
/// path.
pub fn run(vault: &Vault, query: &Query) -> Vec<NoteHits> {
    // On every core: each note is searched on its own.
    let found = crate::vault::par_map(&vault.notes, |note| note_hits(note, query));
    let mut hits: Vec<(bool, NoteHits)> = found
        .into_iter()
        .enumerate()
        .filter_map(|(i, f)| f.map(|(named, hits)| (named, hit(i, hits))))
        .collect();
    // Stable: notes are already sorted by path.
    hits.sort_by_key(|(named, _)| !named);
    hits.into_iter().map(|(_, h)| h).collect()
}

fn hit(note: usize, matches: Vec<(usize, usize)>) -> NoteHits {
    let mut lines: Vec<(usize, usize)> = Vec::new();
    for m in matches {
        if lines.last().is_none_or(|l| l.0 != m.0) {
            lines.push(m);
        }
    }
    let total = lines.len();
    lines.truncate(MAX_LINES_PER_NOTE);
    NoteHits { note, lines, total }
}

/// Whether `note` matches, whether by its name, and the matches in its text.
fn note_hits(note: &Note, query: &Query) -> Option<(bool, Vec<(usize, usize)>)> {
    let Query::Expr(expr) = query else {
        return None;
    };
    let (named, mut lines) = eval(note, expr)?;
    lines.sort_unstable();
    lines.dedup();
    Some((named, lines))
}

/// Whether `note` matches `expr`: `Some((matched by its name, matches))`.
fn eval(note: &Note, expr: &Expr) -> Option<(bool, Vec<(usize, usize)>)> {
    match expr {
        Expr::Term(term) => term_hits(note, term),
        Expr::Not(inner) => eval(note, inner).is_none().then_some((false, Vec::new())),
        Expr::And(all) => {
            let mut named = false;
            let mut lines = Vec::new();
            for e in all {
                let (n, l) = eval(note, e)?;
                named |= n;
                lines.extend(l);
            }
            Some((named, lines))
        }
        Expr::Or(any) => {
            let found: Vec<_> = any.iter().filter_map(|e| eval(note, e)).collect();
            if found.is_empty() {
                return None;
            }
            let named = found.iter().any(|f| f.0);
            Some((named, found.into_iter().flat_map(|f| f.1).collect()))
        }
    }
}

/// Whether `note` has `text` (smart case) in `hay`.
fn has(hay: &str, text: &str) -> bool {
    !mdedit::search::highlights(hay, text).is_empty()
}

/// A task line's done state and where its text starts (`- [x] text`).
fn task(line: &str) -> Option<(bool, usize)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let rest = trimmed
        .strip_prefix("- [")
        .or_else(|| trimmed.strip_prefix("* ["))
        .or_else(|| trimmed.strip_prefix("+ ["))?;
    let mut chars = rest.chars();
    let state = chars.next()?;
    chars.next().filter(|&c| c == ']')?;
    Some((matches!(state, 'x' | 'X'), indent + 6))
}

fn term_hits(note: &Note, term: &Term) -> Option<(bool, Vec<(usize, usize)>)> {
    match term {
        Term::Text(text) => {
            let named = has(&note.name(), text);
            let lines = mdedit::search::matches(&note.lines, text);
            (named || !lines.is_empty()).then_some((named, lines))
        }
        Term::Tag(tag) => {
            let tag = tag.to_lowercase();
            let has = note.tags.iter().any(|t| {
                let t = t.to_lowercase();
                t == tag || t.strip_prefix(&tag).is_some_and(|r| r.starts_with('/'))
            });
            has.then(|| {
                (
                    false,
                    mdedit::search::matches(&note.lines, &format!("#{tag}")),
                )
            })
        }
        Term::File(text) => has(&note.name(), text).then_some((true, Vec::new())),
        Term::Property(name, value) => note
            .properties
            .iter()
            .any(|(key, v)| {
                key.eq_ignore_ascii_case(name) && value.as_ref().is_none_or(|want| has(v, want))
            })
            .then_some((false, Vec::new())),
        Term::Path(text) => {
            let rel = note.rel.to_string_lossy().replace('\\', "/");
            has(&rel, text).then_some((false, Vec::new()))
        }
        Term::Line(words) => {
            let lines: Vec<(usize, usize)> = note
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| words.iter().all(|w| has(line, w)))
                .map(|(i, line)| {
                    let col = mdedit::search::matches(std::slice::from_ref(line), &words[0])
                        .first()
                        .map_or(0, |m| m.1);
                    (i, col)
                })
                .collect();
            (!lines.is_empty()).then_some((false, lines))
        }
        Term::Task(done, text) => {
            let lines: Vec<(usize, usize)> = note
                .lines
                .iter()
                .enumerate()
                .filter_map(|(i, line)| {
                    let (is_done, start) = task(line)?;
                    if done.is_some_and(|d| d != is_done) {
                        return None;
                    }
                    if text.is_empty() {
                        return Some((i, 0));
                    }
                    let body: String = line.chars().skip(start).collect();
                    mdedit::search::matches(&[body], text)
                        .first()
                        .map(|m| (i, start + m.1))
                })
                .collect();
            (!lines.is_empty()).then_some((false, lines))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    fn vault(name: &str) -> Vault {
        let dir = scratch(name);
        write(
            &dir,
            &[
                ("Cooking.md", "nothing here"),
                ("a.md", "one cooking line\nand Cooking again\n#food"),
                ("b.md", "---\ntags: food/italian\n---\nPasta"),
                ("c.md", "#foodie"),
            ],
        );
        Vault::open(&dir).unwrap()
    }

    fn names(vault: &Vault, hits: &[NoteHits]) -> Vec<String> {
        hits.iter().map(|h| vault.notes[h.note].name()).collect()
    }

    #[test]
    fn queries() {
        assert_eq!(Query::parse("  "), Query::Empty);
        assert_eq!(Query::parse("tag:"), Query::Empty);
        assert_eq!(
            Query::parse(" pasta ").highlight().as_deref(),
            Some("pasta")
        );
        assert_eq!(
            Query::parse("tag:#food").highlight().as_deref(),
            Some("#food")
        );
        assert_eq!(
            Query::parse("tag: food").highlight().as_deref(),
            Some("#food")
        );
        assert_eq!(
            Query::parse("-x \"star wars\" y").highlight().as_deref(),
            Some("star wars"),
            "the first word that must match"
        );
    }

    /// A vault for the search terms.
    fn terms_vault(name: &str) -> Vault {
        let dir = scratch(name);
        write(
            &dir,
            &[
                ("Cooking.md", "nothing here"),
                ("a.md", "one cooking line\nand Cooking again\n#food"),
                ("b.md", "---\ntags: food/italian\n---\nPasta"),
                (
                    "Lists/Shop.md",
                    "- [ ] buy pasta\n- [x] cook dinner\nstar wars",
                ),
            ],
        );
        Vault::open(&dir).unwrap()
    }

    fn found(v: &Vault, query: &str) -> Vec<String> {
        let mut n = names(v, &run(v, &Query::parse(query)));
        n.sort();
        n
    }

    #[test]
    fn words_phrases_or_not_and_brackets() {
        let v = terms_vault("search-terms");
        assert_eq!(
            found(&v, "cooking line"),
            ["a"],
            "every word, anywhere in the note"
        );
        assert_eq!(found(&v, "pasta OR nothing"), ["Cooking", "Shop", "b"]);
        assert_eq!(found(&v, "cooking -again"), ["Cooking"], "not with again");
        assert_eq!(
            found(&v, "\"line and\""),
            Vec::<String>::new(),
            "a phrase is one piece"
        );
        assert_eq!(found(&v, "\"star wars\""), ["Shop"]);
        assert_eq!(
            found(&v, "(pasta OR nothing) -tag:food"),
            ["Cooking", "Shop"]
        );
        let hits = run(&v, &Query::parse("cooking line"));
        assert_eq!(hits[0].lines, [(0, 4), (1, 4)], "a match per line");
    }

    #[test]
    fn file_path_line_and_task_terms() {
        let v = terms_vault("search-ops");
        assert_eq!(found(&v, "file:cook"), ["Cooking"]);
        assert_eq!(found(&v, "path:lists/"), ["Shop"]);
        assert_eq!(found(&v, "line:(cooking again)"), ["a"]);
        assert_eq!(
            found(&v, "line:(one again)"),
            Vec::<String>::new(),
            "not on one line"
        );
        assert_eq!(found(&v, "task:pasta"), ["Shop"]);
        assert_eq!(
            found(&v, "task:star"),
            Vec::<String>::new(),
            "not a task line"
        );
        assert_eq!(
            found(&v, "task-todo:cook"),
            Vec::<String>::new(),
            "it's done"
        );
        assert_eq!(found(&v, "task-done:cook"), ["Shop"]);
        assert_eq!(found(&v, "[tags]"), ["b"], "a property");
        assert_eq!(found(&v, "[tags:italian]"), ["b"]);
        assert_eq!(found(&v, "[tags:french]"), Vec::<String>::new());
        let hits = run(&v, &Query::parse("task-todo:"));
        assert_eq!(hits[0].lines, [(0, 0)], "any open task");
    }

    #[test]
    fn text_search_is_smart_case_and_puts_name_matches_first() {
        let v = vault("search-text");
        let hits = run(&v, &Query::parse("cooking"));
        assert_eq!(names(&v, &hits), ["Cooking", "a"]);
        assert_eq!(hits[1].lines, [(0, 4), (1, 4)]);
        let hits = run(&v, &Query::parse("Cooking"));
        assert_eq!(names(&v, &hits), ["Cooking", "a"]);
        assert_eq!(hits[1].lines, [(1, 4)], "a capital: case-sensitive");
    }

    #[test]
    fn tag_search_includes_nested_tags_but_not_longer_names() {
        let v = vault("search-tag");
        let hits = run(&v, &Query::parse("tag:food"));
        assert_eq!(names(&v, &hits), ["a", "b"]);
        assert_eq!(hits[0].lines, [(2, 0)]);
        assert_eq!(hits[1].lines, [], "the tag is in the frontmatter");
    }

    #[test]
    fn many_matches_are_counted_but_not_all_listed() {
        let dir = scratch("search-many");
        write(&dir, &[("n.md", &"x x\n".repeat(25))]);
        let v = Vault::open(&dir).unwrap();
        let hits = run(&v, &Query::parse("x"));
        assert_eq!(hits[0].lines.len(), MAX_LINES_PER_NOTE);
        assert_eq!(hits[0].total, 25, "one per line");
    }
}
