//! Bases' expression language (BA-20 … BA-33): properties (`price`,
//! `note.price`, `file.name`, `formula.total`, `this.file`), literals,
//! `+ - * / %`, comparisons, `! && ||`, date arithmetic with durations
//! (`today() + "1w"`), global functions (`if`, `date`, `now`, `max` …) and
//! methods on each type (`name.lower()`, `file.hasTag("x")`,
//! `list.filter(value > 2)`).

use std::cmp::Ordering;

use chrono::{Datelike, Duration, Months, NaiveDateTime, Timelike};

use crate::plugins::dataview::index::Page;
use crate::plugins::dataview::value::{Value as DvValue, link_name, parse_date};

/// A value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    Date(NaiveDateTime),
    List(Vec<Value>),
    Object(Vec<(String, Value)>),
    /// A link to a note, by name or path as written.
    Link(String),
    /// A note (`file`), by its index in the pages.
    File(usize),
    /// A regular expression, as written (`/^Du/i`).
    Regex(String),
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::Text(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Object(o) => !o.is_empty(),
            Value::Date(_) | Value::Link(_) | Value::File(_) | Value::Regex(_) => true,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Value::Null => true,
            Value::Text(s) => s.is_empty(),
            Value::List(l) => l.is_empty(),
            Value::Object(o) => o.is_empty(),
            _ => false,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::Text(_) => "string",
            Value::Date(_) => "date",
            Value::List(_) => "list",
            Value::Object(_) => "object",
            Value::Link(_) => "link",
            Value::File(_) => "file",
            Value::Regex(_) => "regexp",
        }
    }

    /// From a Dataview index value.
    pub fn from_dv(v: &DvValue) -> Value {
        match v {
            DvValue::Null => Value::Null,
            DvValue::Bool(b) => Value::Bool(*b),
            DvValue::Number(n) => Value::Number(*n),
            DvValue::Text(t) => Value::Text(t.clone()),
            DvValue::Date(d) => Value::Date(*d),
            DvValue::Link(l) => Value::Link(l.clone()),
            DvValue::List(l) => Value::List(l.iter().map(Value::from_dv).collect()),
        }
    }

    /// The value as shown in a cell.
    pub fn display(&self, pages: &[Page]) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(true) => "☑".into(),
            Value::Bool(false) => "☐".into(),
            Value::Number(n) => number(*n),
            Value::Text(s) => s.clone(),
            Value::Date(d) if d.time() == chrono::NaiveTime::MIN => {
                d.format("%Y-%m-%d").to_string()
            }
            Value::Date(d) => d.format("%Y-%m-%d %H:%M").to_string(),
            Value::List(l) => l
                .iter()
                .map(|v| v.display(pages))
                .collect::<Vec<_>>()
                .join(", "),
            Value::Object(o) => o
                .iter()
                .map(|(k, v)| format!("{k}: {}", v.display(pages)))
                .collect::<Vec<_>>()
                .join(", "),
            Value::Link(l) => link_name(l),
            Value::File(i) => pages.get(*i).map(|p| p.name.clone()).unwrap_or_default(),
            Value::Regex(r) => r.clone(),
        }
    }

    /// Plain text (for string functions and comparisons).
    fn text(&self, pages: &[Page]) -> String {
        match self {
            Value::Bool(b) => b.to_string(),
            v => v.display(pages),
        }
    }

    pub fn compare(&self, other: &Value, pages: &[Page]) -> Ordering {
        match (self, other) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Greater,
            (_, Value::Null) => Ordering::Less,
            (Value::Number(a), Value::Number(b)) => a.total_cmp(b),
            (Value::Date(a), Value::Date(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (a, b) => a
                .text(pages)
                .to_lowercase()
                .cmp(&b.text(pages).to_lowercase()),
        }
    }

    fn equals(&self, other: &Value, pages: &[Page]) -> bool {
        match (self, other) {
            (Value::Link(_) | Value::File(_), Value::Link(_) | Value::File(_)) => self
                .display(pages)
                .eq_ignore_ascii_case(&other.display(pages)),
            (Value::Null, Value::Null) => true,
            (Value::Null, _) | (_, Value::Null) => false,
            (Value::Text(_), Value::Number(_)) | (Value::Number(_), Value::Text(_)) => false,
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.equals(y, pages))
            }
            _ => self.compare(other, pages) == Ordering::Equal,
        }
    }
}

