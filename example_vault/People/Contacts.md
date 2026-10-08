# Contacts

A **contacts list**: a note for every person in `People/`, with what you
want to keep: `kind` (friend, family, work), `company` (a link to the
company's note), `role`, `email`, `phone`, `birthday`, `city`, when you
were last in touch (`last contact`) and how often you'd like to be
(`keep in touch`: `2 weeks`, `1 month` …). Each person's note keeps a log
of what you talked about. All of it is [[Dataview]] queries, no
JavaScript.

A new contact: make a note in `People/` and give it the same properties
(**Alt+P**). After a call, add a line to their log and change
`last contact`.

## Time to get in touch

```dataview
TABLE WITHOUT ID file.link AS Name, last-contact AS "Last time", keep-in-touch AS Every, (date(today) - (last-contact + keep-in-touch)).days + " days ago" AS Due
FROM "People"
WHERE last-contact + keep-in-touch <= date(today)
SORT last-contact + keep-in-touch ASC
```

## Birthdays coming up

```dataview
TABLE WITHOUT ID file.link AS Name, dateformat(next, "MMMM d") AS Birthday, (next - date(today)).days AS "In days", next.year - birthday.year AS Turns
FROM "People"
WHERE birthday
FLATTEN date(dateformat(date(today), "yyyy") + dateformat(birthday, "-MM-dd")) AS this-year
FLATTEN choice(this-year < date(today), this-year + dur(1 year), this-year) AS next
SORT next ASC
LIMIT 5
```

## Everyone

```dataview
TABLE WITHOUT ID file.link AS Name, kind AS Kind, company AS Company, email AS Email, phone AS Phone, city AS City
FROM "People"
WHERE contains(file.tags, "#person")
SORT kind, file.name
```

## By company

```dataview
TABLE WITHOUT ID key AS Company, join(map(rows, (p) => p.file.link + choice(p.role, " (" + p.role + ")", "")), ", ") AS People
FROM "People"
WHERE company
GROUP BY company
```

## Latest conversations

```dataview
TABLE WITHOUT ID file.link AS Name, l.text AS Note
FROM "People"
FLATTEN file.lists AS l
SORT l.text DESC
LIMIT 6
```
