# Tracker

**Tracker** shows what your daily notes track, a value a day: a
```` ```tracker ```` block says what to look for, where, and how to show
it. A note's day is in its name (`Journal/2026-07-14`, `dateFormat:
YYYY-MM-DD`). Click a chart to see a day's values in the status bar; in
a month view, `‹` and `›` change the month, and a click on a day shows
its value (a second click opens its note). Press **Ctrl+E** to see a
block's source.

What it can read (`searchType`): `tag` (`#run` counts 1, `#run:5`
counts 5), `frontmatter` (a property), `frontmatter.exists`, `dvField`
(`sleep:: 7`), `task` / `task.done` / `task.notdone`, `text` (a regular
expression, its `(?<value>…)` group the value), `wiki` (links to a
note), `table` and `fileMeta` (`numWords`, `size` …).

## How I slept in July

```tracker
searchType: frontmatter
searchTarget: sleep
folder: Journal
startDate: 2026-07-11
endDate: 2026-07-24
summary:
    template: "Average {{average()::%.1f}} h a night, best {{max()}} h on {{maxDate()::D MMMM}}; {{numDaysHavingData()}} nights written down"
```

```tracker
searchType: frontmatter
searchTarget: sleep
folder: Journal
startDate: 2026-07-11
endDate: 2026-07-24
line:
    title: Sleep
    yAxisLabel: hours
    yMin: 4
    yMax: 9
```

## Mood, in words

Words become numbers with `textValueMap`:

```tracker
searchType: frontmatter
searchTarget: mood
folder: Journal
startDate: 2026-07-11
endDate: 2026-07-24
textValueMap:
    great: 5
    good: 4
    ok: 3
    tired: 2
bar:
    title: Mood (5 great … 2 tired)
    barColor: gold
```

## A good night's sleep

Days with seven hours or more, circled; days next to each other are a
streak:

```tracker
searchType: frontmatter
searchTarget: sleep
folder: Journal
month:
    threshold: 6.9
    initMonth: 2026-07
    startWeekOn: Mon
```

## Tasks done, out of all

```tracker
searchType: task.done, task
searchTarget: ['', '']
folder: Journal
pie:
    title: Journal tasks
    data: '{{sum(dataset(0))}}, {{sum(dataset(1)) - sum(dataset(0))}}'
    dataName: Done, Open
    dataColor: green, gray
```

```tracker
searchType: task.done
searchTarget: ''
folder: Journal
bullet:
    title: Tasks done in the journal (the goal at 4)
    value: '{{sum()}}'
    range: 2, 4, 6
    markerValue: 4
    valueUnit: tasks
```
