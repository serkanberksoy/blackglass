//! Advanced Tables: edit a Markdown table as a table. After the Advanced
//! Tables community plugin (`requirements/tables_requirements.md`).
//!
//! While the cursor is on a table line, Tab and Shift+Tab go to the next
//! and previous cell and Enter to the next row (a new row at the end;
//! Enter on an empty last row leaves the table), and every move lines the
//! table up. Palette commands insert, delete and move rows and columns,
//! align, sort, transpose, export CSV and evaluate `TBLFM` formulas. Each
//! change replaces the table's lines: one undo step.

mod formula;
mod table;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::settings::{Kind, Setting, Values};
use super::{Context, Effect, Manifest, Plugin, PluginCommand};
use crate::vault::Vault;
use table::{Align, Table, cell_at, cell_end};

/// The plugin's id.
const ID: &str = "tables";

pub struct Tables {
    /// Tab and Shift+Tab move between cells.
    bind_tab: bool,
    /// Enter goes to the next row.
    bind_enter: bool,
    /// Cells padded to their column's width.
    pad: bool,
    /// The Table tab in the sidebar (the controls).
    controls: bool,
}

impl Tables {
    pub fn new() -> Self {
        Tables {
            bind_tab: true,
            bind_enter: true,
            pad: true,
            controls: true,
        }
    }
}

impl Default for Tables {
    fn default() -> Self {
        Tables::new()
    }
}

/// The table at the cursor: its lines' range in the note, the table, and
/// the cursor's row and column in it.
struct AtCursor {
    from: usize,
    to: usize,
    table: Table,
    row: usize,
    col: usize,
}

fn at_cursor(ctx: &Context) -> Option<AtCursor> {
    let note = ctx.note?;
    let lines: Vec<&str> = note.text.lines().collect();
    let range = table::find(&lines, note.row)?;
    let table = Table::parse(&lines[range.clone()]);
    let row = table.row_of_line(note.row - range.start);
    let col = cell_at(lines[note.row], note.col).min(table.columns() - 1);
    Some(AtCursor {
        from: range.start,
        to: range.end,
        table,
        row,
        col,
    })
}

impl AtCursor {
    /// Writes the table back, the cursor at the end of cell (`row`,
    /// `col`)'s text; `after` lines follow it.
    fn write(self, pad: bool, row: usize, col: usize, after: Vec<String>) -> Effect {
        let mut lines = self.table.format(pad);
        let line = Table::line_of_row(row.min(self.table.rows.len() - 1));
        let col = cell_end(&lines[line], col.min(self.table.columns() - 1));
        lines.extend(after);
        Effect::ReplaceLines {
            from: self.from,
            to: self.to,
            lines,
            cursor: (self.from + line, col),
        }
    }
}

impl AtCursor {
    /// Writes the table back and puts the cursor on the (blank) line after
    /// it, adding one if there's none.
    fn leave(self, pad: bool, ctx: &Context) -> Effect {
        let blank_after = ctx
            .note
            .and_then(|n| n.text.lines().nth(self.to))
            .is_some_and(|l| l.trim().is_empty());
        let mut lines = self.table.format(pad);
        let out = self.from + lines.len();
        if !blank_after {
            lines.push(String::new());
        }
        Effect::ReplaceLines {
            from: self.from,
            to: self.to,
            lines,
            cursor: (out, 0),
        }
    }
}

const NOT_IN_TABLE: &str = "Advanced Tables: the cursor is not in a table";

