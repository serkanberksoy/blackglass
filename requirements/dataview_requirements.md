# Dataview: what blackglass has and what's missing

A comparison of blackglass's Dataview plugin (W-42 … W-44) with
[Obsidian Dataview](https://blacksmithgu.github.io/obsidian-dataview/)
(its documentation, read 2026-09-29). Each piece is a requirement with an
ID (`DV-xx`). All of them are done as of blackglass 0.77.0; the tests are
in `src/plugins/dataview/` and `tests/workspace.rs`.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## 1. Query types

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| DV-01 | `LIST` with an optional value per page | ✅ | |
| DV-02 | `TABLE` with columns and `AS "name"` | ✅ | |
| DV-03 | `TABLE WITHOUT ID` | ✅ | |
| DV-04 | `LIST WITHOUT ID` (values only) | ✅ | `eval::sources_of_the_current_note_and_without_id` |
| DV-05 | `TASK` grouped by page | ✅ | |
| DV-06 | `TASK` rows can be checked off from the result | ✅ | `workspace::dataview_tasks_are_checked_off_from_results`: Enter or a click toggles the task in its note (with ✅ date when completion tracking is on); a setting opens the task instead |
| DV-07 | `CALENDAR date-field`: a month grid with a dot per page | ✅ | `eval::calendars_put_pages_on_their_days`, `render::a_calendar_month`: each month with a `•` on days, and the day's notes below (clickable) |

## 2. Data commands

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| DV-10 | `FROM` tags (with sub-tags), folders, single files | ✅ | |
| DV-11 | `FROM [[Note]]` (incoming links) | ✅ | |
| DV-12 | `FROM outgoing([[Note]])` | ✅ | `eval::sources_of_the_current_note_and_without_id` |
| DV-13 | `FROM [[]]` / `[[#]]`: the current note | ✅ | same |
| DV-14 | `and`, `or`, `-` / `!`, parentheses in `FROM` | ✅ | |
| DV-15 | `WHERE` (repeatable) | ✅ | |
| DV-16 | `SORT` with several keys, `ASC` / `DESC` | ✅ | |
| DV-17 | `LIMIT` | ✅ | |
| DV-18 | Commands in any order, run in the order written | ✅ | `eval::commands_run_in_the_order_written` |
| DV-19 | `GROUP BY expr [AS name]` with `rows` and swizzling (`rows.file.link`) | ✅ | `eval::groups_and_flattening` |
| DV-20 | `FLATTEN expr [AS name]`: one row per list entry | ✅ | same |

## 3. Expressions

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| DV-21 | Numbers, text, booleans, `null`, links, `date(...)` | ✅ | text escapes: `query::texts_with_escapes` |
| DV-22 | `+ - * /`, comparisons, `and` / `or` / `!` | ✅ | |
| DV-23 | `%` (modulo) | ✅ | `eval::the_expression_language` |
| DV-24 | Durations: `dur(1 day)`, `date(today) - dur(1 month)`, duration values | ✅ | `value::durations`, `eval::durations_and_dates` |
| DV-25 | List literals `[1, 2]` and object literals `{a: 1}` | ✅ | `eval::the_expression_language` (`[[` is always a link, as in Dataview) |
| DV-26 | Indexing: `list[0]`, `object["key"]`, `object.key` on values | ✅ | same |
| DV-27 | Link indexing: `[[Note]].field` | ✅ | same |
| DV-28 | Lambdas `(x) => expr` for `map`, `filter`, `reduce`, `any`, `all` | ✅ | same |
| DV-29 | `this` (the current note) | ✅ | |

## 4. Functions

| ID | Functions | Now | Test |
|----|-----------|-----|------|
| DV-30 | Constructors: `object`, `list`, `link`, `embed`, `elink`, `typeof`, `dur`, `date(text, format)` | ✅ | `eval::text_number_and_list_functions` |
| DV-31 | Numeric: `trunc`, `floor`, `ceil`, `min`, `max`, `product`, `average`, `minby`, `maxby`, `reduce` | ✅ | same |
| DV-32 | Lists: `econtains`, `containsword`, `extract`, `sort`, `reverse`, `nonnull`, `firstvalue`, `all`, `any`, `none`, `join`, `filter`, `unique`, `map`, `flat`, `slice` | ✅ | same |
| DV-33 | Text: `regextest`, `regexmatch`, `regexreplace`, `replace`, `split`, `padleft`, `padright`, `substring`, `truncate` | ✅ | same |
| DV-34 | Utility: `display`, `hash`, `striptime`, `durationformat`, `currencyformat`, `localtime`, `meta` | ✅ | `eval::durations_and_dates` |

## 5. Data (fields)

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| DV-35 | Frontmatter fields, inline `key:: value`, `[key:: v]`, `(key:: v)` | ✅ | |
| DV-36 | `file.name`, `path`, `folder`, `ext`, `link`, `size`, `ctime`, `cday`, `mtime`, `mday`, `day`, `tags`, `etags`, `inlinks`, `outlinks`, `tasks` | ✅ | `file.tags` has the parents of nested tags |
| DV-37 | `file.aliases`, `file.lists`, `file.frontmatter`, `file.starred` | ✅ | `eval::list_items_task_fields_and_file_data` (`starred`: in Bookmarks) |
| DV-38 | Task fields: `status`, `completed`, `text`, `line` | ✅ | |
| DV-39 | Task fields: `checked`, `fullyCompleted`, `children`, `parent`, `section`, `tags`, `outlinks`, `path`, `link`, `blockId`, `annotated`, `lineCount`, `visual`; list items | ✅ | same |
| DV-41 | Task emoji dates: ✅ completion, 📅 due, ➕ created, 🛫 start, ⏳ scheduled; `[due:: …]` on tasks | ✅ | same |
| DV-42 | `file.day` from `yyyymmdd` names | ✅ | same |

## 6. Beyond DQL blocks

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| DV-40 | Clickable results: open a page from a result | ✅ | blackglass 0.9.0 |
| DV-43 | Inline queries: `` `= this.file.name` `` shows one value in the text | ✅ | `workspace::dataview_inline_queries_show_their_values` (mdedit's rendered spans) |
| DV-44 | Settings: date format, date-time format, "No results" text, page column header | ✅ | also task clicks and completion tracking |
| DV-45 | Results follow edits live (before saving) | ✅ | `workspace::dataview_results_follow_unsaved_edits` |
| DV-46 | DataviewJS (```` ```dataviewjs ````) | ✅ | blackglass 0.8.0 (W-55) |
| DV-47 | Inline JS (`` `$= …` ``), `dv.io`, `dv.view`, Luxon dates, `dv.markdownTable` …, swizzling (`pages.file.name`) | ✅ | `js::swizzling_files_views_dates_and_markdown`. `dv.io` and `dv.view` only read files inside the vault; Luxon is a shim with the common calls |
