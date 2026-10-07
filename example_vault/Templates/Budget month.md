---
month: <% tp.file.title.slice(-7) %>
income: <% tp.system.prompt("Income this month", "2800") %>
---
# <% tp.date.now("MMMM YYYY", 0, tp.file.title.slice(-7) + "-01", "YYYY-MM-DD") %>

[[Report <% tp.file.title.slice(-7) %>|The report]] · [[Budget]]

## Fill the envelopes

- [fill:: [[Rent]]] [amount:: 1100]
- [fill:: [[Groceries]]] [amount:: 400]
- [fill:: [[Utilities]]] [amount:: 180]
- [fill:: [[Transport]]] [amount:: 120]
- [fill:: [[Eating out]]] [amount:: 150]
- [fill:: [[Fun]]] [amount:: 100]
- [fill:: [[Gifts]]] [amount:: 50]
- [fill:: [[Holiday]]] [amount:: 200]
- [fill:: [[Emergency fund]]] [amount:: 500]

## Spending

- <% tp.file.cursor() %>
