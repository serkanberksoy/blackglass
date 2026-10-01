# Zettelkasten: what blackglass needs

The Zettelkasten method (Niklas Luhmann's slip box) as people practise it in
Obsidian: small atomic notes with unique IDs, linked to each other densely,
reached through links, structure notes and search rather than folders;
fleeting, literature and permanent notes. This file lists what blackglass
needs to support it completely, compared with Obsidian's core plugins
(Unique note creator, Note composer, Backlinks, Outgoing links, Page
preview, Graph view, Bookmarks, Random note) and the community plugins
Zettelkasten users rely on (Folgezettel / Luhmann IDs, Front Matter Title,
Breadcrumbs, Strange New Worlds, Citations / Zotero Integration), read
2026-10-01. Each piece has an ID (`ZK-xx`), a rough effort (S / M / L) and
notes. Requested 2026-10-01 for M3 (queue 15).

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

Built (0.72.0 … 0.76.0) as two plugins, off until installed: **Zettelkasten**
(`src/plugins/zettelkasten/`: linking, structure, refactoring, retrieval)
and **Citations** (`src/plugins/citations/`: literature notes).

## 1. Capture: unique, atomic notes

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| ZK-01 | A new note named by its ID (`202610011445`), unique (the next free time if taken) | ✅ | – | W-101 "New unique note" |
| ZK-02 | The ID's format (moment tokens, default `YYYYMMDDHHmm`), its folder and a template (the original's settings) | 🟡 | S | W-124 (0.68.0) the format; W-119 (0.72.0) the folder and template of "New unique note linked from here" |
| ZK-03 | The ID and a title together: `202610011445 Title`, or the ID as the name and the title in a `title` / `aliases` property (asked when the note is made) | ✅ | – | W-124 (0.68.0): Note IDs in the name, as the name or in the properties, for every new note |
| ZK-04 | "New unique note linked from here": a new ID note, and a link to it at the cursor (or around the selection) | ✅ | – | W-119 (0.72.0): the selection becomes the link's alias |
| ZK-05 | Note types from templates: fleeting, literature, permanent (Templater's templates, a folder each) | ✅ | – | Templater (W-45 …) and folder templates |
| ZK-06 | A fleeting-notes inbox: today's daily note, or a quick capture command that appends a line to an inbox note without leaving the note you're in | ✅ | – | W-122 (0.75.0): "Quick capture" to the inbox note (a setting), a template for the line |

## 2. Linking

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| ZK-10 | `[[` note suggestions, aliases too | ✅ | – | W-23, W-102 |
| ZK-11 | `[[Note#` heading suggestions (the note's headings); `[[##` headings in the whole vault | ✅ | – | W-119 (0.72.0) |
| ZK-12 | `[[Note#^` block suggestions (the note's paragraphs and list items): choosing one adds a `^id` to it if it has none; `[[^^` blocks in the whole vault | ✅ | – | W-119 (0.72.0): the id written into the other note (`Effect::EditNote`) |
| ZK-13 | "Copy link to this block" / "Copy link to this heading" / "Copy link to this note" (an id added if needed) | ✅ | – | W-119 (0.72.0) |
| ZK-14 | Block and heading links followed and embedded | ✅ | – | W-35, mdedit E-02 / E-03 |
| ZK-15 | Unresolved links styled; a click creates the note (with the unique-note template, a setting) | ✅ | – | W-22 (0.70.0) dimmed; W-99 a click creates it (templates: Templater's folder templates) |
| ZK-16 | Renaming keeps every link (titles, aliases, headings, blocks) | ✅ | – | W-30 |
| ZK-17 | Show a note's title instead of its ID: in the file explorer, tabs, the switcher, link suggestions, backlinks and query results (the `title` property, a setting; Front Matter Title) | ✅ | – | W-120 (0.73.0): the `title` property (a setting), everywhere a note is named but the status bar (its file) |
| ZK-18 | Link counts beside links and headings in the note (how many notes link here: Strange New Worlds) | ✅ | – | W-122 (0.75.0): a setting; mdedit 3.13.0 link badges |

## 3. Structure and sequence

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| ZK-20 | Folgezettel IDs (Luhmann): `1`, `1a`, `1a1`, `1b` …: "New sequel note" (the next at this level: `1a` → `1b`) and "New branch note" (one level down: `1a` → `1a1`), each linked from the current note | ✅ | – | W-120 (0.73.0) |
| ZK-21 | Notes sorted by their Folgezettel ID (`1a2` before `1a10`), and the sequence around a note: previous, next, parent, children (a sidebar tab) | ✅ | – | W-120 (0.73.0): the Sequence tab; parent / next / previous |
| ZK-22 | Hierarchy fields (`up`, `down`, `next`, `prev`, `related`): breadcrumbs above the note (`Home › Project › Note`) and commands to go up / next / previous (Breadcrumbs) | ✅ | – | W-120 (0.73.0): the trail in the status bar |
| ZK-23 | Structure notes (maps of content): queries of the notes linking here or tagged | ✅ | – | Dataview, Bases, `query` blocks |
| ZK-24 | Nested tags as a tree, a tag's sub-tags counted | ✅ | – | W-32 (0.70.0) |
| ZK-25 | Tag and property suggestions while typing | ✅ | – | W-24 (0.70.0) |
| ZK-26 | Bookmarks (entry points into the slip box), in a sidebar tab | ✅ | – | W-27 (0.70.0), the Bookmarks plugin |

## 4. Refactoring (keeping notes atomic)

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| ZK-30 | Extract the selection to a new note, leaving a link | ✅ | – | Ctrl+X (W-60) |
| ZK-31 | Extract to an existing note (appended, or at its start) or a new one; leave a link, an embed or nothing (a setting); a template with `{{content}}`, `{{fromTitle}}`, `{{newTitle}}`, `{{date:FORMAT}}` (Note composer) | ✅ | – | W-121 (0.74.0) |
| ZK-32 | Extract a heading's section to a new note named by the heading | ✅ | – | W-121 (0.74.0) |
| ZK-33 | Split a note at every heading of a level into notes linked from it | ✅ | – | W-121 (0.74.0) |
| ZK-34 | Merge a note into another (at its end or start), every link to it updated, the note removed (asks first; a setting) | ✅ | – | W-121 (0.74.0): asks first (a setting) |

## 5. Retrieval and serendipity

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| ZK-40 | Backlinks, outgoing links, unlinked mentions (linked in place) | ✅ | – | W-25, W-106, W-107 |
| ZK-41 | Link preview without leaving the note | ✅ | – | W-103 Alt+P |
| ZK-42 | Random note | ✅ | – | W-108 |
| ZK-43 | Orphans (no links in or out) and dead ends (no links out), unresolved links (with the notes that have them), as commands or a sidebar tab | ✅ | – | W-122 (0.75.0): pages listing them |
| ZK-44 | A local graph in the terminal: the note's neighbours to a depth (a tree of links in and out), Enter opens | ✅ | – | W-122 (0.75.0): the Links tab, a depth setting |
| ZK-45 | Search by ID, title, alias, tag, property, text | ✅ | – | W-100, W-102 |
| ZK-46 | Notes side by side (write a permanent note beside its source) | ⬜ | L | W-112 (M8) |

## 6. Literature notes

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| ZK-50 | A bibliography (BibTeX `.bib` or CSL JSON, exported from Zotero) read from the vault or a path (a setting) | ✅ | – | W-123 (0.76.0): the Citations plugin |
| ZK-51 | "Insert citation": search the references, insert `[@key]` (or a link to its literature note) | ✅ | – | W-123 (0.76.0) |
| ZK-52 | "Open literature note": the reference's note (made from a template: `{{title}}`, `{{authors}}`, `{{year}}`, `{{citekey}}`, `{{abstract}}`, `{{url}}`) | ✅ | – | W-123 (0.76.0) |
| ZK-53 | `[@key]` shown as the author and year in the live preview, a click opens its literature note | 🟡 | M | W-123 (0.76.0): shown as author and year; "Open literature note" with the cursor on it opens its note (a click doesn't yet) |
| ZK-54 | Zotero annotations and PDF highlights imported | ✗ | – | Needs Zotero's local API or the PDF annotations; later, if asked |

## 7. Suggested order

1. **Linking** (M): ZK-11, ZK-12, ZK-13, ZK-15 (W-22), ZK-04, ZK-02, ZK-03.
2. **Structure** (M): ZK-20, ZK-21, ZK-22, ZK-24, ZK-26, ZK-17.
3. **Refactoring** (M): ZK-31 … ZK-34.
4. **Retrieval** (M): ZK-43, ZK-44, ZK-18, ZK-25, ZK-06.
5. **Literature notes** (M): ZK-50 … ZK-53.

## 8. Checked against

- Obsidian help: [Unique note creator](https://obsidian.md/help/plugins/unique-note),
  [Note composer](https://obsidian.md/help/plugins/note-composer),
  [Internal links](https://obsidian.md/help/links) (headings, blocks,
  `[[##` / `[[^^`) (2026-10-01).
- Community plugins: Folgezettel / Luhmann IDs, Front Matter Title,
  Breadcrumbs, Strange New Worlds, Note Refactor, Citations and Zotero
  Integration (their READMEs, 2026-10-01).
