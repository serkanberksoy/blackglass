# blackglass: instructions for Claude Code

An Obsidian-style note vault in the terminal, written in Rust with ratatui.
Start with `README.md`. The plan is in `documentation/project_plan.md`, the
features in `requirements/features.md`, and the stack and standards in
`documentation/technology_and_skills.md`.

## Scope

blackglass is the **wrapper project** mdedit's plan (section 6) asks for:
the vault, the sidebar (files, search, tags), tabs, and every feature that
needs to know about other notes. The editor is **mdedit**, used as a
library through a path dependency (`../mdedit`), following
`../mdedit/documentation/embedding.md`: one `Shared`, one `EditorView` per
tab, `ui::EditorWidget`, and our `resolver::VaultResolver`.

- **Never copy mdedit code here.** If blackglass needs something from the
  editor (a hook for `[[` autocomplete, cursor placement by click, styling
  unresolved links), add a generic API to mdedit, with its own tests and
  version bump there, and use it here.
- mdedit must stay free of vault code. Keep host code (tabs, file lists,
  vault index) here.
- ratatui must stay the same version as mdedit's (Frames and Buffers cross
  the boundary).

## Way of working (mandatory)

1. **Every feature is implemented with TDD.** Write the failing test first,
   watch it fail for the right reason, then implement, then refactor. Use
   the `blackglass-feature-workflow` skill for every feature or bugfix; it
   drives `test-driven-development`.
2. **Every feature is automatically tested.** Each row in
   `requirements/features.md` names its test (`file::test_name`), and
   `tests/project.rs` fails if a ✅ / 🟡 row's test doesn't exist.
   Workspace behavior is tested in `tests/workspace.rs` by keys, clicks and
   the drawn screen (`TestBackend`); logic has unit tests in its module.
3. **After every implemented feature:** bump the version in `Cargo.toml`
   (MINOR for a feature, PATCH for a fix), add the `VERSION.md` entry at
   the top, update `README.md` (version, feature count, tables) and, where
   useful, `example_vault/` so the feature can be tried.
   `tests/project.rs` enforces that the versions agree.

A feature is **done** only when `scripts/check.sh` passes (rustfmt,
`clippy -D warnings`, all tests) and its status in
`requirements/features.md` and the project plan is updated.

## Commands

`cargo` is in `~/.cargo/bin` and may not be on `PATH`. Use `export
PATH=$HOME/.cargo/bin:$PATH` or the full path.

```bash
scripts/check.sh                     # the gate
cargo test --test workspace          # the workspace as a user drives it
cargo test --test project            # versions, feature ↔ test mapping, --help
./run.sh [note.md]                   # try it: latest debug build on the example vault
```

## Code standards

- Default rustfmt; no clippy warnings; no `unsafe`; `#[expect(lint, reason = "…")]`
  instead of `#[allow]`.
- Expected failures (I/O) → `Result` (or a message in the status bar);
  broken invariants → panic with a message (`expect("why")`).
- Every module starts with `//!` docs. Match the surrounding code's comment
  density and naming.
- **Every plugin has its own folders**: its code in `src/plugins/<name>/`
  (`mod.rs` with the `Plugin`, other modules beside it; `<name>` is the
  id's first word) and its data in the vault in
  `.blackglass/plugins/<id>/` (settings in `settings.toml`, from
  `plugins::settings_file`). `.blackglass/plugins.toml` only lists what's
  installed and enabled. A requirements file per plugin compares it with
  the Obsidian original: `requirements/<name>_requirements.md`.
  `plugins::tests::each_plugin_has_its_own_folder` checks the code side.
- Every color goes through `ui::theme::Theme` (fitted to the terminal's
  color depth) and every symbol has an ASCII fallback (`Theme::glyph`).
  Draw with the roles (`TEXT`, `ACCENT`, `BG_SIDEBAR` …: a `Paint`), not
  fixed colors, so themes apply; a new role is a `Role` variant, a key in
  `ROLES` and a line in every file in `themes/` (a test checks).
- Skills: `rust-skills` (idiomatic rules), `rust-testing` (test patterns).
  References are in `documentation/references/`.

## Gotchas

- blackglass gets every key first (`App::host_key`), through the keymap
  (`keymap::Keymap`, the user's `keys.toml`): a key some command has runs
  it; an editor command on its own key goes on to the editor. Only give
  host commands default keys that mdedit doesn't use (see mdedit's README
  key table), or the editor loses them.
- Double-size headings are forced off (`App::new`): the editor shares its
  rows with the sidebar, and line sizes apply to whole terminal rows.
- Paths in the vault are canonical (`Vault::open` canonicalizes the root);
  canonicalize any path from outside before comparing.
- Plugins live in `App::plugins` (`Rc<RefCell<Plugins>>`), shared with
  the editor's code block processor (`plugins::Blocks`), which borrows it
  while drawing. Never hold a `borrow_mut()` across a draw; call
  `vault_changed` after every change to notes (scan, save, new note).
- A plugin's block rendering runs every frame: cache it (see
  `plugins::dataview`), and clear the cache when the vault changes.
