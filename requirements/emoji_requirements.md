# Emoji Shortcodes: what blackglass has and what's missing

A comparison of blackglass's Emoji Shortcodes plugin (W-129) with the
community plugin [Emoji Shortcodes](https://github.com/phibr0/obsidian-emoji-shortcodes)
by phibr0 (its README and source, read 2026-10-06). IDs are `ES-xx`.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

| ID | Feature | Now | Test / notes |
|----|---------|-----|--------------|
| ES-01 | A suggester after `:` and a letter: shortcodes containing what's typed, with their emoji | ✅ | `workspace::emoji_shortcodes_are_suggested_and_replaced`. The exact one, then those starting with it, come first; not after a letter or digit (`10:30`), not in code |
| ES-02 | Immediate replace: the chosen emoji goes in; off: the shortcode and a space | ✅ | `workspace::emoji_shortcodes_can_stay_shortcodes` |
| ES-03 | Shortcodes in the text shown as their emoji (the original's reading view) | ✅ | In the live preview, not on the line being edited (mdedit's rendered spans) |
| ES-04 | Recently used first, a history limit, clearing the history | ✅ | Kept in `.blackglass/plugins/emoji-shortcodes/history`; "Clear emoji history" |
| ES-05 | The same shortcodes (GitHub's) | ✅ | From the `emojis` crate through mdedit, without the sequences terminals draw too wide |
| ES-06 | Definition lists (the original has them, turned off) | ✗ | Not part of the plugin as shipped |
