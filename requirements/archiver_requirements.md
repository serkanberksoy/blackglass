# Task Archiver: what blackglass has and what's missing

A comparison of blackglass's Task Archiver plugin (W-127) with the
community plugin [Task Archiver](https://github.com/ivan-lednev/obsidian-task-archiver)
by Ivan Lednev (its README and source, read 2026-10-06). IDs are `TA-xx`.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## Commands

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| TA-01 | Archive tasks in this file: done tasks (with what's under them) go under an "Archived" heading; tasks already there stay | ✅ | `archive::tests::done_tasks_go_under_the_archive_heading`, `workspace::the_task_archiver_archives_deletes_sorts_and_checks_off` |
| TA-02 | Archive tasks including nested tasks (done subtasks of open tasks) | ✅ | `archive::tests::nested_tasks_are_archived_deeply` |
| TA-03 | Delete tasks in this file | ✅ | `archive::tests::deleting_and_archiving_a_heading` |
| TA-04 | Archive heading under cursor: the section and its subsections, one level below the archive heading | ✅ | same |
| TA-05 | Sort tasks in list under cursor: plain items, open tasks, done tasks, at every level | ✅ | `archive::tests::sorting_a_list` |
| TA-06 | Toggle task done and archive it | ✅ | `archive::tests::checking_off_and_archiving_one_task` |
| TA-07 | Works in reading view (on the file) | ✗ | blackglass's view mode is read-only by design; the commands work on the open note |

## Settings

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| TA-10 | Placeholders `{{date}}`, `{{obsidianTasksCompletedDate}}`, `{{sourceFileName}}`, `{{sourceFilePath}}`, `{{heading}}`, `{{headingChain}}` | ✅ | `archive::tests::placeholders_metadata_replacement_and_order`. `{{completedDate}}` is the task's ✅ date (the original's name works too). The old `%` is left out (it would replace every `%`) |
| TA-11 | Archive to this note, a custom note (placeholders), or the daily note | ✅ | `archive::tests::rules_and_other_notes`; the daily note's folder and format are Periodic Notes' |
| TA-12 | Archive under a heading in a separate note, or at its top | ✅ | same |
| TA-13 | Heading hierarchy (several levels, placeholders) with the first level's depth | ✅ | `headings` setting, levels after ` > ` |
| TA-14 | List item hierarchy (a date tree), merged into what's there | ✅ | `archive::tests::a_date_tree_of_list_items` |
| TA-15 | A date format per level | ✅ | As `{{date:FORMAT}}` in the level's text (the original has a format field per level) |
| TA-16 | Newest first / last, sort alphabetically | ✅ | blackglass puts the newest first when asked (the original reverses tasks archived together) |
| TA-17 | Only when subtasks are done; all checked statuses; an extra pattern tasks must match | ✅ | `archive::tests::which_tasks_are_done` |
| TA-18 | Replace text (a regular expression) before archiving | ✅ | `$1`, `$&` as in JavaScript |
| TA-19 | Text appended to archived tasks (`🔒 [[{{date}}]] 🕸️ {{headingChain}}` by default) | ✅ | |
| TA-20 | Blank lines around headings and archived tasks | ✅ | |
| TA-21 | Rules: statuses, path patterns, text patterns; move to a note (or the daily note) or delete | ✅ | `archive::tests::rules_and_other_notes`; one regular expression per pattern field (the original takes one per line); "Add an archiving rule" in Settings or the palette |
| TA-22 | Indentation from the vault's tab settings | 🟡 | Copied from the note's own lists; a setting (tab, 2 or 4 spaces) when it has none |
