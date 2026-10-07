# Dates and durations

Dates subtract into durations, and durations add to dates.

## How long each book took

```dataview
TABLE WITHOUT ID
  file.link AS Book,
  read AS Finished,
  read - date(2026-07-01) AS "Since July 1st",
  durationformat(read - date(2026-07-01), "d 'days'") AS Days,
  read + dur(2 weeks) AS "Review by"
FROM "Books"
WHERE read
SORT read
```

## The journal in July

```dataview
LIST dateformat(file.day, "EEEE")
FROM "Journal"
WHERE file.day >= date(2026-07-01) AND file.day < date(2026-07-01) + dur(1 month)
SORT file.day
LIMIT 7
```

## A month of journal days

```dataview
TABLE WITHOUT ID key AS Month, length(rows) AS Days, round(average(rows.waistline), 1) AS "Avg waistline"
FROM "Journal"
WHERE waistline
GROUP BY dateformat(file.day, "yyyy-MM")
SORT key
```

## On a calendar

A dot on each day with a book finished; the books are listed below.

```dataview
CALENDAR read
FROM "Books"
```
