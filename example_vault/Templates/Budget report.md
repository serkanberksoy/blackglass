---
month: <% tp.file.title.slice(-7) %>
---
# Budget report: <% tp.date.now("MMMM YYYY", 0, tp.file.title.slice(-7) + "-01", "YYYY-MM-DD") %>

[[Budget]]

## The month in numbers

```dataview
TABLE WITHOUT ID default(sum(filter(rows, (r) => startswith(r.l.text, "income")).amount), 0) AS Income, round(sum(filter(rows, (r) => startswith(r.l.text, "[[") AND !contains(r.l.text, "]] +")).amount), 2) AS Spent, round(default(sum(filter(rows, (r) => startswith(r.l.text, "income")).amount), 0) - sum(filter(rows, (r) => startswith(r.l.text, "[[") AND !contains(r.l.text, "]] +")).amount), 2) AS Kept, length(filter(rows, (r) => startswith(r.l.text, "[[") AND !contains(r.l.text, "]] +"))) AS Payments
FROM "Journal"
FLATTEN file.lists AS l
WHERE meta(l.section).subpath = "Money"
FLATTEN choice(l.for, l.for, dateformat(file.day, "yyyy-MM")) AS month
WHERE month = this.month
FLATTEN number(l.text) AS amount
GROUP BY true
```

## Envelopes

Each envelope's share and what was added or moved in (Filled), what it
paid out, and what's in it at the month's end (with what was left from
earlier months):

```dataview
TABLE WITHOUT ID file.link AS Envelope, monthly + round(default(sum(map(filter(here, (l) => contains(l.text, "]] +")), (l) => number(l.text) * choice(l.outlinks[0] = me, 1, -1))), 0), 2) AS Filled, round(default(sum(map(filter(here, (l) => !contains(l.text, "]] +")), (l) => number(l.text))), 0), 2) AS Spent, round(monthly * months + default(sum(map(upto, (l) => number(l.text) * choice(contains(l.text, "]] +") AND l.outlinks[0] = me, 1, -1))), 0), 2) AS "At the end"
FROM "Budget/Envelopes"
WHERE since <= date(this.month)
FLATTEN file.link AS me
FLATTEN (date(this.month).year - since.year) * 12 + date(this.month).month - since.month + 1 AS months
FLATTEN list(filter(flat(file.inlinks.file.lists), (l) => meta(l.section).subpath = "Money" AND contains(l.outlinks, me))) AS lines
FLATTEN list(filter(lines, (l) => choice(l.for, l.for, dateformat(link(l.path).file.day, "yyyy-MM")) = this.month)) AS here
FLATTEN list(filter(lines, (l) => choice(l.for, l.for, dateformat(link(l.path).file.day, "yyyy-MM")) <= this.month)) AS upto
SORT monthly DESC
```

## Overspent

Envelopes below zero at the month's end (move money over from another
one with a `+` line to cover them):

```dataview
TABLE WITHOUT ID file.link AS Envelope, -left AS "Short by"
FROM "Budget/Envelopes"
WHERE since <= date(this.month)
FLATTEN file.link AS me
FLATTEN (date(this.month).year - since.year) * 12 + date(this.month).month - since.month + 1 AS months
FLATTEN list(filter(flat(file.inlinks.file.lists), (l) => meta(l.section).subpath = "Money" AND contains(l.outlinks, me) AND choice(l.for, l.for, dateformat(link(l.path).file.day, "yyyy-MM")) <= this.month)) AS upto
FLATTEN round(monthly * months + default(sum(map(upto, (l) => number(l.text) * choice(contains(l.text, "]] +") AND l.outlinks[0] = me, 1, -1))), 0), 2) AS left
WHERE left < 0
```

## Where the money went

```dataview
TABLE WITHOUT ID key AS Envelope, round(sum(rows.amount), 2) AS Spent, padleft("", round(sum(rows.amount) / 40), "█") AS " "
FROM "Journal"
FLATTEN file.lists AS l
WHERE meta(l.section).subpath = "Money" AND startswith(l.text, "[[") AND !contains(l.text, "]] +")
FLATTEN choice(l.for, l.for, dateformat(file.day, "yyyy-MM")) AS month
WHERE month = this.month
FLATTEN number(l.text) AS amount
GROUP BY l.outlinks[0]
SORT sum(rows.amount) DESC
```

## Needs, wants and savings

What each kind of envelope paid out:

```dataview
TABLE WITHOUT ID key AS Kind, round(sum(rows.amount), 2) AS Spent
FROM "Journal"
FLATTEN file.lists AS l
WHERE meta(l.section).subpath = "Money" AND startswith(l.text, "[[") AND !contains(l.text, "]] +")
FLATTEN choice(l.for, l.for, dateformat(file.day, "yyyy-MM")) AS month
WHERE month = this.month
FLATTEN number(l.text) AS amount
GROUP BY l.outlinks[0].kind
```

## The five biggest payments

```dataview
TABLE WITHOUT ID file.link AS Day, l.outlinks[0] AS Envelope, amount AS Amount, l.text AS Line
FROM "Journal"
FLATTEN file.lists AS l
WHERE meta(l.section).subpath = "Money" AND startswith(l.text, "[[") AND !contains(l.text, "]] +")
FLATTEN choice(l.for, l.for, dateformat(file.day, "yyyy-MM")) AS month
WHERE month = this.month
FLATTEN number(l.text) AS amount
SORT amount DESC
LIMIT 5
```

## Day by day

```dataview
TABLE WITHOUT ID key AS Day, round(sum(rows.amount), 2) AS Spent, length(rows) AS Payments
FROM "Journal"
FLATTEN file.lists AS l
WHERE meta(l.section).subpath = "Money" AND startswith(l.text, "[[") AND !contains(l.text, "]] +")
FLATTEN choice(l.for, l.for, dateformat(file.day, "yyyy-MM")) AS month
WHERE month = this.month
FLATTEN number(l.text) AS amount
GROUP BY file.link
SORT key
```
