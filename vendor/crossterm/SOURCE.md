crossterm 0.29.0, copied from crates.io on 2026-09-29 (MIT; see LICENSE).

Local changes (blackglass), for the mouse's back / forward side buttons:
- `src/event.rs`: `MouseButton::Back` and `MouseButton::Forward`.
- `src/event/sys/unix/parse.rs`: `parse_cb` maps xterm buttons 8 and 9 to
  them (crossterm dropped these events), plus a test.
- `src/terminal/sys/unix.rs`: removed unneeded parentheses (a compiler
  warning, which blackglass's gate treats as an error).

Used through `[patch.crates-io]` in blackglass's `Cargo.toml`, so ratatui
and mdedit get the same crossterm. Drop this copy when crossterm supports
the side buttons itself.
