# Tables

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

Put the cursor in the table above and run "Evaluate table formulas": the
`TBLFM` lines under it fill the Total column and the sums.

Times and durations: `;hm` shows hours:minutes, `;dt` a date and time.

| Task | Start | End | Took |
|---|---|---|---|
| Write | 2026-10-01 09:00 | 2026-10-01 11:30 | |
| Review | 2026-10-01 13:15 | 2026-10-01 14:05 | |
| **Total** | | | |
<!-- TBLFM: $4=($3-$2);hm::@>$4=sum(@I..@-1);hm -->

Start a new table by typing a header line and pressing Enter:

| Name | Role |
