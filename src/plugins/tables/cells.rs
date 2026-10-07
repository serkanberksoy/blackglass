//! Spreadsheet cells (AT-44): a cell starting with `=` is a formula,
//! shown as its result while the cursor is out of the table (mdedit's
//! table cells hook), as written while it's in. References as in the
//! Table Calc plugin: columns A, B, C …, rows from 1 (the header), so
//! data starts at row 2: `=B2*C2`, `=SUM(D2:D4)`. Functions: SUM,
//! AVERAGE, MEDIAN, MIN, MAX, COUNT, COUNTA, PRODUCT, ABS, ROUND, FLOOR,
//! CEILING, TRUNC, INT, SQRT, POWER, MOD, IF; operators `+ - * / ^`,
//! comparisons `= <> < > <= >=`. Errors: `#ERR` (a bad formula, a text
//! cell in arithmetic, division by zero, a loop), `#NAME?` (no such
//! function).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use super::table::cells;

/// A cell's value.
#[derive(Debug, Clone, PartialEq)]
enum V {
    Num(f64),
    Text(String),
    Bool(bool),
    /// `#ERR` or `#NAME?`.
    Err(&'static str),
}

const ERR: V = V::Err("#ERR");

impl V {
    /// As a number: a blank is 0; text can't be.
    fn num(&self) -> Result<f64, V> {
        match self {
            V::Num(n) => Ok(*n),
            V::Bool(b) => Ok(f64::from(u8::from(*b))),
            V::Text(t) if t.trim().is_empty() => Ok(0.0),
            V::Text(_) => Err(ERR),
            V::Err(_) => Err(self.clone()),
        }
    }

    fn truthy(&self) -> Result<bool, V> {
        match self {
            V::Bool(b) => Ok(*b),
            V::Err(_) => Err(self.clone()),
            other => Ok(other.num().unwrap_or(1.0) != 0.0),
        }
    }

    fn shown(&self) -> String {
        match self {
            V::Num(n) => number(*n),
            V::Text(t) => t.clone(),
            V::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
            V::Err(e) => (*e).into(),
        }
    }
}

/// A number as shown: whole, or with up to 6 decimals.
fn number(n: f64) -> String {
    if (n - n.round()).abs() < 1e-9 {
        return format!("{}", n.round() as i64);
    }
    let s = format!("{n:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A formula, parsed.
#[derive(Debug, Clone, PartialEq)]
enum E {
    Num(f64),
    Str(String),
    /// A cell: (row, column), from 0.
    Ref(usize, usize),
    /// A range of cells: its corners.
    Range(usize, usize, usize, usize),
    Neg(Box<E>),
    Bin(String, Box<E>, Box<E>),
    Call(String, Vec<E>),
    /// Something that isn't a formula.
    Bad,
}

/// `A1` as (row, column) from 0.
fn cell_ref(word: &str) -> Option<(usize, usize)> {
    let letters = word.len()
        - word
            .trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .len();
    let (col, row) = word.split_at(letters);
    if letters == 0 || row.is_empty() || !row.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let col = col
        .to_ascii_uppercase()
        .bytes()
        .fold(0usize, |n, b| n * 26 + usize::from(b - b'A' + 1));
    let row: usize = row.parse().ok()?;
    (row >= 1).then(|| (row - 1, col - 1))
}

struct Parser {
    tokens: Vec<String>,
    at: usize,
}

fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if i < chars.len() && matches!(chars[i], 'e' | 'E') {
                i += 1;
                if i < chars.len() && matches!(chars[i], '+' | '-') {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            out.push(chars[start..i].iter().collect());
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(chars[start..i].iter().collect());
        } else if c == '"' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            i = (i + 1).min(chars.len());
            out.push(chars[start..i].iter().collect());
        } else if matches!(
            (c, chars.get(i + 1)),
            ('<', Some('>' | '=')) | ('>', Some('='))
        ) {
            out.push(chars[i..i + 2].iter().collect());
            i += 2;
        } else {
            out.push(c.to_string());
            i += 1;
        }
    }
    out
}

impl Parser {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.at).map(String::as_str)
    }

    fn next(&mut self) -> Option<String> {
        let t = self.tokens.get(self.at).cloned();
        self.at += 1;
        t
    }

    fn eat(&mut self, token: &str) -> bool {
        if self.peek() == Some(token) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> E {
        let left = self.add();
        match self.peek() {
            Some(op @ ("=" | "<>" | "<" | ">" | "<=" | ">=")) => {
                let op = op.to_string();
                self.at += 1;
                let right = self.add();
                E::Bin(op, Box::new(left), Box::new(right))
            }
            _ => left,
        }
    }

    fn add(&mut self) -> E {
        let mut left = self.mul();
        while let Some(op @ ("+" | "-")) = self.peek() {
            let op = op.to_string();
            self.at += 1;
            left = E::Bin(op, Box::new(left), Box::new(self.mul()));
        }
        left
    }

    fn mul(&mut self) -> E {
        let mut left = self.pow();
        while let Some(op @ ("*" | "/")) = self.peek() {
            let op = op.to_string();
            self.at += 1;
            left = E::Bin(op, Box::new(left), Box::new(self.pow()));
        }
        left
    }

    fn pow(&mut self) -> E {
        let left = self.unary();
        if self.eat("^") {
            return E::Bin("^".into(), Box::new(left), Box::new(self.pow()));
        }
        left
    }

    fn unary(&mut self) -> E {
        if self.eat("-") {
            return E::Neg(Box::new(self.unary()));
        }
        if self.eat("+") {
            return self.unary();
        }
        self.primary()
    }

    fn primary(&mut self) -> E {
        let Some(t) = self.next() else {
            return E::Bad;
        };
        if t == "(" {
            let e = self.expr();
            return if self.eat(")") { e } else { E::Bad };
        }
        if let Some(text) = t.strip_prefix('"') {
            return E::Str(text.strip_suffix('"').unwrap_or(text).to_string());
        }
        if let Ok(n) = t.parse::<f64>() {
            return E::Num(n);
        }
        if let Some((r, c)) = cell_ref(&t) {
            if self.eat(":") {
                return match self.next().as_deref().and_then(cell_ref) {
                    Some((r2, c2)) => E::Range(r.min(r2), c.min(c2), r.max(r2), c.max(c2)),
                    None => E::Bad,
                };
            }
            return E::Ref(r, c);
        }
        if t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && self.eat("(") {
            let mut args = Vec::new();
            if !self.eat(")") {
                loop {
                    args.push(self.expr());
                    if self.eat(")") {
                        break;
                    }
                    if !self.eat(",") {
                        return E::Bad;
                    }
                }
            }
            return E::Call(t.to_ascii_uppercase(), args);
        }
        if matches!(t.to_ascii_uppercase().as_str(), "TRUE" | "FALSE") {
            return E::Num(f64::from(u8::from(t.eq_ignore_ascii_case("true"))));
        }
        E::Bad
    }
}

