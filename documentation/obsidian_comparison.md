# blackglass compared with Obsidian

Every Obsidian feature area, checked against blackglass 0.21.0 and mdedit
3.1.0 (2026-09-30). Obsidian's side is from its help pages (the 30 core
plugins, search, links, properties, the workspace) and, for the editor,
mdedit's `requirements/obsidian_features.md` (98 syntax and editing
items). Where blackglass plans a feature, its ID is given (W-xx in
`requirements/features.md`, a queue item in `project_plan.md`).

**Now:** ✅ there · 🟡 partly · ⬜ missing · ✗ not planned (with the reason)

## Done since this comparison (0.22.0 … 0.67.0)

The "next batch" (plan queue 6) closed these gaps; the tables below are
as they were on 2026-09-30 before it.

| Gap | Now | Where |
|-----|-----|-------|
| Web links open in the browser | ✅ | W-97 (0.22.0), mdedit 3.2.0 `Outcome::OpenUrl` |
| Word count | ✅ | W-98 (0.23.0), in the status bar |
| A link to a missing note creates it | ✅ | W-99 (0.24.0), mdedit 3.2.0 `Outcome::MissingLink` |
| Search terms: words, `-`, `"phrase"`, `OR`, `( )`, `file:`, `path:`, `line:`, `task:`, `task-todo:`, `task-done:`, `[property]` | ✅ | W-100 (0.25.0), W-102 |
| Unique note creator | ✅ | W-101 (0.26.0) |
| Rename notes and folders with link updates, new folder | ✅ | W-30 (0.27.0) |
| Properties: aliases in the switcher and links, a properties list, rename a property, a typed editor | ✅ | W-102 (0.28.0), W-118 (0.41.0), mdedit 3.6.0 |
| Page preview (a key, not hover) | ✅ | W-103 (0.29.0), Alt+P |
| Mermaid diagrams (flowcharts, sequence diagrams, pie, Gantt) | 🟡 | W-104 (0.30.0, 0.43.0), a plugin; other diagram types show their source |
| PDF text into a note | ✅ | W-105 (0.31.0) |
| Random note | ✅ | W-108 (0.32.0) |
| Outline | ✅ | W-26 (0.33.0), Alt+O |
| Outgoing links, unlinked mentions | ✅ | W-106, W-107 (0.34.0), in the Alt+L pane |
| Git: backup, sync, changes, history, diffs, branches, margin marks, blame, hunks, clone, conflicts | ✅ | W-93 … W-96, W-111 (0.35.0 … 0.42.0), the Git plugin |
| Footnotes and `%%comments%%` | ✅ | W-109, W-110 (0.40.0), mdedit 3.5.0 |
| Color themes (five built in, your own, chosen with a preview) | ✅ | W-69 … W-71 (0.44.0 … 0.46.0, 0.64.0: the editor's colors, mdedit 3.8.0's palette) |
| Advanced Tables: tables edited as tables, formulas, controls | ✅ | W-90 … W-92 (0.47.0 … 0.49.0, 0.63.3, 0.67.0) |
| Tasks: dates, priorities, recurrence, queries, suggestions, tree and columns | 🟡 | W-78 … W-81 (0.50.0 … 0.53.0, 0.66.0); JavaScript functions and styled fields to come |
| Bases: filters, formulas, table / list / cards / kanban views, `.base` files, embeds, editing | ✅ | W-74 … W-77 (0.54.0 … 0.57.0, 0.65.0) |

Still to come, by priority (plan section 1): AI (M5); side by side, math,
graph view, file recovery, publishing and a web clipper (M8).

## Summary

| Area | ✅ | 🟡 | ⬜ | ✗ | Notes |
|------|----|----|----|----|-------|
| Markdown and editing (mdedit) | 71 | 2 | 24 | 1 | Missing: math, Mermaid, footnotes, comments, HTML, property types, block embeds; audio / video / PDF embeds are placeholders |
| Core plugins (30) | 4 | 6 | 16 | 4 | See section 2 |
| Links | 6 | 1 | 5 | 0 | Missing: unresolved-link styling, block links, renaming with link updates, unlinked mentions, making a note from a link to a missing one |
| Search | 3 | 1 | 4 | 0 | Text and `tag:` only; no operators, OR / NOT, regex |
| Properties | 1 | 1 | 5 | 0 | Shown dimmed; no typed editor, no properties view |
| Files and the vault | 4 | 3 | 3 | 0 | |
| Workspace and interface | 9 | 0 | 5 | 2 | No split panes, pinned tabs, workspaces, right sidebar |
| Customizing | 3 | 1 | 2 | 0 | Themes are planned (M6); no CSS (a terminal) |
| Services and platforms | 0 | 1 | 1 | 5 | Sync and Publish are Obsidian's services; Git (queued) covers syncing |

