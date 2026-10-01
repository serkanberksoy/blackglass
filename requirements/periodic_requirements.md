# Periodic Notes: what blackglass has and what's missing

A comparison of blackglass's Periodic Notes plugin (W-48 … W-50) with
Liam Cain's [Periodic Notes](https://github.com/liamcain/obsidian-periodic-notes)
and its maintained 1.0 fork
([philoserf/obsidian-periodic-notes](https://github.com/philoserf/obsidian-periodic-notes)),
read 2026-09-29. Missing pieces have IDs (`PN-xx`), a rough effort (S / M
/ L) and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

## 1. Notes and commands

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| PN-01 | Daily, weekly, monthly, yearly notes (and quarterly, from the 1.0 betas) | ✅ | – | |
| PN-02 | Per granularity: enabled, format (moment.js), folder, template | ✅ | – | `settings.toml` in `.blackglass/plugins/periodic-notes/` |
| PN-03 | Nested paths from the format (`YYYY/MM/YYYY-MM-DD`) | ✅ | – | |
| PN-04 | "Open today's daily note", "this week's", "this month's" …: create from the template if new | ✅ | – | |
| PN-05 | Jump forwards / backwards to the closest existing note | 🟡 | S | One "Next / Previous periodic note" pair for the active note's granularity; the original has a pair per granularity ("Jump forwards to closest weekly note") |

## 2. Template tags

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| PN-10 | `{{title}}`, `{{date}}`, `{{date:FMT}}`, `{{time}}`, `{{time:FMT}}` | ✅ | – | |
| PN-11 | Daily: `{{yesterday}}`, `{{tomorrow}}` | ✅ | – | |
| PN-12 | Weekly: `{{monday:FMT}}` … `{{sunday:FMT}}` | ✅ | – | |
| PN-13 | Offsets: `{{date+1d:FMT}}`, `{{date-1w:FMT}}`, `{{time+2h:HH:mm}}` | ⬜ | S | |
| PN-14 | Monthly `{{month:FMT}}`, `{{month-1M:FMT}}`; yearly `{{year:FMT}}`, `{{year-1y:FMT}}`; quarterly `{{quarter:FMT}}` | ⬜ | S | |
| PN-15 | Templater commands in the template (Templater's "trigger on new file creation") | ✅ | – | Prompts are asked too |

## 3. Settings and views

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| PN-20 | Week start day | ⬜ | S | Weeks start on Monday today (weekday tags, weekly notes) |
| PN-21 | Locale weeks: `ww` / `gggg` (locale) versus `WW` / `GGGG` (ISO) | 🟡 | S | Both are ISO today; differs from the original for a Sunday week start |
| PN-22 | Open today's daily note on startup | ⬜ | S | Also the homepage plugin (`plugin_candidates.md`) |
| PN-23 | Calendar view in the sidebar: click a day / week / month to open or create its note, notes highlighted, week numbers | ✅ | – | blackglass 0.7.0: Alt+C; keys too (←→↑↓, PgUp/PgDn, Enter, w, m, t); no yearly / quarterly cells |
| PN-24 | Delete a note from the calendar (right-click) | ⬜ | S | With PN-23 |
| PN-25 | Calendar sets: several configurations (e.g. work and personal journals) | ⬜ | M | 1.0 beta feature |
| PN-26 | A settings screen | ✅ | – | blackglass 0.6.0: every period's enabled / format / folder / template |
| PN-27 | Ribbon icons (one per granularity), Ctrl-click to open in a split | ✗ | – | No ribbon or split panes in blackglass; the palette and the calendar (PN-23) cover opening |
