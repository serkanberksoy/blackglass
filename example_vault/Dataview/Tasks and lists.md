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
FROM "Dataview/Tasks and lists"
FLATTEN file.tasks AS t
WHERE t.children
```

## Every list item, with its section

```dataview
TABLE WITHOUT ID l.section AS Section, l.text AS Item, l.task AS "Task?"
FROM "Dataview/Tasks and lists"
FLATTEN file.lists AS l
```
