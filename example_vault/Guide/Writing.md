# Writing

Notes are shown as they read: only the line under the cursor shows its
Markdown. **Alt+V** cycles the modes: live preview → source (every line
raw) → view (read-only; Tab goes from link to link, Enter follows).

## Formatting

(Every kind of formatting, one page each, is in the `Formatting/` folder:
[[Headings and sections]], [[Text styles]], [[Lists]],
[[Quotes and callouts]], [[Code blocks]], [[Tables]],
[[Footnotes and comments]].)

**Bold**, *italic*, ~~struck~~, `code`, ==highlighted==. Select text and type
`*`, `_`, `~`, `=` or `` ` `` to wrap it.

Highlights come in colors: ==🔴urgent==, ==🟠soon==, ==🟡someday==,
==🟢done==, ==🔵info==, ==🟣idea==. Type `==` for a list of colors, or select
text and run "Highlight in blue" (Ctrl+P); "Remove highlight color" makes
one plain again.

Emoji: **Ctrl+E** opens the emoji picker (see [[Emoji Shortcodes]] for
`:smile:`).

## Lists and tasks

- **Enter** continues a list, **Tab** / **Shift+Tab** nest an item.
- **Ctrl+T** makes the line a task, **Ctrl+L** closes or reopens it:
- [ ] try it on this line
- [x] a done one

The [[Tasks]] plugin adds dates, priorities and queries.

## Callouts

> [!tip] A callout
> `> [!tip]`, `[!warning]`, `[!example]` … with a title.

> [!note]- Folded
> A `-` after the type folds it; **Ctrl+K** opens and closes it (and any
> heading's section or list item).

## Footnotes and comments

A footnote[^1] is listed by "Footnotes" in the palette. %%A comment is only
seen while editing it.%%

[^1]: Like this one.

## Code

```rust
fn main() {
    println!("highlighted by language");
}
```

## Tables

| Fruit | Count |
|-------|-------|
| Apple | 3 |
| Pear  | 5 |

[[Advanced Tables]] edits them as tables.

## Finding text

**Ctrl+F** finds in the note (**F3** the next match), **Ctrl+R** finds and
replaces, **Ctrl+Z** / **Ctrl+Y** undo and redo (1000 steps), **Ctrl+S** saves.
