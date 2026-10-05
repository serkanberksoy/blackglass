# blackglass

**Your second brain, in the terminal. Free, open source, and fast.**

blackglass is a full note-taking workspace that runs where you already
work: the terminal. Point it at a folder of Markdown files and you get a
file explorer, vault-wide search, tags, backlinks and tabs, with every
note a live-preview editor: headings, tables, callouts, tasks, images
and embeds rendered as you type, the Markdown right there when you
need it.

- **Free, forever.** MIT licensed, no account, no subscription, no cloud.
  It never phones home; it only goes online when you ask it to (a Git
  sync, a web request in a template).
- **Your notes stay yours.** Plain Markdown files in a folder you choose.
  Open them in any editor, sync them with anything, keep them in Git.
  Nothing is locked in a database.
- **Fast, really.** Written in Rust. On a vault of 5,000 notes it opens
  in about a seventh of a second, answers a keystroke (redrawn screen
  included) in under 3 ms, and searches every note in 13 ms. One 14 MB
  program, no runtime, no browser engine.
- **Anywhere you have a shell.** Over SSH on a server, in a tmux pane, on
  a laptop with no window manager. The keyboard drives everything (every
  command can have your own keys), and the mouse works too.
- **Linked thinking built in.** `[[Links]]` that find notes anywhere,
  backlinks and unlinked mentions, block and heading links, embeds,
  properties, nested tags, bookmarks, a daily note, and a full
  Zettelkasten toolkit: unique IDs, Folgezettel sequences, breadcrumbs,
  literature notes from your Zotero library.
- **Plugins included, not hunted for.** Queries over your notes
  (Dataview, DataviewJS), templates with JavaScript (Templater), tasks
  with due dates and recurrence, database views (Bases), Git backup,
  diagrams, advanced tables, all built in and switched on when you want
  them.

