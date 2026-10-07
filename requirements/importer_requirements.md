# Importer: what blackglass will have (queued)

Importing other apps' notes into the vault (W-132, W-138 … W-143),
compared with Obsidian's [Importer](https://github.com/obsidianmd/obsidian-importer)
plugin (its format list, read 2026-10-07), plus the formats asked for
that it doesn't have (Word, Excel, Joplin, Day One). Requested
2026-10-07; nothing is implemented yet. IDs are `IM-xx`.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## 1. The importer itself (W-132)

| ID | Feature | Now | Notes |
|----|---------|-----|-------|
| IM-01 | An Importer plugin: "Import notes" asks for the format, the file or folder, and the vault folder to put them in (a form) | ⬜ | `src/plugins/importer/`, one module per format |
| IM-02 | Notes written as Markdown with properties (title, created, modified, tags, source, aliases); unique names; links between imported notes kept | ⬜ | |
| IM-03 | Attachments (images, PDFs, files) copied beside the notes (an attachments folder) and embedded with `![[…]]` | ⬜ | |
| IM-04 | Big exports imported on a thread, with progress in the status bar; the vault rescanned at the end | ⬜ | Never block the UI thread |
| IM-05 | An import report: what was imported, skipped or failed, as a note | ⬜ | |
| IM-06 | One undo step for the whole import (`journal::Files`) | ⬜ | |
| IM-07 | An HTML → Markdown converter shared by the formats (headings, lists, checklists as tasks, tables, links, emphasis, code, images) | ⬜ | Used by Evernote, Keep, HTML, OneNote, Apple Notes |

## 2. Formats

| ID | Format | Row | Now | Notes |
|----|--------|-----|-----|-------|
| IM-10 | Evernote `.enex` exports (ENML, checklists, tags, dates, base64 attachments, note links) | W-132 | ⬜ | Obsidian uses Yarle |
| IM-11 | Google Keep (Google Takeout's Keep folder: JSON and HTML; checklists, labels, colors, pinned / archived / trashed, attachments) | W-132 | ⬜ | |
| IM-12 | HTML files (`.html`, `.htm`), and `.mht` web archives | W-139 | ⬜ | |
| IM-13 | Markdown and text files (`.md`, `.txt`): copied in, links fixed | W-139 | ⬜ | |
| IM-14 | TextBundle / TextPack (`.textbundle`, `.textpack`) | W-139 | ⬜ | Bear and others write it |
| IM-15 | CSV files: a note per row (columns as properties), or a table | W-139 | ⬜ | |
| IM-16 | Notion's export (a zip of Markdown and CSV): pages, sub-pages as folders, databases as notes with properties (and a base), IDs dropped from names, links fixed | W-138 | ⬜ | Notion's API (Obsidian has it) not planned at first |
| IM-17 | Bear (`.bear2bk`: TextBundles in a zip; tags, nested tags, dates) | W-140 | ⬜ | |
| IM-18 | Apple Notes: from exported folders (HTML / text / Markdown, e.g. the Exporter app); reading macOS's NoteStore database directly | W-140 | ⬜ | The database only exists on a Mac |
| IM-19 | Microsoft OneNote: pages exported as `.docx` or `.mht`; Microsoft Graph (online, as Obsidian does) later | W-141 | ⬜ | `.onepkg` is a closed format |
| IM-20 | Word `.docx` (headings, lists, tables, emphasis, links, images) | W-141 | ⬜ | Not in Obsidian's Importer |
| IM-21 | Excel `.xlsx` / `.xls` / `.ods`: each sheet as a table (Advanced Tables' format) in a note | W-141 | ⬜ | Not in Obsidian's Importer; e.g. the `calamine` crate |
| IM-22 | Joplin (`.jex` archives and RAW folders: notes, notebooks as folders, tags, resources) | W-142 | ⬜ | Not in Obsidian's Importer |
| IM-23 | Day One (JSON export zip: entries as dated notes, tags, photos, location) | W-142 | ⬜ | Not in Obsidian's Importer |
| IM-24 | Logseq (Markdown folders: pages, journals as daily notes, `key:: value` properties, `((block refs))`, assets) | W-143 | ⬜ | |
| IM-25 | Roam Research (JSON export: pages, blocks as outline lists, `((uid))` block refs, `{{TODO}}`, daily pages) | W-143 | ⬜ | |
| IM-26 | Tomboy / Gnote (`.note`), Airtable, Apple Journal | – | ✗ | Not asked for; can be added later |
