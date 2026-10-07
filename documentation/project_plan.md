# blackglass: project plan

blackglass is the wrapper project from mdedit's plan (section 6): an
Obsidian-style vault app with a sidebar and tabs, using mdedit as its
editor. Features and their tests: `requirements/features.md`.

## 1. Milestones

| Milestone | Goal | Items | Status |
|-----------|------|-------|--------|
| **M1** Workspace | A daily-usable vault: file explorer, tabs, quick switcher, vault search, tags, vault-wide links | W-01 … W-21 | ✅ 0.1.0 (W-05, W-20 finished with W-29, W-34) |
| **M2** Commands & plugins | A command palette, an Obsidian-like plugin structure (settings screens, panels, a JavaScript sandbox), Dataview (with DataviewJS), Templater (with JavaScript), Periodic Notes (with a calendar); view mode, clickable links, extract, help, settings with customizable shortcuts, move and delete, today's note button, folder templates, apply template, indentation width, the settings window, Recent Files; all of Templater (W-125, 0.69.0) | W-37 … W-73, W-89, W-125 | ✅ 0.2.0 … 0.21.0, W-42 complete in 0.77.0 |
| **M3** Vault features | Obsidian's vault features: backlinks, outline, bookmarks, navigation history, file operations, daily notes, autocomplete; **now:** the next batch (queue 6: web links, word count, missing links, search, unique note, renaming, properties, link preview, Mermaid, PDF text, outline, outgoing links, unlinked mentions, random note), then Git (queue 7), footnotes, comments and sync (queue 8); **done:** Zettelkasten (queue 15, W-119 … W-123 in 0.72.0 … 0.76.0; note IDs W-124 in 0.68.0) | W-22 … W-34, W-93 … W-111, W-118 (property editor, 0.41.0) | ✅ complete for 1.0.0 (W-23 in 0.3.0, W-28, W-31, W-25 backlinks in 0.19.0, W-88 open vault in 0.20.0; the core list W-22 … W-34 done in 0.70.0: click to place the cursor, dimmed missing links, watching the vault, the session, nested tags, Bookmarks, tag and property suggestions; every action a command with keys to change, 0.71.0; Zettelkasten and Citations, 0.72.0 … 0.76.0) |
| **M4** Queries | Block embeds, query embeds, Bases (queue 7), the Tasks plugin (queue 8) | W-35, W-36, W-74 … W-81 | 🟡 embeds 0.58.0 … 0.59.0, Tasks and Bases 0.50.0 … 0.57.0 (their parts left in queue 10 and 11) |
| **M5** AI | An AI plugin: chat with a model about the notes, custom commands and Quick Ask in the note, coding agents (Claude Code …) working in the vault, inline edits with a diff, vault search (after Claudian and Copilot for Obsidian) | W-82 … W-87 | ⬜ (requested 2026-09-30) |
| **M6** Appearance | Color themes: a default theme file, five example themes, a palette command to choose one | W-69 … W-71 | ✅ 0.44.0 … 0.46.0, 0.64.0 (the editor's colors, mdedit 3.8.0) |
| **M7** More plugins | Advanced Tables (tables edited as tables, formulas) | W-90 … W-92 | ✅ 0.47.0 … 0.49.0, 0.63.3, 0.67.0 |
| **M8** Later | Notes side by side (split panes), math, a graph view, file recovery, publishing, a web clipper | W-112 … W-117 | ⬜ (requested 2026-09-30) |

### Execution queue (requested 2026-09-29)

In order:

1. ✅ **Finish the project structure and a running POC** (0.1.0): the
   project scaffolding, standards, skills and quality gate carried over
   from mdedit; the workspace (M1) running on the example vault.
2. ✅ **Command palette (Ctrl+P), W-37 / W-38** (0.2.0). Every command in one
   filterable list to choose from and run: blackglass's own (new note,
   close tab, search, toggle sidebar, next tab, rescan, quit …), the
   editor's (save, save as, undo, redo, find, replace, source mode, toggle
   task, fold, emoji …), insert commands (**insert hyperlink**, wiki link,
   tag, callout, table, code block, horizontal rule, date), every enabled
   plugin's commands, and **"Open plugins"**. Each command shows its key.
   The quick switcher stays on Ctrl+O, as in Obsidian.
3. ✅ **Plugins window, W-39** (0.2.0). A window listing the available plugins with
   their name, version, author and description, where you **install and
   uninstall** each one (and enable / disable an installed one).
