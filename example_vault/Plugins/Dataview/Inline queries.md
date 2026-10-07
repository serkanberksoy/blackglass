---
project: blackglass
status: writing
started: 2026-09-15
---
# Inline queries

An inline query shows one value inside the text: a code span that starts
with an equals sign (or with a dollar sign and one, for JavaScript). In the
live preview you see the value; on the line being edited, the code.

- This note: `= this.file.name`, in `= this.file.folder`.
- Project `= this.project` is `= upper(this.status)`, for `= date(today) - this.started`.
- Dune's author is `= [[Dune]].author`, its file is
  `= [[Dune]].file.path`, and the link `= meta([[Dune#Part one]]).subpath`
  points to a heading.
- A week from today: `= date(today) + dur(1 week)`.
- The best book: `= [[The Left Hand of Darkness]].file.name`, rated `= [[The Left Hand of Darkness]].rating`★.
- Sci-fi books: `$= dv.pages('"Books"').where(p => p.genre == "scifi").length`.
- Pages read: `$= dv.pages('"Books"').pages.array().reduce((a, b) => a + b, 0)`.
- Today is a `$= dv.luxon.DateTime.now().toFormat("EEEE")`.
- Books by name: `$= dv.pages('"Books"').file.name.sort().join(", ")`.

Inline queries follow your edits before you save: change `status` above
and the line updates.
