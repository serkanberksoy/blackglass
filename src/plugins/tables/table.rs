//! A Markdown table as cells: finding it in a note, reading it, writing it
//! back lined up (AT-01), and the row and column operations (AT-20 …
//! AT-26). Widths are terminal columns, so wide characters line up.

use std::cmp::Ordering;
use std::ops::Range;

use unicode_width::UnicodeWidthStr;

/// A column's alignment, from the separator row's colons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// A table: the header row, then the body rows (the separator row isn't
/// a row here; it becomes [`Table::aligns`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// The whitespace before the table's lines (a table in a list).
    pub indent: String,
    pub rows: Vec<Vec<String>>,
    pub aligns: Vec<Align>,
    /// Whether it has a separator row (a header line alone doesn't).
    pub separator: bool,
}

/// Whether `line` is a table line: it starts with `|`.
pub fn is_table_line(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

/// The lines of the table around line `row`, if it's in one (not in a
/// code block).
pub fn find(lines: &[&str], row: usize) -> Option<Range<usize>> {
    if !lines.get(row).is_some_and(|l| is_table_line(l)) {
        return None;
    }
    let fences = lines[..row]
        .iter()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("```") || t.starts_with("~~~")
        })
        .count();
    if fences % 2 == 1 {
        return None;
    }
    let start = (0..row)
        .rev()
        .take_while(|&i| is_table_line(lines[i]))
        .last()
        .unwrap_or(row);
    let end = (row..lines.len())
        .take_while(|&i| is_table_line(lines[i]))
        .last()
        .map_or(row + 1, |i| i + 1);
    Some(start..end)
}

/// Every table in `lines` (outside code blocks).
pub fn find_all(lines: &[&str]) -> Vec<Range<usize>> {
    let mut all = Vec::new();
    let mut fence = false;
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
        } else if !fence && is_table_line(lines[i]) {
            let start = i;
            while i < lines.len() && is_table_line(lines[i]) {
                i += 1;
            }
            all.push(start..i);
            continue;
        }
        i += 1;
    }
    all
}

/// The byte offsets of the unescaped `|`s in `line`.
fn pipes(line: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match c {
            '\\' if !escaped => escaped = true,
            '|' if !escaped => out.push(i),
            _ => escaped = false,
        }
        if c != '\\' {
            escaped = false;
        }
    }
    out
}

/// A table line's cells, trimmed.
pub fn cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let bars = pipes(line);
    let mut bounds: Vec<usize> = bars.clone();
    if bars.first() != Some(&0) {
        bounds.insert(0, 0);
    }
    let closed = bars.last().is_some_and(|&b| b + 1 == line.len()) && bars.len() > 1;
    if !closed {
        bounds.push(line.len());
    }
    bounds
        .windows(2)
        .map(|w| {
            let from = if line[w[0]..].starts_with('|') {
                w[0] + 1
            } else {
                w[0]
            };
            line[from..w[1]].trim().to_string()
        })
        .collect()
}

fn separator_cell(cell: &str) -> Option<Align> {
    let dashes = cell.trim_start_matches(':').trim_end_matches(':');
    if dashes.is_empty() || !dashes.chars().all(|c| c == '-') {
        return None;
    }
    Some(match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        (false, false) => Align::None,
    })
}

/// Which cell of `line` char column `col` is in.
pub fn cell_at(line: &str, col: usize) -> usize {
    let byte = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
    let before = pipes(line).into_iter().filter(|&p| p < byte).count();
    before.saturating_sub(1)
}

/// The char column just after cell `cell`'s text on a written line (or
/// after its first space when it's empty).
pub fn cell_end(line: &str, cell: usize) -> usize {
    let bars = pipes(line);
    let (Some(&open), close) = (bars.get(cell), bars.get(cell + 1).copied()) else {
        return line.chars().count();
    };
    let close = close.unwrap_or(line.len());
    let inner = &line[open + 1..close];
    let end = match inner.trim_end().len() {
        0 => open + 1 + inner.len().min(1),
        n => open + 1 + n,
    };
    line[..end].chars().count()
}

