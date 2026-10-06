# Natural language dates: what blackglass has and what's missing

blackglass's natural language dates (W-128, built in) compared with the
community plugin [Natural Language Dates](https://github.com/argenos/nldates-obsidian)
(its README and source, read 2026-10-06). IDs are `ND-xx`.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| ND-01 | Suggestions after a trigger (`@`): Today, Yesterday, Tomorrow; `next` / `last` / `this` + week, month, year, weekdays; `in N` minutes … months, `N days ago` …; `time:` now, ±15 minutes, ±1 hour; anything else it reads | ✅ | `nldates::tests::suggested_while_typing`, `workspace::dates_in_words_become_links_to_their_days` |
| ND-02 | Enter puts in `[[date]]` (or the date, with links off); Shift+Enter keeps the words as the alias (`[[2026-10-07\|tomorrow]]`) | ✅ | same. A terminal must tell Shift+Enter from Enter (most with the kitty keyboard protocol) |
| ND-03 | Not after a letter or digit (an e-mail address), not in code | ✅ | same |
| ND-04 | Parse natural language date (`[[date]]`), as link (`[text](date)`), as plain text; parse natural language time | ✅ | same |
| ND-05 | Insert the current date, time, date and time | ✅ | `workspace::dates_follow_their_settings` ("Insert today's date" is the current date) |
| ND-06 | Date picker | 🟡 | A prompt for a date in words, showing the date it reads; no calendar to click (the Periodic Notes calendar is one) |
| ND-07 | Settings: date format, time format, separator, week start, trigger, autosuggest on/off, insert as link | ✅ | The Dates page; the vault's `.blackglass/dates.toml` |
| ND-08 | Phrases: today, tomorrow, yesterday, weekdays, this/next/last week/month/year/weekday, in N units, N units ago / from now, ±N units, month and day (with year, ordinals), the Nth, end of / last day of / mid month, Christmas, ISO dates, `m/d[/y]`, a time after `at` | ✅ | `nldates::tests::days_in_words`, `times_in_words` |
| ND-09 | chrono-node's whole grammar: ranges (`17 August - 19 August`), other languages, British day-first order | ⬜ | English, month first |
| ND-10 | A URI action (`nldates?day=…`) | ✗ | blackglass has no URI handler |
| ND-11 | The `parseDate` API for other plugins | 🟡 | `nldates::parse_date` / `parse_time` for blackglass's own code |