impl Plugin for Tables {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Advanced Tables",
            version: "0.1.0",
            author: "blackglass, after Tony Grosinger's Advanced Tables",
            description: "Edit tables as tables: Tab, Shift+Tab and Enter move between cells and \
                          line the table up; commands insert, delete, move, align and sort rows \
                          and columns, transpose, export CSV and evaluate TBLFM formulas.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        let on = |key: &str| values.get("", key).is_none_or(|v| v != "false");
        self.bind_tab = on("bind_tab");
        self.bind_enter = on("bind_enter");
        self.pad = on("pad");
        self.controls = on("controls");
    }

    fn settings(&self) -> Vec<Setting> {
        let toggle = |key: &str, label: &str, help: &str| {
            Setting::new("", key, label, help, Kind::Toggle, "true")
        };
        vec![
            toggle(
                "bind_tab",
                "Tab moves between cells",
                "In a table, Tab and Shift+Tab go to the next and previous cell",
            ),
            toggle(
                "bind_enter",
                "Enter goes to the next row",
                "In a table, Enter goes to the same column in the next row",
            ),
            toggle(
                "pad",
                "Pad cells with spaces",
                "Line the columns up; off: one space around each cell",
            ),
            toggle(
                "controls",
                "Table controls",
                "A Table tab in the sidebar: the table operations, for the table at the cursor",
            ),
        ]
    }

    fn commands(&self) -> Vec<PluginCommand> {
        let mut all: Vec<PluginCommand> = COMMANDS
            .iter()
            .map(|&(id, name)| PluginCommand::new(id, name))
            .collect();
        if self.controls {
            all.push(PluginCommand::new("open-controls", "Open table controls"));
        }
        all
    }

    fn sidebar_tab(&self) -> Option<&'static str> {
        self.controls.then_some("Table")
    }

    fn sidebar_hint(&self) -> &'static str {
        "⏎ run on the table at the cursor"
    }

    fn sidebar_rows(&self, _ctx: &Context) -> Vec<super::SidebarRow> {
        CONTROLS
            .iter()
            .map(|id| super::SidebarRow {
                label: COMMANDS
                    .iter()
                    .find(|(c, _)| c == id)
                    .map_or("", |(_, n)| n)
                    .to_string(),
                detail: String::new(),
            })
            .collect()
    }

    fn sidebar_key(&mut self, row: usize, key: KeyEvent, ctx: &Context) -> Effect {
        match (key.code, CONTROLS.get(row)) {
            (KeyCode::Enter, Some(id)) => self.run(id, ctx),
            _ => Effect::None,
        }
    }

    fn takes_key(&self, key: &KeyEvent) -> bool {
        let plain = key.modifiers.difference(KeyModifiers::SHIFT).is_empty();
        plain
            && match key.code {
                KeyCode::Tab | KeyCode::BackTab => self.bind_tab,
                KeyCode::Enter => self.bind_enter && key.modifiers.is_empty(),
                _ => false,
            }
    }

    fn editor_key(&mut self, key: KeyEvent, ctx: &Context) -> Option<Effect> {
        let mut at = at_cursor(ctx)?;
        let (rows, cols) = (at.table.rows.len(), at.table.columns());
        let (row, col) = (at.row, at.col);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let (row, col) = match key.code {
            KeyCode::BackTab | KeyCode::Tab if shift => match (row, col) {
                (r, c) if c > 0 => (r, c - 1),
                (r, _) if r > 0 => (r - 1, cols - 1),
                _ => (0, 0),
            },
            KeyCode::Tab if col + 1 < cols => (row, col + 1),
            KeyCode::Tab if row + 1 < rows => (row + 1, 0),
            KeyCode::Tab => {
                at.table.insert_row(rows);
                (rows, 0)
            }
            // A header line alone: the separator row and a first row.
            KeyCode::Enter if !at.table.separator && rows == 1 => {
                at.table.insert_row(1);
                (1, 0)
            }
            KeyCode::Enter
                if row + 1 == rows
                    && row > 0
                    && at.table.rows[row].iter().all(String::is_empty) =>
            {
                at.table.delete_row(row);
                return Some(at.leave(self.pad, ctx));
            }
            KeyCode::Enter if row + 1 < rows => (row + 1, col),
            KeyCode::Enter => {
                at.table.insert_row(rows);
                (rows, col)
            }
            _ => return None,
        };
        Some(at.write(self.pad, row, col, Vec::new()))
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        match id {
            "open-controls" => return Effect::ShowSidebarTab,
            "format-all" => return self.format_all(ctx),
            "next-cell" | "previous-cell" => {
                let key = if id == "next-cell" {
                    KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)
                } else {
                    KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)
                };
                return self
                    .editor_key(key, ctx)
                    .unwrap_or_else(|| Effect::Message(NOT_IN_TABLE.into()));
            }
            _ => {}
        }
        // On a formula line, the table above it.
        let moved;
        let ctx = match (id, formula_line_table(ctx)) {
            ("evaluate-formulas", Some(row)) => {
                moved = Context {
                    note: ctx.note.map(|n| super::ActiveNote { row, col: 0, ..n }),
                    ..*ctx
                };
                &moved
            }
            _ => ctx,
        };
        let Some(mut at) = at_cursor(ctx) else {
            return Effect::Message(NOT_IN_TABLE.into());
        };
        let (row, col) = (at.row, at.col);
        let t = &mut at.table;
        let (row, col) = match id {
            "format" => (row, col),
            "insert-row" => {
                t.insert_row(row);
                (row.max(1), col)
            }
            "insert-column" => {
                t.insert_column(col);
                (row, col)
            }
            "delete-row" => {
                if !t.delete_row(row) {
                    return Effect::Message("Advanced Tables: the header row stays".into());
                }
                (row.min(t.rows.len() - 1), col)
            }
            "delete-column" => {
                if !t.delete_column(col) {
                    return Effect::Message("Advanced Tables: a table needs a column".into());
                }
                (row, col.min(t.columns() - 1))
            }
            "move-row-up" | "move-row-down" => {
                let to = t.move_row(row, id == "move-row-up");
                (to.unwrap_or(row), col)
            }
            "move-column-left" | "move-column-right" => {
                let to = t.move_column(col, id == "move-column-left");
                (row, to.unwrap_or(col))
            }
            "align-left" | "align-center" | "align-right" | "align-none" => {
                t.aligns[col] = match id {
                    "align-left" => Align::Left,
                    "align-center" => Align::Center,
                    "align-right" => Align::Right,
                    _ => Align::None,
                };
                (row, col)
            }
            "sort-ascending" | "sort-descending" => {
                t.sort(col, id == "sort-descending");
                (row, col)
            }
            "transpose" => {
                t.transpose();
                (col, row)
            }
            "export-csv" => return export_csv(t, ctx),
            "evaluate-formulas" => {
                let lines: Vec<&str> = ctx
                    .note
                    .map(|n| n.text.lines().collect())
                    .unwrap_or_default();
                let formulas = formula::formulas(lines[at.to..].iter().copied());
                if formulas.is_empty() {
                    return Effect::Message(
                        "Advanced Tables: no <!-- TBLFM: … --> line under the table".into(),
                    );
                }
                if let Err(e) = formula::evaluate(t, &formulas) {
                    return Effect::Message(format!("Advanced Tables: {e}"));
                }
                (row, col)
            }
            "move-out" => return at.leave(self.pad, ctx),
            _ => return Effect::None,
        };
        at.write(self.pad, row, col, Vec::new())
    }
}

