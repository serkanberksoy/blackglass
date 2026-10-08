---
kind: company
tags: [company]
---
# Contoso

A company hiring for a platform role.

## People

```dataview
TABLE WITHOUT ID file.link AS Name, role AS Role, email AS Email
FROM "People"
WHERE company = this.file.link
SORT file.name
```
