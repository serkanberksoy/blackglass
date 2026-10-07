---
monthly: 100
kind: Wants
---
# Fun

Concerts, books, games, museums. Filled with `= this.monthly` a month; one of the wants.

## Month by month

```dataview
TABLE WITHOUT ID key AS Month, round(sum(filter(rows.l, (x) => x.fill).amount), 2) AS Filled, round(sum(filter(rows.l, (x) => x.spent).amount), 2) AS Spent
FROM "Budget/Months"
FLATTEN file.lists AS l
WHERE l.fill = this.file.link OR l.spent = this.file.link
GROUP BY file.link
```

## In the envelope now

```dataview
TABLE WITHOUT ID round(sum(filter(rows.l, (x) => x.fill).amount), 2) AS "Filled so far", round(sum(filter(rows.l, (x) => x.spent).amount), 2) AS "Spent so far", round(sum(filter(rows.l, (x) => x.fill).amount) - sum(filter(rows.l, (x) => x.spent).amount), 2) AS "In it now"
FROM "Budget/Months"
FLATTEN file.lists AS l
WHERE l.fill = this.file.link OR l.spent = this.file.link
GROUP BY true
```
