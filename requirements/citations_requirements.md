# Citations: what blackglass has and what's missing

A comparison of blackglass's Citations plugin (W-123) with the community
plugins it follows, [Citations](https://github.com/hans/obsidian-citation-plugin)
and [Zotero Integration](https://github.com/mgmeyers/obsidian-zotero-desktop-connector)
(their READMEs, read 2026-10-01). Missing pieces have IDs (`CI-xx`), a
rough effort (S / M / L) and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| CI-01 | A bibliography: BibTeX (`.bib`) or CSL JSON (`.json`), in the vault or anywhere | ✅ | – | Zotero's Better BibTeX exports either |
| CI-02 | Insert a citation (`[@key]`, its format a setting) | ✅ | – | Searched by author, year and title |
| CI-03 | Insert a link to a reference's literature note | ✅ | – | |
| CI-04 | Open a literature note, made from a template the first time (`{{title}}`, `{{authors}}`, `{{year}}`, `{{citekey}}`, `{{abstract}}`, `{{url}}`); its name and folder settings | ✅ | – | Templater commands in the template run too |
| CI-05 | `[@key]` shown as "Author Year" in the live preview (several keys, locators) | ✅ | – | mdedit 3.13.0 rendered spans |
| CI-06 | A click on a citation opens its literature note | 🟡 | S | "Open literature note" with the cursor on it does |
| CI-07 | Reload the bibliography when its file changes | 🟡 | S | Read when the vault changes (a rescan, F5) |
| CI-08 | More template fields: `{{containerTitle}}`, `{{DOI}}`, `{{publisher}}`, `{{zoteroSelectURI}}` | ⬜ | S | |
| CI-09 | Annotations and PDF highlights from Zotero | ✗ | – | Needs Zotero's local API or the PDFs' annotations (ZK-54) |
| CI-10 | A bibliography at the end of a note (CSL styles) | ⬜ | L | Needs a CSL processor |