/// A number as shown: integers without decimals.
pub fn number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// An expression, parsed.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    List(Vec<Expr>),
    /// A name: a property, `note`, `file`, `formula`, `this`, `value` ….
    Name(String),
    Member(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Method(Box<Expr>, String, Vec<Expr>),
    Unary(char, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
}

/// Parses an expression.
pub fn parse(text: &str) -> Result<Expr, String> {
    let tokens = tokenize(text)?;
    let mut p = Parser { tokens, at: 0 };
    let e = p.or()?;
    if p.at < p.tokens.len() {
        return Err(format!("unexpected {:?} in {text:?}", p.tokens[p.at]));
    }
    Ok(e)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Text(String),
    /// `/pattern/flags`.
    Regex(String),
    Name(String),
    Op(String),
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/'
            && !matches!(
                out.last(),
                Some(Token::Number(_) | Token::Text(_) | Token::Name(_) | Token::Regex(_))
            )
            && !matches!(out.last(), Some(Token::Op(o)) if o == ")" || o == "]")
        {
            // Where a value is expected, `/` starts a regular expression.
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '/' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            if i >= chars.len() {
                return Err(format!("an unclosed /regular expression/ in {text:?}"));
            }
            i += 1;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            out.push(Token::Regex(chars[start..i].iter().collect()));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            out.push(Token::Number(
                s.parse().map_err(|_| format!("bad number {s}"))?,
            ));
        } else if c == '"' || c == '\'' {
            let mut s = String::new();
            i += 1;
            while i < chars.len() && chars[i] != c {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                s.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() {
                return Err(format!("an unclosed {c} in {text:?}"));
            }
            i += 1;
            out.push(Token::Text(s));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Token::Name(chars[start..i].iter().collect()));
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            if ["==", "!=", "<=", ">=", "&&", "||"].contains(&two.as_str()) {
                out.push(Token::Op(two));
                i += 2;
            } else if "+-*/%<>!().,[]".contains(c) {
                out.push(Token::Op(c.to_string()));
                i += 1;
            } else {
                return Err(format!("unexpected {c:?} in {text:?}"));
            }
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek_op(&self, ops: &[&str]) -> Option<String> {
        match self.tokens.get(self.at) {
            Some(Token::Op(o)) if ops.contains(&o.as_str()) => Some(o.clone()),
            _ => None,
        }
    }

    fn expect(&mut self, op: &str) -> Result<(), String> {
        if self.peek_op(&[op]).is_some() {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("{op} expected"))
        }
    }

    fn binary(
        &mut self,
        ops: &[&str],
        next: fn(&mut Parser) -> Result<Expr, String>,
    ) -> Result<Expr, String> {
        let mut left = next(self)?;
        while let Some(op) = self.peek_op(ops) {
            self.at += 1;
            let right = next(self)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn or(&mut self) -> Result<Expr, String> {
        self.binary(&["||"], Parser::and)
    }

    fn and(&mut self) -> Result<Expr, String> {
        self.binary(&["&&"], Parser::equality)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        self.binary(&["==", "!="], Parser::comparison)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        self.binary(&["<", ">", "<=", ">="], Parser::sum)
    }

    fn sum(&mut self) -> Result<Expr, String> {
        self.binary(&["+", "-"], Parser::product)
    }

    fn product(&mut self) -> Result<Expr, String> {
        self.binary(&["*", "/", "%"], Parser::unary)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if let Some(op) = self.peek_op(&["!", "-"]) {
            self.at += 1;
            let c = op.chars().next().expect("an operator");
            return Ok(Expr::Unary(c, Box::new(self.unary()?)));
        }
        self.postfix()
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if self.peek_op(&[")"]).is_some() {
            self.at += 1;
            return Ok(args);
        }
        loop {
            args.push(self.or()?);
            if self.peek_op(&[","]).is_some() {
                self.at += 1;
            } else {
                self.expect(")")?;
                return Ok(args);
            }
        }
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            if self.peek_op(&["."]).is_some() {
                self.at += 1;
                let Some(Token::Name(name)) = self.tokens.get(self.at).cloned() else {
                    return Err("a name after .".into());
                };
                self.at += 1;
                if self.peek_op(&["("]).is_some() {
                    self.at += 1;
                    e = Expr::Method(Box::new(e), name, self.args()?);
                } else {
                    e = Expr::Member(Box::new(e), name);
                }
            } else if self.peek_op(&["["]).is_some() {
                self.at += 1;
                let index = self.or()?;
                self.expect("]")?;
                e = Expr::Index(Box::new(e), Box::new(index));
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let token = self
            .tokens
            .get(self.at)
            .cloned()
            .ok_or("an expression expected")?;
        self.at += 1;
        match token {
            Token::Number(n) => Ok(Expr::Literal(Value::Number(n))),
            Token::Text(s) => Ok(Expr::Literal(Value::Text(s))),
            Token::Regex(r) => Ok(Expr::Literal(Value::Regex(r))),
            Token::Name(n) if n == "true" => Ok(Expr::Literal(Value::Bool(true))),
            Token::Name(n) if n == "false" => Ok(Expr::Literal(Value::Bool(false))),
            Token::Name(n) if n == "null" => Ok(Expr::Literal(Value::Null)),
            Token::Name(n) => {
                if self.peek_op(&["("]).is_some() {
                    self.at += 1;
                    Ok(Expr::Call(n, self.args()?))
                } else {
                    Ok(Expr::Name(n))
                }
            }
            Token::Op(o) if o == "(" => {
                let e = self.or()?;
                self.expect(")")?;
                Ok(e)
            }
            Token::Op(o) if o == "[" => {
                let mut items = Vec::new();
                if self.peek_op(&["]"]).is_some() {
                    self.at += 1;
                    return Ok(Expr::List(items));
                }
                loop {
                    items.push(self.or()?);
                    if self.peek_op(&[","]).is_some() {
                        self.at += 1;
                    } else {
                        self.expect("]")?;
                        return Ok(Expr::List(items));
                    }
                }
            }
            Token::Op(o) => Err(format!("unexpected {o}")),
        }
    }
}

/// What an expression runs against.
pub struct Scope<'a> {
    pub pages: &'a [Page],
    /// The row's note.
    pub row: Option<usize>,
    /// The note the base is in (`this`).
    pub this: Option<usize>,
    /// The base's formulas, by name, parsed.
    pub formulas: &'a [(String, Expr)],
    /// `value`, `index`, `acc`, `values` while running a lambda or a
    /// summary.
    pub locals: Vec<(String, Value)>,
    pub now: NaiveDateTime,
    /// Formulas being computed (to find cycles).
    pub depth: usize,
}

impl Scope<'_> {
    fn page(&self) -> Option<&Page> {
        self.row.and_then(|i| self.pages.get(i))
    }

    /// A note property of the row.
    fn property(&self, name: &str) -> Value {
        self.page()
            .and_then(|p| {
                p.fields
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(name))
                    .map(|(_, v)| Value::from_dv(v))
            })
            .unwrap_or(Value::Null)
    }

    fn formula(&mut self, name: &str) -> Result<Value, String> {
        let Some((_, e)) = self.formulas.iter().find(|(n, _)| n == name) else {
            return Err(format!("no formula {name}"));
        };
        if self.depth > 32 {
            return Err(format!("the formula {name} uses itself"));
        }
        self.depth += 1;
        let v = eval(e, self);
        self.depth -= 1;
        v
    }
}

/// The page a file-like value is, if it's one.
fn as_page(v: &Value, scope: &Scope) -> Option<usize> {
    match v {
        Value::File(i) => Some(*i),
        Value::Link(l) => {
            let name = link_name(l).to_lowercase();
            scope.pages.iter().position(|p| {
                p.name.to_lowercase() == name
                    || p.rel.trim_end_matches(".md").eq_ignore_ascii_case(l.trim())
            })
        }
        _ => None,
    }
}

/// A file property: `file.name` and the others.
fn file_field(i: usize, name: &str, scope: &Scope) -> Result<Value, String> {
    let p = &scope.pages[i];
    Ok(match name {
        "name" => Value::Text(p.name.clone()),
        "basename" => Value::Text(p.name.clone()),
        "path" => Value::Text(p.rel.clone()),
        "folder" => Value::Text(p.folder.clone()),
        "ext" => Value::Text("md".into()),
        "size" => Value::Number(p.size as f64),
        "ctime" => p.ctime.map_or(Value::Null, Value::Date),
        "mtime" => p.mtime.map_or(Value::Null, Value::Date),
        "tags" => Value::List(p.tags.iter().cloned().map(Value::Text).collect()),
        "links" => Value::List(p.outlinks.iter().cloned().map(Value::Link).collect()),
        "backlinks" => {
            let name = p.name.to_lowercase();
            Value::List(
                scope
                    .pages
                    .iter()
                    .enumerate()
                    .filter(|(_, q)| q.outlinks.contains(&name))
                    .map(|(j, _)| Value::File(j))
                    .collect(),
            )
        }
        "properties" => Value::Object(
            p.fields
                .iter()
                .map(|(k, v)| (k.clone(), Value::from_dv(v)))
                .collect(),
        ),
        "file" => Value::File(i),
        other => return Err(format!("no file property {other}")),
    })
}

/// Runs `e`.
pub fn eval(e: &Expr, scope: &mut Scope) -> Result<Value, String> {
    match e {
        Expr::Literal(v) => Ok(v.clone()),
        Expr::List(items) => Ok(Value::List(
            items
                .iter()
                .map(|i| eval(i, scope))
                .collect::<Result<_, _>>()?,
        )),
        Expr::Name(n) => {
            if let Some((_, v)) = scope.locals.iter().rev().find(|(k, _)| k == n) {
                return Ok(v.clone());
            }
            Ok(match n.as_str() {
                "file" => scope.row.map_or(Value::Null, Value::File),
                "note" | "formula" | "this" => {
                    Value::Object(vec![("__".into(), Value::Text(n.clone()))])
                }
                _ => scope.property(n),
            })
        }
        Expr::Member(target, name) => {
            // note.x, formula.x, this.file, file.x.
            if let Expr::Name(root) = target.as_ref()
                && !scope.locals.iter().any(|(k, _)| k == root)
            {
                match root.as_str() {
                    "note" => return Ok(scope.property(name)),
                    "formula" => return scope.formula(name),
                    "this" => {
                        let Some(this) = scope.this else {
                            return Ok(Value::Null);
                        };
                        if name == "file" {
                            return Ok(Value::File(this));
                        }
                        let saved = scope.row;
                        scope.row = Some(this);
                        let v = scope.property(name);
                        scope.row = saved;
                        return Ok(v);
                    }
                    _ => {}
                }
            }
            let v = eval(target, scope)?;
            member(&v, name, scope)
        }
        Expr::Index(target, index) => {
            let v = eval(target, scope)?;
            let i = eval(index, scope)?;
            Ok(match (&v, &i) {
                (Value::List(l), Value::Number(n)) => {
                    let n = *n as i64;
                    let n = if n < 0 { l.len() as i64 + n } else { n };
                    l.get(n as usize).cloned().unwrap_or(Value::Null)
                }
                (_, Value::Text(key)) => member(&v, key, scope)?,
                _ => Value::Null,
            })
        }
        Expr::Unary('!', e) => Ok(Value::Bool(!eval(e, scope)?.truthy())),
        Expr::Unary(_, e) => match eval(e, scope)? {
            Value::Number(n) => Ok(Value::Number(-n)),
            v => Err(format!("can't negate a {}", v.type_name())),
        },
        Expr::Binary(op, a, b) => {
            if op == "&&" {
                return Ok(Value::Bool(
                    eval(a, scope)?.truthy() && eval(b, scope)?.truthy(),
                ));
            }
            if op == "||" {
                let left = eval(a, scope)?;
                return Ok(if left.truthy() { left } else { eval(b, scope)? });
            }
            let (a, b) = (eval(a, scope)?, eval(b, scope)?);
            binary(op, a, b, scope)
        }
        Expr::Call(name, args) => call(name, args, scope),
        Expr::Method(target, name, args) => {
            let v = eval(target, scope)?;
            method(v, name, args, scope)
        }
    }
}

fn member(v: &Value, name: &str, scope: &Scope) -> Result<Value, String> {
    if let Some(i) = as_page(v, scope).filter(|_| matches!(v, Value::File(_))) {
        return file_field(i, name, scope);
    }
    Ok(match (v, name) {
        (Value::Text(s), "length") => Value::Number(s.chars().count() as f64),
        (Value::List(l), "length") => Value::Number(l.len() as f64),
        (Value::Object(o), key) => o
            .iter()
            .find(|(k, _)| k == key)
            .map_or(Value::Null, |(_, v)| v.clone()),
        (Value::Date(d), field) => Value::Number(match field {
            "year" => d.year() as f64,
            "month" => d.month() as f64,
            "day" => d.day() as f64,
            "hour" => d.hour() as f64,
            "minute" => d.minute() as f64,
            "second" => d.second() as f64,
            "millisecond" => 0.0,
            other => return Err(format!("dates have no {other}")),
        }),
        (Value::Null, _) => Value::Null,
        (v, name) => return Err(format!("a {} has no {name}", v.type_name())),
    })
}

/// A duration's milliseconds and months: `1M`, `2 weeks`, `3d`, `1h`.
fn duration(text: &str) -> Option<(i64, u32)> {
    let text = text.trim();
    let digits: String = text
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    let n: i64 = if digits.is_empty() {
        1
    } else {
        digits.parse().ok()?
    };
    let unit = text[digits.len()..].trim();
    let ms = |x: i64| Some((x * n, 0));
    match unit {
        "y" | "year" | "years" => Some((0, (12 * n).try_into().ok()?)),
        "M" | "month" | "months" => Some((0, n.try_into().ok()?)),
        "w" | "week" | "weeks" => ms(7 * 86_400_000),
        "d" | "day" | "days" => ms(86_400_000),
        "h" | "hour" | "hours" => ms(3_600_000),
        "m" | "minute" | "minutes" => ms(60_000),
        "s" | "second" | "seconds" => ms(1000),
        _ => None,
    }
}

fn shift(d: NaiveDateTime, (ms, months): (i64, u32), forward: bool) -> Option<NaiveDateTime> {
    let d = if forward {
        d.checked_add_months(Months::new(months))?
    } else {
        d.checked_sub_months(Months::new(months))?
    };
    let delta = Duration::milliseconds(ms);
    if forward {
        d.checked_add_signed(delta)
    } else {
        d.checked_sub_signed(delta)
    }
}

fn binary(op: &str, a: Value, b: Value, scope: &Scope) -> Result<Value, String> {
    let pages = scope.pages;
    match op {
        "==" => return Ok(Value::Bool(a.equals(&b, pages))),
        "!=" => return Ok(Value::Bool(!a.equals(&b, pages))),
        "<" | ">" | "<=" | ">=" => {
            if matches!(a, Value::Null) || matches!(b, Value::Null) {
                return Ok(Value::Bool(false));
            }
            let o = a.compare(&b, pages);
            return Ok(Value::Bool(match op {
                "<" => o.is_lt(),
                ">" => o.is_gt(),
                "<=" => o.is_le(),
                _ => o.is_ge(),
            }));
        }
        _ => {}
    }
    match (op, &a, &b) {
        ("+" | "-", Value::Date(d), Value::Text(t)) => {
            let dur = duration(t).ok_or_else(|| format!("can't read the duration {t:?}"))?;
            shift(*d, dur, op == "+")
                .map(Value::Date)
                .ok_or_else(|| "a date out of range".into())
        }
        ("-", Value::Date(x), Value::Date(y)) => {
            Ok(Value::Number((*x - *y).num_milliseconds() as f64))
        }
        ("+", Value::Text(_), _) | ("+", _, Value::Text(_)) => {
            Ok(Value::Text(format!("{}{}", a.text(pages), b.text(pages))))
        }
        (_, Value::Number(x), Value::Number(y)) => Ok(Value::Number(match op {
            "+" => x + y,
            "-" => x - y,
            "*" => x * y,
            "/" => x / y,
            _ => x % y,
        })),
        (_, Value::Null, _) | (_, _, Value::Null) => Ok(Value::Null),
        _ => Err(format!("can't do {} {op} {}", a.type_name(), b.type_name())),
    }
}

fn arg(args: &[Expr], i: usize, scope: &mut Scope) -> Result<Value, String> {
    match args.get(i) {
        Some(e) => eval(e, scope),
        None => Ok(Value::Null),
    }
}

fn to_date(v: &Value) -> Option<NaiveDateTime> {
    match v {
        Value::Date(d) => Some(*d),
        Value::Text(t) => parse_date(t),
        _ => None,
    }
}

fn call(name: &str, args: &[Expr], scope: &mut Scope) -> Result<Value, String> {
    Ok(match name {
        "if" => {
            if arg(args, 0, scope)?.truthy() {
                arg(args, 1, scope)?
            } else {
                arg(args, 2, scope)?
            }
        }
        "now" => Value::Date(scope.now),
        "today" => Value::Date(scope.now.date().and_time(chrono::NaiveTime::MIN)),
        "date" => to_date(&arg(args, 0, scope)?).map_or(Value::Null, Value::Date),
        "duration" => match arg(args, 0, scope)? {
            Value::Text(t) => {
                let (ms, months) = duration(&t).ok_or_else(|| format!("can't read {t:?}"))?;
                Value::Number(ms as f64 + months as f64 * 30.0 * 86_400_000.0)
            }
            v => v,
        },
        "number" => match arg(args, 0, scope)? {
            Value::Number(n) => Value::Number(n),
            Value::Bool(b) => Value::Number(if b { 1.0 } else { 0.0 }),
            Value::Text(t) => t.trim().parse().map_or(Value::Null, Value::Number),
            Value::Date(d) => Value::Number(d.and_utc().timestamp_millis() as f64),
            _ => Value::Null,
        },
        "list" => match arg(args, 0, scope)? {
            Value::List(l) => Value::List(l),
            Value::Null => Value::List(Vec::new()),
            v => Value::List(vec![v]),
        },
        "link" => Value::Link(arg(args, 0, scope)?.text(scope.pages)),
        "file" => {
            let v = arg(args, 0, scope)?;
            let target = Value::Link(v.text(scope.pages).trim_end_matches(".md").to_string());
            as_page(&target, scope).map_or(Value::Null, Value::File)
        }
        "max" | "min" => {
            let mut values: Vec<Value> = Vec::new();
            for a in args {
                match eval(a, scope)? {
                    Value::List(l) => values.extend(l),
                    v => values.push(v),
                }
            }
            let values = values.into_iter().filter(|v| !matches!(v, Value::Null));
            let best = if name == "max" {
                values.max_by(|a, b| a.compare(b, scope.pages))
            } else {
                values.min_by(|a, b| a.compare(b, scope.pages))
            };
            best.unwrap_or(Value::Null)
        }
        "random" => {
            // Not for secrets: a number in [0, 1) from the clock.
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos());
            let mut x = u64::from(nanos) ^ 0x9e37_79b9_7f4a_7c15;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            Value::Number((x % 1_000_000) as f64 / 1_000_000.0)
        }
        "escapeHTML" | "html" | "icon" | "image" => {
            Value::Text(arg(args, 0, scope)?.text(scope.pages))
        }
        other => return Err(format!("no function {other}")),
    })
}

