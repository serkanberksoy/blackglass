# Code blocks

## Fenced

Three backticks, the language, the code, three backticks. The language
picks the colors:

```python
def greet(name: str) -> str:
    return f"Hello, {name}!"
```

```json
{ "name": "blackglass", "notes": 42, "synced": true }
```

```bash
for f in *.md; do wc -l "$f"; done
```

Without a language, it's plain:

```
Plain text, kept as written.
    Spaces too.
```

## With tildes

Three tildes work as well, handy around code that has backticks in it:

~~~markdown
A *Markdown* example with a code span.
~~~

## Indented

After a blank line, four spaces (or a tab) make a code block:

    indented code
    is code too

## Inline

`code` inside a sentence; see [[Text styles#Code with backticks inside]].

## Special blocks

Some languages are run instead of shown: `mermaid` draws a diagram
([[Mermaid]]), `dataview` and `dataviewjs` run a query ([[Dataview]]),
`query` runs a search ([[Search and tags]]), `base` shows a base
([[Bases]]).
