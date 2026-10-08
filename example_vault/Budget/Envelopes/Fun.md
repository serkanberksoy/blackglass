---
monthly: 100
kind: Wants
since: 2026-08-01
---
# Fun

Concerts, books, games, museums. Filled with `= this.monthly` a month; one of the wants.

## In the envelope now

```dataview
TABLE WITHOUT ID monthly * months AS "Its shares", round(default(sum(map(lines, (l) => number(l.text) * choice(contains(l.text, "]] +") AND l.outlinks[0] = me, 1, -1))), 0), 2) AS "Moved and spent", round(monthly * months + default(sum(map(lines, (l) => number(l.text) * choice(contains(l.text, "]] +") AND l.outlinks[0] = me, 1, -1))), 0), 2) AS "In it now"
FROM "Budget/Envelopes"
WHERE file.link = this.file.link AND since <= date(today)
FLATTEN file.link AS me
FLATTEN (date(today).year - since.year) * 12 + date(today).month - since.month + 1 AS months
FLATTEN list(filter(flat(file.inlinks.file.lists), (l) => meta(l.section).subpath = "Money" AND contains(l.outlinks, me))) AS lines
```

## Month by month

Its share each month, what was added or moved in, and what went out:

```dataview
TABLE WITHOUT ID key AS Month, this.monthly AS Share, round(default(sum(filter(rows.change, (c) => c > 0)), 0), 2) AS "Added", round(default(-sum(filter(rows.change, (c) => c < 0)), 0), 2) AS "Out"
FROM "Journal"
FLATTEN file.lists AS l
WHERE meta(l.section).subpath = "Money" AND contains(l.outlinks, this.file.link)
FLATTEN choice(l.for, l.for, dateformat(file.day, "yyyy-MM")) AS month
FLATTEN number(l.text) * choice(contains(l.text, "]] +") AND l.outlinks[0] = this.file.link, 1, -1) AS change
GROUP BY month
SORT key
```

## Every line

```dataview
TABLE WITHOUT ID file.link AS Day, l.text AS Line
FROM "Journal"
FLATTEN file.lists AS l
WHERE meta(l.section).subpath = "Money" AND contains(l.outlinks, this.file.link)
SORT file.day DESC
```