fn method(v: Value, name: &str, args: &[Expr], scope: &mut Scope) -> Result<Value, String> {
    let pages = scope.pages;
    // For every type.
    match name {
        "isTruthy" => return Ok(Value::Bool(v.truthy())),
        "isEmpty" => return Ok(Value::Bool(v.is_empty())),
        "toString" => return Ok(Value::Text(v.text(pages))),
        "isType" => {
            let t = arg(args, 0, scope)?.text(pages);
            return Ok(Value::Bool(v.type_name() == t.to_lowercase()));
        }
        _ => {}
    }
    match &v {
        Value::File(i) => return file_method(*i, name, args, scope),
        Value::Link(_) => {
            let page = as_page(&v, scope);
            return match name {
                "asFile" => Ok(page.map_or(Value::Null, Value::File)),
                "linksTo" => {
                    let target = arg(args, 0, scope)?;
                    let name = target.display(scope.pages).to_lowercase();
                    Ok(Value::Bool(
                        page.is_some_and(|p| scope.pages[p].outlinks.contains(&name)),
                    ))
                }
                other => Err(format!("links have no {other}()")),
            };
        }
        _ => {}
    }
    Ok(match (&v, name) {
        (Value::Text(s), _) => string_method(s, name, args, scope)?,
        (Value::Number(n), "abs") => Value::Number(n.abs()),
        (Value::Number(n), "ceil") => Value::Number(n.ceil()),
        (Value::Number(n), "floor") => Value::Number(n.floor()),
        (Value::Number(n), "round") => {
            let digits = match arg(args, 0, scope)? {
                Value::Number(d) => d as i32,
                _ => 0,
            };
            let f = 10f64.powi(digits);
            Value::Number((n * f).round() / f)
        }
        (Value::Number(n), "toFixed") => {
            let digits = match arg(args, 0, scope)? {
                Value::Number(d) => d as usize,
                _ => 0,
            };
            Value::Text(format!("{n:.digits$}"))
        }
        (Value::Date(d), "date") => Value::Date(d.date().and_time(chrono::NaiveTime::MIN)),
        (Value::Date(d), "time") => Value::Text(d.format("%H:%M:%S").to_string()),
        (Value::Date(d), "format") => {
            let fmt = arg(args, 0, scope)?.text(pages);
            Value::Text(crate::plugins::moment::format(d, &fmt))
        }
        (Value::Date(d), "relative") => {
            let days = (*d - scope.now).num_days();
            Value::Text(match days {
                0 => "today".into(),
                1 => "tomorrow".into(),
                -1 => "yesterday".into(),
                n if n > 0 => format!("in {n} days"),
                n => format!("{} days ago", -n),
            })
        }
        (Value::List(l), _) => list_method(l, name, args, scope)?,
        (Value::Object(o), "keys") => {
            Value::List(o.iter().map(|(k, _)| Value::Text(k.clone())).collect())
        }
        (Value::Object(o), "values") => Value::List(o.iter().map(|(_, v)| v.clone()).collect()),
        (Value::Null, _) => Value::Null,
        (v, name) => return Err(format!("a {} has no {name}()", v.type_name())),
    })
}

