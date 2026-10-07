# Text styles

## Emphasis

| Write | Shows |
|-------|-------|
| `**bold**` or `__bold__` | **bold** |
| `*italic*` or `_italic_` | *italic* |
| `***both***` | ***both*** |
| `~~struck~~` | ~~struck~~ |
| `==highlighted==` | ==highlighted== |
| `` `code` `` | `code` |

They nest: **bold with *italic* inside**, *italic with `code`*,
~~struck **and bold**~~.

Select text and type `*`, `_`, `~`, `=` or `` ` `` to wrap it.

## Highlight colors

A colored circle first: ==🔴urgent==, ==🟠soon==, ==🟡someday==,
==🟢done==, ==🔵info==, ==🟣idea==. Type `==` for the colors, or select text
and run "Highlight in red" … from the palette (**Ctrl+P**).

## Code with backticks inside

Two backticks around code that has one: ``a `tick` inside``.

## Links

- A note: [[Welcome]], with other text: [[Welcome|the start page]].
- A web page: [the Rust book](https://doc.rust-lang.org/book/), or written
  out: https://www.rust-lang.org, <https://crates.io>.
- A link defined below: [ratatui][tui].

[tui]: https://ratatui.rs

## Escapes

A backslash shows the next character as written: \*not italic\*,
\#not-a-tag, \[\[not a link\]\], \==not highlighted\==.

## Line breaks

Lines next to each other are one paragraph when read; a blank line starts
the next paragraph. Two spaces at the end of a line  
break it there.

## Emoji

**Ctrl+E** opens the emoji picker; `:` and a few letters suggest one
(:sparkles:). The common ones are in [[Emoji Shortcodes]].
