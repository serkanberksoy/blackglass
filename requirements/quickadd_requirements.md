# QuickAdd: what blackglass has and what's missing

A comparison of blackglass's QuickAdd plugin (W-146) with the community
plugin [QuickAdd](https://github.com/chhoumann/quickadd) by Christian
Bager Bach Houmann (its documentation of choices and the format syntax).
blackglass has its **Capture** choices: a line, put together from
questions, added to a note. IDs are `QA-xx`.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## Choices

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| QA-01 | Capture choices: a name, a format, where it goes, a heading to put it under | ✅ | `workspace::a_quickadd_capture_asks_and_adds_a_line_to_todays_note`; in Settings → QuickAdd ("Add a capture"), saved in `.blackglass/plugins/quickadd/settings.toml` |
| QA-02 | Capture to today's daily note, or to a note by path (the path may use the format syntax: `Logs/{{DATE:YYYY}}.md`); the note is made if it isn't there | ✅ | same; `format::tests::a_path_is_formatted_too` |
| QA-03 | Insert after a heading (at the end of the list under it); the heading is made at the note's end if missing; without one, at the end | ✅ | same |
| QA-04 | Each choice is a command ("QuickAdd: <name>", so it can have keys), and "QuickAdd: Run QuickAdd" lists them | ✅ | same |
| QA-05 | Template choices (a new note from a template) | ✗ | Templater's "Create new note from template" and folder templates do this |
| QA-06 | Macro choices (JavaScript) | ✗ | No user JavaScript beyond Templater's sandbox |
| QA-07 | Multi choices (folders of choices) | ⬜ | |
| QA-08 | Capture to the active note, at the cursor | ⬜ | |

## Format syntax

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| QA-10 | `{{VALUE}}` / `{{NAME}}`: asks for a value | ✅ | `format::tests::values_dates_and_choices` |
| QA-11 | `{{VALUE:name}}`: a named value, asked once however often it's used | ✅ | same |
| QA-12 | `{{VALUE:a,b,c}}`: choose one of these | ✅ | same |
| QA-13 | `{{DATE}}`, `{{DATE:format}}` (moment.js formats) | ✅ | same |
| QA-14 | `{{VDATE:name, format}}`: asks for a date (in words too: `tomorrow`, `next friday`), formatted | ✅ | same |
| QA-15 | `{{LINKCURRENT}}`: a link to the active note; `{{SELECTED}}`: its selection | ✅ | same |
| QA-16 | `{{NOTE:folder}}` (blackglass's): choose a note in a folder, put in by its name (`[[{{NOTE:Budget/Envelopes}}]]` for a link) | ✅ | same; QuickAdd itself would leave it as written |
| QA-17 | `{{FIELD:name}}`: suggest a property's values in the vault | ⬜ | Needs a question that suggests and takes new text too |
| QA-18 | `{{MACRO:…}}`, `{{TEMPLATE:…}}`, `{{GLOBAL_VAR:…}}`, `{{MVALUE}}`, `{{CLIPBOARD}}`, `{{RANDOM:n}}` | ⬜ | Unknown ones are kept as written |
