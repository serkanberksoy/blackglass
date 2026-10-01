---
name: blackglass-feature-workflow
description: The mandatory workflow for implementing any feature or bugfix in blackglass. Use it whenever you implement, change or fix workspace behavior (vault, sidebar, tabs, search, tags, prompts, drawing), or pick up an item from requirements/features.md. Covers the TDD loop with workspace tests, the feature ↔ test mapping, changes that belong in mdedit, the version bump, and the VERSION.md/README update.
---

# blackglass feature workflow

Every feature is **test-first**, **automatically tested**, and **released**
with a version bump. No exceptions without the user's explicit permission.

Related skills: `test-driven-development` (the discipline), `rust-testing`
(test patterns), `rust-skills` (code rules).

## 0. Pick the item

- Find its row in `requirements/features.md` (IDs `W-xx`) and its
  milestone in `documentation/project_plan.md`. Work on the current
  milestone unless the user says otherwise.
- **Does it need mdedit?** If the editor must do something new (a hook, a
  position API, link styling), that part is an mdedit feature: do it in
  `../mdedit` with mdedit's own `mdedit-feature-workflow` (its tests, its
  version bump), keeping it generic (no vault code in mdedit). Then come
  back and use it here.

## 1. RED: write the test first

- **Workspace behavior** (keys, clicks, what's on screen): a test in
  `tests/workspace.rs`. Build a vault with `vault(name, files)`, drive it
  with `key`, `ctrl`, `alt`, `typing`, `click`, and check `screen(…)` rows
  or the `App`'s state. Assert on what a user sees, not on internals.
- **Logic** (scanning, tags, search, ranking, resolving, sidebar rows): a
  unit test in the module's `#[cfg(test)] mod tests`. File tests use
  `vault::tests::scratch` (a temp dir), never `example_vault/`.

Then **watch it fail for the right reason:**
```bash
cargo test <test_name>     # must FAIL (not a compile error you didn't intend)
```
If it passes already, the test is wrong or the feature already exists.
Stop and re-check.

## 2. GREEN: minimal implementation

Write the least code that makes the tests pass. Follow
`documentation/technology_and_skills.md` §2: rustfmt, no clippy warnings,
no `unsafe`, errors as `Result` or a status message, bugs as panics with
messages, module docs, colors through `Theme`, ASCII fallbacks.

```bash
cargo test                 # EVERYTHING passes, not just your test
```

## 3. REFACTOR

Clean up with the tests green. Keep modules balanced: split one when it
grows too big (as `ui/` is split into sidebar, prompt and theme).

## 4. Update the status

In `requirements/features.md`, set **Now** to ✅ (done) or 🟡 (partial,
and say what's missing in Notes), and put the test in **Test**
(`file::test_name`). `tests/project.rs` fails if that test doesn't exist.

## 5. Release: version, VERSION.md, README

1. `Cargo.toml` `version`: **MINOR** for a feature, **PATCH** for a fix,
   refactor, docs or tests only (rules in `VERSION.md`).
2. `VERSION.md`: new entry at the **top**, `## X.Y.Z (YYYY-MM-DD)` with
   *Added / Changed / Fixed* naming the feature IDs, and update
   `Current version: **X.Y.Z**`.
3. `README.md`: `**Version:** X.Y.Z`, the feature count ("N of M tracked
   features"), and the Features or Keys table if the change is
   user-visible. A new key must also be in `cli::USAGE` (tested).
4. `documentation/project_plan.md`: update the item's status.
5. `example_vault/`: add what's needed to try the feature by hand.

## 6. Gate

```bash
scripts/check.sh    # fmt --check, clippy -D warnings, cargo test
```
It must end with `OK`. Report the result to the user, including any
failure, by name.

## Batching

Several small features in one session: do steps 1–4 **per feature**, one
version bump per feature, each with its own VERSION.md entry.
