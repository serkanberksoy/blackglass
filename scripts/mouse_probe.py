#!/usr/bin/env python3
"""Shows what the terminal sends for mouse buttons (for blackglass's
back / forward buttons, W-28). Run it in the terminal to check, press the
mouse's buttons over this window, then q to quit.

xterm-style terminals send the side buttons as SGR mouse reports with
button 128 (back) and 129 (forward): ESC [ < 128 ; x ; y M.
"""
import os
import sys
import termios
import tty

fd = sys.stdin.fileno()
saved = termios.tcgetattr(fd)
# Button presses and releases, SGR encoding.
sys.stdout.write("\x1b[?1000h\x1b[?1006h")
sys.stdout.write("Press mouse buttons here (the side ones too); q quits.\r\n")
sys.stdout.flush()
try:
    tty.setraw(fd)
    while True:
        data = os.read(fd, 64)
        if data == b"q":
            break
        text = data.decode(errors="replace")
        note = ""
        if text.startswith("\x1b[<"):
            button = int(text[3:].split(";")[0])
            names = {0: "left", 1: "middle", 2: "right", 64: "wheel up", 65: "wheel down",
                     128: "BACK (side button)", 129: "FORWARD (side button)"}
            note = "  <- " + names.get(button & ~(4 | 8 | 16), f"button code {button}")
        sys.stdout.write(repr(text) + note + "\r\n")
        sys.stdout.flush()
finally:
    sys.stdout.write("\x1b[?1006l\x1b[?1000l")
    termios.tcsetattr(fd, termios.TCSADRAIN, saved)
