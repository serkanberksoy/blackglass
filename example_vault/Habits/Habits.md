---
read goal: 20
water goal: 8
---
# Habits

A **habit tracker**: each month is a note here ([[Habits 2026-10]]) with
one line a day, its habits as inline fields:

`- [date:: 2026-10-07] [walk:: true] [meditate:: false] [read:: 30] [water:: 7]`

Add today's line at the end of the month's note (a new month is a new
note, `Habits 2026-11`), or run the **Habits today** capture of
[[QuickAdd]] (Ctrl+P, "Habits today"): it asks, adds the line, and makes
a new month's note when there isn't one yet. The goals are this note's properties
(`read goal`, `water goal`; **Alt+P**). Everything below is
[[Dataview]] queries over the lines, no JavaScript.

## The last two weeks

```dataview
TABLE WITHOUT ID l.date AS Day, dateformat(l.date, "ccc") AS " ", choice(l.walk, "✓", "·") AS Walk, choice(l.meditate, "✓", "·") AS Meditate, l.read + " min" AS Read, padleft("", l.water, "▮") AS Water
FROM "Habits"
FLATTEN file.lists AS l
WHERE l.date
SORT l.date DESC
LIMIT 14
```

## Week by week

One square a day, Monday first: ■ done, □ not.

```dataview
TABLE WITHOUT ID key AS Week, join(map(rows.l, (x) => choice(x.walk, "■", "□")), "") AS Walk, join(map(rows.l, (x) => choice(x.meditate, "■", "□")), "") AS Meditate, join(map(rows.l, (x) => choice(x.read >= this.read-goal, "■", "□")), "") AS Read, sum(rows.l.read) + " min" AS "Read in all"
FROM "Habits"
FLATTEN file.lists AS l
WHERE l.date
SORT l.date ASC
GROUP BY dateformat(l.date, "kkkk-'W'WW")
SORT key DESC
```

## Streaks

Days in a row up to the last day logged.

```dataview
TABLE WITHOUT ID (max(rows.l.date) - max(filter(rows.l, (x) => !x.walk).date)).days AS Walk, (max(rows.l.date) - max(filter(rows.l, (x) => !x.meditate).date)).days AS Meditate, (max(rows.l.date) - max(filter(rows.l, (x) => x.read < this.read-goal).date)).days AS Read, (max(rows.l.date) - max(filter(rows.l, (x) => x.water < this.water-goal).date)).days AS Water
FROM "Habits"
FLATTEN file.lists AS l
WHERE l.date
GROUP BY true
```

## Month by month

```dataview
TABLE WITHOUT ID dateformat(date(key), "MMMM yyyy") AS Month, length(filter(rows.l, (x) => x.walk)) + " / " + length(rows) AS Walk, round(length(filter(rows.l, (x) => x.meditate)) / length(rows) * 100) + "%" AS Meditate, round(average(rows.l.read)) + " min" AS "Read a day", length(filter(rows.l, (x) => x.water >= this.water-goal)) + " days" AS "Water goal met"
FROM "Habits"
FLATTEN file.lists AS l
WHERE l.date
GROUP BY dateformat(l.date, "yyyy-MM")
SORT key DESC
```
