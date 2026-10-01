# Recent Files: what blackglass needs

blackglass's Recent Files plugin (id `recent-files`, built in 0.21.0 as
W-89) compared with the
[Recent Files](https://github.com/tgrosinger/recent-files-obsidian)
community plugin (its README and source, `main.ts`: data, settings,
commands, events, read 2026-09-30). Each piece has an ID (`RF-xx`), a
rough effort (S / M / L) and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

**Already there:** the quick switcher (Ctrl+O) lists recently *changed*
notes first, and `[[` suggests the 5 latest changed; back / forward
(W-28) remembers the notes you were on, in this session only. What's
missing is a lasting list of the files you *opened*, in the sidebar.

It follows the plugin rules: code in `src/plugins/recent/`, settings in
`.blackglass/plugins/recent-files/settings.toml`, the list itself in
`.blackglass/plugins/recent-files/recent.toml` (per vault, like the
original's data file). It needs one new plugin event: a note was opened
(and renamed / moved / deleted: blackglass's move and delete, W-64 /
W-65).

## 1. The list

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| RF-01 | Record every note opened, newest first, each once (opening it again moves it to the top) | ✅ | – | `Plugin::on_note_opened`, from the workspace's history tracking: every way of going to a note (tabs, links, the switcher, back / forward, the Recent tab) |
| RF-02 | "Update the list when a file is": opened (default) or changed (only after it's edited, so notes just looked at don't count) | ✅ | S | 0.60.0: a setting; "changed" counts a note when it's saved; "Changed": on the first edit or save |
| RF-03 | List length (25 by default in blackglass; 50 upstream) | ✅ | – | |
| RF-04 | Saved per vault, read on start; kept when blackglass quits | ✅ | – | `recent.toml` |
| RF-05 | A moved or renamed note keeps its place under its new path; a deleted one leaves the list; files that no longer exist aren't shown | ✅ | – | Move and delete (W-64 / W-65) tell plugins; a rescan catches changes made elsewhere |
| RF-06 | Leave out: path patterns (regular expressions, one per line), frontmatter tag patterns, bookmarked notes | 🟡 | S | 0.60.0: path and tag patterns (regular expressions, separated by `;`); bookmarked notes wait for bookmarks (W-27); Needs the `regex` crate |

## 2. Showing it

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| RF-10 | A "Recent" panel in the sidebar: the notes, newest first, each with its folder; the open note marked | ✅ | – | A fourth sidebar panel (Files, Search, Tags, Recent), or a plugin panel (W-52); a sidebar tab fits the original's view best |
| RF-11 | Enter or a click opens the note; Ctrl+click opens it in a new tab | 🟡 | S | Enter and a click open it (in its tab if it's open: blackglass has one tab per note); a second tab of the same note isn't possible yet |
| RF-12 | Remove an entry (Delete, or its × with the mouse) | ✅ | – | |
| RF-13 | "Recent Files: Open" command (shows the panel with the focus), and "Recent Files: Clear list" | ✅ | – | Palette; keys can be set (W-62) |
| RF-14 | Show a note's frontmatter `title` instead of its name (a setting; the original works with the Front Matter Title plugin) | ✅ | S | 0.60.0: the `title` property (a setting) |
| RF-15 | Insert a link to a recent note (drag to the editor upstream): a key on the entry inserts `[[Note]]` at the cursor | ✅ | S | 0.60.0: `l` on an entry inserts `[[Note]]` at the editor's cursor |
| RF-16 | Hover preview | ✗ | – | No hover previews in blackglass yet; a page-preview feature would add it everywhere |
| RF-17 | Drag to a folder to move, drag to a pane header | ✗ | – | "Move note to a folder" (W-65) covers moving |

## 3. Suggested order

One step (S): RF-01 … RF-05, RF-10 … RF-13, then RF-06, RF-14, RF-15.

## 4. Checked against

- [Recent Files README](https://github.com/tgrosinger/recent-files-obsidian)
  and its source, [`main.ts`](https://github.com/tgrosinger/recent-files-obsidian/blob/main/main.ts):
  `DEFAULT_DATA` (list length 50, omitted paths / tags, omit bookmarks,
  update on open), the settings tab, the "Open" command, the open /
  rename / delete events and the view's actions (2026-09-30).
