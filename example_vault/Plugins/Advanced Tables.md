# Advanced Tables

With the **Advanced Tables** plugin, put the cursor in a table: **Tab** and
**Shift+Tab** go from cell to cell, **Enter** to the next row, and the
table lines itself up. Ctrl+P → "Advanced Tables" lists the rest: insert,
delete and move rows and columns, align, sort, transpose, export as CSV; the
same operations are in the sidebar's Table tab ("Open table controls").

| Fruit   | Qty | Price | Total |
| ------- | --: | ----: | ----: |
| apple   |   4 |   0.5 |  2.00 |
| kiwi    |  12 |  0.25 |  3.00 |
| fig     |   4 |   1.2 |  4.80 |
| **Sum** |  20 |       |  9.80 |
<!-- TBLFM: $4=($2*$3);%.2f -->
<!-- TBLFM: @>$4=sum(@I..@-1);%.2f::@>$2=sum(@I..@-1) -->

The `TBLFM` lines under a table fill its Total column and the sums by
themselves: change a quantity above and press Tab, or move out of the
table, and the totals follow. (The formula lines show only while the cursor
is on them.) Settings → Advanced Tables → Recalculate formulas: "on
command" leaves it to "Evaluate table formulas".

## Spreadsheet cells

A cell starting with `=` is a formula, shown as its result; put the cursor
in the table to see (and change) the formulas. `A1` is the header's first
cell, so the first row below the header is row 2.

| Expense   | Monthly | Months | Total             |
| --------- | ------: | -----: | ----------------: |
| Rent      |     900 |     12 | =B2*C2            |
| Food      |     350 |     12 | =B3*C3            |
| Transport |      80 |     12 | =B4*C4            |
| **Sum**   | =SUM(B2:B4) |    | =SUM(D2:D4)       |
| Average   | =ROUND(AVERAGE(B2:B4), 1) | | =IF(D5>15000, "over", "under") |

Functions: SUM, AVERAGE, MEDIAN, MIN, MAX, COUNT, COUNTA, PRODUCT, ABS,
ROUND, FLOOR, CEILING, TRUNC, INT, SQRT, POWER, MOD, IF. The file keeps
the formulas (other apps show them as written); a `TBLFM` line keeps the
numbers instead.

Times and durations: `;hm` shows hours:minutes, `;dt` a date and time.

| Task | Start | End | Took |
|---|---|---|---|
| Write | 2026-10-01 09:00 | 2026-10-01 11:30 | |
| Review | 2026-10-01 13:15 | 2026-10-01 14:05 | |
| **Total** | | | |
<!-- TBLFM: $4=($3-$2);hm::@>$4=sum(@I..@-1);hm -->

Start a new table by typing a header line and pressing Enter:

| Name | Role |
