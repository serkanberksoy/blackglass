# blackglass help

blackglass shows a **vault** (a folder of Markdown notes): the sidebar on
the left (files, search, tags), your notes in tabs on the right. Notes are
plain `.md` files; nothing else is needed.

A vault is any folder: **Open vault** in the command palette opens another
one, or makes a new folder for one. Press **{command-palette}** for every
command by name. **Esc** closes a
window. This page is read-only; **{close-tab}** closes it.

## Moving around

- **{focus-sidebar-editor}** moves to the next open pane: the sidebar, the
  calendar, the note, the backlinks;
  **Esc** in the sidebar goes back to the note, **{toggle-sidebar}** hides
  the sidebar. In the Tags tab, nested tags (`#project/alpha`) are under
  their parent: **→** opens one, **←** closes it, Enter finds its notes.
- **{go-to-note}** goes to a note by name (Enter on a new name creates it).
- **{search-the-vault}** searches every note. Several words must all be
  in a note; `"a phrase"` is one piece, `-word` leaves notes out, `OR`
  gives either, `( )` group. `tag:reading` finds a tag, `file:` a name,
  `path:` a folder, `line:(a b)` words on one line, `task:`, `task-todo:`,
  `task-done:` task lines, `[status]` or `[status:done]` a property.
- **{edit-properties}** edits the note's properties in a window: Enter
  edits a value (or switches a checkbox), **a** adds one, **r** renames,
  Delete removes.
- **Show properties** lists the vault's properties (Enter finds their
  notes); **Rename property** renames one in every note. A note's
  `aliases` find it in **{go-to-note}** and in `[[` suggestions.
- **{next-tab}** and **{previous-tab}** switch tabs; **{go-to-tab-1}** …
  **{go-to-tab-8}** go to a tab, **{go-to-last-tab}** to the last one.
- **{go-back}** and **{go-forward}** go back and forward through the notes
  you were on (the mouse's side buttons too, where the terminal passes
  them on; Konsole keeps them for its tabs).
- **{follow-link-under-cursor}** on a `[[link]]` opens it in a new tab. A
  click opens it too. A link to a note that doesn't exist yet offers to
  create it; a web link opens in the browser. **{preview-link-under-cursor}**
  shows the linked note in a popup (Enter opens it, Esc closes).
- The status bar counts the note's words (the selection's, when there is
  one).
- **{toggle-backlinks}** shows the note's links under it: its backlinks
  (the notes that link to it, with their lines), unlinked mentions (notes
  naming it without a link; **l** makes the link) and its outgoing links.
  Enter or a click opens one, **Esc** goes back to the note; again hides
  them.
- **{outline}** lists the note's headings; type to filter, Enter jumps.
  **Footnotes** lists its footnotes the same way.
  **Open random note** is in the palette.

## Notes

- **PDF to note** (palette) writes a PDF's text into a new note beside it.
- **New unique note** (palette) makes a note named by the time
  (`202609301542`).
- **{new-note}** makes a new note in the selected folder; in its window,
  **Tab** makes it from a template (with the Templater plugin).
- **{save}** saves, **{save-as}** saves under another name.
- **{extract-selection-to-a-new-note}** with text selected moves it to a
  new note and puts a link to it in its place.
- **{rename-note}** renames what's chosen in the file explorer (a note,
  an image or any file, which keeps its extension, or a folder), else the
  open note; every link to it is updated. **Rename folder** and
  **New folder** are in the palette.
- **Move note to a folder** moves the file or folder chosen in the file
  explorer (else the open note), with every link to it; **Delete note**
  is in the command palette too ({command-palette}). A deleted note goes
  to the vault's `.trash/` folder, so it can be brought back.
- New notes can get a unique ID (in the name, as the name or in the
  properties, in a time format of your choosing): the settings' Notes page.
- Typing `[[` suggests notes to link to; `#` suggests the vault's tags, and
  in the frontmatter its property names and their values (Enter or Tab
  puts one in). A link to a note that doesn't exist yet is dimmed.
- Highlights in color: `==🔴text==` (🔴 🟠 🟡 🟢 🔵 🟣); typing `==`
  suggests a color. **{highlight-in-red}** … **{highlight-in-purple}**
  highlight the selection, or recolor the highlight at the cursor;
  **{remove-highlight-color}** makes it plain again.
- With the Tasks plugin, **Alt+T** (Tasks: Create or edit task) opens a window
  with every field of the task at the cursor (or a new one): ↑↓ field,
  typing edits, ←→ choose, Enter saves; on a date, a calendar beside it
  (Shift+arrows a day or a week, PgUp/PgDn a month, a click a day). Its
  Estimate field writes `[estimate:: 2h]`. With no note open, the window's
  task goes to today's daily note (under its Tasks heading), and
  **Ctrl+T** adds a quick task there: just its text (Tasks: Add quick
  task in the palette, from anywhere).