fn parse(text: &str) -> E {
    let mut p = Parser {
        tokens: tokenize(text),
        at: 0,
    };
    let e = p.expr();
    if p.at == p.tokens.len() { e } else { E::Bad }
}

/// A cell's formula: what follows its `=` (`==text==` is a highlight).
fn formula(cell: &str) -> Option<&str> {
    cell.trim()
        .strip_prefix('=')
        .filter(|rest| !rest.starts_with('='))
}

/// A table's cells, worked out on demand (each once; a loop is an error).
struct Sheet {
    grid: Vec<Vec<String>>,
    done: RefCell<HashMap<(usize, usize), V>>,
    busy: RefCell<HashSet<(usize, usize)>>,
}

impl Sheet {
    fn value(&self, r: usize, c: usize) -> V {
        if let Some(v) = self.done.borrow().get(&(r, c)) {
            return v.clone();
        }
        let raw = self
            .grid
            .get(r)
            .and_then(|row| row.get(c))
            .map_or("", |s| s.trim());
        let Some(formula) = formula(raw) else {
            return match raw.parse::<f64>() {
                Ok(n) => V::Num(n),
                Err(_) => V::Text(raw.to_string()),
            };
        };
        if !self.busy.borrow_mut().insert((r, c)) {
            return ERR;
        }
        let v = self.eval(&parse(formula));
        self.busy.borrow_mut().remove(&(r, c));
        self.done.borrow_mut().insert((r, c), v.clone());
        v
    }

    /// An argument's values: a range's cells, or the one value.
    fn values(&self, e: &E) -> Vec<V> {
        match e {
            E::Range(r1, c1, r2, c2) => (*r1..=*r2)
                .flat_map(|r| (*c1..=*c2).map(move |c| (r, c)))
                .map(|(r, c)| self.value(r, c))
                .collect(),
            other => vec![self.eval(other)],
        }
    }

