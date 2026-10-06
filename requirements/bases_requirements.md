# Bases: what blackglass needs

blackglass's Bases plugin (id `bases`, built in 0.54.0 … 0.57.0 as W-74 …
W-77) against Obsidian's core
[Bases](https://obsidian.md/help/bases) plugin (its help pages, read
2026-09-30: overview, create a base, syntax, functions, views, and the
table, list, cards and kanban views). Each piece has
an ID (`BA-xx`), a rough effort (S / M / L) and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

A base is a database-like view of the vault's notes: every note is a row,
its properties (frontmatter) and file facts are the columns. It's written
in YAML, in a `.base` file or a ```` ```base ```` block, and shown as a
table, list, cards or kanban board. All data stays in the notes: editing a
cell writes the note's frontmatter.

It follows the plugin rules: code in `src/plugins/bases/`, settings in
`.blackglass/plugins/bases/`, rendered through the code block processor
(like Dataview), results clickable through mdedit's row actions.

**Reuse:** Dataview's index already reads frontmatter, `file.*`, tags and
links (`plugins::dataview::index`), and has an expression evaluator
(`eval.rs`) with dates. Bases' expressions are a different language
(methods: `price.toFixed(2)`, `file.hasTag("x")`, `&&` / `||`), so it
gets its own parser, and shares the index and the value types where they
fit. YAML is read with `yaml-rust2` (pure Rust).

## 1. Files and embedding

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| BA-01 | ```` ```base ```` code blocks: the YAML inside, rendered in the note (the source while the cursor is in it) | ✅ | M | 0.54.0; Through `CodeBlockProcessor`, like Dataview |
| BA-02 | `.base` files: shown in the file explorer, opened in a tab as the rendered base (not as text) | ✅ | M | 0.56.0: rendered in a read-only tab; "Bases: Edit base source" opens the YAML; A tab that isn't a note: like the help tab, with its own view; editing the YAML in source mode |
| BA-03 | Embeds: `![[File.base]]` and `![[File.base#View]]` (one view) | ✅ | M | 0.65.0: `![[File.base]]` (the whole base) and `![[File.base#View]]` (one view), through mdedit 3.8.0's embed hook; mdedit asks the resolver for embeds; needs a generic "render this embed" hook in mdedit (like the code block processor) |
| BA-04 | Commands: "Bases: Create new base" (in the selected folder), "Bases: Insert new base" (a new `.base` file, embedded at the cursor) | 🟡 | S | 0.56.0: "Create new base" makes `Untitled.base` in the selected folder; "Insert new base" inserts a ```` ```base ```` block (no embeds yet); Palette; also "New base" for a folder |
| BA-05 | `this`: the base file, the note that embeds it, or the active note | 🟡 | S | 0.54.0: the note a block is in; not in a `.base` file's page; Filters like `file.hasLink(this.file)` |

## 2. Syntax (YAML)

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| BA-10 | `filters`: a statement, or `and` / `or` / `not` of lists of them, nested; for every view | ✅ | S | 0.54.0 |
| BA-11 | `formulas`: named expressions, used as `formula.<name>`; formulas can use each other (no cycles: an error) | ✅ | M | 0.54.0: a formula that uses itself is an error |
| BA-12 | `properties`: per property `displayName` (column headers) | ✅ | S | 0.55.0 |
| BA-13 | `summaries`: custom summary formulas over `values` (`values.mean().round(3)`) | ✅ | S | 0.55.0 |
| BA-14 | `views`: a list; each with `type`, `name`, `limit`, `groupBy` (`property`, `direction` ASC / DESC), `filters` (AND-ed with the base's), `order` (the columns shown, in order), `sort`, `summaries` (column → summary name) | ✅ | M | 0.54.0 … 0.55.0; 0.65.0: views added from the palette; 0.54.0 … 0.55.0 |
| BA-15 | Errors shown in the block (the YAML, the expression and where), not as a crash | ✅ | S | 0.54.0 |

## 3. Properties and expressions

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| BA-20 | Note properties: `price` or `note.price` (frontmatter) | ✅ | S | 0.54.0; The Dataview index has them |
| BA-21 | File properties: `file.name`, `path`, `folder`, `ext`, `size`, `ctime`, `mtime`, `tags`, `links`, `backlinks`, `embeds`, `properties`, `file` | 🟡 | S | 0.54.0: all but `embeds`; `ext` is always md (bases list notes); Most are in the Dataview index; `embeds` and `properties` are new |
| BA-22 | Operators: `+ - * / %`, `( )`, `== != > < >= <=`, `! && \|\|` | ✅ | S | 0.54.0 |
| BA-23 | Types: strings, numbers, booleans, dates, lists (`list[0]`), objects (`obj.key`), links (wikilinks in properties become links; `==` compares them) | ✅ | M | 0.54.0; 0.66.0: `/…/flags` regular expressions; was: 0.54.0: no regular expressions |
| BA-24 | Date arithmetic with durations: `date + "1M"`, `now() - "1 week"`; units y, M, w, d, h, m, s (and their words) | ✅ | S | 0.54.0 |
| BA-25 | Global functions: `date`, `duration`, `file`, `link`, `list`, `if`, `max`, `min`, `now`, `today`, `number`, `random`, `escapeHTML` | ✅ | S | 0.54.0 |
| BA-26 | `html()`, `image()`, `icon()` | 🟡 | S | 0.54.0: their text; Terminal: `image()` as an image where mdedit can draw one, else its name; `icon()` as a symbol or its name; `html()` as its text |
| BA-27 | Any: `isTruthy`, `isType`, `toString` | ✅ | S | 0.54.0 |
| BA-28 | Date: `date()`, `format(fmt)`, `time()`, `relative()`, `isEmpty()`; fields `year`, `month`, `day`, `hour` … | ✅ | S | 0.54.0: `relative()` in days; `format` with moment.js tokens: `plugins::moment` |
| BA-29 | String: `contains`, `containsAll`, `containsAny`, `startsWith`, `endsWith`, `isEmpty`, `lower`, `title`, `trim`, `replace` (text or regex), `repeat`, `reverse`, `slice`, `split`; `length` | ✅ | S | 0.54.0; 0.66.0: `replace` with a regex (all with `g`); was: 0.54.0: `replace` with text, not a regex |
| BA-30 | Number: `abs`, `ceil`, `floor`, `round(digits)`, `toFixed`, `isEmpty` | ✅ | S | 0.54.0 |
| BA-31 | List: `contains`, `containsAll`, `containsAny`, `filter`, `map`, `reduce` (with `value`, `index`, `acc`), `flat`, `join`, `reverse`, `slice`, `sort`, `unique`, `isEmpty`; `length` | ✅ | M | 0.54.0 |
| BA-32 | Link: `asFile`, `linksTo`; File: `asLink`, `hasLink`, `hasProperty`, `hasTag` (nested tags too), `inFolder` (subfolders too) | ✅ | S | 0.54.0 |
| BA-33 | Object: `keys`, `values`, `isEmpty`; Regexp (`/…/flags`): `matches` | ✅ | S | 0.54.0; 0.66.0: `matches(/…/)`; was: 0.54.0: `keys`, `values`, `isEmpty`; no regex; Needs the `regex` crate |

## 4. Views

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| BA-40 | Table: files as rows, the `order` properties as columns (display names), the file's name as a link | ✅ | M | 0.54.0; Like Dataview's table rendering |
| BA-41 | Table: grouping (`groupBy`): a heading per value, the group's summaries at its top | ✅ | S | 0.55.0 |
| BA-42 | Table: a summaries row: Average, Min, Max, Sum, Range, Median, Stddev (numbers); Earliest, Latest, Range (dates); Checked, Unchecked (checkboxes); Empty, Filled, Unique (any); custom ones | ✅ | M | 0.55.0 |
| BA-43 | Table: row height (short … extra tall) | ✗ | – | One text row in a terminal; long values wrap or are cut |
| BA-44 | List view: bullets, numbers or no markers; the properties inline with a separator (default `, `) or indented under the item | ✅ | S | 0.55.0: `markers: bullet / number / none`, `separator`, `indentProperties` |
| BA-45 | Cards view: a grid of cards (card size), each with its properties; a cover image from an image property (a file, a URL or a hex color) with fit and aspect ratio | 🟡 | M | 0.55.0: boxes of text side by side (`cardSize`); no cover images; Cards as boxes of text; images through mdedit's image support where the terminal has it, else a colored band or the name |
| BA-46 | Kanban view: a column per value of the grouped property, cards in them (the first property as the title); notes without a value in **None**; hide empty columns, column width; `groupOrder` (Obsidian 1.14): the columns shown, in order, an empty column for a value no note has, `null` for None, a value left out hidden | ✅ | M | 0.55.0; 0.83.0: `groupOrder`, None, colors; `bases::board::tests::columns_follow_the_group_order`, `workspace::a_kanban_board_is_moved_around_by_key` |
| BA-47 | Kanban: move a card to another column (writes the property in the note; `file.folder` moves the file); reorder columns; a new note in a column; collapse a column | 🟡 | M | 0.83.0 on a `.base` page: ←→↑↓ choose, Shift+←→ move the card (a list property swaps the one value; None removes it), Alt+Shift+←→ move the column (saved as `groupOrder`), `n` new note with the column's value, Space collapse, Enter open; each a command too. No mouse dragging, and not yet on boards embedded in notes (needs a mdedit API for actions on part of a line); Only notes, and not for formula / file properties (as upstream) |
| BA-47a | Colors: `groupColors` (group → color) for columns, `cardColor` (a property or formula naming a color) for cards | ✅ | S | 0.83.0, blackglass's own keys (Obsidian ignores them; it has no colors); named colors (red, orange, yellow, green, blue, purple, pink, cyan, gray), so themes change them |
| BA-47b | The Group menu for every layout: reorder, hide, add groups; collapsible groups in table, cards and list (Obsidian 1.14) | ⬜ | M | The kanban's own keys do it for boards; next: palette commands for every layout |
| BA-48 | Map view (pins on a map) | ✗ | – | Needs a map; not in a terminal |
| BA-49 | `limit`, sorting (`sort`, several properties, ASC / DESC) | ✅ | S | 0.54.0 |

## 5. The toolbar and working with a base

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| BA-50 | Switch views (the base's `views`), the results count | ✅ | S | 0.56.0: the views named above the results with the count; "Bases: Switch view"; 0.56.0: the views named above the results with the count; "Bases: Switch view" |
| BA-51 | Change a view without writing YAML: sort, filters (and / or / not groups), properties shown and their order, group by, limit; saved back to the YAML | 🟡 | L | 0.65.0: sort, group, limit, properties shown and a new filter from the palette (questions), written back to the YAML (its comments go); removing a filter is in the YAML; Popups over the base, like the settings window |
| BA-51a | A filter pane on a `.base` page: the base's (every view's) or this view's filters as rows (property, operator, value; all / any / none groups, nested), edited with keys, the results following every key; a new base opens with it; shown or hidden with Alt+F, half / nearly all / one line with Alt+M | ✅ | M | 0.84.0: `workspace::a_bases_filters_are_changed_in_a_pane_with_live_results`, `bases::filters::tests`; operators: is, is not, contains, does not contain, starts with, ends with, is empty, is not empty, > ≥ < ≤, has tag, is in folder; other expressions stay as formula rows; written as the original writes filters (`and:` lists of expressions) |
| BA-52 | Formulas: add and edit a formula in a popup, with an error shown as you type | 🟡 | M | 0.65.0: "Add formula", checked before it's written; editing one is in the YAML |
| BA-53 | Search the rows by their shown values | ✅ | S | 0.65.0: "Search view": rows with a shown value containing it, named in the header |
| BA-54 | New note from the view (in the base's folder; kanban: with the column's value) | ✅ | S | 0.65.0: in the folder `file.inFolder` names (else beside the base), with the properties `prop == value` filters ask for |
| BA-55 | Copy the results (as a Markdown table) and export them as CSV | ✅ | S | 0.65.0: "Copy view" (a Markdown table) and "Export view as CSV"; 0.56.0: "Export view as CSV" (beside the note or base file); no copy |
| BA-56 | Open a row's note (Enter, a click); results update when notes change | ✅ | S | 0.54.0: a click or Enter opens the row's note; Row actions exist (W-58); the cache is cleared on `vault_changed` |

## 6. Editing properties in the table

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| BA-60 | Move between cells: arrows, Home / End, PgUp / PgDn, Tab | 🟡 | M | 0.65.0: a row's menu instead of moving between cells (one row per screen row in a terminal); A focusable table: a base tab (BA-02) or view mode's row cursor |
| BA-61 | Edit a cell (Enter): text, number, date, list, checkbox (Enter toggles); the note's frontmatter is written (created if missing) | ✅ | M | 0.65.0: a row's menu: a checkbox switches, other values are asked (the current one offered), written to the frontmatter (made if missing); 0.57.0: "Bases: Edit a property" asks the note, the property and the value, and writes the frontmatter (created if missing); Writes through the vault; open tabs of that note reload |
| BA-62 | Select cells (Shift), copy / paste, Backspace clears, Esc clears the selection | ⬜ | M | |
| BA-63 | Undo / redo of property changes | ⬜ | M | |

## 7. Suggested order

1. **Read and show** (M): BA-01, BA-10, BA-11, BA-14, BA-15, BA-20 … BA-25,
   BA-27 … BA-33, BA-40, BA-49, BA-56: ```` ```base ```` blocks with
   filters, formulas and a table view.
2. **More views** (M): BA-41, BA-42, BA-44, BA-45, BA-46, BA-12, BA-13.
3. **Files** (M): BA-02, BA-04, BA-05, BA-50, BA-53 … BA-55; BA-03 with a
   mdedit embed hook.
4. **Editing** (L): BA-60 … BA-63, BA-47, BA-51, BA-52.

## 8. Checked against

- [Bases](https://obsidian.md/help/bases),
  [Create a base](https://obsidian.md/help/bases/create-base),
  [Bases syntax](https://obsidian.md/help/bases/syntax),
  [Functions](https://obsidian.md/help/bases/functions),
  [Views](https://obsidian.md/help/bases/views):
  [table](https://obsidian.md/help/bases/views/table),
  [list](https://obsidian.md/help/bases/views/list),
  [cards](https://obsidian.md/help/bases/views/cards),
  [kanban](https://obsidian.md/help/bases/views/kanban) (2026-09-30).
