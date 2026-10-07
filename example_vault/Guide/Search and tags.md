# Search and tags

## The vault search

**Ctrl+G** searches every note; Enter opens a result at the match and **F3**
finds the next one. The search reads:

| Type | Finds |
|------|-------|
| `spice desert` | notes with both words |
| `"plain text"` | the phrase |
| `spice -desert` | spice, not desert |
| `dune OR emma` | either |
| `tag:reading` | notes tagged #reading (and its nested tags) |
| `file:dune`, `path:Books` | by name, by folder |
| `line:(spice desert)` | both words on one line |
| `task-todo:paint` | open tasks with the word |
| `[author]`, `[rating:5]` | notes with a property (with a value) |

## Tags

The **Tags** tab lists every tag with how many notes have it; nested tags
form a tree (`#project/garden` under `#project`), → / ← open and close.
Typing `#` in a note suggests the vault's tags. #project/garden

## A search in a note

A `query` block runs a search where it is; click a result to open it:

```query
tag:reading
```