impl Table {
    /// Reads a table's lines.
    pub fn parse(lines: &[&str]) -> Table {
        let first = lines.first().copied().unwrap_or_default();
        let indent = first[..first.len() - first.trim_start().len()].to_string();
        let mut rows: Vec<Vec<String>> = lines.iter().map(|l| cells(l)).collect();
        let mut aligns = Vec::new();
        let mut separator = false;
        if let Some(second) = rows.get(1)
            && let Some(found) = second
                .iter()
                .map(|c| separator_cell(c))
                .collect::<Option<Vec<_>>>()
        {
            aligns = found;
            separator = true;
            rows.remove(1);
        }
        let width = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
        for row in &mut rows {
            row.resize(width, String::new());
        }
        aligns.resize(width, Align::None);
        Table {
            indent,
            rows,
            aligns,
            separator,
        }
    }

    pub fn columns(&self) -> usize {
        self.aligns.len()
    }

    /// The table's row for line `line` of it (the separator is the
    /// header's).
    pub fn row_of_line(&self, line: usize) -> usize {
        if self.separator && line >= 1 {
            line - 1
        } else {
            line
        }
    }

    /// The line of row `row` once written (always with a separator).
    pub fn line_of_row(row: usize) -> usize {
        if row == 0 { 0 } else { row + 1 }
    }

    /// The table lined up: every cell padded to its column's width (or
    /// only single spaces without `pad`), a separator row after the
    /// header.
    pub fn format(&self, pad: bool) -> Vec<String> {
        let widths: Vec<usize> = (0..self.columns())
            .map(|c| {
                self.rows
                    .iter()
                    .map(|r| r[c].width())
                    .max()
                    .unwrap_or(0)
                    .max(3)
            })
            .collect();
        let line = |cells: Vec<String>| format!("{}| {} |", self.indent, cells.join(" | "));
        let row = |r: &Vec<String>| {
            let cells = r.iter().enumerate().map(|(c, text)| {
                if !pad {
                    return text.clone();
                }
                let room = widths[c] - text.width();
                match self.aligns[c] {
                    Align::Right => format!("{}{text}", " ".repeat(room)),
                    Align::Center => {
                        format!(
                            "{}{text}{}",
                            " ".repeat(room / 2),
                            " ".repeat(room - room / 2)
                        )
                    }
                    Align::None | Align::Left => format!("{text}{}", " ".repeat(room)),
                }
            });
            line(cells.collect())
        };
        let separator = self.aligns.iter().enumerate().map(|(c, a)| {
            let w = if pad { widths[c] } else { 3 };
            match a {
                Align::None => "-".repeat(w),
                Align::Left => format!(":{}", "-".repeat(w - 1)),
                Align::Right => format!("{}:", "-".repeat(w - 1)),
                Align::Center => format!(":{}:", "-".repeat(w - 2)),
            }
        });
        let mut out = vec![row(&self.rows[0])];
        out.push(line(separator.collect()));
        out.extend(self.rows[1..].iter().map(row));
        out
    }

    /// An empty row at `at` (never before the header).
    pub fn insert_row(&mut self, at: usize) {
        let at = at.clamp(1, self.rows.len());
        self.rows.insert(at, vec![String::new(); self.columns()]);
    }

    /// Removes body row `row`; `false` for the header.
    pub fn delete_row(&mut self, row: usize) -> bool {
        if row == 0 || row >= self.rows.len() {
            return false;
        }
        self.rows.remove(row);
        true
    }

    pub fn insert_column(&mut self, at: usize) {
        let at = at.min(self.columns());
        for row in &mut self.rows {
            row.insert(at, String::new());
        }
        self.aligns.insert(at, Align::None);
    }

    /// Removes column `col`; `false` for the last one left.
    pub fn delete_column(&mut self, col: usize) -> bool {
        if self.columns() <= 1 || col >= self.columns() {
            return false;
        }
        for row in &mut self.rows {
            row.remove(col);
        }
        self.aligns.remove(col);
        true
    }

    /// Swaps body row `row` with the one above (`up`) or below; returns
    /// its new place.
    pub fn move_row(&mut self, row: usize, up: bool) -> Option<usize> {
        let to = if up { row.checked_sub(1)? } else { row + 1 };
        if row == 0 || to == 0 || to >= self.rows.len() {
            return None;
        }
        self.rows.swap(row, to);
        Some(to)
    }