4. ✅ **Plugin structure like Obsidian's, W-40 / W-41** (0.2.0). A `Plugin` trait with
   a manifest (id, name, version, author, description), `on_load` /
   `on_unload`, commands added to the palette, and **code block
   processors** (Obsidian's `registerMarkdownCodeBlockProcessor`): a
   plugin renders ```` ```lang ```` blocks of its languages in the
   editor. Plugins are compiled in; which ones are installed and enabled
   is saved per vault in `.blackglass/plugins.toml` (like
   `.obsidian/community-plugins.json`). Needs one generic mdedit API: a
   `CodeBlockProcessor` in `Shared` that the editor asks when drawing a
   fenced block the cursor isn't in (mdedit 2.2.0).
5. ✅ **The first plugin: Dataview, W-42 … W-44** (0.2.0; complete in 0.77.0). An implementation of
   [Obsidian Dataview](https://blacksmithgu.github.io/obsidian-dataview/):
   a ```` ```dataview ```` block runs its query over the vault and shows
   the result in place of the code; with the cursor in the block, its
   source is shown to edit it.
   - Query types: `LIST`, `TABLE` (with columns and `AS` names), `TASK`.
   - `FROM` folders (`"Journal"`), tags (`#reading`), links (`[[Note]]`),
     with `and`, `or`, `-` (not).
   - `WHERE`, `SORT … ASC/DESC`, `LIMIT`; expressions with `=`, `!=`,
     `<`, `>`, `<=`, `>=`, `and`, `or`, `!`, `contains(…)`.
   - Fields: frontmatter properties, inline `key:: value` fields, and the
     implicit `file.name`, `file.path`, `file.folder`, `file.link`,
     `file.tags`, `file.ctime`, `file.mtime`, `file.size`,
     `file.day` (from a date in the name), `file.tasks`.
   - Commands: "Dataview: refresh", "Dataview: insert query".
6. ✅ **The next batch** (M3, 0.22.0 … 0.34.0; prioritized 2026-09-30 after the comparison
   with Obsidian, `documentation/obsidian_comparison.md`), in order:
   1. ✅ **W-97 web links** (0.22.0) open in the system browser (a click, Ctrl+Enter).
   2. ✅ **W-98 word count** (0.23.0): words and characters in the status bar (of the
      selection when there is one).
   3. ✅ **W-99 links to missing notes** (0.24.0): following one offers to create the
      note.
   4. ✅ **W-100 search terms** (0.25.0): several words (all must match), `-word`,
      `"exact phrase"`, `OR`, `file:`, `path:`, `line:`, `task:`,
      `task-todo:`, `task-done:`.
   5. ✅ **W-101 unique note** (0.26.0): a new note named by the time
      (`YYYYMMDDHHmm`), from the palette.
   6. ✅ **W-30 renaming** (0.27.0; any file or folder chosen in the explorer,
      renamed or moved, W-126 in 0.69.0): rename a note or folder (links to it updated in
      every note), new folder.
   7. ✅ **W-102 properties** (0.28.0): `aliases` in the quick switcher and `[[`
      suggestions; a properties view (every property in the vault and its
      notes); rename a property in every note.
   8. ✅ **W-103 link preview** (0.29.0): the note under the cursor's link in a popup
      (read-only, scrollable), from a key.
   9. ✅ **W-104 Mermaid** (0.30.0) (a plugin): ```` ```mermaid ```` flowcharts and
      sequence diagrams drawn in text.
   10. ✅ **W-105 PDF text** (0.31.0): a PDF's text into a new note (palette), for
       reading and linking.
   11. ✅ **W-26 outline** (0.33.0), **W-106 outgoing links**, **W-107
       unlinked mentions** (0.34.0), **W-108 random note** (0.32.0): the
       small wins.
7. ✅ **Git, W-93 … W-96** (M3, 0.35.0 … 0.39.0; W-93 back up in 0.35.0 … 0.36.0, W-94 see changes in 0.37.0, W-95 chores in 0.38.0 (no clone, submodules); queued 2026-09-30, moved to M3 the same day). A blackglass plugin
    (`src/plugins/git/`, id `git`) after the Git community plugin, running
    the user's `git` program in the background. Requirements (GT-xx) and
    order: `requirements/git_requirements.md`.
    - **W-93 back up:** the repository, commit-and-sync (commit, pull,
      push) with a message template, pull methods, reloading notes a pull
      changed, automatic commit-and-sync on a timer, pull on start, the
      status bar, the settings page.
    - **W-94 see changes:** the source control view (stage, unstage,
      discard, commit), history, diffs, conflicts.
    - **W-95 repository chores:** init, clone, `.gitignore`, remotes,
      branches, submodules, raw commands.
    - **W-96 in the editor:** change signs in the margin, hunks, line
      authors (`git blame`); needs a mdedit "line marks" API.
8. ✅ **Footnotes, comments, sync, W-109 … W-111** (M3, 0.39.0 … 0.40.0; prioritized
   2026-09-30): footnotes and `%%comments%%` in the editor (mdedit X-04 …
   X-08, then a footnotes list here); sync of the vault between machines
   (through the Git plugin: automatic commit-and-sync, pull on start).
9. ✅ **Color themes, W-69 … W-71** (M6, 0.44.0 … 0.46.0, 0.64.0;
   requested 2026-09-30): every blackglass color, the folder colors, the
   editor's colors (mdedit 3.8.0's palette), the plugins' results, five
   themes, choosing with a preview, the user's own themes.
   - **W-69 theme files.** A theme is a TOML file of named colors
     (`#rrggbb`, a 256-palette number or a color name), fitted to the
     terminal's color depth like every color today. Every color
     `ui::theme` has now becomes a key: the chrome, sidebar, tabs, raised
     and selected rows, prompts, text, muted, faint, accent, title, tags,
     the unsaved dot and the eight folder colors. The editor's colors
     (text, headings 1–6, links, code, quotes, tasks, highlights) need a
     generic mdedit API first (a palette in `Shared` that its renderer
     uses, with its own tests and version bump). A missing key falls back
     to the default theme's; a bad value is a warning in the status bar.
     - `themes/default.toml` in the repository: today's dark colors,
       embedded in the binary (`include_str!`), and the reference for
       every key, each with a comment.
   - **W-70 five example themes** in `themes/`, also embedded: a light
     theme ("Paper"), a warm dark one ("Ember"), a blue dark one
     ("Ocean"), a green one ("Forest"), and a high-contrast one for
     readability. Users add their own in `~/.config/blackglass/themes/`
     (the same format; a file with the name of a built-in replaces it).
   - **W-71 choose a theme**: "Choose theme" in the command palette (and
     an Appearance entry in the settings window) lists the themes, built-in
     and the user's; ↑/↓ preview each one at once, Enter keeps it, Esc
     goes back to the one before. The choice is saved in the user's config
     folder (`~/.config/blackglass/`) and used at start.
   - Tests: every built-in theme parses and sets every key; a partial
     theme falls back to the default; choosing, previewing, cancelling and
     saving in `tests/workspace.rs` (drawn colors checked on the
     `TestBackend` buffer); the palette command exists.
10. ✅ **Bases, W-74 … W-77** (M4, 0.54.0 … 0.57.0, 0.65.0; left in the
   requirements: regular expressions, cover images, keys on a kanban board,
   moving between cells). Obsidian's core
   Bases plugin as a blackglass plugin (`src/plugins/bases/`): database
   views of the notes, written in YAML. Requirements, checked online, with
   IDs (BA-xx) and an order: `requirements/bases_requirements.md`.
   - **W-74 read and show:** ```` ```base ```` blocks with `filters`
     (and / or / not), `formulas`, the expression language (operators,
     dates and durations, the functions for strings, numbers, lists,
     dates, links, files) and a table view; rows open their note.
   - **W-75 more views:** grouping, summaries (average, sum, median …),
     list, cards and kanban views, display names, custom summaries.
   - **W-76 `.base` files:** opened in a tab as the base, embeds
     (`![[File.base#View]]`, needs a mdedit embed hook), "Create new base"
     and "Insert new base", switching views, search, copy and CSV export,
     new note from a view.
   - **W-77 editing:** move between cells and edit properties in place
     (written to the notes' frontmatter), kanban moves, changing a view's
     sort / filters / columns / formulas in popups.
   Map view and row heights are left out (not for a terminal).
11. 🟡 **Tasks plugin, W-78 … W-81** (M4, 0.50.0 … 0.53.0, 0.66.0; left:
   JavaScript functions, styled fields in notes). The Tasks
   community plugin's features as a blackglass plugin (`src/plugins/tasks/`).
   Today's task handling stays **core** and unchanged: checkboxes, task
   states, Ctrl+T / Ctrl+L (mdedit) and Dataview's `TASK` queries; the
   plugin adds the rest on top. Requirements, checked online, with IDs
   (TK-xx) and an order: `requirements/tasks_requirements.md`.
   - **W-78 read and query:** the emoji fields (dates, priority), the
     global filter, ```` ```tasks ```` blocks with the status, date, file,
     description and tag filters, AND / OR / NOT, sort, group, limit and
     hide / show; checking a task off from a result.
   - **W-79 done dates and recurrence:** statuses with types, ✅ / ❌ dates
     when toggled, 🔁 rules making the next task, 🏁 on completion.
   - **W-80 editing:** the create-or-edit popup, suggestions while typing,
     postpone.
   - **W-81 the rest:** dependencies (🆔 ⛔, blocked / blocking), urgency,
     tree and columns layouts, `explain`, the global query, functions in
     JavaScript, the Dataview format, styled fields in notes (with mdedit).
12. ⬜ **AI, W-82 … W-87** (M5, requested 2026-09-30). One plugin, **AI**
   (`src/plugins/ai/`), after [Claudian](https://github.com/YishenTu/claudian)
   and [Copilot for Obsidian](https://github.com/logancyang/obsidian-copilot).
   Requirements, checked online, with IDs (AI-xx), where each comes from
   and an order: `requirements/ai_requirements.md`. Keys stay out of the
   vault; nothing is sent without a request.
   - **W-82 chat:** providers (Claude by default, OpenAI-compatible, local
     Ollama), a streaming chat pane, the active note and selection as
     context, instructions, the settings page.
   - **W-83 commands in the note:** Quick Ask on a selection, custom
     commands as notes with variables (`{}`, `{activeNote}`, `{[[Note]]}`
     …), insert / replace / copy.
   - **W-84 context:** `@` notes, folders, tags, URLs, images and PDFs;
     chat tabs, a side chat, saved history.
   - **W-85 agents:** Claude Code (and Codex, opencode …) with the vault as
     its folder, tool calls shown, approvals and modes, edits as diffs,
     skills, MCP through the agent.
   - **W-86 inline edit:** select, ask, see the change as a word diff in
     the note, accept or reject (needs a mdedit API for proposed changes).
   - **W-87 search:** vault questions with cited notes, semantic search
     (opt-in embeddings), relevant notes, projects.
13. ✅ **Advanced Tables, W-90 … W-92** (M7, 0.47.0 … 0.49.0, 0.63.3,
    0.67.0). A
    blackglass plugin (`src/plugins/tables/`, id `tables`) after the
    Advanced Tables community plugin. Every operation changes the note's
    text (one undo step), so no mdedit API is needed; Tab / Enter are
    taken only while the cursor is on a table line. Requirements (AT-xx)
    and order: `requirements/tables_requirements.md`.
    - **W-90 format and move:** tables padded and lined up (as you edit and
      by command), Tab / Shift+Tab between cells, Enter to the next row,
      a new table from a header line, the settings page.
    - **W-91 rows and columns:** insert, delete, move, align, sort,
      transpose, export as CSV; a table controls panel in the sidebar.
    - **W-92 formulas:** `<!-- TBLFM: … -->` lines: cell and range
      references, `sum` / `mean`, `if`, formats, chaining.
14. ⬜ **Later, W-112 … W-117** (M8, prioritized 2026-09-30): notes side
    by side (split panes, then saved workspaces), math (`$…$` as Unicode),
    a graph view (a note's links as a text tree, the vault's as a map),
    file recovery (snapshots of notes), publishing (the vault as a static
    web site), a web clipper (a URL into a note).
15. ✅ **Zettelkasten, W-119 … W-123** (M3, requested 2026-10-01; done 0.72.0 … 0.76.0 as the Zettelkasten and Citations plugins): everything
    the Zettelkasten method needs, compared with Obsidian's core plugins
    and the community plugins its users rely on. Requirements (ZK-xx) and
    an order: `requirements/zettelkasten_requirements.md`. Note IDs for
    every new note (W-124, 0.68.0: in the name, as the name or in the
    properties, in any time format) are done. Much is there already
    (unique notes, backlinks, mentions, previews, block links, renaming,
    extract, structure notes as queries).
    - **W-119 linking:** heading and block link suggestions (a block id
      added when chosen), vault-wide `[[##` / `[[^^`, copy a link to a
      block / heading / note, a unique note linked from here, the unique
      note's format, folder, template and title.
    - **W-120 structure:** Folgezettel IDs (sequel and branch notes, the
      sequence beside a note), breadcrumbs from `up` / `next` / `prev`,
      titles instead of IDs; with nested tags (W-32) and bookmarks (W-27).
    - **W-121 refactoring:** Note composer's extract and merge, extracting
      a heading, splitting a note at its headings.
    - **W-122 retrieval:** orphans, dead ends and unresolved links, a
      local graph as a tree, link counts, quick capture to an inbox.
    - **W-123 literature notes:** a bibliography, citations, literature
      notes from a template.
16. ✅ **Task Archiver, W-127** (M4, requested 2026-10-06; 0.78.0): the
    community plugin, compared in `requirements/archiver_requirements.md`.
17. **Requested 2026-10-06, in this order:**
    - ✅ **Natural language dates (core, W-128, 0.79.0):** `@today`,
      `@tomorrow`, `@next friday` … turned into links to their days as
      you type, after the community plugin.
    - ✅ **Emoji Shortcodes** (plugin, W-129, 0.80.0): `:smile:`
      suggested and replaced.
    - ✅ **Colored highlights (W-130, 0.81.0):** `==🔴text==` in colors,
      as Obsidian 1.14 has them.
    - ✅ **Periodic Notes, the rest (0.82.0):** its gaps in
      `requirements/periodic_requirements.md` (PN-27 ribbon icons ✗).
    - ✅ **Table formulas and undo (0.90.0 … 0.92.0):** TBLFM formulas
      worked out by themselves, spreadsheet cells (`=SUM(B2:B4)`), 1000
      undo steps, undoing changes to other notes (W-133).
    - **Importing (W-132, planned):** Evernote `.enex` exports and Google
      Keep's Takeout folder into notes (text as Markdown, checklists as
      tasks, labels as tags, dates as properties, attachments beside them).
    - ✅ **Bases kanban (0.83.0):** Obsidian 1.14's kanban (`groupOrder`,
      None), keys to move cards and columns, new notes in a column,
      colors. Next: the Group menu and collapsible groups for every
      layout (BA-47b), and cards on boards embedded in notes (a mdedit
      API for actions on part of a line).
18. **Requested 2026-10-07** (from the ad hoc list):
    - ✅ **W-134 encrypt / decrypt (0.93.0):** a selection, with a password
      and an optional hint, in Meld Encrypt's format (its older texts read
      too). Whole-note encryption isn't wanted (decided).
    - ✅ **W-135 task estimates (0.94.0):** `[estimate:: 2h]` on tasks, a
      duration as in Dataview, summed by one-line queries; no extra
      display (decided).
    - ✅ **W-137 query blocks and the mouse (0.95.0, mdedit 3.20.0):**
      hover frame with a `</>` button, clicks on links and tasks in
      results without showing the source; the source by keys or the
      button.
    - **W-136 sequenced hotkeys:** not now (decided); a leader key and a
      popup of what can follow, if it comes back.
    - **W-115 file recovery** (snapshots with a diff) moved up from M8.
19. **Queued 2026-10-07, in this order** (decided, not started):
    - **Importer (W-132, W-138 … W-143):** a plugin with one module per
      format and a shared HTML → Markdown converter; Evernote and Google
      Keep first, then HTML / Markdown / text / TextBundle / CSV, Notion,
      Apple Notes and Bear, OneNote / Word / Excel (sheets as tables),
      Joplin and Day One, Logseq and Roam
      (`requirements/importer_requirements.md`).
    - **Publish (W-116):** notes with `publish: true` as a static site in
      a folder, with backlinks, tags, search and queries as their
      results, and a local preview (`requirements/publish_requirements.md`).
20. **Queued 2026-10-07: macOS binaries (W-144)** (not started, for later):
    - A GitHub Actions workflow in each repository (blackglass and
      mdedit), run when a release is published: build on macOS runners
      for Apple Silicon (`aarch64-apple-darwin`) and Intel
      (`x86_64-apple-darwin`), join them into one universal binary
      (`lipo`), and upload `<name>-<version>-macos.tar.gz` and checksums
      to the same release. blackglass's checks out mdedit beside it (a
      path dependency). Then run it on the latest releases too.
    - Not cross-compiled here: Apple's SDK may only be used on Apple
      hardware, and there's no Mac to test on.
    - Pushing `.github/workflows/` needs a token with the "Workflows"
      permission (the current fine-grained token may lack it), or the
      files are committed from GitHub's website.
    - Not notarized (that needs a paid Apple Developer account): the
      release notes say to clear the quarantine flag once
      (`xattr -d com.apple.quarantine blackglass`), or to download with
      `curl`.
    - The code already handles macOS: `pbcopy` / `pbpaste`, `open`.
21. **Queued 2026-10-07: Windows support (W-145)** (not started, for
    later; it compiles for `x86_64-pc-windows-gnu` with no errors or
    warnings, checked 2026-10-07):
    - The config folder: `%APPDATA%\blackglass` on Windows (now only
      `$XDG_CONFIG_HOME` / `$HOME`, which Windows usually lacks, so
      settings, keys, the theme and recent vaults aren't kept).
    - Copy and paste: `clip.exe` and PowerShell's `Get-Clipboard` (in
      blackglass and mdedit).
    - Opening files and links: `cmd /C start "" <path>`.
    - Shell commands (the Git plugin's script, Templater's user
      commands): `cmd /C` instead of `sh -c`.
    - Paths: about 90 places turn paths into text; check each keeps `/`
      in the vault's relative paths (the explorer, renames, the session),
      as Dataview's index does.
    - The mouse's side buttons: crossterm's Windows input reads only the
      left, middle and right buttons.
    - Builds and tests: a Windows runner in GitHub Actions (with the
      macOS workflow, W-144) running the test suite and building
      `blackglass.exe` / `mdedit.exe` for every release.
22. **Backlog 2026-10-07: mdedit on crates.io** (not started, for later):
    publish mdedit as a library on crates.io so blackglass builds without
    a `../mdedit` checkout beside it (simpler source builds, a simpler
    Homebrew formula, a step towards homebrew-core) and others can embed
    the editor (docs on docs.rs).
    - The name `mdedit` is taken on crates.io (another project); free
      ones (checked 2026-10-07): `mdedit-tui` (suggested),
      `mdedit-live`, `tui-mdedit`, `blackglass-mdedit`. The code keeps
      saying `mdedit` (Cargo renames the dependency). `blackglass` is
      free too.
    - mdedit's `Cargo.toml` needs its metadata (description, license,
      repository, homepage, readme, keywords, categories) and which
      files to ship (the package is ~2 MB with its tests).
    - blackglass depends on the published version; development keeps
      `../mdedit` through a local override. README build steps, the
      release script and the Homebrew formula (no mdedit resource) change.
    - Needs a crates.io account and API token (the user's). Published
      versions are permanent (only yanked); mdedit is published before a
      blackglass release that needs it; its API follows semver (a
      breaking change is 4.0).

## 2. M1: Workspace ✅

Done in 0.1.0: see `VERSION.md`. W-05 and W-20 were finished by M3's
file watching (W-29) and clicks in the text (W-34).

## 3. M2: Commands & plugins

Done in 0.2.0: see the execution queue above and `VERSION.md`. What
Dataview (W-42) has all of Obsidian's (0.77.0); each piece and its test:
`requirements/dataview_requirements.md`.

Next plugins: the user's Obsidian plugins ranked by effort are in
`requirements/plugin_candidates.md`.

## 4. M3: Vault features

Suggested order:

1. ✅ **W-31 daily notes** (the Periodic Notes plugin, 0.5.0); ✅ **W-28
   back / forward** with the mouse's back / forward buttons (0.10.0).
2. **W-25 backlinks** and **W-26 outline**: new sidebar panels (the
   sidebar's panel list is built for more).
3. **W-30 rename / move / delete** with link updates, and **W-29 file
   watching**.
4. **W-33 remember tabs**, **W-27 bookmarks**, **W-32 nested tags**.
5. Needs mdedit first (a generic API there, then use it here):
   - W-22 unresolved-link styling: mdedit calls `Resolver::exists` when
     drawing links;
   - W-24 tag / property autocomplete (W-23, link autocomplete, is done
     in 0.3.0 with mdedit's public API, so W-24 can be done the same way);
   - W-34 click to place the cursor: a screen position → text position
     API on `EditorView`.

## 5. M4: Queries

Block embeds (`![[Note#^id]]`, mdedit E-03), `query` blocks (E-08), Bases
(E-09; W-74 … W-77, `requirements/bases_requirements.md`) and the Tasks
plugin (C-02; W-78 … W-81, `requirements/tasks_requirements.md`) run over
the vault index here, and are drawn through the code block processor and
embed APIs. Dataview (M2) is the model for these.