impl Tables {
    /// "Format all tables": every table in the note lined up.
    fn format_all(&self, ctx: &Context) -> Effect {
        let Some(note) = ctx.note else {
            return Effect::Message("Open a note first".into());
        };
        let lines: Vec<&str> = note.text.lines().collect();
        let tables = table::find_all(&lines);
        if tables.is_empty() {
            return Effect::Message("Advanced Tables: this note has no tables".into());
        }
        let mut out: Vec<String> = Vec::new();
        let mut at = 0;
        let mut row = note.row;
        for range in tables {
            out.extend(lines[at..range.start].iter().map(|l| l.to_string()));
            let written = Table::parse(&lines[range.clone()]).format(self.pad);
            if range.end <= note.row {
                row = (row + written.len()).saturating_sub(range.len());
            }
            out.extend(written);
            at = range.end;
        }
        out.extend(lines[at..].iter().map(|l| l.to_string()));
        Effect::ReplaceLines {
            from: 0,
            to: lines.len(),
            cursor: (row.min(out.len().saturating_sub(1)), 0),
            lines: out,
        }
    }
}

/// The last line of the table above the cursor's `TBLFM` line, if the
/// cursor is on one.
fn formula_line_table(ctx: &Context) -> Option<usize> {
    let note = ctx.note?;
    let lines: Vec<&str> = note.text.lines().collect();
    let is_formula = |l: &str| !formula::formulas([l]).is_empty();
    if !is_formula(lines.get(note.row)?) {
        return None;
    }
    let above = (0..note.row).rev().find(|&i| !is_formula(lines[i]))?;
    table::is_table_line(lines[above]).then_some(above)
}

/// Writes the table as `<note>.csv` beside the note.
fn export_csv(t: &Table, ctx: &Context) -> Effect {
    let Some(path) = ctx.note.and_then(|n| n.path) else {
        return Effect::Message("Advanced Tables: save the note first".into());
    };
    let file = path.with_extension("csv");
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match mdedit::files::write_atomic(&file, &t.csv()) {
        Ok(()) => Effect::FilesChanged(format!("Advanced Tables: the table is in {name}")),
        Err(e) => Effect::Message(format!("Advanced Tables: cannot write {name}: {e}")),
    }
}

/// The table controls (the Table tab), in order: the commands that work
/// on the table at the cursor.
const CONTROLS: [&str; 18] = [
    "format",
    "insert-row",
    "insert-column",
    "delete-row",
    "delete-column",
    "move-row-up",
    "move-row-down",
    "move-column-left",
    "move-column-right",
    "align-left",
    "align-center",
    "align-right",
    "sort-ascending",
    "sort-descending",
    "transpose",
    "evaluate-formulas",
    "export-csv",
    "format-all",
];

/// The commands, in palette order: (id, name).
const COMMANDS: [(&str, &str); 22] = [
    ("format", "Format table"),
    ("format-all", "Format all tables in this note"),
    ("insert-row", "Insert row before"),
    ("insert-column", "Insert column before"),
    ("delete-row", "Delete row"),
    ("delete-column", "Delete column"),
    ("move-row-up", "Move row up"),
    ("move-row-down", "Move row down"),
    ("move-column-left", "Move column left"),
    ("move-column-right", "Move column right"),
    ("align-left", "Align column left"),
    ("align-center", "Align column center"),
    ("align-right", "Align column right"),
    ("align-none", "Align column: none"),
    ("sort-ascending", "Sort rows ascending"),
    ("sort-descending", "Sort rows descending"),
    ("transpose", "Transpose"),
    ("export-csv", "Export table as CSV"),
    ("evaluate-formulas", "Evaluate table formulas"),
    ("move-out", "Move cursor out of table"),
    ("next-cell", "Next cell"),
    ("previous-cell", "Previous cell"),
];