fn texts(args: &[Expr], scope: &mut Scope) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for i in 0..args.len() {
        out.push(arg(args, i, scope)?.text(scope.pages));
    }
    Ok(out)
}

/// `/pattern/flags` compiled (cached: expressions run for every row).
fn compiled(source: &str) -> Result<regex::Regex, String> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<String, regex::Regex>> =
            std::cell::RefCell::default();
    }
    if let Some(re) = CACHE.with(|c| c.borrow().get(source).cloned()) {
        return Ok(re);
    }
    let re = crate::plugins::slash_regex(source)?;
    CACHE.with(|c| c.borrow_mut().insert(source.to_string(), re.clone()));
    Ok(re)
}

fn string_method(s: &str, name: &str, args: &[Expr], scope: &mut Scope) -> Result<Value, String> {
    // With a regular expression: `matches`, `replace` (all with `g`).
    if matches!(name, "matches" | "replace")
        && let Value::Regex(source) = arg(args, 0, scope)?
    {
        let re = compiled(&source)?;
        if name == "matches" {
            return Ok(Value::Bool(re.is_match(s)));
        }
        let with = arg(args, 1, scope)?.text(scope.pages);
        let all = source
            .rsplit('/')
            .next()
            .is_some_and(|flags| flags.contains('g'));
        let replaced = if all {
            re.replace_all(s, with.as_str())
        } else {
            re.replace(s, with.as_str())
        };
        return Ok(Value::Text(replaced.into_owned()));
    }
    let a = texts(args, scope)?;
    let first = a.first().cloned().unwrap_or_default();
    let int = |i: usize| a.get(i).and_then(|x| x.parse::<i64>().ok());
    Ok(match name {
        "contains" => Value::Bool(s.contains(&first)),
        "containsAll" => Value::Bool(a.iter().all(|x| s.contains(x.as_str()))),
        "containsAny" => Value::Bool(a.iter().any(|x| s.contains(x.as_str()))),
        "startsWith" => Value::Bool(s.starts_with(&first)),
        "endsWith" => Value::Bool(s.ends_with(&first)),
        "lower" => Value::Text(s.to_lowercase()),
        "upper" => Value::Text(s.to_uppercase()),
        "title" => Value::Text(
            s.split(' ')
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| {
                            f.to_uppercase()
                                .chain(c.flat_map(char::to_lowercase))
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect::<Vec<String>>()
                .join(" "),
        ),
        "trim" => Value::Text(s.trim().to_string()),
        "replace" => Value::Text(s.replace(&first, a.get(1).map_or("", String::as_str))),
        "repeat" => Value::Text(s.repeat(int(0).unwrap_or(0).max(0) as usize)),
        "reverse" => Value::Text(s.chars().rev().collect()),
        "slice" => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as i64;
            let at = |n: i64| if n < 0 { (len + n).max(0) } else { n.min(len) } as usize;
            let start = at(int(0).unwrap_or(0));
            let end = at(int(1).unwrap_or(len));
            Value::Text(chars[start..end.max(start)].iter().collect())
        }
        "split" => Value::List(
            s.split(first.as_str())
                .map(|p| Value::Text(p.to_string()))
                .collect(),
        ),
        other => return Err(format!("strings have no {other}()")),
    })
}

