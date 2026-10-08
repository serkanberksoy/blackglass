# Tasks: what blackglass needs

blackglass's Tasks plugin (id `tasks`, built in 0.50.0 … 0.53.0 as W-78 …
W-81) against the
[Tasks](https://publish.obsidian.md/tasks/Introduction) community plugin
(its user guide, read 2026-09-30: task format, statuses, editing, queries
and the quick reference). Each piece has an ID (`TK-xx`), a rough effort
(S / M / L) and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

**What stays core (not the plugin):** checkboxes and task states are the
editor's (mdedit): `- [ ]` / `- [x]`, Ctrl+T creates or toggles, Ctrl+L
closes or reopens, numbered tasks, the states `[/]` `[>]` `[-]` `[!]` `[?]`
and any single character (mdedit L-04 … L-10, V-14), and Dataview's
`TASK` queries. The Tasks plugin adds what's below on top; with it off,
tasks work as now.

It follows the plugin rules: code in `src/plugins/tasks/`, settings in
`.blackglass/plugins/tasks/settings.toml`, ```` ```tasks ```` blocks
rendered through the code block processor, results through mdedit's row
actions. **Reuse:** the vault index's tasks (Dataview's `file.tasks`),
Periodic Notes' date parsing, `plugins::moment`.

## 1. The task format

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| TK-01 | Read a task's fields from its line (emoji format): created ➕, scheduled ⏳, start 🛫, due 📅, done ✅, cancelled ❌ (`YYYY-MM-DD`) | ✅ | S | 0.50.0; mdedit shows the emoji as text (C-01) |
| TK-02 | Priority: 🔺 highest, ⏫ high, 🔼 medium, (none) normal, 🔽 low, ⏬ lowest | ✅ | S | 0.50.0 |
| TK-03 | Recurrence 🔁: `every day`, `every 2 weeks`, `every month on the 15th`, `every weekday`, `when done` … | ✅ | M | 0.51.0: day, weekday, week (on days), month (on the Nth / last), year, every N, `when done`; not `every January on the 15th`; A recurrence parser (Tasks uses rrule's text) |
| TK-04 | On completion 🏁: `keep` or `delete` | ✅ | S | 0.51.0 |
| TK-05 | Dependencies: 🆔 id, ⛔ depends on (ids, comma-separated); a task is blocked while one it depends on is open, blocking while an open task depends on it | ✅ | M | 0.53.0 |
| TK-06 | The Dataview format as an alternative: `[due:: 2026-10-01]`, `[priority:: high]` … (a setting) | ✅ | S | 0.53.0: read always; written with the Task format setting |
| TK-07 | Global filter: only lines with a given tag (e.g. `#task`) are tasks; the tag hidden in results (a setting) | ✅ | S | 0.50.0: and hidden in results (a setting) |
| TK-08 | Invalid dates are kept and reported (`due date is invalid`) | ✅ | S | 0.53.0: kept, found with `due date is invalid` |
| TK-09 | Styling the fields in the note: dates, priority, recurrence as muted chips | ✅ | S | 1.5.0: each field (its emoji and value: dates, priority, recurrence, ID, depends on, on completion) in the muted color on a raised background, as written, through mdedit 3.22.0's `set_marks`; ✅ ❌ ➕ only with their date, as they're common in prose (`workspace::task_fields_in_a_note_are_muted_chips`) |

## 2. Statuses and toggling

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| TK-10 | Statuses with a name, a type (TODO, IN_PROGRESS, ON_HOLD, DONE, CANCELLED, NON_TASK) and the next status when toggled; the core ones and custom ones in the settings | ✅ | M | 0.51.0: custom ones in the settings, `symbol|name|next|TYPE`; The characters mdedit already shows |
| TK-50 | Default statuses for the characters mdedit shows, including `[.]` as a log (type NON_TASK), so `status.type is NON_TASK` finds logs without setting anything up | ✅ | S | 0.51.0; With TK-10 |
| TK-11 | Toggling done writes ✅ today (a setting); cancelling writes ❌ today; reopening removes them | ✅ | S | 0.51.0; mdedit R-08 is the editor's side of this; here as the plugin's toggle command |
| TK-12 | Completing a recurring task adds the next one (above or below, a setting), its dates moved from the done date or the due date per the rule | ✅ | M | 0.51.0: above (or below, a setting) |
| TK-13 | 🏁 `delete`: a completed task is removed | ✅ | S | 0.51.0 |
| TK-14 | Commands: "Tasks: Toggle task done", "Tasks: Create or edit task" | ✅ | S | 0.51.0 … 0.52.0: and "Postpone task"; Palette; keys can be set (W-62) |

## 3. Editing

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| TK-20 | Create or edit a task in a popup: description, priority, recurrence, the six dates (typed as `today`, `tomorrow`, `fri`, `in 2 weeks`, or a date), status, dependencies (before / after this task, chosen from the vault's tasks) | ✅ | L | 0.85.0: a window of every field at once (Alt+T): description (its tags too), status, priority, due, scheduled, start, recurs, created, done, cancelled, ID, depends on (IDs typed), on completion, each with its value or default (priority none …); dates in words, or on a calendar beside the window (0.89.0: Shift+arrows, PgUp / PgDn, a click; `workspace::task_dates_are_chosen_on_a_calendar`). 1.7.0: "Blocked by" and "Blocks" chosen from the vault's open tasks (type to find, Enter adds, Backspace takes the last off); a task without an ID gets one, written in its line (`workspace::task_dependencies_are_chosen_from_the_vaults_tasks`) |
| TK-21 | Suggestions while typing a task: the date emoji and a date, priorities, 🔁 rules, ids | ✅ | M | 0.66.0: on a task line, a typed word offers the fields (`du` → 📅 due date), a date's emoji offers dates, 🔁 offers rules; ↑/↓, Enter or Tab, Esc; was: Like `[[` link suggestions (W-23) |
| TK-22 | Postpone from a result: due (or scheduled) + 1 day, a week … | ✅ | S | 0.52.0: "Postpone task" on the cursor's task; 0.86.0: on the query result the cursor is on too (view mode), as is the task window (Alt+T); `workspace::a_tasks_result_is_postponed_and_edited_where_it_is` |

## 4. ```` ```tasks ```` queries

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| TK-30 | The block: one instruction per line, `#` comments, rendered in the note (the source while the cursor is in it) | ✅ | M | 0.50.0 |
| TK-31 | Status filters: `done`, `not done`, `status.name` / `status.type` is / includes / regex | ✅ | S | 0.50.0; 0.66.0: `regex matches /…/flags` too; was: 0.50.0: no regex |
| TK-49 | Filter by the status character itself: `status.symbol is .`, `status.symbol is not x` (blackglass's own; upstream needs `filter by function task.status.symbol === '.'`) | ✅ | S | 0.50.0; For logs written as `- [.]`; Dataview already does it: `TASK WHERE status = "."` |
| TK-32 | Date filters for done, created, starts, scheduled, due, cancelled and `happens` (the earliest of start, scheduled, due): on / before / after / on or before / on or after a date; `in` a range; `last` / `this` / `next` week, month, quarter, year; `YYYY-Www`, `YYYY-mm`, `YYYY-Qq`, `YYYY`; `has` / `no` date; `date is invalid` | ✅ | M | 0.50.0; Dates written as `today`, `tomorrow`, `next monday` too |
| TK-33 | `is recurring` / `is not recurring`, `recurrence includes …`; `priority is (above, below, not) …` | ✅ | S | 0.50.0 |
| TK-34 | File filters: `path`, `root`, `folder`, `filename`, `heading` includes / does not include / regex; `{{query.file.path}}` and the other placeholders; `preset this_file` | ✅ | S | 0.50.0, 0.66.0: includes / does not include / is, `regex matches` / `does not match`, placeholders, `preset this_file`; was: 0.50.0: includes / does not include / is, placeholders, `preset this_file`; no regex |
| TK-35 | `description` includes / regex; `has tags`, `no tags`, `tag includes …` | ✅ | S | 0.50.0, 0.66.0: with regular expressions; was: 0.50.0: no regex |
| TK-36 | Dependency filters: `has id`, `no id`, `id includes`, `has depends on`, `is blocked`, `is not blocked`, `is blocking`, `is not blocking` | ✅ | S | 0.53.0; With TK-05 |
| TK-37 | Boolean combinations: `(a) AND (b)`, `OR`, `NOT`, `XOR`, `AND NOT`, `OR NOT`, nested brackets | ✅ | M | 0.50.0; 0.67.1: without brackets too (`a AND b`) |
| TK-38 | `sort by` status, status.name, status.type, id, any date, happens, recurring, priority, urgency, path, filename, heading, description, tag (n); `reverse` | ✅ | M | 0.50.0: `sort by tag` sorts by the first tag |
| TK-39 | Urgency: Tasks' score from due, scheduled, start and priority; `sort by urgency`, `group by urgency`, `show urgency` | ✅ | S | 0.53.0 |
| TK-40 | `group by` status, dates, happens, recurring, recurrence, priority, urgency, path, root, folder, filename, heading, backlink, tags, id | ✅ | M | 0.50.0 |
| TK-41 | `limit to N tasks`, `limit groups to N tasks` | ✅ | S | 0.50.0 |
| TK-42 | Layout: `hide` / `show` each field (dates, priority, recurrence, id, depends on, on completion, tags), the backlink, the task count, the edit and postpone buttons, the toolbar; `short mode` / `full mode`; `show group count` | ✅ | M | 0.50.0: hide / show the fields, backlink, task count, urgency, short mode; no edit / postpone buttons or toolbar; 1.4.0: the edit and postpone buttons (TK-48); 1.8.0: the toolbar (as in 7.23.0): a filter by description that leaves the query as it is, and "Copy results" (headings and task lines as Markdown, no count); `hide toolbar` (`workspace::a_tasks_toolbar_filters_and_copies_the_results`) |
| TK-43 | `show tree` (sub-items under their task), `exclude sub-items` | ✅ | M | 0.66.0: `show tree` (sub-tasks under their task), `exclude sub-items` |
| TK-44 | `explain`: the query in words, with the global query and placeholders expanded | ✅ | S | 0.53.0 |
| TK-45 | Global query (a setting) added to every query; `ignore global query` | ✅ | S | 0.53.0: instructions separated by `;` |
| TK-46 | `filter by function`, `sort by function [reverse]`, `group by function [reverse]` (JavaScript) | ✅ | M | 1.1.0 (`workspace::tasks_queries_filter_sort_and_group_by_function`): one sandbox run per query; `task` has the original's fields (status, priority, dates as `TasksDate`s with `.moment` and `formatAsDate()`, file, tags, urgency …) and `moment()` (with `isBefore` / `isAfter` / `isSame`, `diff`) |
| TK-47 | `view columns by …` (a kanban of tasks) | ✅ | M | 0.66.0: `columns by <field>`: one column per group, side by side |
| TK-48 | Toggle a task from a result (Enter, a click) and the line in its note changes; edit and postpone buttons on each result | ✅ | M | 0.50.0: a click or Enter toggles (in the note, open or not); no edit / postpone buttons; Dataview's DV-06 too: one "write a task's line" function for both; 1.0.3: only the box checks a task (`workspace::a_tasks_result_checks_on_its_box_and_opens_on_its_backlink`), the backlink opens its note at the task; 1.3.0: a checked task stays in the results, shown checked, for a moment (a setting, 0–10 s, 3.5 by default) to click again (`workspace::a_checked_task_stays_in_its_query_for_a_moment`); 1.4.0: ✎ (the task window) and ⇥ (postpone) buttons on each result, `hide edit button` / `hide postpone button` (`workspace::task_results_have_edit_and_postpone_buttons`) |
| TK-52 | Time estimates (W-135): `[estimate:: 2h]` / `30m` on a task, Dataview's inline field, so the line stays the original's; summed with Dataview (`= sum(this.file.tasks.estimate)`) | ✅ | S | 0.94.0, through Dataview (`workspace::task_estimates_are_summed_by_short_queries`) |
| TK-53 | Time spent (`[started:: …]`, `[lasted:: …]`, after PlainTasks) | ✗ | M | Not wanted (decided 2026-10-07): estimates and short queries are enough |
| TK-54 | A statistics line or totals shown under queries | ✗ | S | Not wanted (decided 2026-10-07); `= length(filter(this.file.tasks, (t) => t.completed))` and the like do it in a line |
| TK-55 | Quick task (blackglass extra): with no note open, Ctrl+T asks for one line and adds `- [ ] text` to today's daily note under its Tasks heading (made from the daily template if it isn't there); the task window works there too | ✅ | S | 0.97.0 (`workspace::with_no_note_open_tasks_go_to_todays_daily_note`) |
| TK-56 | An Estimate field in the task window (blackglass extra, before Due): `[estimate:: 2h]` after the description, filled in when editing | ✅ | S | 0.97.0 (`workspace::the_task_window_has_an_estimate`) |

## 5. Suggested order

1. **Read and query** (M): TK-01, TK-02, TK-07, TK-30 … TK-35, TK-37,
   TK-38, TK-40, TK-41, TK-42 (the basics), TK-48 (toggle from results).
2. **Done dates and recurrence** (M): TK-03, TK-04, TK-10 … TK-14.
3. **Editing** (L): TK-20 … TK-22.
4. **The rest** (M): TK-05, TK-36, TK-39, TK-43 … TK-47, TK-06, TK-08,
   TK-09 (with mdedit).

## 6. Checked against

- [Tasks user guide](https://publish.obsidian.md/tasks/Introduction),
  [quick reference](https://publish.obsidian.md/tasks/Quick+Reference),
  [emoji format](https://publish.obsidian.md/tasks/Reference/Task+Formats/Tasks+Emoji+Format),
  [create or edit task](https://publish.obsidian.md/tasks/Editing/Create+or+edit+Task),
  [filters](https://publish.obsidian.md/tasks/Queries/Filters),
  [sorting](https://publish.obsidian.md/tasks/Queries/Sorting),
  [grouping](https://publish.obsidian.md/tasks/Queries/Grouping)
  (2026-09-30).
