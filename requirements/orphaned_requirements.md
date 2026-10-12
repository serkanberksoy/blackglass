# Orphaned files: what blackglass has and what's missing

A comparison of blackglass's Orphaned files plugin (W-150) with the
community plugin
[Find orphaned files and broken links](https://github.com/Vinzent03/find-unlinked-files)
by Vinzent03, against its README (read 2026-10-13). IDs are `OF-xx`.

The original writes each search to its own note; blackglass writes one
report with a section each (`Orphaned files and broken links.md` at the
top of the vault), as asked, and has a command for each search too (the
report then has that section alone). Settings are in
`.blackglass/plugins/orphaned-files/settings.toml`, set in Settings →
Orphaned files. Tests: `orphaned::tests::orphans_broken_links_and_empty_things`
(`orphaned::`) and
`workspace::a_report_of_orphaned_files_broken_links_and_empty_ones`
(`workspace::`).

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## Finding

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| OF-01 | Orphaned files: notes and other files (images, PDFs …) no note links to or embeds; a note's link to itself and the report's links don't count | ✅ | `orphaned::`, `workspace::` |
| OF-02 | Broken links: wiki links, embeds and Markdown links to files that don't exist, each with the note it's in (not in code, not web links) | ✅ | same |
| OF-03 | Empty files: notes with nothing but frontmatter and blank lines; other files of no bytes | ✅ | same |
| OF-04 | Empty folders (no files; hidden files and folders don't count), the outermost of nested ones | ✅ | same |
| OF-05 | The report in a note at the top of the vault, opened; brought up to date when run again (one undo step); a count of each in the status bar | ✅ | `workspace::` |
| OF-06 | A command each: Find orphaned files and broken links (all four), Find orphaned files, Find broken links, Find empty files, Find empty folders | ✅ | `orphaned::` (one section alone); the commands in the palette and Settings → Keyboard shortcuts |

## Settings

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| OF-20 | The report's file name | ✅ | `workspace::` (the default) |
| OF-21 | Folders left out (and everything in them) | ✅ | `orphaned::`, `workspace::` |
| OF-22 | Notes with certain tags left out (nested tags too) | ✅ | `orphaned::` |
| OF-23 | File types left out (`pdf, png`) | ✅ | `orphaned::`, `workspace::` |
| OF-24 | Files that link to certain notes aren't orphans | ✅ | `orphaned::` |
| OF-25 | Orphans sorted by name or by size | ✅ | `orphaned::` |

## Acting on the results

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| OF-40 | Delete the orphaned files, the empty files, the empty folders (after asking) | ⬜ | Report only for now: open a file from the report and delete it (Delete note), undoable |
| OF-41 | Archive the orphaned files (to a folder, keeping their paths) | ⬜ | |
| OF-42 | Create the notes broken links point to | ✅ | A click on a broken link in the report makes the note, as anywhere |
| OF-43 | Delete or archive the media files the report lists, by extension | ⬜ | |
