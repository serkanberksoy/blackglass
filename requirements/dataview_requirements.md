# Dataview: what blackglass has and what's missing

A comparison of blackglass's Dataview plugin (W-42 … W-44) with
[Obsidian Dataview](https://blacksmithgu.github.io/obsidian-dataview/)
(its documentation, read 2026-09-29). Each missing piece is a requirement
with an ID (`DV-xx`), a rough effort (S / M / L) and a milestone.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## 1. Query types

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| DV-01 | `LIST` with an optional value per page | ✅ | – | |
| DV-02 | `TABLE` with columns and `AS "name"` | ✅ | – | |
| DV-03 | `TABLE WITHOUT ID` | ✅ | – | |
| DV-04 | `LIST WITHOUT ID` (values only) | ⬜ | S | Parser accepts `WITHOUT ID` only after `TABLE` |
| DV-05 | `TASK` grouped by page | ✅ | – | |
| DV-06 | `TASK` rows can be checked off from the result | ⬜ | M | Needs clickable results (DV-40) and a write to the task's line |
| DV-07 | `CALENDAR date-field`: a month grid with a dot per page | ⬜ | M | Drawable in text: a month view with `•` on days |

## 2. Data commands

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| DV-10 | `FROM` tags (with sub-tags), folders, single files | ✅ | – | |
| DV-11 | `FROM [[Note]]` (incoming links) | ✅ | – | |
| DV-12 | `FROM outgoing([[Note]])` | ⬜ | S | Index already has outlinks |
| DV-13 | `FROM [[]]` / `[[#]]`: the current note | ⬜ | S | `this` is already known |
| DV-14 | `and`, `or`, `-` / `!`, parentheses in `FROM` | ✅ | – | |
| DV-15 | `WHERE` (repeatable) | ✅ | – | |
| DV-16 | `SORT` with several keys, `ASC` / `DESC` | ✅ | – | |
| DV-17 | `LIMIT` | ✅ | – | |
| DV-18 | Commands in any order, run in the order written (e.g. `LIMIT` before `WHERE`) | 🟡 | M | Today: FROM, then all WHERE, SORT, LIMIT in that order |
| DV-19 | `GROUP BY expr [AS name]` with `rows` and field swizzling (`rows.file.link`) | ⬜ | L | Results become groups; tables show `rows.x` as lists |
| DV-20 | `FLATTEN expr [AS name]`: one row per list entry | ⬜ | M | |

## 3. Expressions

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| DV-21 | Numbers, text, booleans, `null`, links, `date(...)` | ✅ | – | |
| DV-22 | `+ - * /`, comparisons, `and` / `or` / `!` | ✅ | – | |
| DV-23 | `%` (modulo) | ⬜ | S | |
| DV-24 | Durations: `dur(1 day)`, `date(today) - dur(1 month)`, duration values in results | ⬜ | M | Today: date ± number of days, date − date = days |
| DV-25 | List literals `[1, 2]` and object literals `{a: 1}` | ⬜ | S | |
| DV-26 | Indexing: `list[0]`, `object["key"]`, `object.key` on values | ⬜ | M | |
| DV-27 | Link indexing: `[[Note]].field` (another page's field) | ⬜ | S | |
| DV-28 | Lambdas `(x) => expr` for `map`, `filter`, `reduce`, `any`, `all` | ⬜ | M | |
| DV-29 | `this` (the current note) | ✅ | – | |

## 4. Functions

Have: `date`, `contains`, `icontains`, `length`, `lower`, `upper`,
`default`, `choice`, `round`, `number`, `string`, `startswith`, `endswith`,
`sum`, `dateformat`.

| ID | Functions | Now | Effort |
|----|-----------|-----|--------|
| DV-30 | Constructors: `object`, `list`, `link`, `embed`, `elink`, `typeof`, `dur`, `date(text, format)` | ⬜ | S |
| DV-31 | Numeric: `trunc`, `floor`, `ceil`, `min`, `max`, `product`, `average`, `minby`, `maxby`, `reduce` | ⬜ | S (M with lambdas) |
| DV-32 | Lists: `econtains`, `containsword`, `extract`, `sort`, `reverse`, `nonnull`, `firstvalue`, `all`, `any`, `none`, `join`, `filter`, `unique`, `map`, `flat`, `slice` | ⬜ | M (needs DV-28) |
| DV-33 | Text: `regextest`, `regexmatch`, `regexreplace`, `replace`, `split`, `padleft`, `padright`, `substring`, `truncate` | ⬜ | S (regex needs the `regex` crate) |
| DV-34 | Utility: `display`, `hash`, `striptime`, `durationformat`, `currencyformat`, `localtime`, `meta` | ⬜ | S |

## 5. Data (fields)

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| DV-35 | Frontmatter fields, inline `key:: value`, `[key:: v]`, `(key:: v)` | ✅ | – | |
| DV-36 | `file.name`, `path`, `folder`, `ext`, `link`, `size`, `ctime`, `cday`, `mtime`, `mday`, `day`, `tags`, `etags`, `inlinks`, `outlinks`, `tasks` | ✅ | – | `file.tags` doesn't split sub-tags (`#a/b` → also `#a`) |
| DV-37 | `file.aliases`, `file.lists`, `file.frontmatter`, `file.starred` | ⬜ | S | `starred` needs bookmarks (W-27) |
| DV-38 | Task fields: `status`, `completed`, `text`, `line` | ✅ | – | |
| DV-39 | Task fields: `checked`, `fullyCompleted`, `children`, `parent`, `section`, `tags`, `outlinks`, `path`, `link`, `blockId`, `annotated`, `lineCount`, `visual`; list items (not only tasks) | ⬜ | M | Needs list nesting in the index |
| DV-41 | Task emoji dates: ✅ completion, 📅 due, ➕ created, 🛫 start, ⏳ scheduled; `[due:: …]` on tasks | ⬜ | S | mdedit already styles the emoji (C-01) |
| DV-42 | `file.day` from `yyyymmdd` names | ⬜ | S | Today only `yyyy-mm-dd` |

## 6. Beyond DQL blocks

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| DV-40 | Clickable results: open a page from a result | ✅ | – | blackglass 0.9.0: Enter in view mode or a click (mdedit 3.0 row actions); a task opens at its line |
| DV-43 | Inline queries: `` `= this.file.name` `` shows one value in the text | ⬜ | M | Needs an inline-span hook in mdedit (like `CodeBlockProcessor`) |
| DV-44 | Settings: date format, date-time format, "No results" text, page column header | ✅ | – | blackglass 0.6.0; the inline prefix waits for inline queries (DV-43) |
| DV-45 | Results follow edits live (before saving) | ⬜ | M | Today: after Ctrl+S |
| DV-46 | DataviewJS (```` ```dataviewjs ````) | ✅ | – | blackglass 0.8.0 (W-55): `dv.pages / current / page / date / func.dateformat`, `dv.header / paragraph / span / el / list / table / taskList`, array helpers; sandboxed, and a setting turns it off |
| DV-47 | Inline JS (`` `$= …` ``), `dv.io`, `dv.view`, Luxon dates, `dv.markdownTable` …, swizzling (`pages.file.name`) | ⬜ | M | Inline JS needs the inline hook (DV-43) |

## 7. Suggested order

1. **Small, high value (S):** DV-04, DV-12, DV-13, DV-23, DV-25, DV-27,
   DV-30, DV-33, DV-41, DV-42.
2. **The common query shapes (M/L):** DV-19 `GROUP BY` and DV-20
   `FLATTEN` (many community queries use them), DV-24 durations, DV-28
   lambdas with DV-31 / DV-32.
3. **Interaction (M, needs mdedit APIs):** DV-40 clickable results, DV-06
   checking tasks, DV-43 inline queries.
