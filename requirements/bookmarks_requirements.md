# Bookmarks: what blackglass has and what's missing

A comparison of blackglass's Bookmarks plugin (W-27) with Obsidian's core
[Bookmarks](https://obsidian.md/help/plugins/bookmarks) plugin (read
2026-10-01). Missing pieces have IDs (`BM-xx`), a rough effort (S / M / L)
and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| BM-01 | Bookmark a note (or any file) | ✅ | – | "Bookmark this note"; again removes it |
| BM-02 | Bookmark a heading | ✅ | – | The heading above the cursor; opens at the heading |
| BM-03 | Bookmark a block (`^id`) | ⬜ | S | Read from `bookmarks.json` as the note itself |
| BM-04 | Bookmark a folder | ✅ | – | The folder chosen in the file explorer; Enter shows it there |
| BM-05 | Bookmark a search | ✅ | – | The sidebar's search; Enter runs it again |
| BM-06 | Bookmark a graph view or a web page | ✗ | – | No graph view yet (W-114); web pages: a link in a note does it |
| BM-07 | A Bookmarks tab in the sidebar: open, rename (title), remove | ✅ | – | Enter, `r`, Delete |
| BM-08 | Groups (folders of bookmarks), nested | ⬜ | M | |
| BM-09 | Reorder by dragging | ⬜ | S | Keys to move one up / down would do |
| BM-10 | Bookmarks follow renamed and moved notes | ✅ | – | `on_note_moved` |
| BM-11 | Kept per vault, in the original's `bookmarks.json` shape | ✅ | – | `.blackglass/plugins/bookmarks/bookmarks.json` (`type`, `path`, `subpath`, `query`, `title`) |
| BM-12 | "Bookmark all open tabs" | ⬜ | S | |
