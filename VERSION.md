# Version history

Current version: **0.77.1**

## Versioning rules

blackglass uses [Semantic Versioning](https://semver.org). Before 1.0:

| Change | Bump | Example |
|--------|------|---------|
| A feature is implemented (a row in `requirements/features.md` moves to ✅ or 🟡) | **MINOR** | 0.1.0 → 0.2.0 |
| Bug fix, refactor, docs or tests only | **PATCH** | 0.2.0 → 0.2.1 |
| Milestone M2 complete | **1.0.0** | |

On every bump:
1. Update `version` in `Cargo.toml`.
2. Add a new entry at the **top** of this file (newest first), and update
   "Current version".
3. Update `README.md` (the version line, the feature count, and the tables
   if the change is user-visible).
4. Update `example_vault/` if the feature needs something to try it on.

`cargo test` fails (`tests/project.rs`) if these disagree.

Entry format: `## X.Y.Z (YYYY-MM-DD)`, followed by *Added / Changed / Fixed*
sections that name feature IDs.

---

## 0.77.1 (2026-10-06)

### Added
- Dataview examples in `example_vault/`: `Dataview/Query language`
  (grouping, flattening, order of commands, expressions, functions),
  `Dataview/Tasks and lists` (emoji dates, checking off, subtasks, list
  items), `Dataview/Dates and durations` (and a calendar),
  `Dataview/Inline queries`, `JavaScript/Dataview extras` (swizzling,
  `dv.io.csv` on `Data/reading-log.csv`, `dv.view` with `Data/views/`,
  Luxon, Markdown). Three more books with genres, pages and read dates.
  `tests/examples.rs` also renders every inline query.

### Fixed
- `FLATTEN` of an empty list drops the row (it kept one with nothing).
- List items have their `section`, as tasks do.
- DataviewJS pages have every `file.*` field the queries have
  (`etags`, `aliases`, `lists`, `frontmatter` …).
- Tests that wait for background work (the watcher) allow 10 s, not 4
  (they could time out on a busy machine).

## 0.77.0 (2026-10-06)

### Added
- **W-42 Dataview, complete** (every gap in
  `requirements/dataview_requirements.md`, DV-04 … DV-47):
  - The query language: commands run in the order written; `GROUP BY …
    AS` (with `rows` and `rows.file.link` swizzling) and `FLATTEN … AS`;
    `LIST WITHOUT ID`; `FROM outgoing([[Note]])` and `FROM [[]]`;
    `CALENDAR date-field` (a month grid with a `•` on each day that has
    notes, and the notes below it).
  - Expressions: `%`, durations (`dur(1 day)`, `date(today) -
    dur(1 month)`, date − date), list and object literals, indexing
    (`list[0]`, `obj.key`, `[[Note]].field`), lambdas (`(x) => …`), and
    about 40 more functions (lists, text with regular expressions,
    numbers, dates, `meta`, `display` …).
  - Data: list items and the remaining task fields (`children`, `parent`,
    `section`, `tags`, `outlinks`, `blockId`, `fullyCompleted` …), task
    emoji dates (✅ 📅 ➕ 🛫 ⏳), `file.aliases`, `file.lists`,
    `file.frontmatter`, `file.starred` (Bookmarks), `file.day` from
    `yyyymmdd` names.
  - In the note: inline queries (`` `= expr` ``, and `` `$= js` `` when
    JavaScript is on); `TASK` results check tasks off (Enter or a click;
    a ✅ date with completion tracking; a setting opens the task
    instead); results follow unsaved edits.
  - DataviewJS: swizzling (`dv.pages().file.name`), `dv.io.load` /
    `dv.io.csv` and `dv.view` (files inside the vault only, read-only),
    `dv.luxon.DateTime`, `dv.markdownTable` / `markdownList` /
    `markdownTaskList`.
- `example_vault/Dataview.md`: grouping, flattening, a calendar, inline
  queries.

### Changed
- W-05 and W-20 are done: watching (W-29) and clicks in the text (W-34)
  were their missing parts.

## 0.76.1 (2026-10-06)

### Changed
- **A Linux release that runs everywhere**: one static x86_64 binary
  (musl), built by `scripts/release-linux.sh`, with mimalloc as its
  allocator (musl's own made opening a 5,000-note vault 18 times slower;
  with mimalloc it's as fast as the usual build).

## 0.76.0 (2026-10-01)

### Added
- **W-123 Citations** (a plugin): a BibTeX or CSL JSON bibliography (as
  Zotero exports it): "Insert citation" (`[@key]`), shown in the live
  preview as "Author Year" (several keys and page locators too); "Open
  literature note" (the one cited at the cursor, or chosen) made from a
  template the first time; "Insert link to literature note". mdedit
  3.13.0 rendered spans. Plugins can render spans their own way
  (`Plugin::rendered_spans`, `render_span`).

## 0.75.0 (2026-10-01)

### Added
- **W-122 Zettelkasten retrieval**: "Show orphan notes", "Show dead-end
  notes", "Show unresolved links" (pages of links); "Show local links" (a
  Links tab: the note's links out and in as a tree, a depth setting);
  link counts beside links (a setting; mdedit 3.13.0 link badges);
  "Quick capture" to an inbox note. Plugins can put a badge beside links
  (`Plugin::link_badge`).

## 0.74.0 (2026-10-01)

### Added
- **W-121 Zettelkasten refactoring** (after Note composer): "Extract
  selection to another note" (new or existing, a template with
  `{{content}}` `{{fromTitle}}` `{{newTitle}}` `{{date:…}}`, leaving a
  link, an embed or nothing), "Extract this heading", "Split note at
  headings", "Merge this note into another" (asks first; every link to it
  follows, it goes to the trash). New effects `DeleteNote` and `Retarget`.

## 0.73.0 (2026-10-01)

### Added
- **W-120 Zettelkasten structure**: Folgezettel IDs (`1`, `1a`, `1a1` …):
  "New sequel note", "New branch note" (linked from here), moving to the
  parent, next and previous note, a Sequence tab in Luhmann's order;
  breadcrumbs from `up` in the status bar, "Go up / down / to next note /
  to previous note" by the `up` / `down` / `next` / `prev` properties;
  titles (a property) shown instead of names in the explorer, tabs, the
  switcher, suggestions, search results and backlinks (a setting).
  Plugins can name notes (`Plugin::display_name`).

## 0.72.0 (2026-10-01)

### Added
- **W-119 Zettelkasten linking** (a plugin, off until installed): `[[Note#`
  suggests the note's headings, `[[Note#^` its blocks (one without an
  id gets one, written into its note), `[[##` / `[[^^` headings and
  blocks anywhere; "Copy link to this note / heading / block"; "New
  unique note linked from here" (the vault's ID format, a folder and a
  template; the selection becomes the alias). Plugins get the vault for
  suggestions, inside links too, and a suggestion can do something when
  chosen. A note made from a template by a plugin runs its Templater
  commands.

---

## 0.71.0 (2026-10-01)

### Added
- **Every action is a command** (W-62), in the palette and in Settings →
  Keyboard shortcuts, its keys yours to change (or to have none):
  - "Go to tab 1" … "Go to tab 8" and "Go to last tab" (Alt+1 … Alt+9 by
    default; they were fixed keys);
  - "Indent list item" and "Unindent list item" (Tab / Shift+Tab).