    fn eval(&self, e: &E) -> V {
        let num = |e: &E| self.eval(e).num();
        let done = |r: Result<f64, V>| match r {
            Ok(n) if n.is_finite() => V::Num(n),
            Ok(_) => ERR,
            Err(e) => e,
        };
        match e {
            E::Num(n) => V::Num(*n),
            E::Str(s) => V::Text(s.clone()),
            E::Ref(r, c) => self.value(*r, *c),
            E::Range(..) | E::Bad => ERR,
            E::Neg(x) => done(num(x).map(|n| -n)),
            E::Bin(op, a, b) if matches!(op.as_str(), "=" | "<>" | "<" | ">" | "<=" | ">=") => {
                let (a, b) = (self.eval(a), self.eval(b));
                if let V::Err(_) = a {
                    return a;
                }
                if let V::Err(_) = b {
                    return b;
                }
                let order = match (a.num(), b.num()) {
                    (Ok(x), Ok(y)) => x.partial_cmp(&y),
                    _ => Some(a.shown().to_lowercase().cmp(&b.shown().to_lowercase())),
                };
                let Some(order) = order else {
                    return ERR;
                };
                V::Bool(match op.as_str() {
                    "=" => order.is_eq(),
                    "<>" => order.is_ne(),
                    "<" => order.is_lt(),
                    ">" => order.is_gt(),
                    "<=" => order.is_le(),
                    _ => order.is_ge(),
                })
            }
            E::Bin(op, a, b) => done(num(a).and_then(|x| {
                let y = num(b)?;
                Ok(match op.as_str() {
                    "+" => x + y,
                    "-" => x - y,
                    "*" => x * y,
                    "/" if y == 0.0 => return Err(ERR),
                    "/" => x / y,
                    _ => x.powf(y),
                })
            })),
            E::Call(name, args) => self.call(name, args),
        }
    }

    fn call(&self, name: &str, args: &[E]) -> V {
        let arg = |i: usize| args.get(i).map_or(Err(ERR), |e| self.eval(e).num());
        let one = |f: fn(f64) -> f64| {
            if args.len() != 1 {
                return ERR;
            }
            match arg(0) {
                Ok(x) if f(x).is_finite() => V::Num(f(x)),
                Ok(_) => ERR,
                Err(e) => e,
            }
        };
        match name {
            "SUM" | "AVERAGE" | "AVG" | "MEDIAN" | "MIN" | "MAX" | "COUNT" | "COUNTA"
            | "PRODUCT" => {
                let mut numbers = Vec::new();
                let mut filled = 0;
                for a in args {
                    let range = matches!(a, E::Range(..));
                    for v in self.values(a) {
                        match &v {
                            V::Err(_) => return v,
                            V::Num(n) => numbers.push(*n),
                            V::Bool(b) if !range => numbers.push(f64::from(u8::from(*b))),
                            V::Text(t) if t.is_empty() => continue,
                            V::Text(_) if !range && name != "COUNTA" && name != "COUNT" => {
                                return ERR;
                            }
                            _ => {}
                        }
                        filled += 1;
                    }
                }
                let n = numbers.len() as f64;
                V::Num(match name {
                    "SUM" => numbers.iter().sum(),
                    "PRODUCT" => numbers.iter().product(),
                    "COUNT" => n,
                    "COUNTA" => f64::from(filled),
                    "MIN" => numbers
                        .iter()
                        .copied()
                        .fold(f64::INFINITY, f64::min)
                        .min(if n == 0.0 { 0.0 } else { f64::INFINITY }),
                    "MAX" => numbers
                        .iter()
                        .copied()
                        .fold(f64::NEG_INFINITY, f64::max)
                        .max(if n == 0.0 { 0.0 } else { f64::NEG_INFINITY }),
                    _ if n == 0.0 => return ERR,
                    "MEDIAN" => {
                        numbers.sort_by(f64::total_cmp);
                        let mid = numbers.len() / 2;
                        if numbers.len() % 2 == 0 {
                            (numbers[mid - 1] + numbers[mid]) / 2.0
                        } else {
                            numbers[mid]
                        }
                    }
                    _ => numbers.iter().sum::<f64>() / n,
                })
            }
            "ABS" => one(f64::abs),
            "SQRT" => match arg(0) {
                Ok(x) if x >= 0.0 && args.len() == 1 => V::Num(x.sqrt()),
                Ok(_) => ERR,
                Err(e) => e,
            },
            "INT" | "FLOOR" => one(f64::floor),
            "CEILING" => one(f64::ceil),
            "TRUNC" => one(f64::trunc),
            "ROUND" => {
                let digits = if args.len() > 1 { arg(1) } else { Ok(0.0) };
                match (arg(0), digits) {
                    (Ok(x), Ok(d)) => {
                        let f = 10f64.powi(d as i32);
                        V::Num((x * f).round() / f)
                    }
                    (Err(e), _) | (_, Err(e)) => e,
                }
            }
            "POWER" | "POW" => match (arg(0), arg(1)) {
                (Ok(x), Ok(y)) if x.powf(y).is_finite() => V::Num(x.powf(y)),
                (Err(e), _) | (_, Err(e)) => e,
                _ => ERR,
            },
            "MOD" => match (arg(0), arg(1)) {
                (Ok(_), Ok(0.0)) => ERR,
                (Ok(x), Ok(y)) => V::Num(x - y * (x / y).floor()),
                (Err(e), _) | (_, Err(e)) => e,
            },
            "IF" => {
                let Some(test) = args.first() else {
                    return ERR;
                };
                match self.eval(test).truthy() {
                    Ok(true) => args.get(1).map_or(V::Bool(true), |e| self.eval(e)),
                    Ok(false) => args.get(2).map_or(V::Bool(false), |e| self.eval(e)),
                    Err(e) => e,
                }
            }
            _ => V::Err("#NAME?"),
        }
    }
}

