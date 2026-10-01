//! Table formulas (AT-30 … AT-37), after `md-advanced-tables`: lines
//! `<!-- TBLFM: $4=($2*$3);%.2f -->` under a table, each `DEST=EXPR`
//! (several joined with `::`), run in order, each on the results of the
//! ones before.
//!
//! Rows are numbered from the header (`@1`); the separator row doesn't
//! count, so `@2` (also `@I`) is the first body row. `@<` / `@>` are the
//! first and last rows, `$<` / `$>` the first and last columns, and
//! `@-1`, `$+2` are relative to the cell being computed. A destination is
//! a cell (`@3$2`), a column (`$4`: every body row) or a row (`@5`: every
//! column). Ranges (`@2..@5`, `@2$1..@3$3`) go into `sum` and `mean`.

use super::table::Table;

/// The formulas in the `TBLFM` comment lines at the start of `lines`
/// (the lines after a table).
pub fn formulas<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    lines
        .into_iter()
        .map_while(|l| {
            let l = l.trim();
            l.strip_prefix("<!--")?
                .trim_start()
                .strip_prefix("TBLFM:")?
                .trim_end()
                .strip_suffix("-->")
                .map(str::trim)
        })
        .flat_map(|f| f.split("::").map(|s| s.trim().to_string()))
        .filter(|f| !f.is_empty())
        .collect()
}

/// Runs `formulas` on `table` in order; on an error the table may be half
/// done, so callers keep the original.
pub fn evaluate(table: &mut Table, formulas: &[String]) -> Result<(), String> {
    for formula in formulas {
        run(table, formula).map_err(|e| format!("Formula `{formula}`: {e}"))?;
    }
    Ok(())
}

fn run(table: &mut Table, formula: &str) -> Result<(), String> {
    let (dest, rest) = formula.split_once('=').ok_or("no `=`")?;
    let (expr, format) = match rest.rsplit_once(';') {
        Some((e, f)) => (e, Some(f.trim())),
        None => (rest, None),
    };
    let rows = table.rows.len();
    let cols = table.columns();
    let mut dest_parser = Parser::new(dest.trim());
    let first = dest_parser.reference()?;
    // A range destination (`@2$3..@4$3`): the cells it covers, row by row.
    if dest_parser.eat("..") {
        let last = dest_parser.reference()?;
        let corner = |r: Reference| -> Result<(usize, usize), String> {
            Ok((
                r.row
                    .ok_or("a range destination's ends have rows")?
                    .at(0, rows)?,
                r.col
                    .or(first.col)
                    .ok_or("a range destination's ends have columns")?
                    .at(0, cols)?,
            ))
        };
        let ((r1, c1), (r2, c2)) = (corner(first)?, corner(last)?);
        let targets: Vec<(usize, usize)> = (r1.min(r2)..=r1.max(r2))
            .flat_map(|r| (c1.min(c2)..=c1.max(c2)).map(move |c| (r, c)))
            .collect();
        let mut parser = Parser::new(expr.trim());
        let value = parser.expr(table, targets[0])?;
        parser.end()?;
        let values = match value {
            Value::List(items) if items.len() != targets.len() => {
                return Err(format!(
                    "{} values for {} cells",
                    items.len(),
                    targets.len()
                ));
            }
            Value::List(items) => items,
            one => vec![one; targets.len()],
        };
        for ((r, c), v) in targets.into_iter().zip(values) {
            table.rows[r][c] = show(v, format)?;
        }
        return Ok(());
    }
    let dest = first;
    let targets: Vec<(usize, usize)> = match (dest.row, dest.col) {
        (Some(r), Some(c)) => vec![(r.at(0, rows)?, c.at(0, cols)?)],
        (None, Some(c)) => {
            let c = c.at(0, cols)?;
            (1..rows).map(|r| (r, c)).collect()
        }
        (Some(r), None) => {
            let r = r.at(0, rows)?;
            (0..cols).map(|c| (r, c)).collect()
        }
        (None, None) => return Err("no destination".into()),
    };
    for (r, c) in targets {
        let mut parser = Parser::new(expr.trim());
        let value = parser.expr(table, (r, c))?;
        parser.end()?;
        table.rows[r][c] = show(value, format)?;
    }
    Ok(())
}

/// A value while computing.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Number(f64),
    Text(String),
    /// A range's cells.
    List(Vec<Value>),
}

impl Value {
    fn of_cell(text: &str) -> Value {
        let t = text.trim();
        if t.is_empty() {
            return Value::Number(0.0);
        }
        t.parse::<f64>()
            .map_or_else(|_| Value::Text(t.to_string()), Value::Number)
    }

