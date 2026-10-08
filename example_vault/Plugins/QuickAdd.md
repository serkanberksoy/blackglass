# QuickAdd

**QuickAdd** adds a line to a note in a few keystrokes: a **capture**
asks its questions one after the other, puts the answers into its format,
and adds the line to today's daily note (or another note), at the end of
the list under a heading. The heading is made if the note hasn't got it.

This vault has three (Ctrl+P, then their names; or "QuickAdd: Run
QuickAdd" to choose one):

| Capture | Asks | Adds |
|---------|------|------|
| **Expense** | an envelope, the amount, what for | `- [[Groceries]] 68.40 weekly shop` under `## Money` in today's note, for the [[Budget]] |
| **Habits today** | walked? meditated? minutes read, glasses of water | today's line in this month's [[Habits]] log |
| **Idea** | the idea | `- an idea (from [[the note you're in]], 14:05)` in `Ideas`, under this month's heading |

Each capture is a command, so it can have its own key: Settings →
Keyboard shortcuts.

## Making a capture

Settings (**Alt+,**) → QuickAdd → "Add a capture" asks for its name; then
set its three settings there:

- **format:** the line. Text as it is, and in double braces:
  - `{{VALUE}}` asks for a value; `{{VALUE:amount}}` names the question
    (used twice, asked once);
  - `{{VALUE:small,medium,big}}` asks you to choose one;
  - `{{DATE}}` today (`2026-10-08`), `{{DATE:HH:mm}}` in any moment.js
    format; `{{VDATE:due, D MMM}}` asks for a date (`tomorrow` works);
  - `{{NOTE:Budget/Envelopes}}` asks you to choose a note in that folder
    and puts in its name: `[[{{NOTE:Budget/Envelopes}}]]` makes it a link;
  - `{{LINKCURRENT}}` a link to the note you're in, `{{SELECTED}}` its
    selected text.
- **note:** where it goes: empty for today's daily note, or a path
  (`Logs/{{DATE:YYYY}}`); a note that isn't there is made.
- **heading:** the heading it goes under (empty: the note's end).