### Changed
- An editor key taken off its command (Tab off "Indent list item") is
  only kept from the editor; the sidebar, suggestions and plugins (a
  table's cells) still get it. Before, it did nothing anywhere.

---

## 0.70.0 (2026-10-01)

### Added
- **W-34 Click to place the cursor**: a click in a note's text puts the
  cursor there; on a rendered line, where the clicked text is in its
  source (hidden markup, bullets, capital headings lined up). mdedit
  3.12.0.
- **W-22 Links to missing notes are dimmed**, until the note is made (a
  quick check by name, no disk access while drawing). mdedit 3.12.0 (K-11).
- **W-29 Changes made elsewhere are picked up**: notes changed, added or
  deleted by another program (a sync tool, Git, another editor) are seen
  within about a second; open notes reload, but unsaved changes are kept,
  with a warning. blackglass's own saves don't count.
- **W-33 The workspace comes back**: the open tabs (with their cursors),
  the active one and the file explorer's open folders, kept in
  `.blackglass/workspace.json` (and kept out of Git by the Git plugin).
- **W-32 Nested tags as a tree** in the Tags tab: → / ← open and close,
  each tag with the notes that have it or a tag nested in it.
- **W-27 Bookmarks** (a plugin): bookmark the note, the heading above the
  cursor, the folder chosen in the file explorer, or the search; a
  Bookmarks tab opens them (a heading's note at the heading, a folder in
  the explorer, a search run again), `r` renames, Delete removes. Kept in
  the original's `bookmarks.json` shape; renamed notes keep theirs.
- **W-24 Tag and property suggestions**: `#` suggests the vault's tags (most
  used first); in the frontmatter, property names, the values a property
  has in other notes, and tags in a `tags:` list.

---

## 0.69.0 (2026-10-01)

### Added
- **W-125 All of Templater** (`requirements/templater_requirements.md`,
  TP-12 … TP-31): every module but the original app's own API (`tp.app`,
  `tp.obsidian`).
  - `tp.web`: `daily_quote()` (a `> [!quote]` callout), `random_picture()`
    and `request(url, path)`. Pages come through `curl` on a thread, so the
    editor never waits. The template runs again once they're in (the
    same for user commands and notes it reads). Off with Web access; only
    web pages (http, https), not local files. A new Templater command
    drops a template still waiting.
  - `tp.file`: `create_new` (its template's commands run too; questions
    asked after the template's own), `move` and `rename` (every link to
    the note follows), `find_tfile`, `cursor_append`, and `include` running
    the included note's commands (JavaScript too, sections and blocks).
  - `tp.system`: multi-line `prompt` (Alt+Enter for a new line),
    `multi_suggester` (Tab marks), `suggester` with `limit`.
  - `tp.config` (`run_mode`, `template_file`, `target_file`,
    `active_file`) and `tp.hooks.on_all_templates_executed`.
  - `tp.user`: the scripts in a folder (CommonJS, Script files folder)
    and system commands from the settings (off by default; the arguments
    as environment variables, with a timeout).
  - A command only JavaScript runs (`tp.web`, `await`, `moment()`) needs
    no `<%* %>`: the line `<% tp.web.daily_quote() %>` from a daily
    template works as it is, in Templater's commands and in Periodic
    Notes' daily notes.
  - Cursors: the lowest `tp.file.cursor(n)` is jumped to and the others
    stay; "Jump to next cursor location" goes on to the next. "Automatic
    jump to cursor" off leaves them all.
  - Settings: file regex templates (before folder templates), trigger on
    new file creation (a new note with commands has them run; an extract,
    a note a plugin makes), excluded folders, startup templates, template
    hotkeys ("Insert <name>", "Create from <name>" commands to give keys),
    Alt+E for "Insert template".
  - `<% … %>` tags are shown as written, in the code color, while
    Templater is enabled (mdedit 3.11.0's verbatim spans).
- **W-126 Rename and move any file or folder** with every link to it: F2
  renames the file (an image keeps its extension) or folder chosen in the
  file explorer; "Move note to a folder" moves the explorer's file or
  folder (else the open note). A renamed or moved folder's attachments
  have their links updated too, not only its notes.

---

## 0.68.0 (2026-10-01)

### Added
- **W-124 Note IDs** (Zettelkasten ZK-02, ZK-03): a vault setting on the
  settings window's new Notes page (Alt+, → Notes), saved in
  `.blackglass/notes.toml`. "Note IDs" gives every new note a unique ID
  made from the time: in its name (`202610011445 Meeting`), as its name
  (`202610011445`, the title in `title` and `aliases`, and a link that
  made it becomes `[[202610011445|Meeting]]`) or in its properties
  (`id: 202610011445`). "ID format" takes moment.js tokens
  (`YYYYMMDDHHmm` by default, `YYYY-MM`, `YYYY-MM-DD` …); a taken ID gets
  `-2`, `-3` …. New notes from Ctrl+N, the quick switcher, a missing
  link, extract and plugins get one (periodic notes keep their dates as
  names); a folder template's properties and cursor are kept. "New unique
  note" uses the same format.

---

## 0.67.5 (2026-10-01)

### Fixed
- **Tasks dates with numbers in words**: `due BEFORE in two weeks`, `due
  AFTER two weeks` (from today), `in a month`, `three days ago`.

### Tests
- Task dashboards in callouts (`>[!danger]` … `>```tasks`): `due AFTER
  yesterday`, `due BEFORE in two weeks`, `due next month`, `no scheduled
  date`, and a block the callout ends before its fence closes.
- A Dataview list of orphans: `list from "" where length(file.inlinks) =0
  and length(file.outlinks) = 0 sort file.mtime desc limit 3` (a link to a
  missing note counts as a link out).

### Docs
- `example_vault/Examples/Dashboard.md` (the task callouts, orphans,
  recent projects) and `Tasks Backlog.md`.
- Zettelkasten requirements, queued for M3 (queue 15, W-119 … W-123):
  `requirements/zettelkasten_requirements.md`.

## 0.67.4 (2026-10-01)

### Fixed
- **Notes keep their creation time when saved** (mdedit 3.10.1): a save
  writes the note in place (after a synced copy), as Obsidian does, so
  `file.ctime` (Dataview, Bases) stays when the note was created; before,
  it became the time of the last save. Hard links are kept too.

### Tests
- `list from "Projects" sort file.mtime desc limit 15`: the folder and
  its sub-folders (not `Projects Old`), newest first, the limit; a saved
  note moves to the top and keeps its creation time.

## 0.67.3 (2026-10-01)

### Tests
- A Dataview `list from "" where contains(tags, "project") and
  !contains(file.folder, …) sort file.mtime desc limit 15` query: lower
  case keywords, `where` on its own line, folders (and folders under
  them) left out, newest first by the file's time, the limit; `tags` is
  the frontmatter's, `file.tags` has the tags in the text too.

## 0.67.2 (2026-10-01)

### Fixed
- **Quoted links in properties are links** (Dataview, Bases): `up:
  "[[Roadmap]]"` and lists of them (`up:` then `- "[[Roadmap]]"`, YAML
  needs the quotes) are links, so `contains(up, this.file.link)` finds
  them, as Dataview does.

## 0.67.1 (2026-10-01)

### Fixed
- **The mouse wheel scrolls the live preview** instead of moving the
  cursor (mdedit 3.9.0's `scroll_rows`): a long Dataview or Tasks result
  scrolls into view row by row, where the wheel used to move the cursor
  into the block and show its query. View mode is as it was.
- **Queries in callouts are rendered** (mdedit 3.10.0): a ```` ```tasks ````
  or ```` ```dataview ```` block inside a callout (`> ```tasks`) shows its
  result, as in Obsidian.
- **Tasks queries read like the original's**: `due before 2026-08-04 AND
  priority is above none` (a combination without brackets), and `sort by
  due date` (`… date` after a date's name).

## 0.67.0 (2026-10-01)

### Added
- **Advanced Tables controls** (W-91, AT-41): a Table tab in the sidebar
  lists the table operations; Enter runs one on the table at the editor's
  cursor (the focus stays, for the next); "Open table controls"; a setting.
- **Table formulas with ranges** (W-92): a range with one cell
  (`(@2$1..@4$1*2)`, `(@2..@4-@1)`) fills a range destination
  (`@2$3..@4$3=…`), cell by cell; one value fills every cell.
- **Git** (W-93, W-94): the Git tab as a tree (changes under their
  folders), squashing the commits waiting into one before a push, and
  diffs side by side (a Before / After table per change): settings.

## 0.66.0 (2026-10-01)

### Added
- **Tasks: suggestions while typing** (W-80, TK-21): on a task line, a word
  being typed offers the fields (`du` → 📅 due date, `hi` → ⏫ high
  priority …); after a date's emoji, dates (today, tomorrow, next monday
  …); after 🔁, rules. ↑/↓ choose, Enter or Tab puts it in, Esc closes.
  Plugins can suggest while typing (`Plugin::suggestions`).
- **Tasks: tree and columns** (W-81, TK-43, TK-47): `show tree` draws
  sub-tasks under their task; `exclude sub-items` leaves nested tasks
  out; `columns by status.name` (any group key) lays groups side by side.
- **Regular expressions** in Tasks (`description regex matches /^Call/i`,
  `regex does not match`) and Bases (`file.name.matches(/^Du/)`,
  `replace(/-/g, "+")`).

## 0.65.0 (2026-10-01)

### Added
- **W-76 Bases, the rest of `.base` files**: embeds, `![[File.base]]` (the
  whole base) and `![[File.base#View]]` (one view); "Search view" (rows
  with a shown value containing the text); "Copy view" (a Markdown
  table); "New note from view" (in the folder the filters name, with the
  properties they ask for).
- **W-77 Bases, editing**: a click (or Enter) on a row opens its menu:
  open the note, or change a shown property (a checkbox switches at once,
  other values are asked with the current one offered). "Sort view",
  "Group view", "Set view limit", "Choose view properties", "Add view
  filter", "Add formula" (checked first) and "Add view" change the base
  without writing YAML.
- Plugins can put text on the clipboard (`Effect::CopyText`; `App::copier`
  uses wl-copy, xclip, xsel or pbcopy).

### Changed
- A click on a base's row opens its menu (its first choice opens the
  note).

## 0.64.0 (2026-10-01)

### Added
- **W-69 themes color the editor too**: `editor_text`, `editor_bg` and
  `color_black` … `color_white` (the named colors the editor draws with:
  links, code, quotes, bullets, callouts …) in theme files, through mdedit
  3.8.0's palette. Paper now has a light page on any terminal; the dark
  themes tune links, code and dim text. Plugins' results (Tasks and Bases
  headings, query blocks, the calendar) take the theme's accent and
  selection.