- **Ctrl+Z** / **Ctrl+Y** undo and redo (1000 steps). What a command
  did to other notes (a kanban card moved, a task edited from a query, a
  rename with its links, a deleted note) is undone the same way when the
  open note has nothing of its own to undo, or with **{undo-last-change-to-other-notes}**.
- Dates in words: type `@` and a date (`@today`, `@tomorrow`, `@next
  friday`, `@in 3 days`, `@oct 20`, `@time` for times): Enter puts in a
  link to that day's note, Shift+Enter keeps your words as its alias.
  **{date-picker}** asks for one; **{parse-natural-language-date}** turns
  the selection into one. Format, trigger and links: the settings' Dates
  page.
- A click in the text puts the cursor there. In a query's results (a
  `dataview`, `tasks`, `query` or `base` block) a click follows a link or
  checks a task, and the query stays as it is; with the mouse over the
  block, its **</>** button (or the arrow keys) shows the query.
- The open tabs and folders come back the next time you open the vault,
  and notes changed by another program (a sync tool, Git) are picked up;
  a note with unsaved changes here is kept, with a warning.

## Editing

The editor shows Markdown as you write it (live preview). **{cycle-modes-live-preview-source-view}**
switches between live preview, source mode (every line raw) and view mode
(read-only, links and results can be followed with Tab and Enter).

- **{undo}** undo, **{redo}** redo, **{select-all}** select all
- **{find-in-note}** find, **{find-and-replace}** find and replace,
  **{find-next}** the next match
- **{toggle-task}** checks a task, **{fold-unfold}** folds a heading or list
- **{insert-emoji}** inserts an emoji

## Plugins and settings

- **{settings}** opens the settings: on the left a search box and the
  pages (Options: Editor, Keyboard shortcuts, Plugins; Plugin options: each
  plugin's settings), on the right the page. Typing searches every page;
  every command's keys can be changed in Keyboard shortcuts.
- Bases: on a `.base` page, **Alt+F** shows the filters above the
  results (rows of property, operator and value; the results follow every
  key; Tab switches between every view's filters and this view's), and
  **Alt+M** makes them half, nearly all or one line of the screen. A
  kanban board moves with keys: ←→↑↓ choose, Shift+←→ move a card,
  Alt+Shift+←→ move a column, **n** a new note in it.
- **{open-plugins}** (the settings' Plugins page) installs plugins:
  Dataview (queries in `dataview` blocks), Templater (templates: insert
  one with **Alt+E**, apply one to the note, folder and file regex
  templates that fill new notes, startup templates; `tp.web` for a daily
  quote, `tp.user` scripts, **Templater: Jump to next cursor location**;
  set in Templater's settings), Periodic
  Notes (daily notes and the calendar, **{toggle-calendar}**), Recent
  Files (a Recent tab in the sidebar with the notes you visited lately),
  Bookmarks (a Bookmarks tab: **Bookmarks: Bookmark this note**, this
  heading, this folder or the search; Enter opens, **r** renames, Delete
  removes), Zettelkasten (`[[Note#` and `[[Note#^` suggest headings and
  blocks, `[[##` / `[[^^` anywhere; copied links; Folgezettel sequences,
  breadcrumbs, titles instead of IDs; extract, split and merge notes;
  orphans, the Links tab, link counts, quick capture), Citations (a
  BibTeX / Zotero bibliography: **Citations: Insert citation**, literature
  notes), Encrypt (**Encrypt: Encrypt selection** with a password and a
  hint, **Encrypt: Decrypt** at the cursor),
  Mermaid (```` ```mermaid ```` diagrams drawn as text), Git (the vault
  backed up and synced: **Git: Commit-and-sync**, on a timer too; a Git
  tab in the sidebar with the changes, **Git: Show the diff of this note**,
  **Git: History**). With
  Periodic Notes, **Alt+D** opens today's note (created from its
  template).
- Your settings are saved in `~/.config/blackglass/` (`%APPDATA%\blackglass\`
  on Windows): `config.toml` (the
  editor) and `keys.toml` (only the shortcuts you changed). A plugin's
  settings are saved in the vault, in `.blackglass/plugins/`.
- **A window of its own:** `blackglass --gui` runs this same app in a
  window (the default when started from a launcher; `--terminal` keeps
  the terminal). There, the settings' Window page sets the font and its
  size (`window.toml`), images show as pictures and emoji in color, and
  Ctrl+C copies the selection.

**{quit}** quits; it asks first about unsaved changes.
