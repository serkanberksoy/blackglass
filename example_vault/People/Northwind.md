---
kind: company
tags: [company]
---
# Northwind

A client: the onboarding and platform projects.

## People

```dataview
TABLE WITHOUT ID file.link AS Name, role AS Role, email AS Email
FROM "People"
WHERE company = this.file.link
SORT file.name
```