### Fixed
- Plugins' headings, counts and errors in their blocks keep their colors
  (mdedit 3.8.1: a block line's own style was dropped).

## 0.63.3 (2026-09-30)

### Fixed
- **Table formulas evaluate like the original** (W-92, AT-32, AT-35):
  results are decimal (`3 * 1.2` is `3.6`, not `3.5999999999999996`);
  text in a cell (a label like `**Sum**`, `12 kg`) counts as 0 instead of
  stopping the formula; dates (`2022-12-31 23:59`) and durations (`1:30`)
  can be added and subtracted, shown with `;dt` and `;hm`. "Evaluate table
  formulas" works with the cursor on a `TBLFM` line too.

## 0.63.2 (2026-09-30)

### Fixed
- **Faster and lighter on large vaults** (measured on 5000 notes,
  `tests/perf.rs`):
  - saving a note updates just that note in the plugins' indexes (Dataview,
    Bases, Tasks) and the tag counts: 500 ms → 9 ms; a note with thousands
    of links no longer takes quadratic time;
  - Dataview and Bases share one index of the vault's pages, and query
    blocks read the vault itself instead of a copy: memory after start
    111 MB → 68 MB;
  - notes are read, and the index built, on every core: opening the vault
    114 ms → 37 ms, the plugins' start 529 ms → 35 ms, a rescan 869 ms →
    93 ms;
  - the vault search runs on every core too (with mdedit 3.7.2's faster
    matching): 288 ms → 12 ms;
  - typing in a 10,000-line note: 4.3 ms → 2.9 ms a key with its frame
    (mdedit 3.7.2; the status bar counts words without joining the note).
- Plugins can update one changed note (`Plugin::on_note_changed`) and
  share the page index (`Plugin::uses_index` / `set_index`).

## 0.63.1 (2026-09-30)

### Fixed
- **Smaller release build**: 13 MB, down from 24 MB (boa without its
  default features, fat LTO in one codegen unit, optimized for size with
  syntect and the regex engines kept at full speed).
- **Long notes with many code blocks draw quickly** (mdedit 3.7.1): a
  frame of a 10,000-line note with 2000 code blocks took about 200 ms,
  now about 1 ms.

## 0.63.0 (2026-09-30)

### Added
- **Git, repository chores and views** (W-95, W-94, W-96): "Initialize a
  new repo" asks whether to leave out `.blackglass/` (a `.gitignore`);
  "Switch to remote branch"; "Delete repository" (asked twice); "Open
  file on GitHub" and "Open file history on GitHub"; `p` pull and `P`
  push in the Git tab; the last commit's time in the status bar (a
  setting); the refresh interval, what line authoring shows (colored by
  the line's age) and the repository's folder in the vault (`base_path`)
  are settings.
- Plugins can quit blackglass (`Effect::Quit`, asking about unsaved notes)
  and open web pages (`Effect::OpenUrl`).

## 0.62.0 (2026-09-30)

### Added
- **Git, sync options** (W-93): "Fetch"; pull before push (a setting);
  the reset pull method and a merge strategy (ours, theirs); automatic
  commits a while after the last change, pulls and pushes on timers of
  their own, automatic commits of staged files only; "Quiet" leaves out
  "nothing to commit" and the like. Commit-and-sync pushes only when
  there's something to push.

## 0.61.0 (2026-09-30)

### Added
- **Git, commits** (W-93): "Commit (staged or all)", "Commit staged",
  "Amend staged", each commit also "with specific message"; "Stage
  current file", "Unstage current file", "Discard all changes" (asks),
  "Commit-and-sync and then close"; a separate template for automatic
  commits, the changed files in the message's body, and a commit message
  script (settings).

## 0.60.0 (2026-09-30)

### Added
- **Recent Files, more** (W-89): count notes when opened or only when
  changed (saved); leave out paths and tags (regular expressions); show a
  note's `title` property; `l` on an entry links it at the editor's
  cursor. Plugins hear of saved notes (`Plugin::on_note_saved`).

### Changed
- The `regex` crate is a direct dependency (it was already in the build).

## 0.59.0 (2026-09-30)

### Added
- **W-36 search query embeds**: a ```` ```query ```` block runs a vault
  search (the search panel's syntax) and shows the notes and their
  matching lines in place; a click opens a note, or a note at a line.
  Core, no plugin needed.

## 0.58.0 (2026-09-30)

### Added
- **W-35 block embeds**: `![[Note#^id]]` shows another note's block (a
  paragraph or list item ending with ` ^id`, or the block above an id on
  its own line), the note found anywhere in the vault; `[[Note#^id]]`
  opens the note at the block (mdedit 3.7.0, E-03).

## 0.57.0 (2026-09-30)

### Added
- **W-77 editing a base's notes**: "Bases: Edit a property" asks which of
  the view's notes, which of its properties and the new value, and writes
  the note's frontmatter (in its tab if it's open); on a kanban board it
  moves the card to another column.

## 0.56.0 (2026-09-30)

### Added
- **W-76 `.base` files**: opened (from the file explorer, a link or the
  switcher) as the rendered base in a read-only tab, drawn again when the
  file is saved; "Bases: Edit base source" opens its YAML. "Bases: Create
  new base" (`Untitled.base` in the selected folder), "Insert new base" (a
  ```` ```base ```` block at the cursor), "Switch view", "Export view as
  CSV". The views are named above the results, with the count.
- Plugins can create files (`Effect::CreateFile`) and open any file as
  text (`Effect::OpenSource`).

## 0.55.0 (2026-09-30)

### Added
- **W-75 more views**: `groupBy` (a heading per value, ascending or
  descending); summaries per column (Average, Sum, Min, Max, Range,
  Median, Stddev, Earliest, Latest, Checked, Unchecked, Empty, Filled,
  Unique, and custom `summaries` formulas over `values`) per group and in
  total; the list view (markers, separator, properties inline or
  indented), cards (boxes side by side, `cardSize`) and kanban boards (a
  column per value of `groupBy`); `properties` display names.

## 0.54.0 (2026-09-30)

### Added
- **W-74 Bases** (plugin `bases`, not installed by default): ```` ```base
  ```` blocks of YAML with `filters` (`and` / `or` / `not`, nested),
  `formulas` and `views`; the expression language: note and file
  properties (`price`, `note.price`, `file.name`, `file.mtime`,
  `formula.x`, `this`), `+ - * / %`, comparisons, `! && ||`, dates with
  durations (`today() + "1w"`), `if`, `date`, `now`, `max`, `min`,
  `number`, `list`, `link`, `file`, and the methods on strings, numbers,
  dates, lists (`filter`, `map`, `reduce` …), links and files (`hasTag`,
  `inFolder`, `hasLink`, `hasProperty`). A table view with `order`,
  `sort` and `limit`; a click opens a row's note; errors are shown in the
  block.

### Changed
- YAML is read with `yaml-rust2` (a new dependency, pure Rust).

## 0.53.0 (2026-09-30)

### Added
- **W-81 Tasks, the rest**: dependencies (🆔 id, ⛔ depends on) with `is
  blocked`, `is blocking`, `has id`, `has depends on`; urgency (`sort by
  urgency`, `group by urgency`, `show urgency`, Tasks' default order);
  `explain`; the settings' global filter (only lines with e.g. `#task`
  are tasks, hidden in results if wanted) and global query (`ignore global
  query`); tasks in the Dataview format (`[due:: …]`, `[priority:: …]`);
  invalid dates kept and found with `due date is invalid`.

## 0.52.0 (2026-09-30)

### Added
- **W-80 Tasks, editing**: "Tasks: Create or edit task" asks the
  description, priority, due, scheduled and start dates (`today`, `fri`,
  `in 2 weeks`, `2026-10-01`), recurrence and status, and writes the
  cursor's task (or makes the line one); "Tasks: Postpone task" moves its
  date by a day to a month.

## 0.51.0 (2026-09-30)

### Added
- **W-79 Tasks, done dates and recurrence**: "Tasks: Toggle task done"
  (and a click in a result) moves a task to its next status, writing ✅
  or ❌ with today's date (removed when reopened); a 🔁 task (`every week`,
  `every month on the 15th`, `every weekday`, `… when done`) gets its next
  occurrence above it, dates moved by the rule; 🏁 `delete` removes a done
  task. Statuses have names, types and a next status; `[.]` is a log
  (NON_TASK) out of the box; custom statuses in the settings.