    /// The value as a number, as the original reads cells: a date
    /// (`2022-12-31 23:59`) or a duration (`23:59`) in milliseconds, other
    /// text 0.
    fn number(&self) -> Result<f64, String> {
        match self {
            Value::Number(n) => Ok(*n),
            Value::Text(t) => Ok(time_value(t).unwrap_or(0.0)),
            Value::List(_) => Err("a range needs sum( ) or mean( )".into()),
        }
    }

    /// The numbers in it (a range's non-empty cells).
    fn numbers(&self) -> Result<Vec<f64>, String> {
        match self {
            Value::List(items) => items
                .iter()
                .filter(|v| !matches!(v, Value::Text(t) if t.is_empty()))
                .map(Value::number)
                .collect(),
            v => Ok(vec![v.number()?]),
        }
    }
}

/// A date (`YYYY-MM-DD`, with ` HH:MM` or not; local time) or a duration
/// (`H:MM`) in milliseconds.
fn time_value(text: &str) -> Option<f64> {
    let t = text.trim();
    for (format, with_time) in [("%Y-%m-%d %H:%M", true), ("%Y-%m-%d", false)] {
        let naive = if with_time {
            chrono::NaiveDateTime::parse_from_str(t, format).ok()
        } else {
            chrono::NaiveDate::parse_from_str(t, format)
                .ok()
                .map(|d| d.and_time(chrono::NaiveTime::MIN))
        };
        if let Some(naive) = naive {
            use chrono::TimeZone;
            let local = chrono::Local.from_local_datetime(&naive).earliest()?;
            return Some(local.timestamp_millis() as f64);
        }
    }
    let (sign, rest) = match t.strip_prefix('-') {
        Some(r) => (-1.0, r),
        None => (1.0, t),
    };
    let (hours, minutes) = rest.split_once(':')?;
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if !digits(hours) || !digits(minutes) || minutes.len() != 2 {
        return None;
    }
    let (h, m): (u32, u32) = (hours.parse().ok()?, minutes.parse().ok()?);
    (m < 60).then(|| sign * f64::from(h * 60 + m) * 60_000.0)
}

fn show(value: Value, format: Option<&str>) -> Result<String, String> {
    let n = match value {
        Value::Text(t) => return Ok(t),
        Value::List(_) => return Err("a range needs sum( ) or mean( )".into()),
        Value::Number(n) => n,
    };
    match format {
        Some("dt") => {
            let date = chrono::DateTime::from_timestamp_millis(n as i64)
                .ok_or("a date out of range")?
                .with_timezone(&chrono::Local);
            return Ok(date.format("%Y-%m-%d %H:%M").to_string());
        }
        Some("hm") => {
            let sign = if n < 0.0 { "-" } else { "" };
            let minutes = (n.abs() / 60_000.0).floor() as u64;
            return Ok(format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60));
        }
        _ => {}
    }
    if let Some(f) = format {
        let digits = f
            .strip_prefix("%.")
            .and_then(|d| d.strip_suffix('f'))
            .and_then(|d| d.parse::<usize>().ok())
            .ok_or_else(|| format!("unknown format {f:?}"))?;
        return Ok(format!("{n:.digits$}"));
    }
    // Decimal results as the original shows them: 3 * 1.2 is 3.6 (not
    // binary floating point's 3.5999999999999996).
    let rounded = format!("{n:.10}");
    let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
    Ok(match trimmed {
        "-0" => "0".into(),
        t => t.to_string(),
    })
}

/// One side of a reference: a number, first, last, the first body row,
/// or relative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Index {
    At(usize),
    First,
    Last,
    FirstBody,
    Relative(isize),
}