/// Runs `body` for each item with `value` and `index` set.
fn each(
    l: &[Value],
    body: &Expr,
    scope: &mut Scope,
    mut f: impl FnMut(usize, &Value, Value) -> Result<(), String>,
) -> Result<(), String> {
    for (i, item) in l.iter().enumerate() {
        scope.locals.push(("value".into(), item.clone()));
        scope.locals.push(("index".into(), Value::Number(i as f64)));
        let r = eval(body, scope);
        scope.locals.truncate(scope.locals.len() - 2);
        f(i, item, r?)?;
    }
    Ok(())
}

fn list_method(l: &[Value], name: &str, args: &[Expr], scope: &mut Scope) -> Result<Value, String> {
    let pages = scope.pages;
    Ok(match name {
        "contains" => {
            let x = arg(args, 0, scope)?;
            Value::Bool(l.iter().any(|v| v.equals(&x, pages)))
        }
        "containsAll" | "containsAny" => {
            let mut xs = Vec::new();
            for i in 0..args.len() {
                xs.push(arg(args, i, scope)?);
            }
            let has = |x: &Value| l.iter().any(|v| v.equals(x, pages));
            Value::Bool(if name == "containsAll" {
                xs.iter().all(has)
            } else {
                xs.iter().any(has)
            })
        }
        "filter" => {
            let body = args.first().ok_or("filter(condition)")?;
            let mut out = Vec::new();
            each(l, body, scope, |_, item, r| {
                if r.truthy() {
                    out.push(item.clone());
                }
                Ok(())
            })?;
            Value::List(out)
        }
        "map" => {
            let body = args.first().ok_or("map(expression)")?;
            let mut out = Vec::new();
            each(l, body, scope, |_, _, r| {
                out.push(r);
                Ok(())
            })?;
            Value::List(out)
        }
        "reduce" => {
            let body = args.first().ok_or("reduce(expression, initial)")?;
            let mut acc = arg(args, 1, scope)?;
            for (i, item) in l.iter().enumerate() {
                scope.locals.push(("value".into(), item.clone()));
                scope.locals.push(("index".into(), Value::Number(i as f64)));
                scope.locals.push(("acc".into(), acc.clone()));
                let r = eval(body, scope);
                scope.locals.truncate(scope.locals.len() - 3);
                acc = r?;
            }
            acc
        }
        "flat" => Value::List(
            l.iter()
                .flat_map(|v| match v {
                    Value::List(inner) => inner.clone(),
                    v => vec![v.clone()],
                })
                .collect(),
        ),
        "join" => {
            let sep = match arg(args, 0, scope)? {
                Value::Null => ", ".to_string(),
                v => v.text(pages),
            };
            Value::Text(
                l.iter()
                    .map(|v| v.text(pages))
                    .collect::<Vec<_>>()
                    .join(&sep),
            )
        }
        "reverse" => Value::List(l.iter().rev().cloned().collect()),
        "slice" => {
            let len = l.len() as i64;
            let n = |v: Value, default: i64| match v {
                Value::Number(n) => n as i64,
                _ => default,
            };
            let at = |x: i64| if x < 0 { (len + x).max(0) } else { x.min(len) } as usize;
            let start = at(n(arg(args, 0, scope)?, 0));
            let end = at(n(arg(args, 1, scope)?, len));
            Value::List(l[start..end.max(start)].to_vec())
        }
        "sort" => {
            let mut out = l.to_vec();
            out.sort_by(|a, b| a.compare(b, pages));
            Value::List(out)
        }
        "unique" => {
            let mut out: Vec<Value> = Vec::new();
            for v in l {
                if !out.iter().any(|o| o.equals(v, pages)) {
                    out.push(v.clone());
                }
            }
            Value::List(out)
        }
        // Summaries' helpers (values.mean() …).
        "mean" | "average" | "sum" | "median" | "stddev" => {
            let nums: Vec<f64> = l
                .iter()
                .filter_map(|v| match v {
                    Value::Number(n) => Some(*n),
                    _ => None,
                })
                .collect();
            Value::Number(stat(name, &nums).unwrap_or(0.0))
        }
        other => return Err(format!("lists have no {other}()")),
    })
}