blackglass also has what Obsidian gets only from community plugins:
Dataview (with DataviewJS), Templater (with JavaScript and folder
templates), Periodic Notes with a calendar, Recent Files, and more queued
(Bases, Tasks, AI, Advanced Tables, Git).

## 1. Markdown and editing (mdedit)

mdedit covers 71 of Obsidian's 98 syntax and editing items fully (the
`obsidian://` URI is left out on purpose): headings,
emphasis, highlights, lists and task states, links and embeds (notes,
headings, images), callouts, code with highlighting, tables, tags,
frontmatter shown dimmed, live preview, source mode, view mode, folding,
find and replace, undo, selection, emoji, soft wrap. What's left:

| Feature | Now | Where | Notes |
|---------|-----|-------|-------|
| Frontmatter as a property editor (types: checkbox, date, list chips) | 🟡 | mdedit P-01 … P-04, P-06 | Shown dimmed, raw when the cursor is in it |
| Math: `$…$`, `$$…$$` | ⬜ | mdedit X-01, X-02 | A Unicode approximation, or code |
| Mermaid diagrams | ⬜ | mdedit X-03 | As code, or ASCII drawings |
| Footnotes (reference, definition, inline) | ⬜ | mdedit X-04 … X-06 | |
| Comments `%%…%%` | ⬜ | mdedit X-07, X-08 | Dimmed |
| HTML inline and in blocks | ⬜ | mdedit X-09, X-10 | |
| Block embeds `![[Note#^id]]` | ⬜ | W-35, mdedit E-03 | |
| Audio, video, PDF embeds | ⬜ | mdedit E-06, E-07 | A placeholder in a terminal; open outside |
| Query and Bases embeds | ⬜ | W-36, W-74 … W-77 | |
| Tasks emoji fields styled | 🟡 | mdedit C-01, W-78 … W-81 | Shown as text |
| Multiple cursors | ⬜ | – | Not tracked |
| Vim key bindings | ⬜ | – | Not tracked |
| Spell check | ⬜ | – | Not tracked; would need a dictionary (hunspell) |
| Drag and drop text or files | ✗ | – | A terminal: paste and "Move note" instead |

## 2. The core plugins

