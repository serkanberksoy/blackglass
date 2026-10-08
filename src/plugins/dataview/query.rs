//! Parses Dataview's query language (DQL): `LIST`, `TABLE`, `TASK` and
//! `CALENDAR` queries with `FROM`, then `WHERE`, `SORT`, `LIMIT`, `GROUP
//! BY` and `FLATTEN` in any order (they run in the order written).
//! Keywords are case-insensitive; clauses may share a line or have their
//! own. Expressions have literals (`[1, 2]`, `{a: 1}`), indexing
//! (`list[0]`, `obj.key`, `[[Note]].field`), lambdas (`(x) => x + 1`) and
//! `+ - * / %`.
//!
//! ```text
//! TABLE author, rating AS "Stars" FROM #reading AND "Books"
//! WHERE rating >= 4 SORT file.name DESC LIMIT 10
//! ```

use super::value::{Value, link_name, parse_date};

/// A parsed query.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub kind: Kind,
    pub from: Option<Source>,
    /// What runs on the rows, in order.
    pub commands: Vec<Command>,
}

/// A data command, run on the rows in the order written.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Where(Expr),
    /// Keys, each descending or not.
    Sort(Vec<(Expr, bool)>),
    Limit(usize),
    /// Rows with the same value make one row (`key`, `rows`); `AS` names
    /// the key.
    GroupBy(Expr, Option<String>),
    /// One row per entry of a list; `AS` names the entry (else the
    /// expression's text does).
    Flatten(Expr, String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// `LIST` with an optional value after each page; `WITHOUT ID` shows
    /// only the values.
    List {
        value: Option<Expr>,
        without_id: bool,
    },
    /// `TABLE`: columns (expression, header); `WITHOUT ID` hides the
    /// page column.
    Table {
        columns: Vec<(Expr, String)>,
        without_id: bool,
    },
    Task,
    /// `CALENDAR date`: a month grid with the pages on their days.
    Calendar(Expr),
}

