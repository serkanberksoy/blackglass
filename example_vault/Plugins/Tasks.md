# Tasks

With the **Tasks** plugin, tasks carry dates and priorities:

- [ ] Renew passport ⏫ 📅 2026-10-15
- [ ] Water the plants 🔁 every week 📅 2026-10-02
- [ ] Draft the talk 🔼 ⏳ 2026-10-05 🛫 2026-10-01
- [ ] Book the venue 🆔 venue
  - [ ] Compare three places
  - [ ] Ask for prices
- [ ] Send invitations ⛔ venue
- [x] Buy a notebook ✅ 2026-09-28
- [.] Called the bank: all fine

The fields (dates, priority, 🔁, 🆔, ⛔) are dimmed, so what's to be done
stands out; move the cursor onto a task to see it as written.

Put the cursor on a task and run "Tasks: Toggle task done" (Ctrl+P): the
done date is written, and a 🔁 task gets its next occurrence. "Tasks:
Create or edit task" (**Alt+T**) opens a window with every field (each
with its value, or a default like no priority); "Tasks: Postpone task"
moves it. Both work on a task in a query's results too: Alt+V to view
mode, the cursor on the result, then the command. In the window, a date
field shows a calendar beside it: Shift+arrows move a day or a week,
PgUp / PgDn a month, a click picks a day. "Blocked by" and "Blocks"
are chosen from the vault's tasks: type a few letters, Enter adds the
highlighted one (a task without an ID gets one).

Above each query's results, a toolbar: **Filter results** shows only the
tasks with some text in their description (the query stays as it is),
**Copy results** copies them as Markdown. `hide toolbar` leaves it out.

## Open, by due date

```tasks
not done
path includes Plugins/Tasks
group by due
sort by priority
```

## As a tree

```tasks
path includes Plugins/Tasks
not done
show tree
hide backlink
```

Type a task and a word like `due` or `high`: the fields are suggested
(dates after 📅, rules after 🔁).

## Waiting on something

```tasks
is blocked
explain
```

## Logs

```tasks
status.type is NON_TASK
```

Click a task in a result to check it off.

More: [[Dashboard]] (task lists in callouts), [[Tasks Backlog]], and the
[[Task Archiver]] for moving done tasks out of the way.