/// The table's lines (its second the separator) with every formula cell
/// worked out; `None` when it has none.
pub fn compute(lines: &[String]) -> Option<Vec<String>> {
    let rows: Vec<(usize, Vec<String>)> = lines
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != 1)
        .map(|(i, l)| (i, cells(l)))
        .collect();
    if !rows
        .iter()
        .any(|(_, r)| r.iter().any(|c| formula(c).is_some()))
    {
        return None;
    }
    let sheet = Sheet {
        grid: rows.iter().map(|(_, r)| r.clone()).collect(),
        done: RefCell::default(),
        busy: RefCell::default(),
    };
    let mut out = lines.to_vec();
    for (r, (i, row)) in rows.iter().enumerate() {
        let shown: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                if formula(cell).is_some() {
                    sheet.value(r, c).shown()
                } else {
                    cell.clone()
                }
            })
            .collect();
        out[*i] = format!("| {} |", shown.join(" | "));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    fn shown(text: &str) -> Vec<Vec<String>> {
        compute(&table(text))
            .expect("formulas")
            .iter()
            .map(|l| super::super::table::cells(l))
            .collect()
    }

    #[test]
    fn formulas_show_their_results() {
        let t = shown(
            "| Item | Qty | Price | Total |\n|---|---|---|---|\n| a | 2 | 1.5 | =B2*C2 |\n| b | 3 | 2 | =B3*C3 |\n| Sum | =SUM(B2:B3) | | =SUM(D2:D3) |",
        );
        assert_eq!(t[2][3], "3");
        assert_eq!(t[3][3], "6");
        assert_eq!(t[4][1], "5");
        assert_eq!(t[4][3], "9", "a formula over formulas");
        assert_eq!(t[0][0], "Item", "the rest as it is");
        assert_eq!(t[1][0], "---", "the separator too");
    }

    #[test]
    fn functions_operators_and_errors() {
        let one = |f: &str| {
            shown(&format!("| A | B | C |\n|---|---|---|\n| 4 | 9 | {f} |"))[2][2].clone()
        };
        assert_eq!(one("=(A2+B2)*2^2"), "52");
        assert_eq!(one("=-A2+1"), "-3");
        assert_eq!(one("=AVERAGE(A2:B2)"), "6.5");
        assert_eq!(one("=MAX(A2:B2)-MIN(A2:B2)"), "5");
        assert_eq!(one("=ROUND(B2/A2, 2)"), "2.25");
        assert_eq!(one("=SQRT(B2)"), "3");
        assert_eq!(one("=IF(A2>B2, 1, 0)"), "0");
        assert_eq!(one("=IF(A2<>B2, \"diff\", \"same\")"), "diff");
        assert_eq!(one("=COUNT(A1:B2)"), "2", "text and blanks aren't counted");
        assert_eq!(one("=MOD(B2, A2)"), "1");
        assert_eq!(one("=B2/0"), "#ERR");
        assert_eq!(one("=A1*2"), "#ERR", "a text cell");
        assert_eq!(one("=NOPE(A2)"), "#NAME?");
        assert_eq!(one("=C2+1"), "#ERR", "itself: a loop");
        assert_eq!(one("=1/3"), "0.333333");
        assert!(
            compute(&table("| A |\n|---|\n| not a formula |")).is_none(),
            "text is text"
        );
        assert!(
            compute(&table("| a |\n|---|\n| 1 |")).is_none(),
            "no formulas"
        );
    }

    #[test]
    fn a_highlight_is_not_a_formula() {
        assert!(
            compute(&table("| a |\n|---|\n| ==marked== |")).is_none(),
            "==text== is a highlight"
        );
        let t = shown("| a | b |\n|---|---|\n| ==🟡x== | =1+1 |");
        assert_eq!(t[2], ["==🟡x==", "2"]);
    }
}
