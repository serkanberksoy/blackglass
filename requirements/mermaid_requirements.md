# Mermaid: what blackglass has and what's missing

blackglass's Mermaid plugin (id `mermaid`, built in 0.30.0 as W-104)
compared with [Mermaid](https://mermaid.js.org/) as Obsidian shows it (a
```` ```mermaid ```` block drawn as an SVG diagram). A terminal draws text,
so diagrams become text drawings; what can't be drawn is shown as its
source. Each piece has an ID (`MM-xx`), a rough effort (S / M / L) and
notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## 1. Flowcharts

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| MM-01 | `graph` / `flowchart` with `TD`, `TB`, `BT`, `LR`, `RL` | 🟡 | – | All drawn as a tree of their links (the direction isn't used) |
| MM-02 | Node shapes `[ ]`, `( )`, `{ }`, `(( ))`, `([ ])`, `[( )]`, `[[ ]]`, `{{ }}`, `[/ /]`, `>  ]`; quoted text | ✅ | – | Shown with their brackets |
| MM-03 | Links `-->`, `---`, `-.->`, `==>`; labels `-->\|text\|` and `-- text -->`; chains `A --> B --> C` | ✅ | – | Dotted `┄`, thick `═`, open `──` |
| MM-04 | A node reached again shown as `↺ name`; several roots; nodes without links | ✅ | – | |
| MM-05 | `A & B --> C`, `subgraph … end` shown as groups | ✅ | S | 0.43.0: `A & B --> C` links each; subgraphs listed after the tree (`▣ Title: A, B`) |
| MM-06 | A real 2D layout (boxes and routed arrows) | ⬜ | L | The tree reads well in a terminal; a layered box layout could be an option |
| MM-07 | `style`, `classDef`, `class`, `click`, `linkStyle` | ✗ | – | Ignored (colors and links in an SVG) |

## 2. Sequence diagrams

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| MM-10 | Participants and actors (`as` names), lifelines, messages `->>`, `-->>`, `->`, `-->`, `-x`, `-)` with their text | ✅ | – | Solid `──▶`, dotted `┄┄▶` |
| MM-11 | Messages to oneself | ✅ | – | `├─↺` |
| MM-12 | `Note over / left of / right of` | ✅ | – | A row at the participant |
| MM-13 | `loop`, `alt`, `else`, `opt`, `par`, `critical`, `break`, `end` | 🟡 | – | A row each (`┄ loop …`), no boxes |
| MM-14 | Too wide for the note: a list of messages | ✅ | – | |
| MM-15 | `activate` / `deactivate`, `autonumber` | 🟡 | S | 0.43.0: `autonumber` numbers the messages; `activate` / `deactivate` ignored |

## 3. Other diagrams

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| MM-20 | Pie charts as bars with their shares, Gantt charts as a timeline (sections, tasks by date, `after`, days and weeks) | ✅ | – | 0.43.0 |
| MM-21 | Class, state, ER, mindmap, timeline, git graph … | ⬜ | M each | Shown as their source with a line saying so |