impl Index {
    /// The 0-based index, with `here` for relative ones, among `len`.
    fn at(self, here: usize, len: usize) -> Result<usize, String> {
        let i = match self {
            Index::At(n) => n.checked_sub(1),
            Index::First => Some(0),
            Index::Last => len.checked_sub(1),
            Index::FirstBody => Some(1),
            Index::Relative(d) => here.checked_add_signed(d),
        };
        i.filter(|&i| i < len)
            .ok_or_else(|| "a reference outside the table".into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reference {
    row: Option<Index>,
    col: Option<Index>,
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Parser { text, at: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn skip_space(&mut self) {
        let rest = self.rest();
        self.at += rest.len() - rest.trim_start().len();
    }

    fn eat(&mut self, s: &str) -> bool {
        self.skip_space();
        if self.rest().starts_with(s) {
            self.at += s.len();
            true
        } else {
            false
        }
    }

    fn end(&mut self) -> Result<(), String> {
        self.skip_space();
        match self.rest() {
            "" => Ok(()),
            rest => Err(format!("unexpected {rest:?}")),
        }
    }

    fn digits(&mut self) -> Option<usize> {
        let n = self.rest().chars().take_while(char::is_ascii_digit).count();
        let value = self.rest()[..n].parse().ok()?;
        self.at += n;
        Some(value)
    }

    fn index(&mut self, body: bool) -> Result<Index, String> {
        if self.eat("<") {
            return Ok(Index::First);
        }
        if self.eat(">") {
            return Ok(Index::Last);
        }
        if body && self.eat("I") {
            return Ok(Index::FirstBody);
        }
        for (sign, factor) in [("+", 1), ("-", -1)] {
            if self.eat(sign) {
                let n = self.digits().ok_or("a number after + or -")?;
                return Ok(Index::Relative(factor * n as isize));
            }
        }
        self.digits()
            .map(Index::At)
            .ok_or_else(|| "a row or column".into())
    }

    fn reference(&mut self) -> Result<Reference, String> {
        let row = if self.eat("@") {
            Some(self.index(true)?)
        } else {
            None
        };
        let col = if self.eat("$") {
            Some(self.index(false)?)
        } else {
            None
        };
        if row.is_none() && col.is_none() {
            return Err(format!("a reference expected at {:?}", self.rest()));
        }
        Ok(Reference { row, col })
    }

    fn expr(&mut self, t: &Table, here: (usize, usize)) -> Result<Value, String> {
        let mut value = self.term(t, here)?;
        loop {
            let op = if self.eat("+") {
                '+'
            } else if self.eat("-") {
                '-'
            } else {
                return Ok(value);
            };
            value = arith(op, value, self.term(t, here)?)?;
        }
    }

    fn term(&mut self, t: &Table, here: (usize, usize)) -> Result<Value, String> {
        let mut value = self.factor(t, here)?;
        loop {
            let op = if self.eat("*") {
                '*'
            } else if self.eat("/") {
                '/'
            } else {
                return Ok(value);
            };
            value = arith(op, value, self.factor(t, here)?)?;
        }
    }

    fn factor(&mut self, t: &Table, here: (usize, usize)) -> Result<Value, String> {
        self.skip_space();
        if self.eat("(") {
            let value = self.expr(t, here)?;
            return self.close(value);
        }
        if self.eat("-") {
            return Ok(Value::Number(-self.factor(t, here)?.number()?));
        }
        let rest = self.rest();
        if rest.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
            let n = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .count();
            let value = rest[..n]
                .parse()
                .map_err(|_| format!("bad number {rest:?}"))?;
            self.at += n;
            return Ok(Value::Number(value));
        }
        if let Some(quoted) = rest.strip_prefix('"') {
            let end = quoted.find('"').ok_or("an unclosed \"")?;
            self.at += end + 2;
            return Ok(Value::Text(quoted[..end].to_string()));
        }
        if rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
            let n = rest.chars().take_while(char::is_ascii_alphanumeric).count();
            let name = &rest[..n];
            self.at += n;
            if !self.eat("(") {
                return Err(format!("( after {name}"));
            }
            return self.function(name, t, here);
        }
        let from = self.reference()?;
        if self.eat("..") {
            let to = self.reference()?;
            return self.range(t, here, from, to);
        }
        let (r, c) = cell(t, here, from)?;
        Ok(Value::of_cell(&t.rows[r][c]))
    }

    fn close(&mut self, value: Value) -> Result<Value, String> {
        if self.eat(")") {
            Ok(value)
        } else {
            Err(format!("a ) expected at {:?}", self.rest()))
        }
    }

    fn function(&mut self, name: &str, t: &Table, here: (usize, usize)) -> Result<Value, String> {
        match name {
            "sum" | "mean" => {
                let numbers = self.expr(t, here)?.numbers()?;
                let sum: f64 = numbers.iter().sum();
                let value = if name == "sum" {
                    sum
                } else if numbers.is_empty() {
                    0.0
                } else {
                    sum / numbers.len() as f64
                };
                self.close(Value::Number(value))
            }
            "if" => {
                let left = self.expr(t, here)?;
                let op = ["<=", ">=", "==", "!=", "<", ">"]
                    .into_iter()
                    .find(|op| self.eat(op))
                    .ok_or("a comparison (< > <= >= == !=) in if( )")?;
                let right = self.expr(t, here)?;
                let holds = compare(&left, op, &right)?;
                let comma = |p: &mut Self| {
                    if p.eat(",") {
                        Ok(())
                    } else {
                        Err(String::from("if(condition, then, else)"))
                    }
                };
                comma(self)?;
                let yes = self.expr(t, here)?;
                comma(self)?;
                let no = self.expr(t, here)?;
                self.close(if holds { yes } else { no })
            }
            _ => Err(format!("unknown function {name}")),
        }
    }

    fn range(
        &self,
        t: &Table,
        here: (usize, usize),
        from: Reference,
        to: Reference,
    ) -> Result<Value, String> {
        let (r1, c1) = cell(t, here, from)?;
        // The second end's column is the first's when it has none.
        let to = Reference {
            col: to.col.or(from.col),
            ..to
        };
        let (r2, c2) = cell(t, here, to)?;
        let mut items = Vec::new();
        for r in r1.min(r2)..=r1.max(r2) {
            for c in c1.min(c2)..=c1.max(c2) {
                let text = t.rows[r][c].trim();
                items.push(if text.is_empty() {
                    Value::Text(String::new())
                } else {
                    Value::of_cell(text)
                });
            }
        }
        Ok(Value::List(items))
    }
}

/// `a op b`, as the original does: numbers, or a range with one cell
/// (each of the range's cells; for `-` and `/` the range comes first).
fn arith(op: char, a: Value, b: Value) -> Result<Value, String> {
    let one = |x: f64, y: f64| -> Result<f64, String> {
        Ok(match op {
            '+' => x + y,
            '-' => x - y,
            '*' => x * y,
            _ if y == 0.0 => return Err("division by zero".into()),
            _ => x / y,
        })
    };
    let each = |items: Vec<Value>, n: f64, range_first: bool| -> Result<Value, String> {
        items
            .iter()
            .map(|v| {
                let x = v.number()?;
                one(
                    if range_first { x } else { n },
                    if range_first { n } else { x },
                )
                .map(Value::Number)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List)
    };
    match (a, b) {
        (Value::List(_), Value::List(_)) => {
            Err("a range with a range (one of them is one cell)".into())
        }
        (Value::List(items), b) => each(items, b.number()?, true),
        (a, Value::List(items)) if matches!(op, '+' | '*') => each(items, a.number()?, false),
        (_, Value::List(_)) => Err("the second of - and / is one cell".into()),
        (a, b) => Ok(Value::Number(one(a.number()?, b.number()?)?)),
    }
}

/// A reference's cell; a missing row or column is `here`'s.
fn cell(t: &Table, here: (usize, usize), r: Reference) -> Result<(usize, usize), String> {
    let row = match r.row {
        Some(i) => i.at(here.0, t.rows.len())?,
        None => here.0,
    };
    let col = match r.col {
        Some(i) => i.at(here.1, t.columns())?,
        None => here.1,
    };
    Ok((row, col))
}

fn compare(a: &Value, op: &str, b: &Value) -> Result<bool, String> {
    let order = match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.total_cmp(y),
        (Value::Text(x), Value::Text(y)) => x.cmp(y),
        _ => return Ok(op == "!="),
    };
    Ok(match op {
        "<" => order.is_lt(),
        ">" => order.is_gt(),
        "<=" => order.is_le(),
        ">=" => order.is_ge(),
        "==" => order.is_eq(),
        _ => order.is_ne(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table::parse(&[
            "| n | a | b |",
            "|---|---|---|",
            "| x | 1 | 2 |",
            "| y | 3 | 4 |",
            "| z |   |   |",
        ])
    }

    fn eval(formula: &str) -> Result<Table, String> {
        let mut t = table();
        evaluate(&mut t, &[formula.to_string()]).map(|()| t)
    }

    #[test]
    fn formulas_come_from_tblfm_comments() {
        let lines = [
            "<!-- TBLFM: $3=$2 -->",
            " <!-- TBLFM: @2$1=1::@3$1=2 -->",
            "text",
            "<!-- TBLFM: $1=0 -->",
        ];
        assert_eq!(formulas(lines), ["$3=$2", "@2$1=1", "@3$1=2"]);
    }

    #[test]
    fn references_ranges_and_functions() {
        let t = eval("$3=($2*10+1)").unwrap();
        assert_eq!(t.rows[1][2], "11");
        assert_eq!(t.rows[3][2], "1", "an empty cell is 0");
        let t = eval("@>$2=sum(@I..@-1)").unwrap();
        assert_eq!(t.rows[3][1], "4");
        let t = eval("@>$3=mean(@2$2..@3$3)").unwrap();
        assert_eq!(t.rows[3][2], "2.5");
        let t = eval("@>=@<").unwrap();
        assert_eq!(t.rows[3], ["n", "a", "b"], "a whole row");
        let t = eval("$>=$-1/4;%.2f").unwrap();
        assert_eq!(t.rows[1][2], "0.25");
        let t = eval("$1=if($2 > 2, \"big\", \"small\")").unwrap();
        assert_eq!(t.rows[1][0], "small");
        assert_eq!(t.rows[2][0], "big");
    }

    #[test]
    fn like_the_original_decimals_text_dates_and_durations() {
        let mut t = Table::parse(&[
            "| a | b | c |",
            "|---|---|---|",
            "| 3 | 1.2 | |",
            "| **Sum** | 0.1 | |",
        ]);
        let run = |t: &mut Table, f: &str| evaluate(t, &[f.to_string()]);
        run(&mut t, "@2$3=(@2$1*@2$2)").unwrap();
        assert_eq!(t.rows[1][2], "3.6", "not 3.5999999999999996");
        run(&mut t, "@3$3=(@3$2+0.2)").unwrap();
        assert_eq!(t.rows[2][2], "0.3");
        run(&mut t, "@3$3=(@3$1+1)").unwrap();
        assert_eq!(t.rows[2][2], "1", "text counts as 0");
        let mut d = Table::parse(&[
            "| start | end | took | min | at |",
            "|---|---|---|---|---|",
            "| 2022-12-31 10:00 | 2022-12-31 12:30 | | | |",
            "| 01:30 | 0:45 | | | |",
        ]);
        run(&mut d, "@2$3=(@2$2-@2$1);hm").unwrap();
        assert_eq!(d.rows[1][2], "02:30");
        run(&mut d, "@2$4=((@2$2-@2$1)/60000)").unwrap();
        assert_eq!(d.rows[1][3], "150");
        run(&mut d, "@2$5=(@2$1+@3$2);dt").unwrap();
        assert_eq!(d.rows[1][4], "2022-12-31 10:45");
        run(&mut d, "@3$3=(@3$1+@3$2);hm").unwrap();
        assert_eq!(d.rows[2][2], "02:15");
        run(&mut d, "@3$4=(@3$2-@3$1);hm").unwrap();
        assert_eq!(d.rows[2][3], "-00:45");
    }

    #[test]
    fn ranges_with_a_cell_fill_a_range() {
        let mut t = Table::parse(&[
            "| a | b | c |",
            "|---|---|---|",
            "| 1 | 10 | |",
            "| 2 | 20 | |",
            "| 3 | 30 | |",
        ]);
        let run = |t: &mut Table, f: &str| evaluate(t, &[f.to_string()]);
        run(&mut t, "@2$3..@4$3=(@2$1..@4$1*2)").unwrap();
        assert_eq!(
            [&t.rows[1][2], &t.rows[2][2], &t.rows[3][2]],
            ["2", "4", "6"]
        );
        run(&mut t, "@2$3..@4$3=(@2$2..@4$2-@2$1)").unwrap();
        assert_eq!(
            [&t.rows[1][2], &t.rows[2][2], &t.rows[3][2]],
            ["9", "19", "29"]
        );
        run(&mut t, "@2$3..@4$3=@2$1..@4$1").unwrap();
        assert_eq!(t.rows[3][2], "3", "copied");
        run(&mut t, "@2$3..@4$3=7").unwrap();
        assert_eq!(t.rows[2][2], "7", "one value in every cell");
        let err = |f: &str| evaluate(&mut t.clone(), &[f.to_string()]).unwrap_err();
        assert!(err("@2$3..@3$3=@2$1..@4$1").contains("3 values for 2 cells"));
        assert!(err("@2$3..@4$3=(@2$1-@2$1..@4$1)").contains("one cell"));
        assert!(err("@2$3..@4$3=(@2$1..@4$1*@2$2..@4$2)").contains("a range with a range"));
        assert!(err("@2$3=(@2$1..@4$1*2)").contains("range"));
    }

    #[test]
    fn errors_say_what_is_wrong() {
        assert!(
            eval("$3=nope(1)")
                .unwrap_err()
                .contains("unknown function nope")
        );
        assert!(eval("$3=@9$1").unwrap_err().contains("outside"));
        assert!(eval("$3=@2..@3").unwrap_err().contains("range"));
        assert!(eval("$3=1/0").unwrap_err().contains("zero"));
        assert!(eval("$3").unwrap_err().contains("="));
    }
}