## 0.50.0 (2026-09-30)

### Added
- **W-78 Tasks** (plugin `tasks`, not installed by default): dates (➕ 🛫
  ⏳ 📅 ✅ ❌), priorities (🔺 ⏫ 🔼 🔽 ⏬) and more read from task lines;
  ```` ```tasks ```` blocks with `done` / `not done`, status (`status.type
  is NON_TASK`, `status.symbol is .`), date (`due before in 7 days`,
  `happens in this week`, `done on 2026-10-01`), priority, recurrence,
  path / folder / filename / heading, description and tag filters,
  combined with `(…) AND / OR / XOR (…)`, `NOT (…)`; `sort by`, `group
  by`, `limit`, `hide` / `show`, `short mode`; `{{query.file.path}}` and
  `preset this_file`. A click (or Enter in view mode) on a result checks
  the task off in its note.
- Plugins can edit any note's lines (`Effect::EditNote`) and act on their
  result rows (`Plugin::row_action`, `plugin:<id>:…` actions).

## 0.49.0 (2026-09-30)

### Added
- **W-92 table formulas**: "Advanced Tables: Evaluate table formulas" runs
  the `<!-- TBLFM: … -->` lines under the table: `DEST=EXPR` with cells
  (`@3$2`), columns (`$4`), rows (`@5`), first / last (`@<`, `@>`, `$<`,
  `$>`), the first body row (`@I`) and relative references (`@-1`, `$+2`),
  ranges (`@I..@-1`, `@2$1..@3$3`), `+ - * /`, `sum`, `mean`,
  `if(a < b, x, y)`, `;%.2f` formats, several formulas chained with `::`
  or on several lines. An error is named in the status bar and leaves the
  table as it was.

## 0.48.0 (2026-09-30)

### Added
- **W-91 table rows and columns**: Advanced Tables commands insert a row
  or column before the cursor's, delete it, move it (up / down, left /
  right, the cursor following), align a column (left, center, right,
  none), sort the rows by the cursor's column (numbers as numbers),
  transpose, and export the table as `<note>.csv`.

## 0.47.0 (2026-09-30)

### Added
- **W-90 Advanced Tables** (plugin `tables`, not installed by default):
  in a table, Tab / Shift+Tab go to the next / previous cell (a new row
  at the end) and Enter to the same column in the next row (on an empty
  last row it leaves the table); every move lines the table up (cells
  padded to their column's width in terminal columns, the separator row
  of dashes with its alignment colons). A header line and Enter start a
  table. "Format table", "Format all tables in this note", "Move cursor
  out of table", "Next cell", "Previous cell". Settings: Tab, Enter, and
  padding. Each change is one undo step.
- Plugins can take editor keys (`Plugin::takes_key` / `editor_key`) and
  replace a note's lines (`Effect::ReplaceLines`); `ActiveNote` has the
  cursor's column.

## 0.46.0 (2026-09-30)

### Added
- **W-71 choose a theme**: "Choose theme" in the command palette (and a
  Theme row on the settings' Editor page) lists every theme; ↑/↓ shows
  each one at once, Enter keeps it, Esc goes back. The choice is saved in
  `~/.config/blackglass/appearance.toml` and used at start (the vault
  launcher too).

## 0.45.0 (2026-09-30)

### Added
- **W-70 five example themes**, built in: Paper (light, for a light
  terminal), Ember (warm dark), Ocean (blue dark), Forest (green) and High
  contrast. Your own go in `~/.config/blackglass/themes/<name>.toml`; one
  named like a built-in replaces it.

## 0.44.0 (2026-09-30)

### Added
- **W-69 theme files**: blackglass's colors come from a theme, a TOML file
  of named colors (`#rrggbb`, 0-255 or a color name): the chrome, sidebar,
  tabs, prompts, selection, text, accent, title, tags, the unsaved dot, the
  eight folder colors and the editor's heading colors (the editor
  settings' heading colors win). `themes/default.toml` holds the default
  colors, each key described, and is built in; a key a theme leaves out
  is the default's, a bad one is a warning in the status bar.

### Changed
- The open note's name in the file explorer is bold in the theme's text
  color (it was white).

## 0.43.1 (2026-09-30)

### Fixed
- **Smaller builds**: the debug binary went from 377 MB to 47 MB (90% of it
  was full debug info: now line numbers only, and none for dependencies),
  the release binary from 35 MB to 23 MB (mdedit 3.6.1 no longer pulls in
  an AV1 encoder and OpenEXR through `ratatui-image`; symbols stripped,
  thin LTO).

## 0.43.0 (2026-09-30)

### Added
- **W-104 Mermaid, more**: `A & B --> C` links each node; `subgraph` groups
  are listed after the tree (`▣ Title: A, B`); `autonumber` numbers the
  messages of a sequence diagram; pie charts are bars with their shares;
  Gantt charts are a timeline: sections, tasks as bars on one scale with
  their dates (`after`, days, weeks). The example vault shows each.

