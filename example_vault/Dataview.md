# Dataview

The Dataview plugin runs the queries below over this vault. Move the cursor
into a block to edit its query; the result comes back when you leave it.
Plugins: **Ctrl+P** → *Open plugins*.

## Books by rating

```dataview
TABLE author AS Author, rating AS Rating FROM "Books" SORT rating DESC
```

## Journal days with a waistline

```dataview
TABLE WITHOUT ID file.day AS Day, waistline, category
FROM "Journal"
WHERE waistline
SORT file.day DESC
```

## Notes about reading

```dataview
LIST FROM #reading AND -"Templates" SORT file.name
```

## Open tasks

```dataview
TASK WHERE !completed
```

## Linking to Dune

```dataview
LIST length(file.outlinks) FROM [[Dune]]
```
