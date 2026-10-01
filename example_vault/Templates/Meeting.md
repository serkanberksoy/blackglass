<%*
// Templater JavaScript: ask, compute, loop. Try it with
// Ctrl+P → "Templater: Create new note from template" → Meeting.
const topic = await tp.system.prompt("Topic", "Weekly sync");
const people = (await tp.system.prompt("Attendees (comma separated)", "Ana, Ben")).split(",").map(s => s.trim()).filter(Boolean);
const kind = await tp.system.suggester(["Decision meeting", "Brainstorm", "Status update"], ["decision", "brainstorm", "status"]);
const next = moment().add(7, "days").format("dddd, MMMM Do");
-%>
---
date: <% tp.date.now("YYYY-MM-DD") %>
kind: <% kind %>
attendees: [<% people.join(", ") %>]
---
# <% topic %>

## Attendees
<%* for (const person of people) { tR += `- [ ] ${person}\n`; } -%>

## Agenda
<%* if (kind === "decision") { -%>
1. The question to decide
2. Options, with their costs
3. **Decision**:
<%* } else if (kind === "brainstorm") { -%>
- Ideas, no judging yet
<%* } else { -%>
- What moved since last time
- What's blocked
<%* } -%>

## Next
- Follow-up on <% next %>
- <% tp.file.cursor() %>
