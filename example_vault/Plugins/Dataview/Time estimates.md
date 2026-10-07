# Time estimates

Give a task an estimate as a field: `[estimate:: 2h]`, `[estimate:: 30m]`,
`[estimate:: 1.5h]` or `[estimate:: 1h 15m]`. It's read as a duration, so
it adds up.

- [ ] Draft the outline [estimate:: 2h]
- [ ] Write the first chapter [estimate:: 1.5h]
- [x] Find a title [estimate:: 30m]
- [ ] Read it aloud [estimate:: 45m]

Left in this note: `= sum(filter(this.file.tasks, (t) => !t.completed).estimate)`

Everything in this note: `= sum(this.file.tasks.estimate)`

Done: `= length(filter(this.file.tasks, (t) => t.completed))` of `= length(this.file.tasks)` tasks.

Each task with its estimate (the field taken out of the task's text):

```dataview
TABLE WITHOUT ID regexreplace(t.text, "\s*\[estimate::[^\]]*\]", "") AS Task, t.estimate AS Estimate, t.completed AS "Done?"
FROM "Plugins/Dataview/Time estimates"
FLATTEN file.tasks AS t
WHERE t.estimate
```

How many tasks, and their estimates added up:

```dataview
TABLE WITHOUT ID length(rows) AS Tasks, length(filter(rows.t, (t) => t.completed)) AS Done, sum(rows.t.estimate) AS "Total estimate"
FROM "Plugins/Dataview/Time estimates"
FLATTEN file.tasks AS t
WHERE t.estimate
GROUP BY true
```

For a whole folder, what's left (put your folder in place of this
note's path):

```dataview
TABLE WITHOUT ID sum(rows.t.estimate) AS "Left to do"
FROM "Plugins/Dataview/Time estimates"
FLATTEN file.tasks AS t
WHERE !t.completed AND t.estimate
GROUP BY true
```

## Over several days

A plan whose tasks have days as well as estimates: [[Project plan]] (a
garden shed, from 12 to 17 October).

Each day: its tasks, the time planned and what's still to do:

```dataview
TABLE WITHOUT ID key AS Day, length(rows) AS Tasks, sum(rows.t.estimate) AS Planned, sum(filter(rows.t, (t) => !t.completed).estimate) AS Left
FROM "Plugins/Dataview/Project plan"
FLATTEN file.tasks AS t
WHERE t.scheduled
GROUP BY t.scheduled
SORT key
```

Each phase (the heading the tasks are under), in the order they start:

```dataview
TABLE WITHOUT ID key AS Phase, length(rows) AS Tasks, sum(rows.t.estimate) AS Planned, min(rows.t.scheduled) AS From, max(rows.t.scheduled) AS To
FROM "Plugins/Dataview/Project plan"
FLATTEN file.tasks AS t
GROUP BY meta(t.section).subpath
SORT min(rows.t.scheduled)
```

The whole project:

```dataview
TABLE WITHOUT ID length(rows) AS Tasks, length(filter(rows.t, (t) => t.completed)) AS Done, sum(rows.t.estimate) AS Total, sum(filter(rows.t, (t) => !t.completed).estimate) AS Left, min(rows.t.scheduled) AS Starts, max(rows.t.scheduled) AS Ends
FROM "Plugins/Dataview/Project plan"
FLATTEN file.tasks AS t
GROUP BY true
```