/// A statistic over numbers: mean, sum, median or stddev.
pub fn stat(name: &str, nums: &[f64]) -> Option<f64> {
    if nums.is_empty() {
        return None;
    }
    let sum: f64 = nums.iter().sum();
    let mean = sum / nums.len() as f64;
    Some(match name {
        "sum" => sum,
        "median" => {
            let mut s = nums.to_vec();
            s.sort_by(f64::total_cmp);
            let m = s.len() / 2;
            if s.len().is_multiple_of(2) {
                (s[m - 1] + s[m]) / 2.0
            } else {
                s[m]
            }
        }
        "stddev" => {
            (nums.iter().map(|n| (n - mean).powi(2)).sum::<f64>() / nums.len() as f64).sqrt()
        }
        _ => mean,
    })
}

fn file_method(i: usize, name: &str, args: &[Expr], scope: &mut Scope) -> Result<Value, String> {
    let a = texts(args, scope)?;
    let p = &scope.pages[i];
    Ok(match name {
        "asLink" => Value::Link(p.name.clone()),
        "hasTag" => Value::Bool(a.iter().any(|t| {
            let t = t.trim_start_matches('#').to_lowercase();
            p.tags.iter().any(|tag| {
                let tag = tag.trim_start_matches('#').to_lowercase();
                tag == t || tag.starts_with(&format!("{t}/"))
            })
        })),
        "inFolder" => {
            let f = a.first().cloned().unwrap_or_default();
            let f = f.trim_matches('/').to_lowercase();
            let folder = p.folder.to_lowercase();
            Value::Bool(folder == f || folder.starts_with(&format!("{f}/")) || f.is_empty())
        }
        "hasProperty" => {
            let key = a.first().cloned().unwrap_or_default();
            Value::Bool(p.fields.iter().any(|(k, _)| k.eq_ignore_ascii_case(&key)))
        }
        "hasLink" => {
            let target = match args.first() {
                Some(e) => eval(e, scope)?,
                None => Value::Null,
            };
            let name = target.display(scope.pages).to_lowercase();
            let p = &scope.pages[i];
            Value::Bool(p.outlinks.contains(&name))
        }
        other => return Err(format!("files have no {other}()")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Note;
    use std::path::PathBuf;

    fn page(rel: &str, lines: &[&str]) -> Page {
        let note = Note {
            path: PathBuf::from("/nowhere").join(rel),
            rel: PathBuf::from(rel),
            lines: lines.iter().map(|l| l.to_string()).collect(),
            tags: Vec::new(),
            properties: Vec::new(),
            aliases: Vec::new(),
            modified: None,
        };
        Page::from_note(&note)
    }

    fn run(text: &str) -> Result<Value, String> {
        let pages = vec![
            page(
                "Books/Dune.md",
                &[
                    "---",
                    "rating: 5",
                    "pages: 412",
                    "genre: [scifi, classic]",
                    "---",
                    "#book [[Emma]]",
                ],
            ),
            page("Emma.md", &["---", "rating: 3", "---"]),
        ];
        let formulas = vec![("half".to_string(), parse("pages / 2").unwrap())];
        let mut scope = Scope {
            pages: &pages,
            row: Some(0),
            this: Some(1),
            formulas: &formulas,
            locals: Vec::new(),
            now: parse_date("2026-09-30").unwrap(),
            depth: 0,
        };
        let e = parse(text)?;
        eval(&e, &mut scope).map(|v| match v {
            Value::File(i) => Value::Text(pages[i].name.clone()),
            v => v,
        })
    }

    fn num(n: f64) -> Result<Value, String> {
        Ok(Value::Number(n))
    }

    fn text(s: &str) -> Result<Value, String> {
        Ok(Value::Text(s.into()))
    }

    #[test]
    fn properties_operators_and_formulas() {
        assert_eq!(run("rating * 2 + 1"), num(11.0));
        assert_eq!(run("note.pages % 100"), num(12.0));
        assert_eq!(run("formula.half"), num(206.0));
        assert_eq!(run("rating > 4 && !(pages < 100)"), Ok(Value::Bool(true)));
        assert_eq!(run("missing || \"none\""), text("none"));
        assert_eq!(run("file.name"), text("Dune"));
        assert_eq!(run("file.folder"), text("Books"));
        assert_eq!(run("this.rating"), num(3.0));
        assert_eq!(run("this.file"), text("Emma"));
        assert_eq!(run("genre[1]"), text("classic"));
        assert!(run("rating +").is_err());
        assert!(run("nope()").unwrap_err().contains("no function"));
    }

    #[test]
    fn functions_and_methods() {
        assert_eq!(run("if(rating >= 5, \"top\", \"ok\")"), text("top"));
        assert_eq!(run("(pages / 3).toFixed(1)"), text("137.3"));
        assert_eq!(run("(pages / 3).round(2)"), num(137.33));
        assert_eq!(run("\"hello world\".title()"), text("Hello World"));
        assert_eq!(run("\"a,b,c\".split(\",\").length"), num(3.0));
        assert_eq!(run("genre.contains(\"scifi\")"), Ok(Value::Bool(true)));
        assert_eq!(
            run("[1, 2, 3, 4].filter(value > 2).map(value * 10)"),
            Ok(Value::List(vec![Value::Number(30.0), Value::Number(40.0)]))
        );
        assert_eq!(run("[1, 2, 3].reduce(acc + value, 0)"), num(6.0));
        assert_eq!(run("[3, 1, 3].unique().sort().join(\"-\")"), text("1-3"));
        assert_eq!(run("max(1, [7, 2], 5)"), num(7.0));
        assert_eq!(run("file.hasTag(\"book\")"), Ok(Value::Bool(true)));
        assert_eq!(run("file.inFolder(\"Books\")"), Ok(Value::Bool(true)));
        assert_eq!(run("file.hasLink(this.file)"), Ok(Value::Bool(true)));
        assert_eq!(run("file.hasProperty(\"rating\")"), Ok(Value::Bool(true)));
    }

    #[test]
    fn regular_expressions() {
        assert_eq!(
            run("\"Hello World\".matches(/wor/i)"),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            run("\"Hello World\".matches(/wor/)"),
            Ok(Value::Bool(false))
        );
        assert_eq!(run("file.name.matches(/^Du/)"), Ok(Value::Bool(true)));
        assert_eq!(run("\"a-b-c\".replace(/-/g, \"+\")"), text("a+b+c"));
        assert_eq!(run("\"a-b-c\".replace(/-/, \"+\")"), text("a+b-c"));
        assert_eq!(run("pages / 4 / 103"), num(1.0), "still division");
        assert_eq!(
            run("[\"ab\", \"cd\"].filter(value.matches(/b$/))"),
            Ok(Value::List(vec![Value::Text("ab".into())]))
        );
        assert!(
            run("\"x\".matches(/(/)")
                .unwrap_err()
                .contains("regular expression")
        );
    }

    #[test]
    fn dates_and_durations() {
        assert_eq!(
            run("(today() + \"1M\").format(\"YYYY-MM-DD\")"),
            text("2026-10-30")
        );
        assert_eq!(run("(date(\"2026-10-07\") - today()) / 86400000"), num(7.0));
        assert_eq!(run("date(\"2026-10-07\").day"), num(7.0));
        assert_eq!(
            run("(now() - \"2 weeks\").date() < today()"),
            Ok(Value::Bool(true))
        );
        assert!(run("today() + \"soon\"").unwrap_err().contains("duration"));
    }
}
