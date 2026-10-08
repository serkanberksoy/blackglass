# Reading tracker

Every book is a note in `Books/` with a few properties: its `author`,
`genre` and `pages`, where it stands (`status`: `to read`, `reading`,
`finished` or `abandoned`), when it was `started` and `read` (finished),
the `page` you're on while reading it, and a `rating` once it's done.
Everything below is a [[Dataview]] query over them: change a property
(**Alt+P** opens a note's properties) and the tracker follows.

## Reading now

```dataview
TABLE WITHOUT ID file.link AS Book, author AS Author, page + " / " + pages AS Page, padleft("", round(page / pages * 20), "█") + padleft("", 20 - round(page / pages * 20), "░") + " " + round(page / pages * 100) + "%" AS Progress, (date(today) - started).days + " days" AS For
FROM "Books"
WHERE status = "reading"
SORT page / pages DESC
```

## Up next

```dataview
LIST author + ", " + pages + " pages"
FROM "Books"
WHERE status = "to read"
SORT pages ASC
```

## Finished

```dataview
TABLE WITHOUT ID file.link AS Book, author AS Author, padleft("", rating, "★") AS Rating, read AS Finished, (read - started).days + 1 AS Days, round(pages / ((read - started).days + 1)) AS "Pages a day"
FROM "Books"
WHERE status = "finished"
SORT read DESC
```

## Month by month

```dataview
TABLE WITHOUT ID dateformat(date(key), "MMMM yyyy") AS Month, length(rows) AS Books, sum(rows.pages) AS Pages, join(rows.file.link, ", ") AS Which
FROM "Books"
WHERE status = "finished"
GROUP BY dateformat(read, "yyyy-MM")
SORT key DESC
```

## By genre

```dataview
TABLE WITHOUT ID key AS Genre, length(rows) AS Books, length(filter(rows, (b) => b.status = "finished")) AS Finished, round(average(filter(rows.rating, (r) => r)), 1) AS "Average rating"
FROM "Books"
WHERE status
GROUP BY genre
SORT length(rows) DESC
```

## This year

```dataview
TABLE WITHOUT ID length(rows) AS Finished, sum(rows.pages) AS Pages, round(average(rows.rating), 1) AS "Average rating", round(sum(rows.pages) / length(rows)) AS "Pages a book"
FROM "Books"
WHERE status = "finished" AND read.year = date(today).year
GROUP BY true
```

## Put down

```dataview
LIST "at page " + page + " of " + pages
FROM "Books"
WHERE status = "abandoned"
```
