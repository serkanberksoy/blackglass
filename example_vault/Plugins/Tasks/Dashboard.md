# Dashboard

Task lists in callouts, and the notes that aren't connected yet. The
tasks come from every note (try `Plugins/Tasks.md`).

## Tasks
>[!danger] Tasks Within Two Weeks
>```tasks
>not done
>due AFTER yesterday
>due BEFORE in two weeks
>sort by due
>limit 10
>```

>[!warning] Next Month
>```tasks
>not done
>due AFTER two weeks
>due next month
>no scheduled date
>sort by due
>limit 10
>```

>[!example] [[Tasks Backlog]]
>```tasks
>not done
>limit 10
>```

## Orphans

Notes nothing links to and that link to nothing, newest first:

```dataview
list from "" where length(file.inlinks) =0 and length(file.outlinks) = 0
sort file.mtime desc
limit 3
```

## Recent projects

```dataview
list
from ""
where contains(tags, "project")
and !contains(file.folder, "Templates")
sort file.mtime desc
limit 15
```
