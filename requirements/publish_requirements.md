# Publish: what blackglass will have (queued)

The vault's notes as a static web site (W-116), compared with Obsidian
Publish (hosted by Obsidian) and the Digital Garden community plugin.
Decided 2026-10-07: notes with `publish: true`, written to a folder,
with a local preview; backlinks and tags, search, and queries shown as
their results. Nothing is implemented yet. IDs are `PB-xx`.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

| ID | Feature | Now | Notes |
|----|---------|-----|-------|
| PB-01 | A Publish plugin: "Publish site" writes the site to a folder chosen in its settings (on a thread, progress in the status bar) | ⬜ | `src/plugins/publish/` |
| PB-02 | What's published: notes whose properties say `publish: true` (as Obsidian Publish and Digital Garden do); `publish: false` never | ⬜ | |
| PB-03 | Notes as HTML: headings, emphasis, highlights (colors too), lists and tasks, callouts (folding), tables, code with highlighting, footnotes, comments left out | ⬜ | |
| PB-04 | Links: wiki links to published notes as links, to unpublished ones as plain text; headings and blocks as anchors | ⬜ | |
| PB-05 | Embeds: notes, sections and blocks inline; images and other attachments copied | ⬜ | |
| PB-06 | Queries as their results (a snapshot when published): `dataview` (lists, tables, tasks), `dataviewjs`, `tasks`, `query`, `base` and inline queries | ⬜ | From the plugins' results, as HTML lists and tables |
| PB-07 | Backlinks on every page (published notes only) | ⬜ | |
| PB-08 | Tags: each page's tags linked to a tag page listing its notes; a contents page | ⬜ | |
| PB-09 | Search: a search box over every published note (a JSON index, a little JavaScript, nothing from outside) | ⬜ | |
| PB-10 | A theme from the vault's theme colors, light and dark, readable on a phone | ⬜ | |
| PB-11 | Local preview: a small web server on a thread (localhost only), opened in the browser; stops on quit | ⬜ | |
| PB-12 | Settings: the folder, the site's title, the home note, what to leave out | ⬜ | |
| PB-13 | Hosting (Obsidian Publish's service), pushing to a git remote or a server | ✗ | Not asked for (decided 2026-10-07): the folder can be hosted anywhere |
| PB-14 | A graph view on the site | ✗ | Not asked for (decided 2026-10-07) |
