# Budget

An **envelope budget** kept in your daily notes: every envelope (rent,
groceries, fun, a holiday …) gets its share of the income each month,
spending comes out of one, and what's left stays in it for next month.
All of it is plain notes and [[Dataview]] queries, no JavaScript.

## Writing it down

In the day's note (**Alt+D** opens today's), under a heading
`## Money`, one short line a payment: the envelope, the amount, what for.

```
## Money
- [[Groceries]] 68.40 weekly shop
- [[Fun]] 28 cinema
```

Type `[[Gro` and Enter to pick the envelope, or run the **Expense**
capture ([[QuickAdd]]: Ctrl+P, "Expense"), which asks for the envelope,
the amount and what for, and adds the line to today's note. The day is the note's, and so
is the month. Besides payments:

| You write | It means |
|-----------|----------|
| `- income 2800` | income came in |
| `- [[Holiday]] +100 extra` | more into an envelope than its monthly share |
| `- [[Fun]] +30 from [[Eating out]]` | moving money: 30 out of Eating out, into Fun |
| `- [[Gifts]] 30 a present [for:: 2026-10]` | a line that counts in another month than its note's |

**The envelopes** are notes in `Budget/Envelopes/`: what each gets every
month (`monthly`), from when (`since`), its kind (`kind`: Needs, Wants,
Savings) and a savings `goal`. An envelope shows up from its `since` on. Nobody types the monthly filling: each
envelope gets its `monthly` share every month from `since` on. (Changing
`monthly` changes the past months too; to keep them, leave it and add a
`+` line each month instead, or start a new envelope.)

**Each month's report** is a note in `Budget/Reports/` ([[Report 2026-10]])
with just its `month`: make one from the `Budget report` template, named
`Report 2026-11`.

## The envelopes now

```dataview
TABLE WITHOUT ID file.link AS Envelope, kind AS Kind, monthly AS "Each month", round(monthly * months + default(sum(map(lines, (l) => number(l.text) * choice(contains(l.text, "]] +") AND l.outlinks[0] = me, 1, -1))), 0), 2) AS "In it now"
FROM "Budget/Envelopes"
WHERE since <= date(today)
FLATTEN file.link AS me
FLATTEN (date(today).year - since.year) * 12 + date(today).month - since.month + 1 AS months
FLATTEN list(filter(flat(file.inlinks.file.lists), (l) => meta(l.section).subpath = "Money" AND contains(l.outlinks, me))) AS lines
SORT monthly DESC
```

## Savings goals

```dataview
TABLE WITHOUT ID file.link AS Goal, saved AS Saved, goal AS Target, padleft("", round(saved / goal * 20), "█") + padleft("", 20 - round(saved / goal * 20), "░") AS Progress
FROM "Budget/Envelopes"
WHERE goal AND since <= date(today)
FLATTEN file.link AS me
FLATTEN (date(today).year - since.year) * 12 + date(today).month - since.month + 1 AS months
FLATTEN list(filter(flat(file.inlinks.file.lists), (l) => meta(l.section).subpath = "Money" AND contains(l.outlinks, me))) AS lines
FLATTEN round(monthly * months + default(sum(map(lines, (l) => number(l.text) * choice(contains(l.text, "]] +") AND l.outlinks[0] = me, 1, -1))), 0), 2) AS saved
```

## Month by month

Income, spending and what was kept, from the daily notes:

```dataview
TABLE WITHOUT ID key AS Month, default(sum(filter(rows, (r) => startswith(r.l.text, "income")).amount), 0) AS Income, round(sum(filter(rows, (r) => startswith(r.l.text, "[[") AND !contains(r.l.text, "]] +")).amount), 2) AS Spent, round(default(sum(filter(rows, (r) => startswith(r.l.text, "income")).amount), 0) - sum(filter(rows, (r) => startswith(r.l.text, "[[") AND !contains(r.l.text, "]] +")).amount), 2) AS Kept, padleft("", round(sum(filter(rows, (r) => startswith(r.l.text, "[[") AND !contains(r.l.text, "]] +")).amount) / 100), "█") AS " "
FROM "Journal"
FLATTEN file.lists AS l
WHERE meta(l.section).subpath = "Money"
FLATTEN choice(l.for, l.for, dateformat(file.day, "yyyy-MM")) AS month
FLATTEN number(l.text) AS amount
GROUP BY month
SORT key
```

## The plan

What the envelopes get each month:

```dataview
TABLE WITHOUT ID key AS Kind, sum(rows.monthly) AS "Each month", join(rows.file.link, ", ") AS Envelopes
FROM "Budget/Envelopes"
GROUP BY kind
```

```dataview
TABLE WITHOUT ID sum(rows.monthly) AS "Into the envelopes each month"
FROM "Budget/Envelopes"
GROUP BY true
```

## Reports

```dataview
LIST
FROM "Budget/Reports"
SORT month DESC
```
