# The query language

Each block is a query; move the cursor into one to read it. Commands
(`WHERE`, `SORT`, `LIMIT`, `GROUP BY`, `FLATTEN`) run in the order written.

## Grouping: books by genre

`GROUP BY` makes one row per group: `key` is the group, `rows` its pages,
and `rows.file.link` every page's link.

```dataview
TABLE length(rows) AS Books, rows.file.link AS Titles, sum(rows.pages) AS Pages
FROM "Books"
WHERE genre
GROUP BY genre
SORT key
```

## Flattening: one row per tag

```dataview
TABLE WITHOUT ID tag AS Tag, file.link AS Book
FROM "Books"
FLATTEN file.etags AS tag
SORT tag, file.name
```

## Order matters

`LIMIT` before `SORT`: the first two books by name, then the best of them
first.

```dataview
LIST rating
FROM "Books"
SORT file.name
LIMIT 2
SORT rating DESC
```

## Values only

```dataview
LIST WITHOUT ID author + " (" + rating + "★)"
FROM "Books"
WHERE author != "various"
SORT author
```

## Links out of this note

This note links to [[Dune]] and [[Foundation]].

```dataview
LIST FROM outgoing([[]])
```

## Expressions

Lambdas, lists, objects and indexing:

```dataview
TABLE WITHOUT ID
  file.link AS Book,
  map(file.etags, (t) => upper(t)) AS Tags,
  {pages: pages, perStar: round(pages / rating)}.perStar AS "Pages per ★",
  [[Dune]].author AS "Dune's author",
  regexreplace(author, "^(\w)\w* ", "$1. ") AS Short
FROM "Books"
WHERE pages AND pages % 2 = 1
```

## Functions

```dataview
TABLE WITHOUT ID
  file.name AS Book,
  padleft(string(pages), 4, "·") AS Pages,
  truncate(author, 10) AS Author,
  choice(rating >= 4, "keep", "give away") AS Shelf,
  any(file.etags, (t) => contains(t, "scifi")) AS "Sci-fi?"
FROM "Books"
WHERE pages
SORT pages DESC
```
