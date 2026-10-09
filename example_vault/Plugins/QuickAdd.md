# QuickAdd

**QuickAdd** adds things in a few keystrokes. Each **choice** is a command
(Ctrl+P, then its name, or give it a key in Settings → Keyboard
shortcuts); "QuickAdd: Run QuickAdd" lists them all, groups first.

- A **capture** asks its questions one after the other, puts the answers
  into its format, and adds the text to a note: today's daily note, a note
  by path, a note you choose from a folder, or the note you're in.
- A **template choice** makes a new note from a template, its name and
  folder from formats; the template's own `{{…}}` are asked too.

This vault has these (the Log and Reading groups in "Run QuickAdd"):

| Choice | Asks | Adds |
|--------|------|------|
| **Expense** (Log) | an envelope, the amount, what for | `- [[Groceries]] 68.40 weekly shop` under `## Money` in today's note, for the [[Budget]] |
| **Habits today** (Log) | walked? meditated? minutes read, glasses of water | today's line in this month's [[Habits]] log (the month's note made when it's new) |
| **Idea** | the idea | `- an idea (from [[the note you're in]], 14:05)` at the top of `Ideas`, under this month's heading, newest first |
| **Tasks for a project** | a project, then tasks (Alt+Enter between them) | one task a line, under the project's `## Tasks` |
| **New book** (Reading) | its title, author, genre (the genres in `Books/`, or a new one), pages, status | a note in `Books/` for the [[Reading tracker]], from `Templates/New book` |

## Making one

Settings (**Alt+,**) → QuickAdd: "Add a capture", "Add a template
choice" or "Add a global variable" asks for a name; then set its rows
there. A capture's:

- **format:** the text. As it is, and in double braces:
  - `{{VALUE}}` asks for a value; `{{VALUE:amount}}` names the question
    (used twice, asked once); `{{VALUE:title|Untitled}}` has a default,
    `{{VALUE:x|label:How much?}}` its own question,
    `{{VALUE:notes|type:multiline}}` takes several lines;
  - `{{VALUE:small,medium,big}}` asks you to choose one (`|custom`: or
    type another, `|multi`: several);
  - `{{FIELD:genre}}` suggests the values a property has in the vault
    (`|folder:Books`, `|tag:`, `|inline:true` for `genre:: x` fields);
  - `{{FILE:Budget/Envelopes}}` asks for a note in that folder and puts in
    its name (`|link` as a link, `|path` its path);
  - `{{DATE}}` (`2026-10-08`), `{{DATE:dddd D MMMM}}`, `{{DATE+7}}` a week
    on, `{{DATE:YYYY-MM-DD|startof:month}}`, `{{TIME}}`;
    `{{VDATE:due, D MMM}}` asks for a date (`next friday` works);
  - `{{LINKCURRENT}}` a link to the note you're in, `{{LINKSECTION}}` to
    its heading, `{{SELECTED}}` its selection, `{{TITLE}}` the note being
    written to;
  - `{{GLOBAL_VAR:signature}}` puts in a global variable,
    `{{TEMPLATE:Templates/x}}` a note's text, `{{CURSOR}}` leaves the
    cursor there, `{{RANDOM:6}}` six random letters and digits;
  - `|case:kebab` (`snake`, `title`, `upper` …) on a value or a date.
- **note:** where it goes: empty for the daily note, a path
  (`Logs/{{DATE:YYYY}}`), `Folder/` or `#tag` to choose a note; **active**
  for the note you're in.
- **place:** under the heading, the top, the bottom; in the note you're
  in, the cursor or a new line below or above it.
- **heading**, **first** (newest first), **heading_missing** (made at the
  top or the bottom), **create** (make the note if it isn't there, from
  **template**), **task**, **per_line**, **day** (ask which day), **link**
  (a link to the note where you are), **open**, **group**.
