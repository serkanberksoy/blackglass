# Periodic Notes

Daily, weekly, monthly, quarterly and yearly notes, each with a name format,
a folder and a template.

- **Alt+D** opens today's daily note (in `Journal/`, made from
  [[Daily]] the first time).
- **Alt+C** shows the calendar under the sidebar: ←→↑↓ move, PgUp / PgDn
  change the month, Enter opens the day's note (asking before making one),
  **w** the week's, **m** the month's, **t** today, Delete deletes the day's
  note (asked first).
- "Next / Previous periodic note", and "Jump forwards to closest daily note"
  … for each period, go to the nearest note that exists.
- "Open weekly note", "Open monthly note" …

Settings → Periodic Notes: each period's format (`YYYY-MM-DD`,
`gggg-[W]ww` …), folder and template; the first day of the week; today's
note at startup; **calendar sets** (a second journal, for work, with its
own folders: "Add a calendar set", "Switch calendar set").

Templates can use `{{date}}`, `{{title}}`, `{{yesterday}}`, `{{tomorrow}}`,
`{{monday:YYYY-MM-DD}}` …, and offsets: `{{date+1d:YYYY-MM-DD}}`,
`{{month-1M:MMMM}}`.
