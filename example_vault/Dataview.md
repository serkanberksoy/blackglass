# Dataview

The Dataview plugin runs the queries below over this vault. Move the cursor
into a block to edit its query; the result comes back when you leave it.
Plugins: **Ctrl+P** → *Open plugins*.

More examples, one subject each: [[Query language]] (grouping,
flattening, expressions, functions), [[Tasks and lists]] (checking tasks
off from results), [[Dates and durations]] (and a calendar),
[[Inline queries]], and in JavaScript [[Queries]] and [[Dataview extras]].

## Books by rating

```dataview
TABLE author AS Author, rating AS Rating FROM "Books" SORT rating DESC
```

## Journal days with a waistline

```dataview
TABLE WITHOUT ID file.day AS Day, waistline, category
FROM "Journal"
WHERE waistline
SORT file.day DESC
```

## Notes about reading

```dataview
LIST FROM #reading AND -"Templates" SORT file.name
```

## Open tasks

```dataview
TASK WHERE !completed
```

## Linking to Dune

```dataview
LIST length(file.outlinks) FROM [[Dune]]
```

## Journal days, grouped by month

```dataview
TABLE WITHOUT ID key AS Month, length(rows) AS Days, rows.file.link AS Notes
FROM "Journal"
GROUP BY dateformat(file.day, "yyyy-MM") AS month
SORT key DESC
```

## Tags, one row each

```dataview
TABLE WITHOUT ID tag AS Tag, file.link AS Note
FROM "Books"
FLATTEN file.etags AS tag
SORT tag
```

## The journal on a calendar

```dataview
CALENDAR file.day
FROM "Journal"
```

## Inline

This note is `= this.file.name`, it has `= length(this.file.outlinks)`
outgoing links, and a week from today is `= date(today) + dur(1 week)`.
