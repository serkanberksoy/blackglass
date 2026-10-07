# Workspace

## The sidebar

- **Files**: folders open and close with → / ← (or Enter, a click); each
  top-level folder has its color. **s** sorts: A→Z, Z→A, newest, oldest.
- **Search** and **Tags**: see [[Search and tags]]. **Tab** moves between the
  sidebar's tabs (plugins add more: Recent, Bookmarks, Git …).
- **Alt+B** hides or shows the sidebar; **Ctrl+B** moves the focus between
  the open panes (the sidebar, the calendar, the note, the backlinks).

## Tabs

Every note opens in its own tab (an open note reuses its tab). **Alt+←/→**
or **Ctrl+PgUp/PgDn** switch tabs, **Alt+1 … Alt+9** go to one, **Ctrl+W**
closes one (asking about unsaved changes, shown as a dot). **Ctrl+Alt+←/→**
go back and forward through the notes you were on (the mouse's back and
forward buttons too). The tabs, their cursors and the open folders come back
the next time you open the vault.

## Finding and doing anything

- **Ctrl+O**: go to a note by name (or by its `aliases`); a new name makes
  the note. Try `dune`.
- **Ctrl+P**: every command, the plugins' too, to filter and run. Every
  command can have keys: **Alt+,** → Keyboard shortcuts.
- **F1** (or **Ctrl+H**): the help page, with every key as it is now.
- **Alt+,**: the settings window: Editor, Notes, Dates, Keyboard shortcuts,
  Plugins and each plugin's settings; typing searches them all.
- **Choose theme** (palette): Default, Paper, Ember, Ocean, Forest, High
  contrast, previewed as you move.
- **Open vault** (palette): another folder as a vault.

## The mouse

Clicks place the cursor, follow links, open files, results and tags, switch
tabs and panels; the wheel scrolls. (`blackglass --no-mouse` leaves the
mouse to the terminal.)

## Changes made elsewhere

The vault is watched: a note changed by another program (a sync tool, Git)
is picked up, and an open one is shown again unless it has unsaved changes.
**F5** scans the vault again.
