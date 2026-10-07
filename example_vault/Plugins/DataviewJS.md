# DataviewJS

`dataviewjs` blocks run JavaScript with Dataview's `dv` object: pages,
tables, lists, task lists, and anything you can compute. The engine is
sandboxed: no files (but the vault's, read-only, through `dv.io`) and no
network. **Settings → Dataview → Enable JavaScript** turns it off.

- [[Queries]]: pages, tables, lists and task lists from JavaScript.
- [[Charts]]: charts and a link graph drawn as text.
- [[Dataview extras]]: `dv.io` (a CSV file), `dv.view` (scripts in
  `Data/views/`), Luxon dates, swizzling, Markdown tables.

A one-line example:

```dataviewjs
dv.paragraph(`${dv.pages('"Books"').length} books, ${dv.pages('"Journal"').length} journal days`);
```

Inline, too: there are `$= dv.pages('"Books"').length` books.
