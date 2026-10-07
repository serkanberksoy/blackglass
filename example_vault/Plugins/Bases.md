# Bases

A **base** is a database view of your notes: a `.base` file (or a `base`
block in a note) says which notes (filters), what to compute (formulas) and
how to show them (views: a table, a list, cards or a kanban board). All the
data stays in the notes' properties.

- [[Library.base]]: the books as a table (grouped, with an average),
  cards and a list. Open it from the file explorer; "Bases: Switch view"
  changes the view.
- [[Kanban]]: a project board ([[Projects.base]]): move cards between
  columns by key, which writes each note's `status`.

On a `.base` page:

- **Alt+F** shows its filters above the results: rows of property, operator
  and value; **a** adds one (properties are suggested as you type), and the
  results follow every key. **Alt+M** makes the filters bigger or one line.
- "Bases: Sort view", "Group view", "Set view limit", "Choose view
  properties", "Add formula", "Add view", "Search view" change it without
  YAML; "Bases: Edit base source" opens the YAML itself.
- Enter (or a click) on a row: open the note, or change one of its
  properties.
- "Copy view" (a Markdown table), "Export view as CSV", "New note from view".

A base in a note, the books with 5 stars:

```base
filters:
  and:
    - file.inFolder("Books")
    - rating == 5
views:
  - type: table
    name: Five stars
    order:
      - file.name
      - author
```

An embedded view of a `.base` file:

![[Library.base#Cards]]

"Bases: Create new base" makes a new one (its filters open at once).