    /// Swaps column `col` with its neighbor; returns its new place.
    pub fn move_column(&mut self, col: usize, left: bool) -> Option<usize> {
        let to = if left { col.checked_sub(1)? } else { col + 1 };
        if to >= self.columns() {
            return None;
        }
        for row in &mut self.rows {
            row.swap(col, to);
        }
        self.aligns.swap(col, to);
        Some(to)
    }

    /// Sorts the body rows by column `col`: numbers as numbers, text
    /// without case.
    pub fn sort(&mut self, col: usize, descending: bool) {
        self.rows[1..].sort_by(|a, b| {
            let order = compare(&a[col], &b[col]);
            if descending { order.reverse() } else { order }
        });
    }

    /// Rows become columns.
    pub fn transpose(&mut self) {
        let rows = (0..self.columns())
            .map(|c| self.rows.iter().map(|r| r[c].clone()).collect())
            .collect::<Vec<Vec<String>>>();
        self.aligns = vec![Align::None; rows[0].len()];
        self.rows = rows;
    }

    /// The table as CSV (RFC 4180 quoting).
    pub fn csv(&self) -> String {
        let field = |s: &String| {
            let s = s.replace("\\|", "|");
            if s.contains([',', '"', '\n']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s
            }
        };
        self.rows
            .iter()
            .map(|r| r.iter().map(field).collect::<Vec<_>>().join(",") + "\n")
            .collect()
    }
}

/// Numbers as numbers, else text without case.
fn compare(a: &str, b: &str) -> Ordering {
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(x), Ok(y)) => x.total_cmp(&y),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_split_on_unescaped_pipes() {
        assert_eq!(cells("| a | b\\|c |"), ["a", "b\\|c"]);
        assert_eq!(cells("|a|"), ["a"]);
        assert_eq!(cells("| a | |"), ["a", ""]);
        assert_eq!(cells("| a | b"), ["a", "b"], "no closing pipe");
        assert_eq!(cells("|"), [""]);
    }

    #[test]
    fn tables_are_found_outside_code_blocks() {
        let lines = ["x", "| a |", "|---|", "| 1 |", "", "```", "| no |", "```"];
        assert_eq!(find(&lines, 2), Some(1..4));
        assert_eq!(find(&lines, 0), None);
        assert_eq!(find(&lines, 6), None, "in a code block");
        assert_eq!(find_all(&lines), vec![1..4]);
    }

    #[test]
    fn a_table_is_lined_up_with_wide_characters() {
        let t = Table::parse(&["| 名前 | n |", "|:-:|-:|", "| a | 10 |", "| b |"]);
        assert_eq!(
            t.format(true),
            [
                "| 名前 |   n |",
                "| :--: | --: |",
                "|  a   |  10 |",
                "|  b   |     |"
            ]
        );
        assert_eq!(t.format(false)[1], "| :-: | --: |");
        assert_eq!(cell_at("| ab | cd |", 6), 1);
        assert_eq!(cell_end("| ab   | cd |", 0), 4);
        assert_eq!(cell_end("|      |", 0), 2, "an empty cell");
    }

    #[test]
    fn rows_and_columns_move_and_sort() {
        let mut t = Table::parse(&["| h | i |", "|---|---|", "| b | 2 |", "| a | 10 |"]);
        t.sort(1, false);
        assert_eq!(t.rows[1], ["b", "2"]);
        t.sort(0, false);
        assert_eq!(t.rows[1], ["a", "10"]);
        assert_eq!(t.move_row(1, true), None, "not above the header");
        assert_eq!(t.move_row(1, false), Some(2));
        assert!(!t.delete_row(0));
        assert_eq!(t.move_column(0, true), None);
        t.transpose();
        assert_eq!(t.rows, [["h", "b", "a"], ["i", "2", "10"]]);
        assert_eq!(
            Table::parse(&["| a, b | \"q\" |"]).csv(),
            "\"a, b\",\"\"\"q\"\"\"\n"
        );
    }
}
