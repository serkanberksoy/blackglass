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
| PN-05 | Jump forwards / backwards to the closest existing note | ✅ | – | blackglass 0.82.0: a pair per granularity ("Jump forwards to closest weekly note" …) besides "Next / Previous periodic note"; `workspace::periodic_notes_sets_jumps_startup_and_deleting` |

## 2. Template tags

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| PN-10 | `{{title}}`, `{{date}}`, `{{date:FMT}}`, `{{time}}`, `{{time:FMT}}` | ✅ | – | |
| PN-11 | Daily: `{{yesterday}}`, `{{tomorrow}}` | ✅ | – | |
| PN-12 | Weekly: `{{monday:FMT}}` … `{{sunday:FMT}}` | ✅ | – | |
| PN-13 | Offsets: `{{date+1d:FMT}}`, `{{date-1w:FMT}}`, `{{time+2h:HH:mm}}` | ✅ | – | 0.82.0: units `y Q M w d h m s`; `{{date:FMT}}` is the note's date at the current time, as the original; `periodic::tests::template_offsets_months_quarters_and_years` |
| PN-14 | Monthly `{{month:FMT}}`, `{{month-1M:FMT}}`; yearly `{{year:FMT}}`, `{{year-1y:FMT}}`; quarterly `{{quarter:FMT}}` | ✅ | – | 0.82.0: in any note (the original limits each to its granularity) |
| PN-15 | Templater commands in the template (Templater's "trigger on new file creation") | ✅ | – | Prompts are asked too |

## 3. Settings and views

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| PN-20 | Week start day | ✅ | – | 0.82.0: Monday, Sunday or Saturday (`[general] week_start`): weekly notes, `{{monday}}` …, the calendar |
| PN-21 | Locale weeks: `ww` / `gggg` (locale) versus `WW` / `GGGG` (ISO) | ✅ | – | 0.82.0: `ww` / `gggg` / `w` / `e` follow the week start (Sunday or Saturday: the week with January 1st is the first); `moment::tests::locale_weeks` |
| PN-22 | Open today's daily note on startup | ✅ | – | 0.82.0: `[general] open_at_startup` |
| PN-23 | Calendar view in the sidebar: click a day / week / month to open or create its note, notes highlighted, week numbers | ✅ | – | blackglass 0.7.0: Alt+C; keys too (←→↑↓, PgUp/PgDn, Enter, w, m, t); no yearly / quarterly cells |
| PN-24 | Delete a note from the calendar (right-click) | ✅ | – | 0.82.0: Delete on a day in the calendar, asked first (no right-click menu in the terminal) |
| PN-25 | Calendar sets: several configurations (e.g. work and personal journals) | ✅ | – | 0.82.0: `[work/daily]` … beside `[daily]` …; "Add a calendar set", "Switch calendar set" (or the Calendar set setting); every command and the calendar use the set in use, whose name the calendar shows |
| PN-26 | A settings screen | ✅ | – | blackglass 0.6.0: every period's enabled / format / folder / template |
| PN-27 | Ribbon icons (one per granularity), Ctrl-click to open in a split | ✗ | – | No ribbon or split panes in blackglass; the palette and the calendar (PN-23) cover opening |
