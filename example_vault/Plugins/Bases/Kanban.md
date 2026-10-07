# Kanban

A **kanban board** is a Bases view (`type: kanban`): a column for each
value of the property it's grouped by. Open
[[Plugins/Bases/Projects.base|Projects.base]] from the file explorer; it's the
same `.base` file the original app (1.14 and later) reads and writes.

| Key | Does |
|-----|------|
| ←→↑↓ | choose a card |
| Enter | open its note |
| Shift+←→ | move the card to the next column: its `status` changes |
| Alt+Shift+←→ | move the column (saved as `groupOrder`) |
| n | a new note in the column, with its status |
| Space | collapse or expand the column |
| Ctrl+Z | undo the last move (Ctrl+Y redoes it) |

Each is a command too (Ctrl+P, "Bases: Move card to next column" …).

**Filters:** press **Alt+F** on the board: the base's filters show above
it as rows (property, operator, value). Add one with **a** (type
`priority`, Enter, `high`): the board follows as you type. **o** changes
the operator, **d** deletes, **Tab** switches to this view's own filters,
**Alt+M** makes the pane bigger or one line, **Esc** goes back to the
board.

What the YAML says:

- `groupBy: status`: the columns. A note with no status goes in **None**.
- `groupOrder`: the columns shown, in order. A value no note has yet is
  an empty column (add one to plan ahead); a value left out is hidden;
  `null` is the None column.
- `groupColors`, `cardColor`: colors for columns and cards (blackglass's
  own keys; the original app ignores them). Colors: red, orange, yellow, green,
  blue, purple, pink, cyan, gray. Here a formula colors urgent cards red.

The board embedded: click a card for its menu (open it, or move it to
another column), a column's heading to fold it.

![[Plugins/Bases/Projects.base]]

## Groups, in every layout

A table, a list or cards grouped by a property (`groupBy`) follow the same
`groupOrder` as the board: its order, the groups it leaves out hidden, an
empty group for one no note has yet. A click on a group's heading folds
it. With the cursor in a base (or on a `.base` page), from the palette:

- **Bases: Reorder groups**: a group, then where it goes;
- **Bases: Show or hide a group**: shown ones are hidden, hidden ones come
  back;
- **Bases: Add a group**: a new, empty group (a column on a board).
