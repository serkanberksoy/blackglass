# Tasks and lists

`TASK` queries list tasks from anywhere. Press Enter on one (or click it)
to check it off in its note; the ✅ date is added for you.

## Open book tasks, by due date

```dataview
TASK
FROM "Books"
WHERE !completed
SORT due
```

## Due or scheduled before November

Tasks carry their emoji dates as fields: 📅 `due`, ⏳ `scheduled`,
🛫 `start`, ➕ `created`, ✅ `completion`.

```dataview
TASK
WHERE !completed AND (due OR scheduled) AND (due < date(2026-11-01) OR scheduled < date(2026-11-01))
```

## Done, with the day

```dataview
TABLE WITHOUT ID t.text AS Task, t.completion AS Done, file.link AS Note
FROM "Books"
FLATTEN file.tasks AS t
WHERE t.completed
SORT t.completion
```

## Subtasks

- [ ] Plan the reading club
  - [x] Pick a book ✅ 2026-09-10
  - [ ] Find a café
  - [ ] Send the invitations

```dataview
TABLE WITHOUT ID t.text AS Task, length(t.children) AS Steps, t.fullyCompleted AS "All done?"
FROM "Plugins/Dataview/Tasks and lists"
FLATTEN file.tasks AS t
WHERE t.children
```

## Every list item, with its section

```dataview
TABLE WITHOUT ID l.section AS Section, l.text AS Item, l.task AS "Task?"
FROM "Plugins/Dataview/Tasks and lists"
FLATTEN file.lists AS l
```

## Time estimates

Give a task an estimate as a field: `[estimate:: 2h]`, `[estimate:: 30m]`,
`[estimate:: 1.5h]` or `[estimate:: 1h 15m]`. It's read as a duration, so
it adds up.

- [ ] Draft the outline [estimate:: 2h]
- [ ] Write the first chapter [estimate:: 1.5h]
- [x] Find a title [estimate:: 30m]
- [ ] Read it aloud [estimate:: 45m]

Left in this note: `= sum(filter(this.file.tasks, (t) => !t.completed).estimate)`

Everything in this note: `= sum(this.file.tasks.estimate)`

Tasks done in this note (all of them, not only these): `= length(filter(this.file.tasks, (t) => t.completed))` of `= length(this.file.tasks)`.

For a whole folder, what's left (put the folder in place of the note's
path):

```dataview
TABLE WITHOUT ID sum(rows.t.estimate) AS "Left to do"
FROM "Plugins/Dataview/Tasks and lists"
FLATTEN file.tasks AS t
WHERE !t.completed AND t.estimate
GROUP BY true
```