## 0.42.0 (2026-09-30)

### Added
- **W-96 one change at a time**: "Git: Preview change", "Stage change"
  and "Reset change" act on the change at the cursor (a patch of just
  that change); a reset reloads the note.
- **W-95 clone and submodules**: "Git: Clone an existing remote repo" (its
  URL and a folder) clones it and opens it as the vault; "Update
  submodules", and a setting to update them after every pull.
- **W-111 merge conflicts**: a pull that stops on a conflict names the
  files and shows their conflict markers in open notes; "Git: Abort
  merge" goes back; after fixing them, commit-and-sync finishes the merge
  (and pushes). Commit-and-sync refuses while conflict markers are left
  unresolved.
- Plugins can open a vault (`Effect::OpenVault`).

### Fixed
- Git pulls with the merge method pass `--no-rebase`, so newer git
  versions don't refuse diverged branches.

## 0.41.0 (2026-09-30)

### Added
- **W-118 the property editor**: Alt+; ("Edit properties") lists the open
  note's properties with their types (text, checkbox, number, date, list,
  nested); Enter edits a value (a list as `a, b, c`, keeping its inline or
  `- item` style) or switches a checkbox, `a` adds one (`name: value`, its
  type from the value), `r` renames one, Delete removes one. Each change
  rewrites only the frontmatter, as one undo step, and the cursor stays
  where it was; nested keys are kept as they are. In the note, properties
  are shown typed (mdedit 3.6.0): checkboxes, numbers, dates, lists as
  chips, tags as tags.
- **W-102** is complete with it.

## 0.40.0 (2026-09-30)

### Added
- **W-109 footnotes**: references `[^1]`, definitions `[^1]: text` and
  inline footnotes `^[text]` are shown as `[1]` / `[text]` in the footnote
  color (mdedit 3.5.0); "Footnotes" in the palette lists the note's
  definitions, Enter goes to one.
- **W-110 comments**: `%%comment%%` is dimmed with its markers hidden, and
  a `%%` … `%%` block is dimmed (mdedit 3.5.0).

## 0.39.0 (2026-09-30)

### Added
- **W-96 Git in the editor**: the open note's changes against the last
  commit are marked in a margin beside its lines (`+` added, `~` changed,
  `-` lines deleted below; a new file is all added), worked out again when
  you switch notes or save; a setting turns it off. "Git: Go to next /
  previous change" jumps between them. "Git: Toggle line author
  information" shows who wrote each line and when (`git blame`), lines not
  committed yet saying so. Staging, previewing and resetting one change
  are still to come. Uses mdedit 3.4.0's margin (`EditorView::margin`).
- Plugins see the cursor's line (`ActiveNote::row`), can set a note's
  margin (`Effect::Margin`) and move the cursor (`Effect::GoToLine`).
- **W-111 sync**: through the Git plugin, commit-and-sync by command or
  every N minutes, and a pull on start.

## 0.38.0 (2026-09-30)

### Added
- **W-95 Git, repository chores**: "Create new branch", "Switch branch" and
  "Delete branch" (choose from the others), "Edit remotes" (a name and its
  URL: adds or changes one), "Remove remote", "Edit .gitignore" (opens it,
  made if missing), "Add file to .gitignore" (the open note), and "Raw
  command" (any `git …`, its output in a read-only tab). Cloning,
  deleting the repository and submodules are still to come.

## 0.37.0 (2026-09-30)

### Added
- **W-94 Git, see changes**: a Git tab in the sidebar (while the plugin is
  enabled) lists the changed files with their state (modified, new,
  deleted, renamed, conflict; staged or not): `s` stages or unstages one,
  `d` discards its changes (after asking), `c` commits the staged files
  (asks the message), Enter opens it. "Git: Show the diff of this note"
  and "Git: History" (choose a commit) open read-only tabs with the diff.
- Pages without a file (help, a diff) can have any title (mdedit 3.3.0
  `EditorView::name`) and are read-only; plugins open them with
  `Effect::ShowText`. A plugin's sidebar tab gives its own key hints.

## 0.36.0 (2026-09-30)

### Added
- **W-93 Git status in the status bar**: the branch, the changed files
  and the commits to push (`main · 2 changed · 1 to push`), worked out in
  the background after every Git command and every 30 seconds. Plugins
  can put a line in the status bar (`Plugin::status`, `Effect::Redraw`).

## 0.35.0 (2026-09-30)

### Added
- **W-93 the Git plugin, stage 1: back up** (not installed by default).
  "Git: Commit-and-sync" stages everything, commits with a message
  template (`vault backup: {{date}}`; `{{hostname}}`, `{{numFiles}}`,
  `{{files}}`), pulls (merge, or rebase with auto-stash) when the branch
  has an upstream, and pushes (the first push sets the upstream). Also
  "Commit all changes", "Pull", "Push", "Initialize a new repo" and
  "Pause / resume automatic routines". Settings: the message and date
  format, automatic commit-and-sync every N minutes, pull on start, the
  pull method, push on or off. Your own `git` runs in the background; a
  pull that changes files rescans the vault and reloads open notes
  without unsaved changes. The Git status in the status bar is still to
  come.
- Plugins get a timer tick (`Plugin::tick`, about twice a second when
  idle) and `Effect::FilesChanged`.

## 0.34.0 (2026-09-30)

### Added
- **W-106 outgoing links** and **W-107 unlinked mentions** in the pane
  under the note (Alt+L), after its backlinks. Unlinked mentions: the
  lines of other notes that name it (or one of its aliases) as a whole
  word without a link, not in code or properties; `l` on one makes the
  link (`[[word]]`) in that note. Outgoing links: the note's links, each
  once, from its text as it is now (missing notes marked); Enter opens the
  note, or offers to make a missing one.

## 0.33.0 (2026-09-30)

### Added
- **W-26 outline**: Alt+O ("Outline") lists the note's headings, indented
  by level (not those in code); type to filter, Enter jumps to one.

## 0.32.0 (2026-09-30)

### Added
- **W-108 random note**: "Open random note" in the palette opens a note
  chosen at random (not the open one).

## 0.31.0 (2026-09-30)

### Added
- **W-105 PDF text**: "PDF to note" in the palette lists the vault's PDFs;
  the one chosen has its text written into a new note beside it (`Name`,
  else `Name (PDF text)`), starting with a link back to the PDF, for
  reading, searching and linking. A damaged PDF gives a message, not a
  crash. Uses the pure-Rust `pdf-extract` crate.

## 0.30.0 (2026-09-30)

