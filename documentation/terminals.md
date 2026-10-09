# Terminals: what works where

blackglass runs in any terminal, but terminals differ in what they pass
on to programs: some keys and mouse buttons are kept by the terminal for
its own use and never reach blackglass. This page lists what's known and
how to work around it.

## The mouse's back / forward (side) buttons

blackglass goes back and forward through the notes you were on with the
mouse's side buttons (W-28), as it does with **Ctrl+Alt+←** / **→**, but
only if the terminal reports them. Terminals that do report them send
them as xterm's buttons 8 and 9 (`ESC [ < 128 ; x ; y M` for back, `129`
for forward); crossterm dropped those, so `vendor/crossterm` is patched
to read them (`vendor/crossterm/SOURCE.md`).

| Terminal | Side buttons reach blackglass? |
|----------|--------------------------------|
| kitty, WezTerm, foot, xterm | Yes |
| Konsole | No: it uses them to switch its own tabs |
| Others | Check with the probe below |

**Konsole** (checked in its source, version 26.08): `MainWindow::eventFilter`
turns the back and forward buttons into its "previous-view" / "next-view"
actions (its tabs), and the terminal view (`TerminalDisplay.cpp`) only
ever reports the left, middle and right buttons to programs. There's no
setting that changes this.

### Checking a terminal

Run the probe in the terminal, press the mouse's buttons over it (the
side ones too), then **q**:

```bash
python3 scripts/mouse_probe.py
```

A terminal that reports the side buttons shows lines like:

```
'\x1b[<128;53;13M'  <- BACK (side button)
'\x1b[<129;53;13M'  <- FORWARD (side button)
```

If only `left`, `right` (and `middle`, the wheel) show up, the terminal
keeps the side buttons, and blackglass can't see them.

### Using them anyway: buttons as keys

Have the desktop turn the side buttons into keys, and give those keys to
blackglass's back and forward:

1. **Map the side buttons to Alt+← and Alt+→.** On KDE Plasma (6.1 and
   later) the mouse's settings can map extra buttons to key combinations;
   on any desktop, `input-remapper` can. Alt+← / Alt+→ are what browsers
   and file managers already use for back and forward, so the buttons
   keep working there.
2. **Give blackglass's back / forward Alt+← and Alt+→.** By default these
   switch tabs, which Ctrl+PgUp / Ctrl+PgDn also do. In Settings →
   Keyboard shortcuts, or in `~/.config/blackglass/keys.toml`:

   ```toml
   go-back = "Alt+Left / Ctrl+Alt+Left"
   go-forward = "Alt+Right / Ctrl+Alt+Right"
   previous-tab = "Ctrl+PgUp"
   next-tab = "Ctrl+PgDn"
   ```

   (`workspace::back_and_forward_can_take_the_side_buttons_keys` checks
   this setup.)

Mapping the buttons straight to Ctrl+Alt+← / → works too, without
changing blackglass's keys, but then the browser's buttons stop going
back and forward.

## Keys a terminal keeps

- **Ctrl+Shift+T** opens a new tab in Konsole (and arrives as Ctrl+T in
  many terminals), so the Tasks window is on **Alt+T**.
- **Ctrl+Shift+Z** (redo) works only where the terminal reports Shift
  with Ctrl and a letter; **Ctrl+Y** redoes everywhere.
- A key a terminal or the desktop takes can be moved: every action is a
  command, with its keys in Settings → Keyboard shortcuts (`keys.toml`).

## Selecting text with the mouse

blackglass takes the mouse (clicks, the wheel, the side buttons). To
select text the terminal's way, hold **Shift** while dragging (most
terminals), or start blackglass with `--no-mouse`.

## Colors and symbols

Colors are fitted to the terminal: true color, 256 or 16 colors. Where
Unicode symbols can't be shown, ASCII ones are used instead. A terminal
with true color and a Nerd Font (or another font with box drawing and
emoji) looks best.

## Windows

Windows Terminal and the Windows console (Windows 10 and later) show every
color, and blackglass knows them without `TERM` or `COLORTERM`. The
mouse's back and forward buttons don't reach programs in a Windows
terminal (crossterm reads only the left, middle and right buttons there);
`blackglass-window.exe` (or `blackglass --gui`) has them. Ctrl+Shift
shortcuts depend on the terminal: Windows Terminal passes most of them.