| Obsidian core plugin | Now | In blackglass | Notes |
|---------------------|-----|---------------|-------|
| Audio recorder | ✗ | – | Recording audio isn't a terminal note-taker's job |
| Backlinks | 🟡 | W-25 (0.19.0): Alt+L, a pane under the note | Unlinked mentions (the note's name in text without a link) missing |
| Bases | ⬜ | Queue 7, W-74 … W-77 | `requirements/bases_requirements.md` |
| Bookmarks | ⬜ | W-27 | Notes, headings, searches |
| Canvas | ⬜ | – | Not tracked. A `.canvas` file could be shown read-only as boxes of text; editing an infinite canvas isn't a terminal fit |
| Command palette | ✅ | W-37 (Ctrl+P), keys changeable (W-62) | Pinned commands missing |
| Daily notes | ✅ | Periodic Notes (W-31, W-48), Alt+D, the calendar | Weekly … yearly too |
| File explorer | 🟡 | W-02 … W-04: folders, sort, rainbow colors | Missing: rename, new folder, drag to move, a menu per file; move and delete are palette commands (W-64, W-65) |
| File recovery | ⬜ | – | Not tracked. Snapshots of notes every few minutes; the Git plugin (queue 11) covers it for Git users |
| Footnotes view | ⬜ | – | Needs footnotes first (mdedit X-04 …) |
| Format converter | ⬜ | – | Not tracked; low value |
| Graph view | 🟡 | – | No graph view; a DataviewJS link graph is in `example_vault/Plugins/DataviewJS/`. A local graph as text (a note, its links and backlinks as a tree) would fit |
| Note composer | 🟡 | W-60: extract a selection to a new note (Ctrl+X) | Merging two notes missing |
| Outgoing links | ⬜ | – | Not tracked; the backlinks pane (W-25) could show both directions |
| Outline | ⬜ | W-26 | The headings of the note, to jump to |
| Page preview | ⬜ | – | Not tracked. Hover has no terminal meaning; a key on a link could show the note in a popup |
| Properties view | ⬜ | – | Not tracked: every property in the vault, renaming one across notes |
| Publish | ✗ | – | Obsidian's hosting service |
| Quick switcher | ✅ | W-13 (Ctrl+O): fuzzy, recent first, creates notes | |
| Random note | ⬜ | – | Not tracked; a palette command (S) |
| Search | 🟡 | W-14, W-15 (Ctrl+G): text and `tag:` | See section 4 |
| Slash commands | ⬜ | – | Not tracked; `/` at the cursor opens the commands (plugin candidate `slash-commander`) |
| Slides | ⬜ | – | Not tracked; a note's `---`-separated parts, one per screen, would work in a terminal |
| Sync | ✗ | Git plugin, queue 11 (W-93 …) | Obsidian's service; Git or Syncthing instead |
| Tags view | 🟡 | W-16: every tag and its count | Nested tags as a tree (W-32) missing |
| Templates | ✅ | Templater (W-45 … W-47, W-67, W-68) | A superset of the core Templates plugin |
| Unique note creator | ⬜ | – | Not tracked; a time-stamped new note (S) |
| Web viewer | ✗ | – | No browser in a terminal; web links should open in the system browser (they don't yet: section 3) |
| Word count | ⬜ | – | Not tracked; words and characters in the status bar (S) |
| Workspaces | ⬜ | W-33 | Remembering open tabs between sessions is W-33; saved layouts need split panes first |

## 3. Links

| Feature | Now | Where | Notes |
|---------|-----|-------|-------|
| `[[Note]]`, `[[Note\|alias]]`, `[[Note#Heading]]`, Markdown links, found anywhere in the vault | ✅ | W-17, mdedit K-xx | |
| Follow a link (Ctrl+Enter, a click), in a tab | ✅ | W-17, W-59 | |
| Note, heading and image embeds | ✅ | mdedit E-01, E-02, E-04 | |
| `[[` suggestions | ✅ | W-23 | Headings (`[[Note#`) and `[[##`, `[[^^` searches missing |
| Backlinks | ✅ | W-25 | |
| Back / forward | ✅ | W-28 | |
| Web links open in the browser | 🟡 | – | Clicking an image opens it outside; `https://` links don't open yet (a small fix: the same opener) |
| Unresolved links styled | ⬜ | W-22, mdedit K-11 | |
| Block links `[[Note#^id]]` | ⬜ | W-35 | |
| Rename a note and update the links to it | ⬜ | W-30 | Move keeps name links working; renames and path links don't update |
| Unlinked mentions | ⬜ | – | With backlinks |
| Link to a new note creates it on follow | ⬜ | – | Following a link to a missing note should offer to make it |

## 4. Search

| Feature | Now | Where | Notes |
|---------|-----|-------|-------|
| Text in every note, smart case, name matches first, open at the match, F3 | ✅ | W-14 | |
| `tag:` with nested tags | ✅ | W-15 | |
| Results in the sidebar with the matching lines | ✅ | W-14 | |
| Operators `file:`, `path:`, `content:`, `line:`, `block:`, `section:`, `task:`, `task-todo:`, `task-done:`, `[property]`, `match-case:`, `ignore-case:` | ⬜ | – | Not tracked |
| Several words (AND), `OR`, `-` (NOT), `"exact phrase"`, parentheses | 🟡 | – | The query is one phrase today |
| Regular expressions `/…/` | ⬜ | – | Needs the `regex` crate |
| Sort results (name, modified, created), explain the query, copy results | ⬜ | – | |
| ```` ```query ```` embeds | ⬜ | W-36 | |

## 5. Properties

| Feature | Now | Where | Notes |
|---------|-----|-------|-------|
| Frontmatter read (tags, aliases, fields for Dataview and Templater) | ✅ | Vault index, Dataview | `aliases` aren't used by links or the switcher yet |
| Frontmatter shown in the note | 🟡 | mdedit P-01 | Dimmed, raw when edited |
| Typed properties (text, list, number, checkbox, date) with an editor | ⬜ | mdedit P-02 | |
| Add a property (Ctrl+;), suggestions for names and values | ⬜ | W-24 | |
| Properties view: all the vault's properties, rename across notes | ⬜ | – | Not tracked |
| Hide / show / source display of properties | ⬜ | mdedit P-04 | |
| `aliases` in the quick switcher and link suggestions | ⬜ | – | Not tracked; small |

## 6. Files and the vault

| Feature | Now | Where | Notes |
|---------|-----|-------|-------|
| A vault is any folder; open, create, switch, recent vaults | ✅ | W-01, W-88 | Obsidian's `.obsidian` vaults open as they are |
| New notes in a folder, `Untitled N` | ✅ | W-18 | |
| Save as, move, delete to `.trash` | ✅ | W-19, W-64, W-65 | |
| Images shown in notes (where the terminal can), other files opened outside | ✅ | mdedit E-04, W-59 | |
| New folder, rename a folder or note | ⬜ | W-30 | |
| Files changed elsewhere noticed (watching) | ⬜ | W-29 | F5 scans again |
| Index kept current | 🟡 | W-05 | Saves update it; changes made elsewhere need F5 |
| Attachments: paste or drop an image into a note, attachment folder setting | ⬜ | – | Not tracked; pasting an image from the clipboard (wl-paste) into an attachments folder would work |
| Import from other apps (Notion, Evernote …) | ⬜ | – | No importer yet; Evernote and Google Keep exports are planned (W-132) |
| Other file types shown in the explorer (PDF, images) | 🟡 | – | Listed; opened outside |

## 7. Workspace and interface

| Feature | Now | Where | Notes |
|---------|-----|-------|-------|
| Left sidebar with Files, Search, Tags (and plugin tabs) | ✅ | W-02, W-16, W-89 | |
| Tabs: open, close, switch, unsaved dot | ✅ | W-06 … W-08 | |
| Status bar | ✅ | W-11 | |
| Panels under the sidebar (the calendar) and the note (backlinks), Ctrl+B through them | ✅ | W-52, W-25 | |
| Settings window with search and groups | ✅ | W-73 | |
| Keyboard shortcuts, all changeable | ✅ | W-62 | |
| Help | ✅ | W-61 | |
| Mouse | ✅ | W-20, W-59 | Placing the cursor by a click in the text is W-34 |
| Live preview, source and view modes | ✅ | W-57 | |
| Split panes (right / down), linked panes | ⬜ | – | Not tracked; two notes side by side is the biggest workspace gap |
| Pinned tabs, stacked tabs, tab groups | ⬜ | – | Not tracked |
| Right sidebar (outline, backlinks, properties there) | ⬜ | – | blackglass uses panes under the note instead |
| Remember open tabs and folders | ⬜ | W-33 | |
| Workspaces (saved layouts) | ⬜ | – | After split panes |
| Several windows | ✗ | – | One terminal window; run blackglass twice |
| Ribbon | ✗ | – | The palette and keys instead |

## 8. Customizing

| Feature | Now | Where | Notes |
|---------|-----|-------|-------|
| Hotkeys for every command | ✅ | W-62 | |
| Editor settings (auto-pair, indentation, done tasks, colors, symbols, heading colors) | ✅ | W-63, W-72 | |
| Plugins: install, enable, settings pages | ✅ | W-39, W-51, W-73 | Built in (compiled); no community plugin loading (`documentation/obsidian_plugin_compatibility.md`) |
| Themes | 🟡 | M6: W-69 … W-71 | Terminal color depth is handled; themes are planned |
| CSS snippets | ⬜ | – | No CSS in a terminal; themes cover colors |
| Community plugins from Obsidian's directory | ⬜ | – | A compatibility layer is analysed in `obsidian_plugin_compatibility.md`; not planned yet |

## 9. Services and platforms

| Feature | Now | Notes |
|---------|-----|-------|
| Sync | ✗ | Obsidian's service; the Git plugin (queue 11) or Syncthing |
| Publish | ✗ | Obsidian's service |
| Mobile apps | ✗ | A terminal app (it runs in Termux) |
| Web Clipper | ✗ | A browser extension |
| `obsidian://` URIs | ✗ | Obsidian's (mdedit K-10) |
| Obsidian CLI | 🟡 | blackglass's own command line opens a vault or a note; no scripting commands |
| AI (not in Obsidian's core) | ⬜ | M5, queue 9 (W-82 … W-87) |

## 10. What would close the most gaps

Small, and not tracked yet (each a day or less):

1. **Web links open in the browser** (the image opener already does it).
2. **Word count** in the status bar.
3. **Outline** (W-26): the note's headings in a pane, Enter jumps.
4. **Outgoing links** beside the backlinks (W-25's pane, a second list).
5. **Unlinked mentions** in the backlinks pane.
6. **Random note** and **unique note** palette commands.
7. **`aliases`** in the quick switcher and `[[` suggestions.
8. **Following a link to a missing note** offers to create it.
9. **Search terms**: several words, `-word`, `"phrase"`, `OR`, `file:`,
   `path:`, `line:`, `task:`.

Larger:

- **Rename with link updates, new folder, rename folder** (W-30).
- **Split panes** (two notes side by side), then workspaces.
- **Properties editor and view** (mdedit P-02 …, a properties pane).
- **Footnotes, comments, math** (mdedit X-01 … X-08).
- **Bookmarks** (W-27), **nested tag tree** (W-32), **remember tabs**
  (W-33), **watch the vault** (W-29).
- The queued plugins: Bases, Tasks, AI, Advanced Tables, Git, and themes.
