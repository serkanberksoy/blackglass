# QuickAdd: what blackglass has and what's missing

A comparison of blackglass's QuickAdd plugin (W-146) with the community
plugin [QuickAdd](https://github.com/chhoumann/quickadd) by Christian
Bager Bach Houmann, against its documentation
([format syntax](https://quickadd.obsidian.guide/docs/FormatSyntax),
[capture](https://quickadd.obsidian.guide/docs/Choices/CaptureChoice),
[template](https://quickadd.obsidian.guide/docs/Choices/TemplateChoice) and
[multi](https://quickadd.obsidian.guide/docs/Choices/MultiChoice) choices;
read 2026-10-08). IDs are `QA-xx`.

Choices are kept in `.blackglass/plugins/quickadd/settings.toml`, a section
a choice, and set in Settings → QuickAdd ("Add a capture", "Add a template
choice", "Add a global variable"). Tests are in `quickadd::format::tests`
(`format::`), `quickadd::tests` and `workspace::` (named in full).

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## Choices

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| QA-01 | Capture choices: a name, a format, where it goes | ✅ | `workspace::a_quickadd_capture_asks_and_adds_a_line_to_todays_note` |
| QA-02 | Each choice is a command (keys in Settings → Keyboard shortcuts); "Run QuickAdd" lists them | ✅ | same |
| QA-03 | Template choices: a new note from a template (its tokens asked), the file name and folder as formats, an empty name asked | ✅ | `workspace::quickadd_templates_groups_and_days` |
| QA-04 | Template choices: when the note exists, a number added ("Budget Review 1"), open it, or append the template to it | ✅ | same (the number); open / append by setting |
| QA-05 | Template choices: open the new note (on by default); a link to it at the cursor | ✅ | same |
| QA-06 | Multi choices: groups; "Run QuickAdd" shows a group, then its choices | ✅ | same; `quickadd::tests::choices_groups_and_globals_come_from_the_settings` |
| QA-07 | Macro choices (JavaScript) | ✗ | No user JavaScript beyond Templater's sandbox |
| QA-08 | Template choices: "Search existing notes before creating"; "Ask for folder each time"; several folders to choose from | ⬜ | |
| QA-09 | Icons and placeholders for multis | ✗ | Lists show names |

## Capture: where it goes

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| QA-20 | Today's daily note (empty note; Periodic Notes' folder and format), made from its template if missing | ✅ | `workspace::a_quickadd_capture_asks_and_adds_a_line_to_todays_note` |
| QA-21 | A note by path (the format syntax works: `Logs/{{DATE:YYYY}}`), made if missing | ✅ | same |
| QA-22 | A note chosen from a folder (`Projects/`) or by tag (`#project`) | ✅ | `workspace::quickadd_captures_go_where_they_are_told` |
| QA-23 | The active note: at the cursor (`{{CURSOR}}` leaves the cursor there), a new line below or above the cursor, or its top / bottom / heading | ✅ | same |
| QA-24 | Under a heading, at the end of its list, or right under it (newest first); a missing heading made at the bottom or the top | ✅ | same; the quick task (Ctrl+T) makes its heading too |
| QA-25 | Top of the note (after the frontmatter), bottom of the note | ✅ | `workspace::quickadd_captures_go_where_they_are_told` (a heading made at the top goes after the frontmatter, as "top" does) |
| QA-26 | Create the note if it isn't there (off: a message), from a template | ✅ | same |
| QA-27 | As a task (`- [ ]`) | ✅ | same |
| QA-28 | One entry per line of `{{VALUE}}` (its prompt takes several lines) | ✅ | same |
| QA-29 | Which day: today, or asked each time (dates and the daily note follow it) | ✅ | `workspace::quickadd_templates_groups_and_days` |
| QA-30 | A link to the captured note at the cursor; open it after | ✅ | `workspace::quickadd_captures_go_where_they_are_told` (the link) |
| QA-31 | "Insert after" any line of text (not a heading), "Insert before", "Consider subsections", ordered heading creation, choose the heading at capture time | ⬜ | Headings only |
| QA-32 | Property captures (write to a frontmatter property) | ⬜ | |
| QA-33 | Canvas captures; link placement in frontmatter; embed links; copy a link to the clipboard; where the note opens (split, window, sidebar) | ✗ | No canvas; one tab per note |
| QA-34 | "Name (pick a day)" commands beside a choice's own | ⬜ | The choice's "day: ask" does it |

## Format syntax

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| QA-40 | `{{VALUE}}` / `{{NAME}}`, `{{VALUE:name}}` (asked once however often it's used) | ✅ | `format::tests::values_dates_and_choices` |
| QA-41 | `{{VALUE:a,b,c}}` (quotes keep commas), `\|custom` (other text too), `\|multi` | ✅ | `format::tests::value_options_defaults_labels_and_kinds` |
| QA-42 | Value options: `\|default` (`\|x`), `\|label:`, `\|type:multiline`, `\|case:`, `\|trim:false` | ✅ | same |
| QA-43 | Value options: `\|type:number`, `\|type:slider`, `\|type:checkbox`, `\|optional`, `\|text:`, `\|format:` (yaml, list) | ⬜ | Taken as text |
| QA-44 | `{{DATE}}`, `{{DATE:format}}`, `{{DATE+3}}`, `{{TIME}}`, `{{TIME:format}}` | ✅ | `format::tests::dates_offsets_times_and_modifiers` |
| QA-45 | `{{VDATE:name, format}}` (dates in words), `\|default`, `\|optional` | ✅ | same |
| QA-46 | Date modifiers: `\|case:`, `\|startof:` / `\|endof:` (day, week, isoweek, month, quarter, year) | ✅ | same |
| QA-47 | `{{FIELD:property}}`: its values in the vault suggested (new text too), `\|folder:`, `\|tag:`, `\|exclude-folder:`, `\|exclude-tag:`, `\|exclude-file:`, `\|inline:true`, `\|default:` | ✅ | `format::tests::field_values_with_filters_and_inline_fields` |
| QA-48 | `{{FIELD}}`'s `\|multi`, `\|default-from:active`, `\|inline-code-blocks:` | ⬜ | |
| QA-49 | `{{FILE:folder}}` (`\|link`, `\|path`, `\|multi`, `\|optional`); blackglass's older `{{NOTE:folder}}` the same | ✅ | `format::tests::file_choices_link_path_many_and_none` |
| QA-50 | `{{LINKCURRENT}}`, `{{LINKSECTION}}`, `{{FILENAMECURRENT}}`, `{{FOLDERCURRENT}}` (`\|name`), `{{SELECTED}}`, `{{TITLE}}`, `{{FOLDER}}` (`\|name`) | ✅ | `format::tests::the_active_note_and_the_note_written_to` |
| QA-51 | `{{RANDOM:n}}`, `{{CURSOR}}`, `{{MVALUE}}` (LaTeX typed) | ✅ | same; `value_options_defaults_labels_and_kinds` |
| QA-52 | `{{GLOBAL_VAR:name}}` (global variables, set in the settings), `{{TEMPLATE:path}}` | ✅ | `format::tests::globals_and_templates_are_put_in_first` |
| QA-53 | `{{DAILY}}`, `{{WEEKLY}}` … (periodic notes' paths), `{{PROPERTY}}`, `{{CLIPBOARD}}` | ⬜ | |
| QA-54 | `{{MACRO:name}}` | ✗ | No macros (QA-07); kept as written |
