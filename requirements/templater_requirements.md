# Templater: what blackglass has and what's missing

A comparison of blackglass's Templater plugin (W-45 … W-47) with
[Obsidian Templater](https://silentvoid13.github.io/Templater/) (its
documentation, read 2026-09-29). Missing pieces have IDs (`TP-xx`), a
rough effort (S / M / L) and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## 1. Syntax

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| TP-01 | `<% command %>` | ✅ | – | Commands: `tp.*`, "text", numbers, `[lists]`, joined with `+` |
| TP-02 | Whitespace control `<%-` / `-%>` (one line break), `<%_` / `_%>` (all) | ✅ | – | |
| TP-03 | `<%+ %>` dynamic commands | 🟡 | – | Run like `<% %>` (deprecated upstream) |
| TP-04 | `<%* %>` JavaScript commands, `tR`, variables across commands, `await` | ✅ | – | blackglass 0.8.0 (W-56): the template runs as one async function in the sandbox; `tp` and a small `moment()` |

## 2. Internal functions

| ID | Module | Now | Effort | Notes |
|----|--------|-----|--------|-------|
| TP-10 | `tp.date.now(format, offset, reference, reference_format)`, `tomorrow`, `yesterday`, `weekday` | ✅ | – | moment.js tokens, ISO 8601 offsets; weeks start on Monday |
| TP-11 | `tp.file.title`, `content`, `tags`, `folder(absolute)`, `path(relative)`, `creation_date`, `last_modified_date`, `exists`, `include` (with `#heading`), `cursor(order)`, `selection()` | ✅ | – | |
| TP-12 | `tp.file.create_new`, `move`, `rename`, `find_tfile`, `cursor_append` | ✅ | – | 0.69.0: done when the template's text is (a created note's template runs too; `move` / `rename` update every link) |
| TP-13 | Jump to the next `tp.file.cursor` (Templater's "jump to next cursor location") | ✅ | – | 0.69.0: the lowest order is jumped to, the others stay as `<% tp.file.cursor(n) %>`; one cursor where Templater makes several of the same order |
| TP-14 | `tp.frontmatter.<key>` | ✅ | – | |
| TP-15 | `tp.system.prompt(text, default)`, `tp.system.suggester(texts, values)` | ✅ | – | Asked in blackglass's popups before rendering; the same prompt is asked once |
| TP-16 | `tp.system.prompt` `multiline`, `suggester` with a JS function for the texts, `multi_suggester` | 🟡 | – | 0.69.0: multi-line prompts (Alt+Enter), `multi_suggester` (Tab marks), `limit`. Esc always stops the template (as `throw_on_cancel`); `default_value` / `select_default_value` aren't preselected |
| TP-17 | `tp.system.clipboard()` | ✅ | – | Through `wl-paste`, `xclip` or `xsel` |
| TP-18 | `tp.web.daily_quote`, `random_picture`, `request` | ✅ | – | 0.69.0: through `curl`, on a thread (the template runs again with the pages); http and https only (redirects too), but the quotes database the settings name; the Web access setting turns it off |
| TP-19 | `tp.config`, `tp.hooks`, user scripts | ✅ | – | 0.69.0: `run_mode`, `template_file`, `target_file`, `active_file`; `on_all_templates_executed`; `.js` files in the Script files folder as `tp.user.<name>` |
| TP-34 | `tp.app`, `tp.obsidian` | ✗ | – | The original app's own API (its vault, workspace, metadata cache): nothing in blackglass to give; using them says so |

## 3. Commands and settings

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| TP-20 | Insert template (palette) | ✅ | – | |
| TP-21 | Create new note from template (in the selected folder) | ✅ | – | |
| TP-22 | Replace templates in the active file | ✅ | – | One undo step |
| TP-23 | Templates folder setting | ✅ | – | `templates_folder` in `.blackglass/plugins/templater/settings.toml` |
| TP-24 | Folder templates: run a template on new notes in a folder | ✅ | – | blackglass 0.16.0 (W-67): `[folder_templates]` in the settings (`Books = "Book"`, `"/"` for the whole vault); the nearest folder up the tree wins; listed, changed and added in the settings screen ("Add a folder template"); the template's questions are asked. Any new empty note: Ctrl+N, the quick switcher, a daily note without its own template |
| TP-25 | Trigger on new file creation, startup templates | ✅ | – | 0.69.0: a new note gets its folder's or regex's template, a new note with commands has them run (not the ones Templater made); startup templates run at the first tick |
| TP-26 | Hotkeys per template, Alt+E for "Insert template" | ✅ | – | 0.69.0: Template hotkeys adds "Insert <name>" and "Create from <name>" commands, keys from the settings' shortcuts |
| TP-27 | Syntax highlighting of `<% %>` in templates | ✅ | – | 0.69.0: shown as written in the code color, nothing in it read as Markdown (mdedit 3.11.0 `set_verbatim`) |
| TP-28 | File regex templates: a template for new notes whose path matches a regex | ✅ | – | 0.69.0: `[file_templates]`, before folder templates (Templater has one mode or the other) |
| TP-29 | Excluded folders for "trigger on new file creation" | ✅ | – | 0.69.0 |
| TP-30 | "Automatic jump to cursor" setting | ✅ | – | 0.69.0: on by default (as blackglass did); off, every marker stays |
| TP-31 | User system command functions: `tp.user.<name>()` runs a shell command set in the settings, with a timeout | ✅ | – | 0.69.0: off by default; arguments as environment variables, `<% %>` in the command run first; only the settings' commands run |
| TP-32 | Settings in a settings screen | ✅ | – | blackglass 0.6.0: the templates folder; 0.16.0: the folder templates |
| TP-33 | Apply a template to the open note (blackglass's own): the template's text on top, its properties joined with the note's (the note's values stay) | ✅ | – | blackglass 0.16.0 (W-68); "Insert template" still inserts at the cursor |

## 4. Checked against

- [Templater documentation](https://silentvoid13.github.io/Templater/): syntax, internal
  functions (`tp.date`, `tp.file`, `tp.system`), settings, commands (2026-09-29);
  `tp.web`, `tp.config`, `tp.hooks`, user functions and Templater's source
  for the web module, `create_new`, `move`, `rename` and the file creation
  trigger (2026-10-01).
