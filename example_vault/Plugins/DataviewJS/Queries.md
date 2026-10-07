# DataviewJS queries

JavaScript with Dataview's `dv` object: `dv.pages(source)` (the same
sources as `FROM`), `dv.current()`, `dv.page(name)`, and output with
`dv.header`, `dv.paragraph`, `dv.list`, `dv.table` and `dv.taskList`.

## Books with stars

```dataviewjs
const books = dv.pages('"Books"').where(p => p.rating).sort(p => p.rating, "desc");
dv.table(["Book", "Author", "Rating"],
  books.map(p => [p.file.name, p.author ?? "", "★".repeat(p.rating) + "☆".repeat(5 - p.rating)]));
```

## Journal in numbers

```dataviewjs
const days = dv.pages('"Journal"').where(p => p.waistline);
const w = days.map(p => p.waistline);
const best = days.sort(p => p.waistline).first();
dv.list([
  `${days.length} days measured`,
  `average ${w.avg().toFixed(1)}, lowest ${w.min()} on ${best.file.name}`,
  `average sleep ${days.where(p => p.sleep).map(p => p.sleep).avg().toFixed(1)} hours`,
]);
```

## Open tasks, by note

```dataviewjs
dv.taskList(dv.pages('-"Templates"').flatMap(p => p.file.tasks).where(t => !t.completed));
```

## Tags in the vault

```dataviewjs
const tags = dv.pages().flatMap(p => p.file.tags).groupBy(t => t).sort(g => g.rows.length, "desc");
dv.paragraph(tags.map(g => `${g.key} (${g.rows.length})`).join("  ·  "));
```

## This note

```dataviewjs
const me = dv.current();
dv.paragraph(`${me.file.name} is in ${me.file.folder} and has ${me.file.size} bytes.`);
```
