# Budget

An **envelope budget**: every bit of income goes into an envelope (rent,
groceries, fun, a holiday …) when it comes in, and spending comes out of
one. What's left in an envelope stays there for next month; an envelope
that runs out is covered by moving money from another. All of it is
plain notes and [[Dataview]] queries, no JavaScript.

- **The envelopes** are notes in `Budget/Envelopes/`: how much each gets
  a month (`monthly`), what kind it is (`kind`: Needs, Wants, Savings),
  and a savings `goal`.
- **Each month** has a ledger in `Budget/Months/` ([[Budget 2026-10]]):
  its `income`, a line filling each envelope, and a line per payment:
  `- Weekly shop [date:: 2026-10-04] [spent:: [[Groceries]]] [amount:: 68.40]`.
  Moving money is two fill lines, one down and one up.
- **Each month's report** is in `Budget/Reports/` ([[Report 2026-10]]).

A new month: choose `Budget/Months` in the file explorer and make a note
(Ctrl+N) named `Budget 2026-11`: the `Budget month` template fills it in
(it asks for the month's income). Its report is a note `Report 2026-11`
in `Budget/Reports`, filled in by the `Budget report` template.

## The envelopes now

```dataview
TABLE WITHOUT ID key AS Envelope, key.kind AS Kind, key.monthly AS "Each month", round(sum(filter(rows.l, (x) => x.fill).amount) - sum(filter(rows.l, (x) => x.spent).amount), 2) AS "In it now"
FROM "Budget/Months"
FLATTEN file.lists AS l
WHERE l.fill OR l.spent
GROUP BY choice(l.fill, l.fill, l.spent)
SORT key.monthly DESC
```

## Savings goals

```dataview
TABLE WITHOUT ID key AS Goal, round(sum(filter(rows.l, (x) => x.fill).amount) - sum(filter(rows.l, (x) => x.spent).amount), 2) AS Saved, key.goal AS Target, padleft("", round((sum(filter(rows.l, (x) => x.fill).amount) - sum(filter(rows.l, (x) => x.spent).amount)) / key.goal * 20), "█") + padleft("", 20 - round((sum(filter(rows.l, (x) => x.fill).amount) - sum(filter(rows.l, (x) => x.spent).amount)) / key.goal * 20), "░") AS Progress
FROM "Budget/Months"
FLATTEN file.lists AS l
WHERE l.fill OR l.spent
GROUP BY choice(l.fill, l.fill, l.spent)
WHERE key.goal
```

## Month by month

```dataview
TABLE WITHOUT ID file.link AS Month, income AS Income, round(sum(filter(file.lists, (l) => l.spent).amount), 2) AS Spent, round(income - sum(filter(file.lists, (l) => l.spent).amount), 2) AS Kept, padleft("", round(sum(filter(file.lists, (l) => l.spent).amount) / 100), "█") AS " "
FROM "Budget/Months"
SORT month
```

## Reports

```dataview
LIST
FROM "Budget/Reports"
SORT month DESC
```

## The envelopes as cards

```base
filters:
  and:
    - file.inFolder("Budget/Envelopes")
views:
  - type: cards
    name: Envelopes
    order:
      - file.name
      - kind
      - monthly
      - goal
```