### Added
- **W-104 the Mermaid plugin**: ```` ```mermaid ```` blocks drawn as text.
  Flowcharts (`graph` / `flowchart`) become a tree of their links: each
  node with its shape as written, labelled links (`├─yes─▶ [OK]`), dotted
  and thick links, a node reached again as `↺ name`. Sequence diagrams get
  their participants side by side, lifelines, arrows with their text,
  messages to oneself, notes and `loop` / `alt` rows (a list when too
  wide). Other diagram types show their source and say so. Installed in
  the example vault: `Examples/Diagrams.md`.

## 0.29.0 (2026-09-30)

### Added
- **W-103 link preview**: Alt+P ("Preview link under cursor") shows the
  note the link under the cursor points to in a popup over the workspace,
  read-only, at the link's heading. ↑/↓, PgUp / PgDn and the mouse wheel
  scroll; Enter opens the note there; Esc (or a click) closes it.

## 0.28.0 (2026-09-30)

### Added
- **W-102 properties**: a note's frontmatter properties are indexed.
  `aliases` find notes in the quick switcher and in `[[` suggestions (the
  alias is shown; the link becomes `[[Note|Alias]]`). Search takes
  `[name]` and `[name:value]`. "Show properties" lists every property with
  its note count; Enter searches for its notes. "Rename property" renames
  one in every note's frontmatter (notes open with unsaved changes are
  left out, and named).

## 0.27.0 (2026-09-30)

### Added
- **W-30 renaming**: "Rename note" (F2: the note chosen in the file
  explorer while it has the focus, else the open one) and "Rename folder"
  (the folder chosen in the explorer) update every link to the renamed
  notes, in their own style: `[[Name]]` stays a name, `[[Folder/Name]]` a
  path, headings, aliases and embeds kept, Markdown links re-encoded; not
  in code. Open notes show the new links; a note with unsaved changes is
  left as it is (the message says so). "New folder" makes a folder (paths
  too) in the chosen folder. "Move note to a folder" updates path links
  the same way now. Questions take Ctrl+U to clear their text.

## 0.26.0 (2026-09-30)

### Added
- **W-101 unique note**: "New unique note" in the palette makes a note
  named by the time (`202609301542`; ` 1`, ` 2` … within the same minute)
  in the folder new notes go to, and opens it.

## 0.25.0 (2026-09-30)

### Added
- **W-100 search terms**: several words must all be in the note (anywhere);
  `"a phrase"` is one piece; `-word` leaves notes out; `OR` gives either
  (it binds less than the words side by side); `( )` group. Operators:
  `tag:` (as before), `file:` (the note's name), `path:` (its path),
  `line:(a b)` (words on one line), `task:`, `task-todo:`, `task-done:`
  (task lines, with an optional text). Each word stays smart case.

## 0.24.0 (2026-09-30)

### Added
- **W-99 links to missing notes**: following a link to a note that isn't
  there (Ctrl+Enter, a click) asks "Create …?"; Enter makes it (at the top
  of the vault, or where the link's path says, folders included; a folder
  template applies) and opens it. Uses mdedit 3.2.0's
  `Outcome::MissingLink`.

## 0.23.0 (2026-09-30)

### Added
- **W-98 word count**: the status bar shows the note's words and
  characters (`120 words · 640 chars`; the properties at the top left
  out), or, with text selected, its words of all (`12 of 120 words`).

## 0.22.0 (2026-09-30)

### Added
- **W-97 web links open in the browser**: a click on a web link
  (`https://…`, `mailto:`), or Ctrl+Enter on it, opens it in the desktop's
  browser (`xdg-open`; `open` on macOS). Uses mdedit 3.2.0's
  `Outcome::OpenUrl`.

## 0.21.0 (2026-09-30)

### Added
- **W-89 the Recent Files plugin** (not installed by default; install it
  on the settings' Plugins page). A Recent tab in the sidebar, after Files,
  Search and Tags: the last 25 notes you visited (opened, or switched to
  their tab), newest first, each with its folder. Enter or a click opens
  one, Delete takes it off the list. Kept per vault in
  `.blackglass/plugins/recent-files/recent.toml`; a moved note keeps its
  place; the length is a setting. "Recent Files: Open" shows the tab,
  "Recent Files: Clear list" empties it. The
  tab and the command are there only while the plugin is enabled.
- Plugins can have a sidebar tab (`Plugin::sidebar_tab`, rows, keys), show
  it (`Effect::ShowSidebarTab`), and hear of every note visited or moved
  (`on_note_opened`, `on_note_moved`).

### Fixed
- The vault picker at start also closes with Ctrl+Q or Ctrl+C, and gives
  the terminal back even if drawing fails.

## 0.20.1 (2026-09-30)

### Fixed
- Ctrl+B goes through every open pane, in screen order, again and again:
  the sidebar, the calendar (when shown), the note, the backlinks (when
  shown). It went back and forth between the sidebar and the note after
  the first time.

## 0.20.0 (2026-09-30)

### Added
- **W-88 open a vault**: a vault is any folder. `blackglass` without a
  folder starts on a vault picker: the recent vaults, or type any folder's
  path (`~` for home; the folders there are listed, Tab completes one);
  Enter opens it, and a folder that doesn't exist yet is made after Enter
  again. "Open vault" in the palette switches to another vault at any time
  (its notes, plugins and settings; not while notes have unsaved
  changes). The recent vaults are kept in `~/.config/blackglass/vaults.toml`.

### Changed
- `blackglass` without a folder no longer opens the current folder; use
  `blackglass .` for that.

## 0.19.0 (2026-09-30)

### Added
- **W-25 backlinks** (core): Alt+L (or "Toggle backlinks") shows the notes
  that link to the open note in a pane under it, with the focus: each
  note's name and folder, then its lines that link (wiki links with an
  alias or a heading, embeds, Markdown links; not in code). ↑/↓ choose,
  Enter or a click opens the note at the line, Esc goes back to the note;
  Alt+L again hides them. They follow the open note and update when notes
  are saved.

## 0.18.1 (2026-09-30)

### Removed
- The **Today** button at the top of the sidebar (W-66). Alt+D still opens
  today's note, and the empty editor side still lists it; plugin commands
  no longer have sidebar buttons (`PluginCommand::button`).

## 0.18.0 (2026-09-30)

### Added
- **W-73 the settings window**: one window in two panes, instead of the
  settings menu, the shortcuts screen, the plugin settings screens and the
  plugins window. On the left a search box and the pages in groups:
  **Options** (Editor, Keyboard shortcuts, Plugins) and **Plugin options**
  (each installed plugin's settings; a Community plugins group can follow).
  On the right the chosen page: each setting with what it does below it
  and its value on the right, sections as headings. Typing searches every
  page (the list keeps only the pages that match). ↑/↓ choose a page (shown
  at once), Enter goes into it, Esc goes back, and closes. Clicks choose
  pages and rows.

### Changed
- "Open plugins" opens the settings on the Plugins page; → (or ⚙) opens a
  plugin's settings page (was `s`). "*Plugin*: Settings" opens its page.

## 0.17.0 (2026-09-30)

### Added
- **W-72 indentation width**: Settings → Editor → Indentation (1 to 8
  spaces; the default stays 2). Tab and Shift+Tab indent and outdent list
  items by that much, and the list levels' guides follow it. Uses mdedit
  3.1.0's `indent_width`.

## 0.16.1 (2026-09-30)

### Changed
- **W-53 calendar**: a day (week, month) without a note asks "Create
  2026-09-12?" before the note is made (Enter creates it; "Don't create
  it" or Esc leaves it). A day with a note opens as before. Alt+D, Today
  and the palette's "Open daily note" still create at once.

## 0.16.0 (2026-09-30)

### Added
- **W-67 folder templates** (Templater): a list of folders and their
  templates, in Templater's settings (Alt+, → Plugins → Templater, or
  "Templater: Settings"): each folder with its template, and "Add a folder
  template" (choose the folder, then the template). A new note in a folder,
  or in a folder inside it, gets the nearest one's template, with its
  questions asked; `/` is for the whole vault. Saved as
  `[folder_templates]` in `.blackglass/plugins/templater/settings.toml`
  (`Books = "Book"`). Also "Templater: Add a folder template" in the palette.
- **W-68 apply a template**: "Templater: Apply template to this note" in
  the palette puts a template on top of the open note; its properties are
  joined with the note's (the note's own values stay). One undo step.
- The example vault gives new notes in `Books/` the Book template.
- Plugins: `Plugin::on_note_created` (fill a new empty note); settings
  screens can have action rows (`Kind::Action`); keys that aren't plain
  words are quoted in settings files.

### Fixed
- A plugin replacing the note's text no longer adds an empty last line.

## 0.15.0 (2026-09-29)

### Added
- **W-66 today's note in one step**: with Periodic Notes enabled, **Alt+D**
  or the **Today** button at the top of the sidebar opens today's daily
  note (created from its template if it's new). The empty editor side
  lists it with its key. The key can be changed in Settings → Keyboard
  shortcuts.
- Plugin commands can have default keys and a button at the top of the
  sidebar (`PluginCommand::keys`, `PluginCommand::button`); a default key
  another command already has stays with that command.

## 0.14.1 (2026-09-29)

### Changed
- The empty editor side (no note open) lists Help too, and shows every
  command's keys as they are now (changed shortcuts included).
- blackglass no longer names another note app anywhere a user sees: the
  help page, `--help`, the README, the example vault, the package
  description and the plugins' credits. `tests/project.rs` checks it.

## 0.14.0 (2026-09-29)

### Added
- **W-64 delete a note**: "Delete note" in the palette asks first, then
  moves the note to the vault's `.trash/` folder (hidden from the vault;
  bring it back from there) and closes its tab.
- **W-65 move a note**: "Move note to a folder" in the palette lists the
  vault's folders (type to filter); the note moves there, and its tab and
  back / forward history follow it.

## 0.13.0 (2026-09-29)

### Added
- **W-61 help**: F1 or Ctrl+H opens the help page (built into blackglass,
  not a note in the vault) in a read-only tab, ending with every command
  and its keys as they are now.
- **W-62 keyboard shortcuts**: every command's keys can be changed in
  Settings → Keyboard shortcuts (type to filter, Enter then the new key,
  Delete for none, Ctrl+D for the default). A key taken from another
  command says so. Saved to `~/.config/blackglass/keys.toml` (only the
  changes) and read at start; plugin commands can have keys too.
- **W-63 settings window**: Alt+, (or Ctrl+, where the terminal sends it,
  or "Settings" in the palette): Editor, Keyboard shortcuts, Plugins. The
  editor's settings (auto-pair, undo steps, done tasks, images, colors,
  symbols, heading colors) are saved to `~/.config/blackglass/config.toml`
  and used at once.
