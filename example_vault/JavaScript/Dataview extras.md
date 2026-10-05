# Dataview extras

What DataviewJS has beyond `dv.pages` and the output calls.

## Swizzling

A page list's field is every page's field (lists are flattened):

```dataviewjs
const books = dv.pages('"Books"');
dv.paragraph("Authors: " + books.author.distinct().sort().join(", "));
dv.paragraph("Tags: " + books.file.etags.distinct().sort().join(" "));
```

## Reading a file: dv.io

`dv.io.csv` reads a CSV file in the vault into objects (numbers stay
numbers); `dv.io.load` reads text. Only files inside the vault, and only
reading.

```dataviewjs
const log = await dv.io.csv("Data/reading-log.csv");
dv.table(["Day", "Book", "Pages", "Note"], log.map(r => [r.date, r.book, r.pages, r.note]));
dv.paragraph(`${log.pages.sum()} pages in ${log.length} sittings`);
```

## Views: dv.view

A view is a script in the vault, run with an input. Here `Data/views/shelf.js`
and `Data/views/stars.js`:

```dataviewjs
await dv.view("Data/views/shelf", {folder: "Books"});
await dv.view("Data/views/stars", {rating: 4});
```

## Dates: dv.luxon

```dataviewjs
const { DateTime } = dv.luxon;
const start = DateTime.fromISO("2026-07-18");
const end = start.plus({ months: 1, days: 15 });
dv.list([
  `Started ${start.toFormat("EEEE d MMMM yyyy")}`,
  `Six weeks later: ${end.toFormat("EEE dd.MM.yyyy")}`,
  `Week ${end.weekNumber}, weekday ${end.weekday}`,
  `${end.diff(start, "days").days} days in between`,
  `Parsed: ${DateTime.fromFormat("02.09.2026", "dd.MM.yyyy").toISODate()}`,
]);
```

## Writing Markdown

```dataviewjs
const books = dv.pages('"Books"').where(p => p.pages).sort(p => p.pages);
dv.paragraph(dv.markdownTable(["Book", "Pages"], books.map(p => [p.file.name, p.pages])));
dv.paragraph(dv.markdownList(books.file.name));
```
