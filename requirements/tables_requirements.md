# Advanced Tables: what blackglass needs

blackglass's Advanced Tables plugin (id `tables`, built in 0.47.0 … 0.49.0
as W-90 … W-92) against the
[Advanced Tables](https://github.com/tgrosinger/advanced-tables-obsidian)
community plugin (its README, help page, source (`main.ts`, the table
controls view) and the formula guide of `md-advanced-tables`, read
2026-09-30). Each piece has an ID (`AT-xx`), a rough effort (S / M / L)
and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

**Already there (the editor's, mdedit):** tables are drawn with box
borders, a bold header, columns as wide as their widest cell, the
separator row's alignment (`:---`, `:---:`, `---:`), and `[[Note\|alias]]`
in cells (mdedit B-11 … B-13); the line being edited is shown raw. The
palette has "Insert table". What's missing is editing a table as a table.

It follows the plugin rules: code in `src/plugins/tables/`, settings in
`.blackglass/plugins/tables/settings.toml`. Every operation is a change
to the note's text (one undo step, like Templater's replace), so it needs
no new mdedit API; the keys (Tab / Enter in a table) are taken by
blackglass before the editor only while the cursor is on a table line.

## 1. Formatting

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| AT-01 | Format the table at the cursor: cells padded with spaces to the column's width, the separator row made of dashes (alignment colons kept), missing cells added, the pipes lined up | ✅ | M | 0.47.0; Width in terminal columns (wide characters, emoji); escaped `\|` stays in its cell |
| AT-02 | Format as you edit: after leaving a cell (Tab, Enter, moving off the row) | ✅ | S | 0.47.0: on Tab, Shift+Tab and Enter; A setting |
| AT-03 | "Format all tables in this file" | ✅ | S | 0.47.0 |
| AT-04 | "Pad cell width using spaces" setting (off: minimal `\| a \| b \|`) | ✅ | S | 0.47.0 |
| AT-05 | A table starts from a header line: typing `\| a \| b \|` and Enter adds the separator row and a new row | ✅ | S | 0.47.0 |

## 2. Moving between cells

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| AT-10 | Tab: the next cell (at the end of a row, the next row's first; at the table's end, a new row); the table is formatted | ✅ | M | 0.47.0; Settings: "Bind Tab to table navigation" (on); list items keep Tab for indenting |
| AT-11 | Shift+Tab: the previous cell | ✅ | S | 0.47.0 |
| AT-12 | Enter: the same column in the next row (a new row at the end) | ✅ | S | 0.47.0; Setting: "Bind Enter to table navigation" (on); on an empty last row, Enter leaves the table |
| AT-13 | "Move cursor out of table" (to the line after it) | ✅ | S | 0.47.0 |

## 3. Rows and columns

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| AT-20 | Insert a row before (above) the cursor's; insert a column before (left of) it | ✅ | S | 0.48.0 |
| AT-21 | Delete the row; delete the column | ✅ | S | 0.48.0; Not the header row |
| AT-22 | Move the row up / down; move the column left / right | ✅ | S | 0.48.0; The header row stays first |
| AT-23 | Align the column left / center / right (the separator row's colons) | ✅ | S | 0.48.0 |
| AT-24 | Sort the rows by the cursor's column, ascending / descending (numbers as numbers) | ✅ | S | 0.48.0 |
| AT-25 | Transpose (rows become columns) | ✅ | S | 0.48.0 |
| AT-26 | Export the table as CSV (a file next to the note, or copied) | ✅ | S | 0.48.0: `<note>.csv` beside the note; Quotes and commas escaped |

## 4. Formulas

Written under the table as `<!-- TBLFM: … -->`, one or more lines (or
several formulas joined with `::`), run in order, each on the previous
one's result; "Evaluate table formulas" writes the results into the
cells.

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| AT-30 | `DESTINATION=SOURCE`; references `@row`, `$col`, `@row$col`; `@<` / `@>` first / last row, `$<` / `$>` first / last column, `@I` the first row below the header, relative `@-1`, `$+2` | ✅ | M | 0.49.0 |
| AT-31 | Ranges `@2..@5`, `@2$3..@5$5` (a column carried over from the first term) | ✅ | S | 0.49.0 |
| AT-32 | Arithmetic in parentheses: `+ - * /` (with the upstream rules on ranges and single cells) | ✅ | S | 0.49.0 … 0.67.0: decimal results, text as 0, a range with one cell (`(@2..@4*2)`) into a range destination (`@2$3..@4$3=`) |
| AT-33 | Functions `sum(…)`, `mean(…)` over a range, row or column | ✅ | S | 0.49.0 |
| AT-34 | `if(a < b, x, y)` with `< > <= >= == !=` on cells | ✅ | S | 0.49.0 |
| AT-35 | Formats: `;%.2f` decimals, `;dt` date-time, `;hm` hours:minutes; dates (`2022-12-31 23:59`) and durations (`23:59`) can be subtracted | ✅ | S | 0.63.3: `;%.Nf`, `;dt` (`YYYY-MM-DD HH:mm`), `;hm` (`HH:MM`); dates and `H:MM` durations are milliseconds, so they subtract |
| AT-36 | Nesting (`sum(@3..@4)+@3$1`), chaining with `::` and several TBLFM lines | ✅ | S | 0.49.0 |
| AT-37 | Errors shown in the status bar, the table unchanged | ✅ | S | 0.49.0 |
| AT-38 | The TBLFM comment line hidden in the live preview (shown while editing it) | ✅ | S | 0.88.0: mdedit 3.16.0's hidden lines (`markdown::set_hidden_lines`); the cursor still moves onto it; `workspace::a_tables_formula_line_hides_until_the_cursor_is_on_it` |
| AT-39 | Formulas worked out by themselves: when the table changes (Tab, Enter, a table command) and when the cursor leaves it; or only by command (a setting) | ✅ | S | 0.90.0, beyond the original (which works them out by command only), after org-mode's automatic rows; `workspace::table_formulas_recalculate_by_themselves`, `workspace::table_formulas_can_wait_for_their_command` |
| AT-44 | Spreadsheet cells: a cell starting with `=` (`=B2*C2`, `=SUM(D2:D4)`, `=IF(A2>3, "high", "low")`) shows its result in the live preview and view mode, the formula while the cursor is in the table; A1 is the header's first cell (as the Table Calc plugin); SUM, AVERAGE, MEDIAN, MIN, MAX, COUNT, COUNTA, PRODUCT, ABS, ROUND, FLOOR, CEILING, TRUNC, INT, SQRT, POWER, MOD, IF; `#ERR`, `#NAME?` | ✅ | M | 0.91.0, beyond the original (after Table Calc, CalcCraft): mdedit 3.18.0's table cells; the file keeps the formulas; `tables::cells::tests`, `workspace::table_cells_starting_with_equals_show_their_results` |

## 5. Commands and controls

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| AT-40 | Every operation as a palette command ("Advanced Tables: …"): format, format all, insert / delete / move row and column, align, sort, transpose, evaluate formulas, export CSV, move out of table | ✅ | S | 0.47.0 … 0.49.0: 22 commands; Keys can be set (W-62) |
| AT-41 | Table controls: a panel at the bottom of the sidebar (like the calendar) with the operations as buttons, shown while the cursor is in a table (the upstream "table controls toolbar") | ✅ | M | 0.67.0: a Table tab in the sidebar: the operations, run on the table at the editor's cursor (Enter), the focus kept for the next; "Open table controls"; a setting |
| AT-42 | Settings: bind Tab, bind Enter, pad with spaces, format as you edit, show the controls | ✅ | S | 0.47.0 … 0.67.0: Tab, Enter, padding, the controls |
| AT-43 | Mobile toolbar | ✗ | – | No mobile app |

## 6. Suggested order

1. **Format and move** (M): AT-01 … AT-05, AT-10 … AT-13, AT-40, AT-42.
2. **Rows and columns** (S): AT-20 … AT-26, AT-41.
3. **Formulas** (M): AT-30 … AT-38.

## 7. Checked against

- [Advanced Tables README](https://github.com/tgrosinger/advanced-tables-obsidian),
  [help](https://github.com/tgrosinger/advanced-tables-obsidian/blob/main/docs/help.md),
  its source: [`main.ts`](https://github.com/tgrosinger/advanced-tables-obsidian/blob/main/src/main.ts)
  (commands, settings, keys) and
  [`table-controls-view.ts`](https://github.com/tgrosinger/advanced-tables-obsidian/blob/main/src/table-controls-view.ts);
  [formulas](https://github.com/tgrosinger/md-advanced-tables/blob/main/docs/formulas.md)
  (2026-09-30).