- Palette: "Command palette", "Help", "Settings", "Extract selection to a
  new note", "Next tab", "Previous tab"; it shows each command's keys as
  they are now.

### Changed
- "Find and replace" is on Ctrl+R (mdedit's key) in the palette; Ctrl+H is
  help.
- Every workspace key goes through the keymap (except Alt+1 … Alt+9).

## 0.12.0 (2026-09-29)

### Added
- **W-60 extract to a new note**: Ctrl+X with text selected opens the
  new-note window ("Extract to a new note"); Enter makes the note with the
  text, Tab makes it from a template (the text goes at the template's
  cursor, else after its text). The selection in the original becomes a
  `[[link]]` to the new note (one undo step, not saved yet), and the new
  note's tab stays open. Esc cancels. Without a selection, Ctrl+X still
  closes the tab.
- The Welcome note lists Ctrl+Q on its own line, and Ctrl+X.

## 0.11.0 (2026-09-29)

### Added
- **A new note from a template**: in the Ctrl+N window, **Tab** shows the
  templates (Templater); the note is created from the one chosen, with
  the name typed (or the template flow asks for one), in the folder
  selected in the file explorer. Without the Templater plugin, Tab says
  how to get it. Also "New note from template" in the palette. (No
  Ctrl+Shift+N: most terminals send it as Ctrl+N, and Konsole keeps it for
  New Window.) Plugins' commands can get the typed name (`Context::name`).
- The example vault's Welcome note lists Ctrl+B (sidebar ↔ note), Tab in
  the new-note window, Alt+C, Alt+V and Ctrl+P.

## 0.10.1 (2026-09-29)

### Added
- `run.sh`: runs the latest debug build on the example vault
  (`./run.sh`, or `./run.sh Dataview.md` for a note; options pass on).

### Fixed
- A crash on a 0-wide screen (a terminal that reports 0×0 at first, as
  some do while a window opens): the editor column's position underflowed.
  The screen-size test now includes 0×0, 0×5 and 5×0.

## 0.10.0 (2026-09-29)

### Added
- **W-28 back / forward**: blackglass remembers the notes you were on and
  where the cursor was. **Ctrl+Alt+← / →** (as in Obsidian), the palette's
  "Go back" / "Go forward", and **the mouse's back / forward buttons** move
  through them; a note whose tab was closed opens again, and one deleted
  since is skipped (the status bar says so). Opening another note clears
  what's forward. Alt+← / → still switch tabs.
- The mouse's side buttons: crossterm dropped them, so blackglass carries
  a patched crossterm 0.29 (`vendor/crossterm`, see its `SOURCE.md`) that
  reads xterm buttons 8 and 9. `scripts/mouse_probe.py` shows what a
  terminal sends.

## 0.9.1 (2026-09-29)

### Fixed
- Clicking a link that follows a plain word with the same letters
  ("charts and queries in [[Charts]]") did nothing: the plain word had the
  link's click area. mdedit 3.0.1 finds the link itself.

## 0.9.0 (2026-09-29)

### Added
- **W-57 view mode** (mdedit 3.0, V-12): **Alt+V** cycles the note's
  mode: live preview → source → view. View mode is read-only and renders
  every line; ↑↓ / PgUp / PgDn move a row cursor over what's drawn
  (embedded notes, images and query results too), Tab / Shift+Tab go to
  the next / previous link or result, Enter follows it, Esc edits again.
  Palette: "View mode", "Edit mode", "Source mode", "Cycle modes"; the
  status bar says VIEW.
- **W-58 actionable query results** (DV-40): a Dataview LIST / TABLE row
  and a TASK page row open the page, a task row opens its note at the
  task; in DataviewJS, pages in `dv.list` / `dv.table`, `dv.taskList`
  tasks and the new `dv.fileLink(name)` do the same.
- **W-59 clickable links**: a click on a link, an embed or a result row
  follows it, in the live preview (not on the line being edited, which is
  raw text) and in view mode; an image opens in the system viewer.

### Changed
- Ctrl+Shift+V stays the terminal's paste (mdedit 3.0); the mode key is
  Alt+V.

## 0.8.1 (2026-09-29)

### Fixed
- **Opening a note opens its vault**: `blackglass Vault/Folder/Note.md`
  used the note's own folder as the vault, so a note in a sub-folder had
  none of the vault's plugins (its `dataview` / `dataviewjs` blocks showed
  their code). The vault is now the nearest folder above the note with
  `.blackglass/` or `.obsidian/`, as in Obsidian; else the note's folder.

## 0.8.0 (2026-09-29)

### Added
- **W-54 a JavaScript engine for plugins**: Boa (pure Rust) in a
  sandbox: scripts see only what the plugin passes in, no files or
  network; loop and recursion limits stop a runaway script with an error.
