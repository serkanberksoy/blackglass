# Templater

Templates live in `Templates/`. Plain text is copied; `<% … %>` is code run
when the template is used.

| Try | How |
|-----|-----|
| Insert a template here | **Alt+E**, then choose one |
| A new note from a template | **Ctrl+N**, type a name, **Tab** |
| Apply one to this note | "Templater: Apply template to this note" |
| The tour of every feature | open [[Templater tour]] |

What's in `Templates/`:

- [[Daily]]: the daily note's (yesterday / tomorrow links, the week).
- [[Book]]: asks for an author; folder templates use it for new notes in
  `Books/`.
- [[Meeting]] and [[Weekly review]]: prompts and choices.
- [[Templater tour]]: dates, the file, prompts, `tp.web` (a quote of the
  day), user scripts (`Scripts/callout.js`), JavaScript.

New notes in `Books/` and `Journal/` get their folder's template (Settings →
Templater: folder templates). After inserting, "Jump to next cursor
location" goes to the next `tp.file.cursor()`.
