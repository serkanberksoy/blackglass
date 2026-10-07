# Technology & Skills

What blackglass is built with, which standards the code follows, and which
Claude Code skills and reference documents the project uses. The stack and
standards are mdedit's, so both projects read and review the same way.

---

## 1. Technology stack

| Area | Technology | Version | Why |
|------|------------|---------|-----|
| Language | Rust | 1.94, edition 2024 | Same as mdedit |
| Editor | [mdedit](../../mdedit) (path dependency) | 3.0 | Live-preview Markdown editing; embedded through its library API (`embedding.md`), with its `CodeBlockProcessor` for plugin blocks |
| TUI framework | [ratatui](https://ratatui.rs) (crossterm backend) | 0.30 | Must be the same version as mdedit's: frames, buffers and rects cross the boundary |
| Terminal I/O | crossterm (re-exported by ratatui), **patched** in `vendor/crossterm` | 0.29 | Raw mode, keys, mouse, bracketed paste, kitty keyboard protocol; the patch reads the mouse's back / forward buttons (`vendor/crossterm/SOURCE.md`) |
| Text width | unicode-width | 0.2 | Column width of names in the sidebar, tabs and prompts |
| Dates | chrono (clock only) | 0.4 | Dataview dates and the local date for "Insert today's date" |
| JavaScript | boa_engine | 0.22 | DataviewJS and Templater's `<%* %>`: a pure-Rust engine, sandboxed by construction (only what a plugin passes in), with loop / recursion limits |
| PDF text | pdf-extract | 0.12 | "PDF to note" (W-105): a PDF's text, pure Rust (lopdf underneath); a damaged file's panic is caught |
| YAML | yaml-rust2 | 0.13 | Bases (W-74): a base's YAML, pure Rust (the maintained fork of yaml-rust, which syntect already brings) |
| JSON | serde_json | 1 | Data into and results out of scripts |
| Formatting | rustfmt (default style) | toolchain | Required by the Rust Style Guide |
| Linting | clippy, run with `-D warnings` | toolchain | Static verification (M-STATIC-VERIFICATION) |

From mdedit blackglass also uses: `search::matches` (smart-case search, so
vault search behaves like Ctrl+F), `links::resolve`, `files::write_atomic`
(safe writes), `terminal::fit_color` (colors for any terminal),
`config::Config` (settings) and `emoji::Recent`.

### Architecture

```
src/
├── main.rs          terminal setup (paste, mouse, kitty keys) + event loop
├── lib.rs           crate root
├── cli.rs           command-line options and --help
├── config.rs        where settings are; folder or note → vault
├── vault/
│   ├── mod.rs       scan: folder tree, notes (text, tags), tag counts
│   ├── tags.rs      tags from frontmatter and text
│   └── search.rs    vault search, tag: queries
├── resolver.rs      VaultResolver: mdedit's Resolver, vault-wide
├── switcher.rs      quick switcher: fuzzy ranking, "create"
├── commands.rs      commands and the command palette
├── plugins/
│   ├── mod.rs       Plugin trait, registry, state file, code block processor
│   ├── settings.rs  plugin settings: schema, settings.toml
│   ├── moment.rs    moment.js dates, shared by the plugins
│   ├── js.rs        the JavaScript sandbox (Boa), shared by the plugins
│   ├── dataview/    the Dataview plugin: value, index, query, eval, render, js
│   ├── templater/   the Templater plugin: engine, js
│   └── periodic/    the Periodic Notes plugin and its calendar
├── sidebar.rs       Files / Search / Tags panels: rows, keys, clicks
├── workspace.rs     App: tabs (EditorViews), focus, prompts, keymap,
│                    mdedit Outcomes, mouse
└── ui/
    ├── mod.rs       layout: tab bar, path, title, editor column, status
    ├── sidebar.rs   drawing the panels, rainbow folders
    ├── prompt.rs    switcher, palette, plugins window, new note, save as,
    │                unsaved changes
    └── theme.rs     colors (fitted to the terminal), folder palette
tests/
├── workspace.rs     the workspace driven by keys and clicks, screen checked
└── project.rs       versions agree; every done feature has a test; --help
```

---

## 2. Coding standards

In order of precedence (the same as mdedit):

1. **[Rust Style Guide](https://doc.rust-lang.org/style-guide/)**, enforced
   by `cargo fmt`.
2. **[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)**:
   `documentation/references/rust-api-guidelines/checklist.md`.
3. **[Microsoft Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/)**:
   `documentation/references/microsoft-pragmatic-rust-guidelines/pragmatic-rust-guidelines.md`.
4. **Clippy**, the default lint set with warnings treated as errors.
5. **`rust-skills` skill**: 265 detailed rules, consulted when writing or
   reviewing code.

### The rules we apply most

| Rule | Source | In blackglass |
|------|--------|---------------|
| Format with rustfmt | Style Guide | `cargo fmt --check` in `scripts/check.sh` |
| No clippy warnings | M-STATIC-VERIFICATION | `cargo clippy --all-targets -- -D warnings` |
| Lint overrides use `#[expect(...)]` with a reason | M-LINT-OVERRIDE-EXPECT | Not `#[allow]` |
| Detected bugs are panics; expected failures are `Result` | M-PANIC-ON-BUG | File errors become status messages; broken invariants panic |
| Panics have helpful messages | M-PANIC-MESSAGE | `expect("why this can't fail")` |
| No `unsafe` | M-UNSAFE | There is none |
| Every module has module docs | M-MODULE-DOCS | `//!` header on every file |
| Magic values are documented | M-DOCUMENTED-MAGIC | e.g. `READABLE_WIDTH`, `SIDEBAR_WIDTH`, `FOLDER_COLORS` |
| Integration tests live under `tests/` | M-INTEGRATION-TESTS | `tests/workspace.rs` |
| Tests don't repeat the implementation | M-TAUTOLOGICAL-TESTS | Tests check what the user sees, written before the code |
| Modules are balanced | M-BALANCED-MODULES | `vault/` and `ui/` are split by concern |
| Latest edition | M-LATEST-EDITION | Edition 2024 |

---

## 3. Claude Code skills

Installed per project under `.claude/skills/` (copied from mdedit). Each
external skill has a `SOURCE.md` (origin, commit, date) and its `LICENSE`.

| Skill | Source | License | Use it for |
|-------|--------|---------|------------|
| `blackglass-feature-workflow` | this project | – | **Every feature and bugfix**: TDD, the feature ↔ test mapping, mdedit changes, the release steps |
| `test-driven-development` | [obra/superpowers](https://github.com/obra/superpowers) | MIT | Red-green-refactor: no production code without a failing test first |
| `rust-testing` | [affaan-m/ECC](https://github.com/affaan-m/ECC) | MIT | Rust test patterns |
| `rust-skills` | [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills) | MIT | 265 idiomatic Rust rules |

Also available globally: `code-review` and `simplify`.

### Reference documents (`documentation/references/`)

| Document | Source | License |
|----------|--------|---------|
| `microsoft-pragmatic-rust-guidelines/pragmatic-rust-guidelines.md` | [microsoft/rust-guidelines](https://github.com/microsoft/rust-guidelines) | MIT |
| `rust-api-guidelines/checklist.md` | [rust-lang/api-guidelines](https://github.com/rust-lang/api-guidelines) | MIT OR Apache-2.0 |

---

## 4. Testing

| Level | Tool | Where | Catches |
|-------|------|-------|---------|
| Unit | `#[test]` in `#[cfg(test)]` modules | `src/**/*.rs` | Scanning, tags, search, ranking, resolving, sidebar rows and keys |
| Workspace | ratatui `TestBackend` | `tests/workspace.rs` | Keys, clicks, prompts, tabs, what's on screen, every screen size |
| Project consistency | `#[test]` | `tests/project.rs` | Versions agree; every ✅ / 🟡 feature names an existing test; `--help` lists the README's keys |

The editor itself is tested in mdedit (fixtures, vt100, Konsole).

## Build sizes

The debug profile keeps line numbers only (`debug = "line-tables-only"`,
none for dependencies besides mdedit): full debug info made the debug
binary about 370 MB.

Release builds (0.63.1) are about 13 MB, down from 24 MB:

| Step | Binary |
|------|--------|
| Stripped, thin LTO | 24.2 MB |
| boa without its default features (Temporal, float16, `xsum`) | 22.9 MB |
| Fat LTO, one codegen unit | 17.2 MB |
| `opt-level = "s"`, but syntect and the regex engines at 3 | 13.4 MB |

Drawing a 10,000-line note is as fast at `"s"` as at 3 (about 1 ms a
frame). `panic = "abort"` would save more but can't be used: "PDF to
note" catches a damaged PDF's panic. The biggest part left is boa (the
JavaScript sandbox, about 5 MB with its parser); `cargo bloat --release
--crates` shows the rest. A release build takes about two minutes.

## Performance

`tests/perf.rs` measures the workspace on a generated vault of 5000 notes
(and a 10,000-line note) with the plugins on: `cargo test --profile perf
--test perf -- --ignored --nocapture` (the `perf` profile is the release
one with thin LTO, to build faster). As of 1.0.0 (0.63.2 in brackets):

| Step | Time | Memory after |
|------|------|--------------|
| Open the vault | 38 ms (37) | 43 MB |
| Plugins start (index) | 50 ms (35; more plugins now) | 84 MB |
| Key + frame, 10,000-line note | 3.0 ms (2.9) | |
| Key + frame, short note | 0.9 ms (0.8) | |
| Save a note | 7 ms (9), 18 ms the long one (16) | |
| Vault search | 14 ms (12) | |
| Quick switcher, a key + frame | 6 ms | |
| Rescan (F5) | 102 ms (93) | 125 MB |
| A tick saving the session | 3.7 ms (once after a change) | |
| Idle tick | 6 µs | |

The release binary (static, x86_64 Linux) is 14.9 MB, 6.4 MB packed
(14.3 MB at 0.76.1). Of its 8.2 MiB of code, the JavaScript engine (Boa)
is 2.8 MiB and blackglass itself 2.0 MiB; the rest of the file is mostly
data (syntax definitions, emoji); `cargo bloat --release --crates`.

What keeps it so: notes are read and indexed on every core
(`vault::par_map`); a save updates one note everywhere (`Vault::update_note`
adjusts tag counts, `Plugin::on_note_changed` updates the index and the
tasks); one page index is shared by the plugins that use it; the App's
vault is an `Rc` that query blocks read through a `Weak` (changes go
through `Rc::make_mut`, which doesn't copy); code blocks are highlighted
when drawn; drawn plugin blocks are cached until notes change.

`target/` grows with every test binary, version and incremental cache
(it had reached 113 GB, mdedit's 78 GB): run `cargo clean` now and then.