- **W-55 DataviewJS** (DV-46): ```` ```dataviewjs ```` blocks run
  JavaScript with Dataview's `dv`: `dv.pages(source)`, `dv.current()`,
  `dv.page()`, `dv.date()`, `dv.func.dateformat()`; output with
  `dv.header`, `dv.paragraph` / `dv.span` (with `{color, bold}` for
  charts), `dv.el`, `dv.list`, `dv.table`, `dv.taskList`, `console.log`;
  page lists with `where`, `map`, `sort`, `groupBy`, `limit`, `sum`, `avg`
  …. A Dataview setting, "Enable JavaScript queries", turns it off.
- **W-56 Templater JavaScript** (TP-04): a template with `<%* %>` runs as
  one async function, as in Templater: `tR`, variables shared between
  commands, loops and conditions around text, `await tp.system.prompt` /
  `suggester` (asked in popups; later questions may depend on earlier
  answers), and `moment()` (`format`, `add`, `subtract`).
- Examples: `example_vault/JavaScript/Charts.md` (a waistline line chart
  in braille, sparklines, sleep bars, moods, a journal heatmap, a link
  graph around Dune), `JavaScript/Queries.md`, and the templates
  `Templates/Meeting` and `Templates/Weekly review`; four weeks of daily
  notes to chart. `tests/examples.rs` keeps them working.

## 0.7.0 (2026-09-29)

### Added
- **W-52 plugin panels**: a plugin can draw a panel at the bottom of the
  sidebar and get its keys and clicks (Obsidian's views). **Alt+C** (or
  "Toggle calendar" in the palette) shows it with the focus, and hides it
  again; Esc goes back to the editor.
- **W-53 the calendar**, Periodic Notes' panel (PN-23): a month with ISO
  week numbers; days, weeks and the month with a note are highlighted,
  today is underlined. ←→↑↓ move by day / week, PgUp / PgDn by month, `t`
  goes to today; Enter (or a click) opens or creates the day's note, `w`
  the week's, `m` the month's; a click on a week number or the month opens
  those, ‹ / › change the month.

## 0.6.0 (2026-09-29)

### Added
- **W-51 plugin settings screens**: every plugin declares its settings
  (text, on / off, or a choice, with a label, help and default), and one
  settings screen edits them, grouped by section. Open it with `s` (or
  a click on ⚙) in the plugins window, or "*Plugin*: Settings" in the
  palette. Changes are saved at once to the plugin's `settings.toml` and
  the plugin reloads them. Settings: Dataview's date and date-time
  formats, "No results" text and page column header (DV-44); Templater's
  templates folder (TP-32); every Periodic Notes period's enabled /
  format / folder / template (PN-26).

### Changed
- moment.js dates (`plugins::moment`) are shared by the plugins.

## 0.5.1 (2026-09-29)

### Changed
- **Every plugin has its own folders**: code in `src/plugins/<name>/`, and
  data in the vault in `.blackglass/plugins/<id>/`. Settings moved:
  `.blackglass/templater.toml` → `.blackglass/plugins/templater/settings.toml`,
  `.blackglass/periodic-notes.toml` →
  `.blackglass/plugins/periodic-notes/settings.toml` (move yours if you
  made one). `.blackglass/plugins.toml` still lists what's installed.
- Templater and Periodic Notes checked against their documentation: the
  gaps are in `requirements/templater_requirements.md` (TP-28 … TP-32
  added) and the new `requirements/periodic_requirements.md` (PN-xx).

## 0.5.0 (2026-09-29)

### Added
- **W-48 … W-50 Periodic Notes**, the third plugin (after Liam Cain's):
  "Open daily / weekly / monthly / quarterly / yearly note" opens the
  note for today, creating it from its template if it's new, and "Next /
  Previous periodic note" jump to the closest existing note of the active
  note's period. Each period has a moment.js name format (defaults as in
  the original: `YYYY-MM-DD`, `gggg-[W]ww`, `YYYY-MM`, `YYYY-[Q]Q`,
  `YYYY`), a folder and a template, in `.blackglass/periodic-notes.toml`;
  a period can be turned off. Templates get Obsidian's `{{title}}`,
  `{{date:format}}`, `{{time}}`, `{{yesterday}}`, `{{tomorrow}}` and
  `{{monday:format}}` … for the note's date, and Templater commands
  (prompts too). This also covers W-31 (daily notes).
- Plugins can open a note (`Effect::Open`).
- The example vault's daily notes use `Templates/Daily` in `Journal/`.

## 0.4.1 (2026-09-29)

### Changed
- With mdedit 2.3: **Ctrl+V pastes** from the clipboard, and **source
  mode is Ctrl+Shift+V** (or Ctrl+Alt+V, which works in every terminal;
  Konsole keeps Ctrl+Shift+V for its own paste unless you unbind it). The
  palette has "Paste" and shows the new key for "Toggle source mode".
- Templater's `tp.system.clipboard()` reads the clipboard the way Ctrl+V
  does.

## 0.4.0 (2026-09-29)

### Added
- **W-45 … W-47 Templater**, the second plugin: templates are the notes in
  `Templates/` (or `templates_folder` in `.blackglass/templater.toml`).
  Palette commands: "Insert template", "Create new note from template"
  (in the folder selected in the file explorer, after asking its name) and
  "Replace templates in the active file" (one undo step). Commands:
  `tp.date.now` / `tomorrow` / `yesterday` / `weekday` with moment.js
  formats, offsets and reference dates; `tp.file.title`, `folder`, `path`,
  `creation_date`, `last_modified_date`, `tags`, `content`, `exists`,
  `include`, `cursor`, `selection`; `tp.frontmatter`; `tp.system.prompt`
  and `suggester` (asked in popups first) and `clipboard`. `<%- -%>` and
  `<%_ _%>` control whitespace. JavaScript (`<%* %>`) isn't supported and
  says so. What's missing: `requirements/templater_requirements.md`.
- Plugins can ask questions (text or a choice), replace the active note's
  text and create notes (`plugins::Effect`); commands get a `Context`
  with the active note and the target folder.
- `requirements/dataview_requirements.md`: Dataview compared with
  Obsidian's, with the missing pieces as requirements (DV-xx).

## 0.3.0 (2026-09-29)

### Added
- **W-23 link autocomplete**: typing `[[` (or `![[`) opens a list under
  the cursor with the 5 most recently changed notes; typing after `[[`
  turns it into a fuzzy search (as in the quick switcher). ↑/↓ choose,
  Enter or Tab (or a click) inserts the link and puts the cursor after
  `]]`; Esc closes the list for that link. A name two notes share is
  linked by its path (`[[Notes/Home]]`). The note being written isn't
  suggested; after `|` (alias) or `#` (heading) the list steps aside.
  The palette's "Insert wiki link" opens it too.

## 0.2.0 (2026-09-29)

### Added
- **W-37 / W-38 command palette** (Ctrl+P): every command in one list to
  filter and run, with its key: the workspace's, the editor's (save, save
  as, undo, redo, find, replace, source mode, tasks, fold, emoji, follow
  link …), insert commands (hyperlink, wiki link, embed, tag, callout,
  table, code block, rule, task, today's date), the enabled plugins', and
  "Open plugins".
- **W-39 plugins window**: install / uninstall (Enter) and enable /
  disable (Space) each plugin, with its version, author and description.
- **W-40 / W-41 plugin structure** like Obsidian's: a `Plugin` has a
  manifest, `on_load` / `on_unload`, commands for the palette, and code
  block processors that render its ```` ```lang ```` blocks in the editor
  (mdedit 2.2's `CodeBlockProcessor`). The state is saved per vault in
  `.blackglass/plugins.toml`.
- **W-42 … W-44 Dataview**, the first plugin: ```` ```dataview ```` blocks
  run `LIST`, `TABLE` and `TASK` queries with `FROM` (folders, tags,
  links, `and` / `or` / `-`), `WHERE`, `SORT` and `LIMIT`, over
  frontmatter, `key:: value` fields, tags, links, tasks and `file.*`, with
  functions (`contains`, `date(today)`, `dateformat`, `length` …) and
  `this`. Results follow saved changes. Not yet: `GROUP BY`, `FLATTEN`,
  `CALENDAR`, DataviewJS, inline queries (W-42 is 🟡).
- `example_vault/Dataview.md` shows queries to try.

### Changed
- **Ctrl+P is the command palette**, as in Obsidian; the quick switcher
  is Ctrl+O only.

## 0.1.0 (2026-09-29)

### Added
- The workspace, with mdedit 2.1 as the editor in every tab:
  - **W-01** open a vault (a folder, or a note in its folder).
  - **W-02 … W-05** the file explorer: folders, rainbow folder colors,
    sorting, the index kept up to date on save (F5 scans again).
  - **W-06 … W-12** tabs, tab keys and the tab bar; the folder path, the
    note's name as a title and a readable line width; unsaved changes
    asked about on close and quit; the status bar; the empty state.
  - **W-13 … W-17** the quick switcher, vault search with `tag:`, the tags
    panel, and links resolved anywhere in the vault.
  - **W-18 … W-21** new notes, Save As within the vault, the mouse, and
    fallbacks for small screens, 16 colors and ASCII.
- The example vault (`example_vault/`), the project's standards, skills and
  quality gate, carried over from mdedit.
