# Running Obsidian community plugins in blackglass: an analysis

Question (2026-09-29): can blackglass get a full JavaScript plugin engine,
compatible with Obsidian's, so community plugins run unchanged?

**Short answer:** full compatibility, no. A compatible *subset*, yes: an
Obsidian API shim that runs unmodified `main.js` bundles for plugins that
work with notes, commands, code blocks, suggesters, modals and settings.
Plugins built around the browser (DOM views, canvas, CodeMirror editor
extensions, Electron / Node) will not run, or only partly.

## 1. What an Obsidian plugin is

- A folder in `.obsidian/plugins/<id>/`: `manifest.json`, `main.js` (a
  CommonJS bundle, usually esbuild output, often with libraries bundled:
  Preact, Svelte, d3, mathjs, moment …), optional `styles.css`, and
  `data.json` (its settings).
- `main.js` exports a class extending `Plugin` from the `obsidian` module,
  which Obsidian provides at runtime. Through it a plugin uses:
  - **App**: `vault` (files, read / write / create / rename, events),
    `metadataCache` (frontmatter, links, tags, headings; `resolved`
    events), `workspace` (leaves, active file, views, layout events),
    `fileManager` (rename with link updates, frontmatter edits).
  - **Registration**: `addCommand` (plain, `editorCallback`,
    `checkCallback`), `addSettingTab`, `addRibbonIcon`, `addStatusBarItem`,
    `registerView` (custom panes), `registerMarkdownCodeBlockProcessor`,
    `registerMarkdownPostProcessor`, `registerEditorSuggest`,
    `registerEditorExtension` (CodeMirror 6), `registerEvent`,
    `registerDomEvent`, `registerInterval`.
  - **UI classes**: `Modal`, `SuggestModal`, `FuzzySuggestModal`,
    `Notice`, `Setting`, `PluginSettingTab`, `ItemView`, `Menu`,
    `MarkdownRenderer` — all built on the **DOM** (`containerEl`,
    `createEl`, `createDiv`, `setText`, CSS classes).
  - **Editor**: `Editor` (`getLine`, `replaceRange`, `getCursor`,
    `setCursor`, `getSelection` …) over **CodeMirror 6**; plugins may add
    CM6 extensions (decorations, view plugins, keymaps).
  - **Environment**: `moment`, `requestUrl` (network), and in Electron
    also Node (`fs`, `path`, `child_process`) and `window` / `document`.

## 2. What running them in blackglass would take

| Layer | What's needed | Difficulty |
|-------|---------------|------------|
| JS engine | ES2022+, fast enough for large bundles (dataview's `main.js` is over a megabyte). Boa (today's sandbox) is too incomplete and slow for this; **QuickJS** (`rquickjs`, small, ES2023) is the practical choice; V8 (`deno_core`) gives the best compatibility but a large binary and build | M |
| Module loader | CommonJS `require` for `obsidian`, `moment`, a few Node built-ins; loading `main.js`, calling `onload` / `onunload` | S |
| `obsidian` API shim | The API surface above, mapped to blackglass: vault → `Vault`, metadata cache → an index like Dataview's, commands → the palette, settings → settings screens, events → the workspace | L (months for a good subset) |
| **DOM** | Almost every UI path writes DOM. A small DOM in the engine (e.g. `linkedom`) plus a **DOM → terminal renderer**: block / inline text, lists, tables, headings, buttons and inputs → text rows and blackglass widgets; CSS mostly ignored | L |
| Modals, suggesters, notices, settings tabs | Map `Modal`, `SuggestModal`, `Setting` rows, `Notice` to blackglass popups, question prompts, settings screen, status messages | M |
| Editor API | `Editor` methods over mdedit's buffer (lines, cursor, selection, `replaceRange` → edits with undo); `EditorSuggest` → the `[[` suggestion popup mechanism | M |
| CodeMirror 6 extensions | Would need CM6 running in the engine with its state mirrored to mdedit, and its decorations translated to terminal styles | XL; not recommended |
| Custom views (`ItemView`) | Render their DOM into a sidebar panel (the calendar's panel API) through the DOM renderer; works for lists, not for canvas / SVG / drag and drop | L |
| Canvas, SVG, WebGL | Not drawable as text; SVG could be rasterized (`resvg`) and shown as an image embed | XL / no |
| Network, Node | `requestUrl` via an HTTP client, `fs` via the vault adapter; both need permission prompts | M |
| Security | Community plugins are arbitrary code with full access in Obsidian. blackglass would sandbox them (no files outside the vault, network only when allowed): safer, but less compatible | – |

## 3. Your plugins, by how far they would run

| Plugin | Uses | Would run |
|--------|------|-----------|
| homepage, recent-files, periodic-notes (commands) | workspace + vault, small UI | **yes**, with the core shim |
| task-progress-bar, chord-sheets, numerals, meld-calc, solve | code block / post processors producing simple DOM; libraries like mathjs | **mostly**, with the DOM renderer |
| tag-wrangler, frontmatter-modified-date | vault, metadata cache, `fileManager`, menus | **mostly** |
| quickadd, templater, cmdr, slash-commander | modals, suggesters, editor, commands; templater and slash-commander also CodeMirror | **partly** |
| dataview | large; DOM (Preact) rendering, CM6 inline fields | **partly**; blackglass's own Dataview stays better in a terminal |
| calendar, contribution-graph, colored-tags, icon-folder, editing-toolbar, outliner, table-editor, tasks | Svelte / DOM views, CSS, CM6 extensions | **little** |
| excalidraw, excalibrain, mind-map, charts, tracker, guitar-chord | canvas, SVG, d3, drag and drop | **no** (images at best) |
| livesync | PouchDB, network, background sync | **no** (use Syncthing or git) |
| style-settings, settings-search | Obsidian's CSS and settings UI | **not applicable** |

## 4. Recommendation

1. **Keep native plugins for the important ones** (Dataview, Templater,
   Periodic Notes, calendar, and next the easy ones in
   `requirements/plugin_candidates.md`): they fit a terminal, are fast,
   and are testable.
2. **If running community plugins matters, build a compatibility layer in
   stages**, each useful on its own, measured against a list of target
   plugins:
   - **Stage A (about 4–6 weeks):** QuickJS, the CommonJS loader, loading
     plugins from `.obsidian/plugins/` (manifest, `data.json`), and the
     core API: `Plugin` lifecycle, `Vault`, `MetadataCache`, commands,
     `Notice`, `moment`, events. Targets: homepage, recent-files (as a
     list), frontmatter-modified-date, tag-wrangler commands.
   - **Stage B (6–8 weeks):** a minimal DOM and the DOM → terminal
     renderer; code block and post processors; `Modal`, `SuggestModal`,
     `Setting`, settings tabs; `Editor` and `EditorSuggest`. Targets:
     task-progress-bar, numerals, quickadd, slash-commander, cmdr.
   - **Stage C (open ended):** `ItemView` panels, network with
     permissions. CodeMirror extensions and graphical plugins stay out.
3. **Decide the security model first**: which permissions a community
   plugin gets (vault read / write, network), asked per plugin on install.

A full, drop-in Obsidian plugin runtime would mean re-creating a browser
(DOM, CSS, canvas) and CodeMirror inside the terminal app; that isn't a
good trade for blackglass.