/// Which pages a query looks at.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// `#tag` (nested tags included), without `#`.
    Tag(String),
    /// `"folder"` (or a note's path).
    Folder(String),
    /// `[[Note]]`: pages linking to the note (`""`: to the current one,
    /// `[[]]`).
    LinksTo(String),
    /// `outgoing([[Note]])`: the pages the note links to (`""`: the
    /// current one).
    Outgoing(String),
    And(Box<Source>, Box<Source>),
    Or(Box<Source>, Box<Source>),
    Not(Box<Source>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Value(Value),
    /// A field path: `rating`, `file.name`, `this.file.day`.
    Field(Vec<String>),
    Call(String, Vec<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    /// `[a, b]`.
    List(Vec<Expr>),
    /// `{key: value}`.
    Object(Vec<(String, Expr)>),
    /// `base[index]`, `base.key`.
    Index(Box<Expr>, Box<Expr>),
    /// `(x, y) => body`, for `map`, `filter` and the like.
    Lambda(Vec<String>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Number(f64),
    Text(String),
    Link(String),
    Tag(String),
    Symbol(&'static str),
}

/// Words that start a clause (and so end the one before).
const CLAUSES: [&str; 7] = ["from", "where", "sort", "limit", "group", "flatten", "as"];

/// Parses a whole query; `Err` says what's wrong, for the block.
pub fn parse(source: &str) -> Result<Query, String> {
    let (tokens, spans) = tokenize(source)?.into_iter().unzip();
    Parser {
        tokens,
        spans,
        at: 0,
        source,
    }
    .query()
}

/// Parses one expression (an inline query's: `` `= this.file.name` ``).
pub fn parse_expr(source: &str) -> Result<Expr, String> {
    let (tokens, spans) = tokenize(source)?.into_iter().unzip();
    let mut p = Parser {
        tokens,
        spans,
        at: 0,
        source,
    };
    let e = p.expr()?;
    if p.peek().is_some() {
        return Err(format!("unexpected {}", p.describe()));
    }
    Ok(e)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    /// Each token's byte range in the source (for column headers).
    spans: Vec<(usize, usize)>,
    at: usize,
    source: &'a str,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.at).cloned();
        self.at += 1;
        t
    }

    /// Whether the next token is the keyword `word`.
    fn at_word(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Token::Word(w)) if w.eq_ignore_ascii_case(word))
    }

    fn eat_word(&mut self, word: &str) -> bool {
        let yes = self.at_word(word);
        if yes {
            self.at += 1;
        }
        yes
    }

    fn eat(&mut self, symbol: &str) -> bool {
        let yes = matches!(self.peek(), Some(Token::Symbol(s)) if *s == symbol);
        if yes {
            self.at += 1;
        }
        yes
    }

    /// Whether a clause starts here (`sort(…)` is the function, though).
    fn at_clause(&self) -> bool {
        self.peek().is_none() || (CLAUSES.iter().any(|c| self.at_word(c)) && !self.call_next())
    }

    /// Whether the token after this one is `(` (a function call).
    fn call_next(&self) -> bool {
        matches!(self.tokens.get(self.at + 1), Some(Token::Symbol("(")))
    }

    fn query(&mut self) -> Result<Query, String> {
        let kind = match self.next() {
            Some(Token::Word(w)) if w.eq_ignore_ascii_case("list") => {
                let without_id = self.without_id()?;
                Kind::List {
                    value: if self.at_clause() {
                        None
                    } else {
                        Some(self.expr()?)
                    },
                    without_id,
                }
            }
            Some(Token::Word(w)) if w.eq_ignore_ascii_case("table") => self.table()?,
            Some(Token::Word(w)) if w.eq_ignore_ascii_case("task") => Kind::Task,
            Some(Token::Word(w)) if w.eq_ignore_ascii_case("calendar") => {
                if self.at_clause() {
                    return Err("CALENDAR needs a date (CALENDAR file.day)".into());
                }
                Kind::Calendar(self.expr()?)
            }
            _ => return Err("a query starts with LIST, TABLE, TASK or CALENDAR".into()),
        };
        let mut query = Query {
            kind,
            from: None,
            commands: Vec::new(),
        };
        while self.peek().is_some() {
            if self.eat_word("from") {
                if query.from.is_some() {
                    return Err("only one FROM".into());
                }
                if !query.commands.is_empty() {
                    return Err("FROM comes before WHERE, SORT and the others".into());
                }
                query.from = Some(self.source_or()?);
            } else if self.eat_word("where") {
                query.commands.push(Command::Where(self.expr()?));
            } else if self.eat_word("sort") {
                let mut keys = Vec::new();
                loop {
                    let key = self.expr()?;
                    let descending = self.eat_word("desc") || self.eat_word("descending");
                    if !descending && !self.eat_word("asc") {
                        self.eat_word("ascending");
                    }
                    keys.push((key, descending));
                    if !self.eat(",") {
                        break;
                    }
                }
                query.commands.push(Command::Sort(keys));
            } else if self.eat_word("limit") {
                match self.next() {
                    Some(Token::Number(n)) if n >= 0.0 && n.fract() == 0.0 => {
                        query.commands.push(Command::Limit(n as usize));
                    }
                    _ => return Err("LIMIT needs a whole number".into()),
                }
            } else if self.eat_word("group") {
                if !self.eat_word("by") {
                    return Err("expected GROUP BY".into());
                }
                let expr = self.expr()?;
                let name = self.name_after_as()?;
                query.commands.push(Command::GroupBy(expr, name));
            } else if self.eat_word("flatten") {
                let start = self.at;
                let expr = self.expr()?;
                let text = self.text_of(start, self.at);
                let name = self.name_after_as()?.unwrap_or(text);
                query.commands.push(Command::Flatten(expr, name));
            } else {
                return Err(format!("unexpected {}", self.describe()));
            }
        }
        Ok(query)
    }

    /// `AS name`, if it's next.
    fn name_after_as(&mut self) -> Result<Option<String>, String> {
        if !self.eat_word("as") {
            return Ok(None);
        }
        match self.next() {
            Some(Token::Text(t) | Token::Word(t)) => Ok(Some(t)),
            _ => Err("AS needs a name".into()),
        }
    }

    /// `WITHOUT ID`, if it's next.
    fn without_id(&mut self) -> Result<bool, String> {
        if !self.at_word("without") {
            return Ok(false);
        }
        self.at += 1;
        if !self.eat_word("id") {
            return Err("expected WITHOUT ID".into());
        }
        Ok(true)
    }

    fn describe(&self) -> String {
        match self.peek() {
            Some(Token::Word(w)) => format!("\"{w}\""),
            Some(Token::Symbol(s)) => format!("\"{s}\""),
            Some(Token::Number(n)) => format!("{n}"),
            Some(Token::Text(t)) => format!("\"{t}\""),
            Some(Token::Link(l)) => format!("[[{l}]]"),
            Some(Token::Tag(t)) => format!("#{t}"),
            None => "end of query".into(),
        }
    }

    fn table(&mut self) -> Result<Kind, String> {
        let without_id = self.without_id()?;
        let mut columns = Vec::new();
        if !self.at_clause() {
            loop {
                let start = self.at;
                let expr = self.expr()?;
                let text = self.text_of(start, self.at);
                let header = if self.eat_word("as") {
                    match self.next() {
                        Some(Token::Text(t) | Token::Word(t)) => t,
                        _ => return Err("AS needs a name".into()),
                    }
                } else {
                    text
                };
                columns.push((expr, header));
                if !self.eat(",") {
                    break;
                }
            }
        }
        Ok(Kind::Table {
            columns,
            without_id,
        })
    }

    /// The source text of tokens `from..to`.
    fn text_of(&self, from: usize, to: usize) -> String {
        match (self.spans.get(from), self.spans.get(to.saturating_sub(1))) {
            (Some(a), Some(b)) => self.source[a.0..b.1].to_string(),
            _ => String::new(),
        }
    }

    fn source_or(&mut self) -> Result<Source, String> {
        let mut left = self.source_and()?;
        while self.eat_word("or") {
            left = Source::Or(Box::new(left), Box::new(self.source_and()?));
        }
        Ok(left)
    }

    fn source_and(&mut self) -> Result<Source, String> {
        let mut left = self.source_one()?;
        while self.eat_word("and") {
            left = Source::And(Box::new(left), Box::new(self.source_one()?));
        }
        Ok(left)
    }

    fn source_one(&mut self) -> Result<Source, String> {
        if self.eat("-") || self.eat("!") {
            return Ok(Source::Not(Box::new(self.source_one()?)));
        }
        if self.eat("(") {
            let inner = self.source_or()?;
            if !self.eat(")") {
                return Err("missing ) in FROM".into());
            }
            return Ok(inner);
        }
        // `outgoing([[Note]])`: the pages it links to.
        if self.at_word("outgoing") {
            self.at += 1;
            let ok = self.eat("(");
            let target = match self.next() {
                Some(Token::Link(l)) if ok => link_name(&l),
                _ => return Err("outgoing needs a link: outgoing([[Note]])".into()),
            };
            if !self.eat(")") {
                return Err("missing ) after outgoing([[…]]".into());
            }
            return Ok(Source::Outgoing(target));
        }
        match self.next() {
            Some(Token::Tag(t)) => Ok(Source::Tag(t)),
            Some(Token::Text(folder)) => Ok(Source::Folder(folder.trim_matches('/').to_string())),
            // `[[]]` and `[[#]]` are the current note.
            Some(Token::Link(l)) => Ok(Source::LinksTo(link_name(&l))),
            _ => {
                self.at -= 1;
                Err(format!(
                    "FROM takes #tags, \"folders\" and [[links]], not {}",
                    self.describe()
                ))
            }
        }
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.binary(0)
    }

    /// Operators by precedence, loosest first.
    fn binary(&mut self, level: usize) -> Result<Expr, String> {
        const LEVELS: [&[(&str, Op)]; 5] = [
            &[("or", Op::Or)],
            &[("and", Op::And)],
            &[
                ("=", Op::Eq),
                ("!=", Op::Ne),
                ("<=", Op::Le),
                (">=", Op::Ge),
                ("<", Op::Lt),
                (">", Op::Gt),
            ],
            &[("+", Op::Add), ("-", Op::Sub)],
            &[("*", Op::Mul), ("/", Op::Div), ("%", Op::Mod)],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        'outer: loop {
            for &(symbol, op) in LEVELS[level] {
                let found = if symbol.chars().all(char::is_alphabetic) {
                    self.eat_word(symbol)
                } else {
                    self.eat(symbol)
                };
                if found {
                    let right = self.binary(level + 1)?;
                    left = Expr::Binary(op, Box::new(left), Box::new(right));
                    continue 'outer;
                }
            }
            return Ok(left);
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat("!") {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.eat("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        let mut e = self.primary()?;
        // Indexing: `x[0]`, `x.key` (`[[Note]].field`, `list[0].text`).
        loop {
            if self.eat("[") {
                let index = self.expr()?;
                if !self.eat("]") {
                    return Err("missing ] after an index".into());
                }
                e = Expr::Index(Box::new(e), Box::new(index));
            } else if self.eat(".") {
                match self.next() {
                    Some(Token::Word(w)) => {
                        for key in w.split('.') {
                            e = Expr::Index(
                                Box::new(e),
                                Box::new(Expr::Value(Value::Text(key.to_string()))),
                            );
                        }
                    }
                    _ => return Err("expected a name after .".into()),
                }
            } else {
                return Ok(e);
            }
        }
    }

    /// `(a, b) =>` or `a =>` starting here: the parameters.
    fn lambda_params(&self) -> Option<(Vec<String>, usize)> {
        let mut at = self.at;
        let word = |at: usize| match self.tokens.get(at) {
            Some(Token::Word(w)) if !w.contains('.') => Some(w.clone()),
            _ => None,
        };
        let symbol =
            |at: usize, s: &str| matches!(self.tokens.get(at), Some(Token::Symbol(x)) if *x == s);
        if let Some(w) = word(at)
            && symbol(at + 1, "=>")
        {
            return Some((vec![w], at + 2));
        }
        if !symbol(at, "(") {
            return None;
        }
        at += 1;
        let mut params = Vec::new();
        if !symbol(at, ")") {
            loop {
                params.push(word(at)?);
                at += 1;
                if symbol(at, ")") {
                    break;
                }
                if !symbol(at, ",") {
                    return None;
                }
                at += 1;
            }
        }
        symbol(at + 1, "=>").then_some((params, at + 2))
    }

    fn primary(&mut self) -> Result<Expr, String> {
        if let Some((params, body)) = self.lambda_params() {
            self.at = body;
            return Ok(Expr::Lambda(params, Box::new(self.expr()?)));
        }
        if self.eat("[") {
            let mut items = Vec::new();
            if !self.eat("]") {
                loop {
                    items.push(self.expr()?);
                    if self.eat("]") {
                        break;
                    }
                    if !self.eat(",") {
                        return Err("missing ] after a list".into());
                    }
                }
            }
            return Ok(Expr::List(items));
        }
        if self.eat("{") {
            let mut entries = Vec::new();
            if !self.eat("}") {
                loop {
                    let key = match self.next() {
                        Some(Token::Word(w) | Token::Text(w)) => w,
                        _ => return Err("an object's keys are names: {name: value}".into()),
                    };
                    if !self.eat(":") {
                        return Err(format!("missing : after {key}"));
                    }
                    entries.push((key, self.expr()?));
                    if self.eat("}") {
                        break;
                    }
                    if !self.eat(",") {
                        return Err("missing } after an object".into());
                    }
                }
            }
            return Ok(Expr::Object(entries));
        }
        match self.next() {
            Some(Token::Number(n)) => Ok(Expr::Value(Value::Number(n))),
            Some(Token::Text(t)) => Ok(Expr::Value(Value::Text(t))),
            // A link in an expression keeps its heading (`section = [[N#H]]`).
            Some(Token::Link(l)) => {
                let target = l.split('|').next().unwrap_or_default().trim();
                Ok(Expr::Value(Value::Link(
                    target.trim_end_matches(".md").to_string(),
                )))
            }
            Some(Token::Tag(t)) => Ok(Expr::Value(Value::Text(format!("#{t}")))),
            Some(Token::Symbol("(")) => {
                let inner = self.expr()?;
                if !self.eat(")") {
                    return Err("missing )".into());
                }
                Ok(inner)
            }
            Some(Token::Word(w)) => {
                let lower = w.to_lowercase();
                let call = matches!(self.peek(), Some(Token::Symbol("(")));
                if (CLAUSES.contains(&lower.as_str()) && !call)
                    || matches!(lower.as_str(), "and" | "or")
                {
                    self.at -= 1;
                    return Err(format!("expected a value before {}", self.describe()));
                }
                match lower.as_str() {
                    "true" => return Ok(Expr::Value(Value::Bool(true))),
                    "false" => return Ok(Expr::Value(Value::Bool(false))),
                    "null" => return Ok(Expr::Value(Value::Null)),
                    _ => {}
                }
                if self.eat("(") {
                    let mut args = Vec::new();
                    if !self.eat(")") {
                        loop {
                            args.push(self.call_arg(&lower)?);
                            if self.eat(")") {
                                break;
                            }
                            if !self.eat(",") {
                                return Err(format!("missing ) after {w}("));
                            }
                        }
                    }
                    return Ok(Expr::Call(lower, args));
                }
                Ok(Expr::Field(w.split('.').map(str::to_string).collect()))
            }
            _ => {
                self.at -= 1;
                Err(format!("expected a value, not {}", self.describe()))
            }
        }
    }

    /// An argument; `date(today)` and `date(2026-08-09)` take bare words,
    /// `dur(1 day 2 hours)` its words as they are.
    fn call_arg(&mut self, function: &str) -> Result<Expr, String> {
        if function == "dur" && !matches!(self.peek(), Some(Token::Text(_))) {
            let start = self.at;
            let mut depth = 0;
            while let Some(t) = self.peek() {
                match t {
                    Token::Symbol("(") => depth += 1,
                    Token::Symbol(")") if depth == 0 => break,
                    Token::Symbol(")") => depth -= 1,
                    Token::Symbol(",") if depth == 0 => break,
                    _ => {}
                }
                self.at += 1;
            }
            return Ok(Expr::Value(Value::Text(self.text_of(start, self.at))));
        }
        if function == "date" {
            match self.peek() {
                Some(Token::Word(w)) if parse_date(w).is_some() || is_date_word(w) => {
                    let w = w.clone();
                    self.at += 1;
                    return Ok(Expr::Value(Value::Text(w)));
                }
                _ => {}
            }
        }
        self.expr()
    }
}

/// `today`, `now`, `tomorrow`, `yesterday`.
pub fn is_date_word(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "today" | "now" | "tomorrow" | "yesterday"
    )
}

/// Splits the query into tokens, each with its byte range.
type Spanned = (Token, (usize, usize));

fn tokenize(source: &str) -> Result<Vec<Spanned>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<(usize, char)> = source.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        let rest = &source[at..];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let (token, len) = if c == '"' {
            // `\"` is a quote and `\\` a backslash; other escapes stay (`\d`
            // in a regex).
            let mut text = String::new();
            let mut end = None;
            let mut it = rest.char_indices().skip(1);
            while let Some((k, ch)) = it.next() {
                match ch {
                    '\\' => match it.next() {
                        Some((_, '"')) => text.push('"'),
                        Some((_, '\\')) => text.push('\\'),
                        Some((_, other)) => {
                            text.push('\\');
                            text.push(other);
                        }
                        None => break,
                    },
                    '"' => {
                        end = Some(k);
                        break;
                    }
                    other => text.push(other),
                }
            }
            let end = end.ok_or("a text in quotes isn't closed")?;
            (Token::Text(text), end + 1)
        } else if let Some(inner) = rest.strip_prefix("[[") {
            let end = inner.find("]]").ok_or("a [[link]] isn't closed")?;
            (Token::Link(inner[..end].to_string()), end + 4)
        } else if c == '#' {
            let len = rest[1..]
                .find(|c: char| !(c.is_alphanumeric() || "_-/".contains(c)))
                .unwrap_or(rest.len() - 1);
            (Token::Tag(rest[1..1 + len].to_string()), len + 1)
        } else if c.is_ascii_digit() {
            // A number, or a date written bare (`2026-08-09`).
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || "-:.".contains(c)))
                .unwrap_or(rest.len());
            let word = &rest[..len];
            match word.parse::<f64>() {
                Ok(n) => (Token::Number(n), len),
                Err(_) if parse_date(word).is_some() => (Token::Word(word.to_string()), len),
                Err(_) => {
                    let len = rest
                        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                        .unwrap_or(rest.len());
                    let n = rest[..len]
                        .parse()
                        .map_err(|_| format!("bad number {}", &rest[..len]))?;
                    (Token::Number(n), len)
                }
            }
        } else if c.is_alphabetic() || c == '_' {
            // A dash before a letter is part of the name, as in Dataview
            // (`due-date`, a property `Due Date`): `a - b` subtracts.
            let mut len = rest.len();
            for (i, ch) in rest.char_indices() {
                let dashed = ch == '-'
                    && rest[i + 1..]
                        .chars()
                        .next()
                        .is_some_and(|n| n.is_alphabetic() || n == '_');
                if !(ch.is_alphanumeric() || "_.".contains(ch) || dashed) {
                    len = i;
                    break;
                }
            }
            (
                Token::Word(rest[..len].trim_end_matches('.').to_string()),
                len,
            )
        } else {
            let symbol = [
                "=>", "!=", "<=", ">=", "=", "<", ">", "+", "-", "*", "/", "%", "!", "(", ")", ",",
                "[", "]", "{", "}", ":", ".",
            ]
            .into_iter()
            .find(|s| rest.starts_with(s))
            .ok_or_else(|| format!("unexpected \"{c}\""))?;
            (Token::Symbol(symbol), symbol.len())
        };
        tokens.push((token, (at, at + len)));
        // `len` is in bytes: step over the chars it covers.
        let end = at + len;
        while i < chars.len() && chars[i].0 < end {
            i += 1;
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(path: &str) -> Expr {
        Expr::Field(path.split('.').map(str::to_string).collect())
    }

    fn num(n: f64) -> Expr {
        Expr::Value(Value::Number(n))
    }

    #[test]
    fn list_and_task_queries() {
        let q = parse("LIST").unwrap();
        assert_eq!(
            q.kind,
            Kind::List {
                value: None,
                without_id: false
            }
        );
        let q = parse("list file.mtime\nfrom #reading").unwrap();
        assert_eq!(
            q.kind,
            Kind::List {
                value: Some(field("file.mtime")),
                without_id: false
            }
        );
        assert_eq!(q.from, Some(Source::Tag("reading".into())));
        assert_eq!(parse("TASK WHERE !completed").unwrap().kind, Kind::Task);
    }

    #[test]
    fn table_columns_with_names() {
        let q = parse("TABLE author, rating * 2 AS \"Double\", file.day AS Day FROM \"Books\"")
            .unwrap();
        let Kind::Table {
            columns,
            without_id,
        } = q.kind
        else {
            panic!("a table")
        };
        assert!(!without_id);
        let headers: Vec<_> = columns.iter().map(|(_, h)| h.as_str()).collect();
        assert_eq!(headers, ["author", "Double", "Day"]);
        assert_eq!(
            columns[1].0,
            Expr::Binary(Op::Mul, Box::new(field("rating")), Box::new(num(2.0)))
        );
        assert_eq!(q.from, Some(Source::Folder("Books".into())));
        let q = parse("TABLE WITHOUT ID file.name AS Name").unwrap();
        assert!(matches!(
            q.kind,
            Kind::Table {
                without_id: true,
                ..
            }
        ));
    }

    #[test]
    fn sources_combine() {
        let q = parse("LIST FROM #a and (\"Journal\" or [[Dune|x]]) and -#b").unwrap();
        let tag = |t: &str| Box::new(Source::Tag(t.into()));
        assert_eq!(
            q.from,
            Some(Source::And(
                Box::new(Source::And(
                    tag("a"),
                    Box::new(Source::Or(
                        Box::new(Source::Folder("Journal".into())),
                        Box::new(Source::LinksTo("Dune".into()))
                    ))
                )),
                Box::new(Source::Not(tag("b")))
            ))
        );
    }

    #[test]
    fn where_sort_limit_with_precedence() {
        let q = parse("LIST WHERE a = 1 or b > 2 and !c SORT file.name DESC, x LIMIT 5").unwrap();
        let bin = |op, l, r| Expr::Binary(op, Box::new(l), Box::new(r));
        assert_eq!(
            q.commands[0],
            Command::Where(bin(
                Op::Or,
                bin(Op::Eq, field("a"), num(1.0)),
                bin(
                    Op::And,
                    bin(Op::Gt, field("b"), num(2.0)),
                    Expr::Not(Box::new(field("c")))
                )
            ))
        );
        assert_eq!(
            q.commands[1],
            Command::Sort(vec![(field("file.name"), true), (field("x"), false)])
        );
        assert_eq!(q.commands[2], Command::Limit(5));
    }

    #[test]
    fn functions_and_dates() {
        let q = parse("LIST WHERE contains(file.tags, #x) and file.day >= date(2026-08-01) and file.day < date(today)").unwrap();
        let text = format!("{:?}", q.commands);
        assert!(text.contains("Call(\"contains\""), "{text}");
        assert!(text.contains("Text(\"2026-08-01\")"), "{text}");
        assert!(text.contains("Text(\"today\")"), "{text}");
    }

    #[test]
    fn texts_with_escapes() {
        let q = parse(r#"LIST WHERE a = "say \"hi\"" and regextest("\d+ \\", b)"#).unwrap();
        let text = format!("{:?}", q.commands);
        assert!(text.contains(r#"Text("say \"hi\"")"#), "{text}");
        assert!(text.contains(r#"Text("\\d+ \\")"#), "{text}");
    }

    #[test]
    fn errors_say_what_is_wrong() {
        assert_eq!(
            parse("SELECT *").unwrap_err(),
            "a query starts with LIST, TABLE, TASK or CALENDAR"
        );
        assert!(parse("LIST FROM 3").unwrap_err().contains("FROM takes"));
        assert!(
            parse("LIST WHERE")
                .unwrap_err()
                .contains("expected a value")
        );
        assert!(parse("LIST LIMIT x").unwrap_err().contains("whole number"));
        assert!(parse("LIST GROUP x").unwrap_err().contains("GROUP BY"));
        assert!(
            parse("LIST WHERE \"open")
                .unwrap_err()
                .contains("isn't closed")
        );
        assert!(
            parse("LIST WHERE a = 1 b")
                .unwrap_err()
                .contains("unexpected \"b\"")
        );
    }
}