- JavaScript (`plugins::js`) runs in a fresh Boa context per run: data
  goes in as JSON (`js::literal`), results come back as a JSON string in
  `globalThis.__result` (errors in `__error`). Natives are plain `fn`s; a
  native that needs Rust state reads a `thread_local!` set around the run
  (see `dataview::js`). Never give scripts file or network access, and
  keep the loop / recursion limits. Templater's `tp.web`, `tp.user`
  commands and note reads aren't the script's: it asks (`__need`), the
  host checks the settings (`outside::allowed`: Web access, only the
  settings' commands, paths inside the vault) and gets them.
- crossterm is **vendored and patched** (`vendor/crossterm`, a workspace
  member; `[patch.crates-io]` in `Cargo.toml`) so the mouse's back /
  forward buttons arrive. Record every change in its `SOURCE.md`; its
  parser tests run in `scripts/check.sh`. When updating ratatui (and so
  crossterm), re-apply the patch or drop it if upstream reads the buttons.
- The back / forward history (`App::track`) runs around every key, click
  and paste and in `App::open`; code that switches notes needs nothing
  more. Back / forward themselves use `App::show`, which doesn't record.
- New commands go in `commands::builtin()` (palette order) with their
  default keys; the id (`kebab-case` of the name) is what `keys.toml` and
  `{id}` in `src/help.md` use, so don't rename commands lightly. Editor
  commands send mdedit its key, so they stay in step with mdedit.
- A plugin can add a sidebar tab (`Plugin::sidebar_tab` / `sidebar_rows`
  / `sidebar_key`, `Panel::Plugin(id)`); `App::refresh_plugin_tabs` keeps
  the tabs and the row count current and runs before drawing and before
  sidebar keys. Plugins hear of every visited note from `App::track`
  (`on_note_opened`) and of moves (`on_note_moved`).
- Background work (the Git plugin) runs on a thread and reports through
  `Plugin::tick`, which the main loop calls about twice a second when idle
  (`App::tick`; tests call it in a loop). Never block the UI thread on a
  process or the network. `Effect::FilesChanged` rescans and reloads open
  notes without unsaved changes.
- All settings live in one window (`settings_window`): a new page is a
  `Page` variant, an entry in `App::settings_nav` (its group) and rows in
  `App::settings_rows`; a plugin's settings page comes from its
  `Plugin::settings` alone. Every row needs a one-line help text.
- Settings are saved only when `App::config_dir` is set (`main` sets it);
  tests set it to a temp dir, never the real `~/.config`.
- Plugins can take editor keys (`Plugin::takes_key` / `editor_key`, asked
  after the host's keys and link suggestions: Advanced Tables' Tab), edit
  the active note's lines (`Effect::ReplaceLines`, one undo step) or any
  note's (`Effect::EditNote`, checked against the lines it read), and act
  on their result rows (`plugin:<id>:<payload>` actions →
  `Plugin::row_action`).
- A save changes one note: `App::saved` calls `Plugins::note_changed`
  (the shared page index and `Plugin::on_note_changed`), not the full
  `vault_changed`. A plugin that reads Dataview's pages says so
  (`uses_index`) and gets the shared index (`set_index`, `None` while it's
  updated: don't keep other clones). `App::vault` is an `Rc<Vault>`: change
  it only through `Rc::make_mut`, then call `App::vault_changed` (or the
  save path). Check big changes with `tests/perf.rs`.
- ```` ```query ```` blocks are core (`query_embed`, asked by the code
  block processor before the plugins), on a copy of the vault that
  `Plugins::vault_changed` keeps current. Plugins hear of saves through
  `Plugin::on_note_saved`, and can quit (`Effect::Quit`) or open a web
  page (`Effect::OpenUrl`).
- A `.base` file opens as a read-only page (`App::show_base`) while Bases
  is enabled; its first line `%% base: <path> %%` tells the plugin which
  file it is. `Effect::OpenSource` opens any file as text.
- Themes reach the editor through mdedit's `Shared::palette` (the named
  colors, text and background it draws with; `App::set_palette` sets it)
  and the plugins through `ui::theme::paint(ACCENT)` (the theme in use):
  a plugin draws with named colors or roles, never fixed RGB. Its cache
  clears on `Plugin::on_theme_changed`.
- Plugins can suggest while typing (`Plugin::suggestions`: the start and
  the choices; Tasks' fields) and put text on the clipboard
  (`Effect::CopyText`, `App::copier`). `VaultResolver::embed` decides what
  `![[file#part]]` shows (a `.base` file: its block, `# view: V` first).
- Tests write vaults under `CARGO_TARGET_TMPDIR` or the system temp dir,
  never into `example_vault/`.
- New notes the user makes go through `App::create_new_note`, which gives
  them a note ID when the vault's Notes settings ask for one
  (`note_ids`, `.blackglass/notes.toml`); only notes with names of their
  own (periodic notes) use `create_note_with` directly. The ID's
  properties are merged after any template has run (`add_pending_id`).
- Templater runs a template as a job (`plugins::templater::Job`): its
  questions are asked, then what it needs from outside (`Output::needs`:
  `tp.web` pages, `tp.user` commands, notes) is got (`templater::outside`:
  notes at once, the rest on threads, polled from `Plugin::tick`) and the
  template runs again with the results, until it has everything; then its
  file actions become effects (`file_effects`: `WriteFile`, `MoveNote`,
  `Many`). Periodic Notes shares this (`created_notes`, `sort_needs`,
  `templater::setup`). Only the settings' user commands may run.
- Moving or renaming files goes through `App::apply_moves` (every link to
  every moved file, attachments too: `files_under` for folders);
  `Effect::MoveNote` and the explorer's F2 / "Move" use it.
- `<% %>` is shown as written through mdedit's `markdown::set_verbatim`
  (`App::template_tags`, whenever plugins change).
- The vault is watched (`watch::Watcher`, a thread polling about once a
  second; `App::outside_changes` from `App::tick`): a change the vault
  index already has (blackglass's own saves, new and moved notes) is
  skipped, so every write must keep the index current (`saved`,
  `rescan`). The session (`session`, `.blackglass/workspace.json`) is
  saved from `App::tick` when it changes and on quit, and restored by
  `App::new`; the Git plugin keeps it out of the repository
  (`.git/info/exclude`).
- Plugins see the sidebar's search (`Context::query`) and can run one or
  show a file in the explorer (`Effect::Search`, `Effect::Reveal`).
  Tag / property suggestions (`tag_suggest`) come before the plugins'
  in the same popup.
- Every action the user can take by key is a command (`commands::builtin`
  or a plugin's), so it's in the palette and in Settings → Keyboard
  shortcuts; no fixed keys in `App::host_key` (only a list's own keys:
  ↑/↓, Enter, Esc in a popup or the sidebar). An editor key taken off its
  command is held back only where it would reach the editor
  (`dispatch_key`, `Focus::Editor`), so Tab still works elsewhere.
- Plugins can: suggest inside links too (`Plugin::suggestions` gets the
  vault; it's asked after `[[Note#`, `[[##`, `[[^^`, not for a note's
  name), with an effect when a suggestion is chosen (`Suggestions::
  effects`); name notes (`Plugin::display_name`, used everywhere a note
  is named through `App::shown_name`; asked while drawing, so cache);
  badge links and render spans in the live preview (`link_badge`,
  `rendered_spans` / `render_span`, through mdedit's `set_link_badge` /
  `set_rendered`, set in `App::template_tags`); delete a note and point
  links elsewhere (`Effect::DeleteNote`, `Effect::Retarget`). A note a
  plugin makes (`Effect::CreateFile`) runs its Templater commands.
