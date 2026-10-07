# Plugin candidates: the Obsidian plugins in use, by effort

The user's Obsidian plugins (2026-09-29), ranked by how much work each is
in blackglass. Effort assumes today's plugin structure: palette commands,
questions (text or a choice), inserting / replacing / creating notes, and
code block processors. "mdedit API" means a generic hook has to be added
to mdedit first.

## Already there

| Plugin | In blackglass |
|--------|---------------|
| dataview | The Dataview plugin (gaps: `dataview_requirements.md`) |
| templater-obsidian | The Templater plugin (gaps: `templater_requirements.md`) |
| obsidian-emoji-toolbar | mdedit's emoji picker (Ctrl+E, T-14, EP-xx) |
| emoji-shortcodes | ✅ The Emoji Shortcodes plugin (W-129, 0.80.0): `emoji_requirements.md` |
| url-into-selection | mdedit: paste a URL over a selection makes a link (V-09) |

## 1. Least effort (hours to a day each; fit the current hooks)

| Plugin | What it takes |
|--------|---------------|
| homepage | Open a chosen note at startup (a setting read on start) |
| recent-files-obsidian | ✅ Done in 0.21.0 (W-89): the Recent Files plugin; the rest in `recent_requirements.md` |
| periodic-notes | ✅ Done in 0.5.0 (W-48 … W-50) |
| obsidian-task-progress-bar | A ```` ```task-progress-bar ```` code block: count the note's tasks, draw `████░░ 4/6` |
| frontmatter-modified-date | On save, set `modified:` in the frontmatter (needs a small "note saved" plugin event) |
| numerals / meld-calc / solve | A math code block (numerals, meld-calc) evaluated line by line with the results beside it; units and variables are the extra work |
| obsidian-icon-folder | An icon (emoji) per folder / file in the explorer, from a settings file |
| contribution-graph | A code block drawing a GitHub-style heatmap of notes per day (the Dataview index has the dates) |
| tag-wrangler | "Rename tag" / "Merge tags" commands rewriting tags across the vault (text questions) |

## 2. Medium (a few days; some need an mdedit API)

| Plugin | What it takes |
|--------|---------------|
| slash-commander | Typing `/` opens the command list at the cursor (the same approach as `[[` autocomplete) |
| cmdr (Commander) | Favorite commands on a clickable toolbar row, set in a settings file |
| editing-toolbar | A clickable formatting toolbar; needs bold / italic / heading commands (mdedit can wrap a selection already) |
| calendar | ✅ Done in 0.7.0 as Periodic Notes' calendar panel (W-53, Alt+C) |
| chord-sheets | A ```` ```chords ```` block: chords over lyrics, highlighted and aligned |
| Mermaid (core in Obsidian) | ✅ Done in 0.30.0 (W-104) as a plugin: `mermaid_requirements.md` |
| obsidian-charts / obsidian-tracker | Text charts (bars, sparklines) from a block's YAML or from note fields |
| obsidian-mind-map | A note's headings and lists drawn as a tree |
| quickadd | Capture and template choices on top of Templater; macros (JavaScript) not |
| meld-encrypt | Requested 2026-10-07 (W-134): encrypt a note or selection with a password (AES-GCM); reading notes Meld encrypted needs its exact format |
| colored-tags | A color per tag; needs a tag-style hook in mdedit (or mdedit's own R-12) |
| guitar-chord | Chord diagrams drawn in text from a chord database |

## 3. Large (weeks; mostly editor work in mdedit)

| Plugin | What it takes |
|--------|---------------|
| obsidian-tasks-plugin | Queued (plan item 8, W-78 … W-81): `tasks_requirements.md` (TK-xx). Today's checkboxes and states stay core |
| table-editor-obsidian (Advanced Tables) | Queued (plan item 10, W-90 … W-92): `tables_requirements.md` (AT-xx): formatting, Tab / Enter between cells, row / column commands, sort, transpose, CSV, `TBLFM` formulas. All are changes to the note's text, so a blackglass plugin (no mdedit API); keys taken while the cursor is in a table |
| obsidian-git (Git) | Stage 1 done in 0.35.0 (W-93); the rest queued (plan item 7): `git_requirements.md` (GT-xx): commit-and-sync on a timer, pull / push, source control, history and diff views, by running the `git` program; change signs and blame in the margin need a mdedit "line marks" API |
| obsidian-outliner | Move list items up / down with their children, better indent / fold: editing in mdedit |
| excalibrain | An interactive graph of related notes in the terminal |

## 4. Not a fit for a terminal (or not needed)

| Plugin | Why |
|--------|-----|
| obsidian-excalidraw-plugin | A drawing canvas; at most, show a drawing's exported PNG with mdedit's image embeds |
| obsidian-livesync | CouchDB sync with its own protocol and encryption; Syncthing or git sync the vault folder as it is |
| obsidian-style-settings | Settings for CSS themes; blackglass's theme would need its own settings instead |
| settings-search | Searches Obsidian's settings screen, which blackglass doesn't have |