Written in Rust with [ratatui](https://ratatui.rs); the editor is mdedit,
a terminal Markdown editor embedded as a library (a separate repository,
checked out next to this one).

```bash
blackglass                      # choose a vault: a recent one, or any folder (a new one is made)
blackglass .                    # the current folder as the vault
blackglass ~/Notes              # open a vault
blackglass ~/Notes/Journal/today.md   # open a note in its vault
blackglass example_vault        # try the example vault
blackglass --help               # all options and keys
```

**Ctrl+P** shows every command, **Ctrl+O** goes to a note, **Ctrl+N** makes
one, **Ctrl+G** searches the vault, **Ctrl+B** moves between the sidebar and
the editor, **Ctrl+Q** quits.

**Version:** 0.76.0 · **Stage:** M1 (workspace) and M2 (commands & plugins) complete; M3 (vault features, Zettelkasten) nearly · [Version history](VERSION.md)

```
 Files  Search  Tags           │ 2026-08-08 ×  2026-08-09 ×  +
 example_vault             A→Z │ Journal / 2026-08-09
▌▸ Articles                    │
▌▸ Books                       │  2026-08-09
▌▾ Journal                     │
▏    2026-08-08                │  << yesterday || tomorrow >>
▏    2026-08-09                │  █ AUGUST 9TH, 2026 - WK2632.7
   Welcome                     │  • 08:30 started Dune again #reading
 Journal/2026-08-09.md  Ln 7, Col 1      ^O go to  ^N new  ^W close  ^G search
```

## Install

**Linux (x86_64):** download `blackglass` from the
[latest release](https://github.com/serkanberksoy/blackglass/releases/latest),
make it executable (`chmod +x blackglass`) and put it on your `PATH`.

**From source** (any platform): blackglass builds with Rust **1.88 or newer**
([rustup](https://rustup.rs)). It needs **mdedit** checked out next to
it, as `../mdedit` (a path dependency):

```bash
mkdir notes-tools && cd notes-tools
git clone https://github.com/serkanberksoy/mdedit.git
git clone https://github.com/serkanberksoy/blackglass.git
cd blackglass
cargo build --release              # the program: target/release/blackglass
cargo install --path .             # or into ~/.cargo/bin
```

It runs in any terminal on Linux and macOS (Windows terminals should work
but aren't tested); a terminal with true color and a Nerd Font or other
Unicode font looks best, and plain ASCII works too. Optional programs it
uses when they're there:

| Program | For |
|---------|-----|
| `wl-paste` / `xclip` / `xsel` (Linux) | The clipboard (paste, copied links) |
| `xdg-open` / `open` | Web links, images and PDFs in their own programs |
| `git` | The Git plugin |
| `curl` | Templater's `tp.web` (a daily quote, web requests) |

## Features

114 of 126 tracked features are done (`requirements/features.md`).

| Area | What you get |
|------|--------------|
| File explorer | Folders open and close; each top-level folder has its own color (a rainbow); sort by name or date |
| Vaults | Any folder is a vault: started without one, blackglass lists the recent vaults and takes any folder's path (Tab completes, a new folder is made after asking); "Open vault" in the palette switches at any time |
| Tabs | One editor per note, an unsaved dot, close buttons, `+` for a new note; an open note reuses its tab; the open tabs and folders come back next time; notes changed elsewhere (a sync tool, Git) are picked up |
| Editor | Everything mdedit does: live preview, tables, callouts, tasks, images, footnotes, `%%comments%%`, search and replace, undo, folding, emoji; notes, sections and blocks (`![[Note#^id]]`) embedded from anywhere in the vault |
| Link autocomplete | Type `[[`: the 5 latest notes, then a fuzzy search as you type; ↑/↓ and Enter or Tab insert `[[Note]]` |
| Links | `[[Note]]` is found anywhere in the vault (the nearest, shortest path wins); Ctrl+Enter or a click opens it in a tab (a missing note is offered to be made); web links open in the browser; embeds and images too; a link to a note that doesn't exist yet is dimmed |
| Quick switcher | Fuzzy search by name or `aliases`, recent notes first, and "Create" for a new name |
| Vault search | Every note, smart case, name matches first; opens at the match and F3 finds the next one. Several words all match; `"a phrase"`, `-word` (not), `OR`, `( )`; `tag:name`, `file:`, `path:`, `line:(a b)`, `task:`, `task-todo:`, `task-done:`, `[property]`, `[property:value]` |
| Tags | Every tag in the vault with its note count, nested tags as a tree (`#project/alpha` under `#project`; → / ← open and close); `#` suggests tags as you type, and the frontmatter its property names and values |
| Query embeds | A ```` ```query ```` block shows a vault search's results in the note (click to open) |
| PDFs | "PDF to note" writes a PDF's text into a new note beside it, linking back to the PDF, to read, search and link |
| Properties | Shown typed in the note (checkboxes, dates, lists as chips, tags); Alt+; edits them in a window (their types from their values); the frontmatter's properties: "Show properties" lists them with their note counts (Enter searches), "Rename property" renames one in every note; `aliases` find notes in the quick switcher and `[[` suggestions (`[[Note\|Alias]]`) |
| Backlinks | Alt+L shows (and hides) the note's links under it: the notes linking to it with their lines, the notes naming it without a link (`l` makes the link), and its outgoing links (missing ones marked); Enter or a click opens |
| Outline and random note | Alt+O lists the note's headings to jump to; "Footnotes" lists its footnotes; "Open random note" in the palette |
| Notes | New notes in the selected folder (`Untitled`, `Untitled 1` …), with a unique ID if you like (Settings → Notes: in the name, as the name or in the properties; any time format, `YYYYMMDDHHmm`, `YYYY-MM` …); Save As anywhere in the vault; rename (F2) and move any note, file (an image, a PDF) or folder, chosen in the file explorer or open, with every link to it updated, delete (to the vault's `.trash/`); new and renamed folders; unsaved changes are asked about |
| Help | F1 or Ctrl+H: a built-in help page with every command and its keys as they are now |
| Settings | Alt+, : one window, a search box and the pages on the left (Options: Editor, Notes, Keyboard shortcuts, Plugins; Plugin options: each plugin's settings), the page on the right with each setting, what it does and its value; typing searches every page; saved in `~/.config/blackglass/` (plugins' in the vault) |
| Command palette | Ctrl+P: every command in one list to filter and run: the workspace's, the editor's (save, undo, find, source mode …), inserts (hyperlink, wiki link, callout, table, code block, date …) and the plugins'; every action is a command, so every one can have keys (Settings → Keyboard shortcuts), none by default for most |
| Plugins | Install, uninstall, enable and disable them on the settings' Plugins page ("Open plugins"); they add commands, render their own code blocks, and have settings pages (→ or ⚙ on the Plugins page, or "*Plugin*: Settings" in the palette) |
| Dataview plugin | ```` ```dataview ```` blocks run `LIST`, `TABLE` and `TASK` queries with `FROM` (folders, tags, links), `WHERE`, `SORT` and `LIMIT` over frontmatter, `key:: value` fields, tags, links, tasks and `file.*`; the result shows in the note, the query when the cursor is in it |
| JavaScript | A sandboxed engine (pure Rust, no files or network): ```` ```dataviewjs ```` blocks with Dataview's `dv` API, and Templater's `<%* %>` commands. Charts, stats, tables and a link graph drawn in JavaScript: `example_vault/JavaScript/` |
| Templater plugin | All of Templater: templates in `Templates/` with `tp.date`, `tp.file` (title, include, cursors, `create_new`, `move`, `rename` …), `tp.frontmatter`, `tp.system` questions (prompts of one or more lines, choices of one or many), `tp.web` (a daily quote, a random picture, web requests, fetched in the background), `tp.config`, `tp.hooks`, and `tp.user` (your scripts, and system commands if you turn them on); JavaScript in `<%* %>`. Insert one (Alt+E), apply one to the open note, create a note from one, run the commands in a note, or at startup; folder and file regex templates fill new notes; "Jump to next cursor location"; `<% %>` tags shown as written (`example_vault/Templates/Templater tour.md`) |
| Git plugin | Backs the vault up to Git: commit-and-sync (commit, pull, push) from the palette or every N minutes, a pull on start, a commit message template; runs your own `git` in the background, and notes a pull changed are shown again; the branch and what's waiting in the status bar; a Git tab in the sidebar lists the changes (`s` stage, `d` discard, `c` commit); a note's diff and the history open in read-only tabs; the open note's changed lines marked in a margin (`+`, `~`, `-`), or who wrote each line; settings for a tree of changes, squashing before a push, side-by-side diffs |
| Advanced Tables plugin | In a table Tab / Shift+Tab / Enter move between cells and line it up; commands insert, delete, move, align and sort rows and columns, transpose, export CSV, and evaluate `<!-- TBLFM: … -->` formulas (`example_vault/Examples/Tables.md`); a Table tab in the sidebar with the operations |
| Bases plugin | Database views of your notes: ```` ```base ```` blocks and `.base` files filter notes by their properties, compute formulas, and show them as a table (grouped, with summaries), a list, cards or a kanban board; switch, sort, group, filter and search views from the palette, embed a base or one of its views (`![[Library.base#Cards]]`), click a row to change its properties, copy or export a view, a new note from a view (`example_vault/Examples/Library.base`) |
| Tasks plugin | Due, scheduled and start dates, priorities, recurrence and dependencies on tasks; ```` ```tasks ```` blocks filter, sort and group every task in the vault (a click checks one off); toggling writes done dates and a recurring task's next occurrence; create, edit and postpone tasks from the palette (`example_vault/Examples/Tasks.md`); fields suggested as you type, `show tree`, `columns by`, regex filters |
| Mermaid plugin | ```` ```mermaid ```` blocks drawn as text: flowcharts as a tree of their links (subgraphs listed), sequence diagrams with lifelines and arrows, pie charts as bars, Gantt charts as a timeline (`example_vault/Examples/Diagrams.md`) |
| Recent Files plugin | A Recent tab after Files, Search and Tags: the last 25 notes you visited (or changed), newest first (Enter opens one, `l` links it at the cursor, Delete takes it off; "Clear list" in the palette); paths and tags can be left out, titles shown; kept per vault; not installed by default |
| Bookmarks plugin | Bookmark the note, the heading above the cursor, the folder chosen in the explorer or the search; a Bookmarks tab opens them, `r` renames, Delete removes; renamed notes keep theirs |
| Zettelkasten plugin | Links to headings and blocks (`[[Note#`, `[[Note#^`, `[[##`, `[[^^`; block ids added), copied links, new unique notes linked from here; Folgezettel sequences (sequel and branch notes, a Sequence tab), breadcrumbs from `up` / `down` / `next` / `prev`, titles shown instead of IDs; extract to another note, extract a heading, split and merge notes; orphans, dead ends, unresolved links, a Links tab, link counts, quick capture (`example_vault/Zettelkasten/`) |
| Citations plugin | A BibTeX / CSL JSON bibliography (from Zotero): insert `[@key]` (shown as "Author Year"), open or link a reference's literature note, made from a template |
| Periodic Notes plugin | Alt+D opens today's note; open today's daily, weekly, monthly, quarterly or yearly note (created from its template), and jump to the next or previous one; a calendar in the sidebar (Alt+C) opens any day's, week's or month's note, or asks to create it; formats, folders and templates in `.blackglass/plugins/periodic-notes/settings.toml` |
| View mode | Alt+V: a read-only view of the note, every line rendered; move over the rendered rows, Tab to a link or a query result, Enter follows it (a Dataview row opens its page, a task its line) |
| Mouse | Click in the text to place the cursor; click links, embeds, images (in the system viewer) and query results; files, results, tags, panels and tabs; the wheel scrolls (`--no-mouse` leaves the mouse to the terminal) |
| Themes | "Choose theme" (or the settings' Theme row): Default, Paper (light), Ember, Ocean, Forest, High contrast, previewed as you move; your own in `~/.config/blackglass/themes/` (the keys are described in `themes/default.toml`); the editor's colors too (links, code, quotes, its page) |
| Terminals | Colors fitted to 256- and 16-color terminals, ASCII symbols where Unicode can't be shown |

## Keys

These are the defaults: every command's keys can be changed in Settings
(Alt+,), Keyboard shortcuts. The editor's keys (editing, Ctrl+S, Ctrl+F, Ctrl+Z, Ctrl+V or
Ctrl+Shift+V to paste …) are mdedit's; see `mdedit --help`. Links, embeds,
images and query results can be clicked. blackglass gets each key first:

| Key | Action |
|-----|--------|
| F1 / Ctrl+H | Help: how blackglass works, and every command with its keys (read-only) |
| Alt+, / Ctrl+, | Settings: editor, keyboard shortcuts, plugins |
| Ctrl+P | Command palette: every command (the editor's too, with their keys), "Insert hyperlink" and other inserts, plugin commands, "Open plugins" |
| Ctrl+O | Go to a note (quick switcher); a new name creates the note |
| Ctrl+N | New note, in the folder selected in the file explorer; in its window, **Tab** picks a template for it (Templater plugin) |
| Ctrl+W, Ctrl+X | Close the tab (asks about unsaved changes) |
| Ctrl+X | With text selected: extract it to a new note: the new-note window opens (Tab: from a template); the text moves there and a link to it replaces it; you stay on the new note |
| Alt+Left/Right, Ctrl+PgUp / Ctrl+PgDn | Previous / next tab |
| Alt+1 / Alt+9 | First tab … last tab ("Go to tab 1" … "Go to last tab") |
| Ctrl+Alt+Left/Right | Back / forward through the notes you were on (also the mouse's back / forward buttons) |
| Ctrl+B | Move the focus to the next open pane: the sidebar, the calendar (if shown), the note, the backlinks (if shown) |
| Alt+B | Hide / show the sidebar |
| Alt+V | Cycle the note's mode: live preview → source → view (read-only; Tab to a link or a query result, Enter or a click follows it, Esc edits) |
| Alt+; | Edit the note's properties: Enter edits a value (or switches a checkbox), a adds, r renames, Delete removes |
| Alt+O | The note's outline: its headings, indented by level; type to filter, Enter jumps |
| Alt+P | Preview the note of the link under the cursor in a popup (↑↓ scroll, Enter opens it, Esc closes) |
| F2 | Rename the note (the one chosen in the file explorer, or the open one); every link to it is updated |
| Alt+L | Show the note's links under it (with the focus): backlinks, unlinked mentions (`l` links one) and outgoing links; ↑↓ choose, Enter opens, Esc back / hide them |
| Alt+D | Today's daily note (Periodic Notes plugin) |
| Alt+C | Show the calendar (with the focus) / hide it: ←→↑↓ move, PgUp / PgDn change month, Enter opens the day's note (asks before creating a new one), `w` / `m` the week's / month's, `t` today, Esc back |
| Ctrl+G, Ctrl+Shift+F | Search the vault |
| F5 | Scan the vault again (after changes made elsewhere) |
| Ctrl+Q | Quit (asks about each note with unsaved changes) |
| Tab, Shift+Tab | In the sidebar: next / previous panel (Files, Search, Tags) |
| Enter | In the sidebar: open a note or folder, a search result, or a tag's notes |
| Right, Left | In the file explorer: open / close a folder |
| s | In the file explorer: sort A-Z, Z-A, newest, oldest |
| Ctrl+U | In search: clear the query |
| Space | On the settings' Plugins page: enable / disable the plugin (Enter installs / uninstalls) |
| Esc | Back to the editor |

## Settings

Settings are changed in the settings window (Alt+,) and saved in your
config folder, `~/.config/blackglass/` (`$XDG_CONFIG_HOME/blackglass/`):

- `config.toml`: the editor's settings, which are mdedit's (`auto_pair`,
  `undo_steps`, `images`, `colors`, `glyphs` …; see `mdedit --help`). Until
  blackglass has its own, it reads mdedit's `~/.config/mdedit/config.toml`.
- `keys.toml`: the keyboard shortcuts you changed, as
  `command-id = "Keys"` (`new-note = "Alt+N"`; several with ` / `, `""`
  for none).

A plugin's settings are the vault's (`.blackglass/plugins/<id>/`), and so
are the Notes page's (note IDs, `.blackglass/notes.toml`).
Recently used emoji are shared with mdedit.

## Development

mdedit must be checked out next to this folder (`../mdedit`). Start with
`CLAUDE.md`; the stack and standards are in
`documentation/technology_and_skills.md`, the plan in
`documentation/project_plan.md` and every tracked feature (with its test)
in `requirements/features.md`.

```bash
scripts/check.sh                     # the gate: fmt, clippy -D warnings, tests
./run.sh                             # try it: the latest debug build on the example vault
./run.sh Dataview.md                 # … opened at a note (options like --no-mouse pass on)
cargo test --test workspace          # the workspace, driven like a user would
cargo build --release                # the release build (small and fast: see Cargo.toml)
```

The layout: `src/workspace.rs` (the app: tabs, keys, effects), `src/ui/`
(drawing), `src/vault/` (the vault's index), `src/plugins/<name>/` (one
folder per plugin), `tests/` (the workspace driven by keys and checked
on a drawn screen), `vendor/crossterm` (patched for the mouse's side
buttons), `example_vault/` (a vault to try everything on).

## License

MIT, see `LICENSE`.
