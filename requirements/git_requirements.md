# Git: what blackglass needs

blackglass's Git plugin (id `git`, stage 1 built in 0.35.0 as W-93) against the
[Git](https://github.com/Vinzent03/obsidian-git) community plugin (its
README, [documentation](https://publish.obsidian.md/git-doc) and source:
`commands.ts`, the default settings in `constants.ts`, read 2026-09-30).
Each piece has an ID (`GT-xx`), a rough effort (S / M / L) and notes.

**Now:** ✅ done · 🟡 partial · ⬜ missing · ✗ not planned (with the reason).

The plugin backs the vault up to a Git repository and keeps it in sync:
commits on a timer, pulls and pushes, and shows what changed.

It follows the plugin rules: code in `src/plugins/git/`, settings in
`.blackglass/plugins/git/settings.toml`. **How it talks to Git:** it runs
the `git` program (`std::process::Command`), like the original's desktop
version, so the user's Git setup (config, SSH keys, credential helpers,
hooks) just works; no Git library is linked. Git runs in the background
(a thread), never blocking typing, and its output comes back as status
messages. Without `git` on the `PATH`, the plugin says so and does
nothing.

## 1. The repository

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| GT-01 | Find the repository: the vault's folder or one above it (a vault inside a bigger repo); `base_path` and a custom `.git` folder in the settings | ✅ | S | 0.63.0: `base_path` (the repository's folder in the vault); a custom `.git` folder through `GIT_DIR` in the environment; 0.35.0: runs in the vault's folder, so a repository above it works |
| GT-02 | "Initialize a new repo" (with a `.gitignore` for `.blackglass/` state the user chooses to leave out) | ✅ | S | 0.63.0: asks whether to leave out `.blackglass/`; 0.35.0: `git init` |
| GT-03 | "Clone an existing remote repo" into a folder (asks the URL and the folder) | ✅ | M | 0.42.0: then opened as the vault |
| GT-04 | "Edit .gitignore" (opens it in a tab), "Add file to .gitignore" | ✅ | S | 0.38.0 |
| GT-05 | Remotes: "Edit remotes" (add or change a URL), "Remove remote", "Set upstream branch" | ✅ | S | 0.38.0: "Edit remotes" adds or changes one; "Remove remote"; the upstream is set by the first push |
| GT-06 | Branches: "Switch branch", "Switch to remote branch", "Create new branch", "Delete branch" | ✅ | S | 0.63.0: "Switch to remote branch" (a local branch following it); 0.38.0: switch, create, delete |
| GT-07 | "Delete repository" (the `.git` folder, after asking twice) | ✅ | S | 0.63.0: asks twice |
| GT-08 | Submodules: update them on commit-and-sync and pull (recursive, a setting) | ✅ | M | 0.42.0: a command, and a setting to update them after every pull |

## 2. Commit and sync

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| GT-10 | "Commit-and-sync": stage everything, commit, pull, push | ✅ | M | 0.35.0: commit (`add -A`), pull when there's an upstream, push (the first with `-u`) |
| GT-11 | Commit message template, default `vault backup: {{date}}`: `{{date}}` (with a moment.js format setting), `{{hostname}}`, `{{numFiles}}`, `{{files}}`; separate templates for manual and automatic commits; list the changed files in the message body (a setting) | ✅ | S | 0.61.0: a separate automatic template; the files in the body (a setting); 0.35.0: `{{date}}` (date format setting), `{{hostname}}`, `{{numFiles}}`, `{{files}}` |
| GT-12 | Commits: "Commit all changes", "Commit" (staged if anything is staged, else all), "Commit staged", each also "with specific message" (asks it); "Amend staged" | ✅ | S | 0.61.0: Commit (staged or all), Commit staged, each with a specific message, Amend staged; 0.35.0: "Commit all changes" |
| GT-13 | "Pull", "Push", "Fetch"; pull before push (a setting, on); "Disable push" | ✅ | S | 0.62.0: Fetch; pull before push (a setting); the Push setting disables pushing; 0.35.0: Pull, Push, a Push setting |
| GT-14 | Sync method for pull: merge, rebase (auto-stash on) or reset; merge strategy | ✅ | S | 0.62.0: merge, rebase or reset; a merge strategy (stop, ours, theirs); 0.35.0: merge or rebase (auto-stash) |
| GT-15 | Merge conflicts: listed, opened in tabs with the conflict markers, a message saying what to do; nothing is pushed until they're resolved | ✅ | M | 0.42.0: the files named, the markers shown in open notes, Abort merge, commit-and-sync finishes the merge |
| GT-16 | Open notes follow a pull: a tab whose file changed reloads (asks if it has unsaved changes); the vault is scanned again | ✅ | M | 0.35.0: the vault is scanned again, open notes without unsaved changes reload |
| GT-17 | "Commit-and-sync and then close" (quit after it) | ✅ | S | 0.61.0: quits (asking about unsaved notes) when it succeeded |
| GT-18 | "Discard all changes" (asks first), "List changed files" | ✅ | S | 0.61.0: "Discard all changes" (asks); 0.37.0: discard one file's changes from the Git tab (asks); the Git tab lists the changed files |
| GT-19 | "Raw command" (type a `git …` command, see its output) | ✅ | S | 0.38.0: the output in a read-only tab |

## 3. Automatic routines

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| GT-20 | Auto commit-and-sync every N minutes (0: off) | ✅ | S | 0.35.0: a timer tick (about twice a second) from the main loop |
| GT-21 | … or N minutes after the last change instead (after a file change) | ✅ | S | 0.62.0: a setting; minutes as decimals too |
| GT-22 | Separate intervals for commit, push and pull | ✅ | S | 0.62.0: pull and push timers of their own |
| GT-23 | Pull on start | ✅ | S | 0.35.0 |
| GT-24 | Auto-commit only staged files; squash commits before push | ✅ | S | 0.62.0: automatic commits of staged files only; 0.67.0: squash before push (a setting) |
| GT-25 | "Pause / resume automatic routines" | ✅ | S | 0.35.0 |
| GT-26 | Messages: errors shown, "nothing to commit" shown or not (settings) | ✅ | S | 0.62.0: "Quiet" leaves out nothing to commit / push; automatic routines' errors say so; 0.35.0: every result and error is a status message |

## 4. Views

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| GT-30 | Source control view (`git status`): the changed files, staged and not; stage / unstage a file or all, discard a file's changes, open a file's diff; commit with a message, push, pull; as a list or a tree (a setting) | ✅ | M | 0.67.0: as a tree (a setting); 0.63.0: `p` pull and `P` push; 0.37.0: the Git tab: changed files with their state, `s` stage / unstage, `d` discard, `c` commit the staged files, Enter opens |
| GT-31 | History view (`git log`): recent commits (message, author, date: settings), each opens to its changed files and their diffs | 🟡 | M | 0.37.0: "Git: History" lists the last 100 commits; the chosen one opens (`git show`) in a read-only tab |
| GT-32 | Diff view: a file's changes against the last commit (or a commit's), unified or side by side (a setting) | ✅ | M | 0.67.0: side by side (a setting: a Before / After table per change); 0.37.0: "Show the diff of this note" (against the last commit), unified |
| GT-33 | "Stage current file", "Unstage current file" | ✅ | S | 0.61.0 |
| GT-34 | Status bar: the branch, when the last commit was, ahead / behind, the number of changed files (settings) | ✅ | S | 0.63.0: the last commit's time, a setting to hide it; 0.36.0: the branch, changed files, commits to push |
| GT-35 | Refresh the views on a timer (a setting) and after every Git action | ✅ | S | 0.63.0: the refresh interval is a setting; 0.37.0: the status and the Git tab after every Git command and every 30 seconds, and soon after a save |

## 5. In the editor

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| GT-40 | Change signs in the note's margin: added, changed, deleted lines against the last commit | ✅ | M | 0.39.0: `+` / `~` / `-` in a margin (mdedit 3.4.0), when the note changes or is saved; a setting |
| GT-41 | Hunks: go to the next / previous change, preview it, stage it, reset it | ✅ | M | 0.39.0: next / previous; 0.42.0: preview, stage and reset the change at the cursor |
| GT-42 | Line authoring (`git blame`): each line's author and date beside it, colored by age (settings: what to show, the date format, colors); "Toggle line author information" | ✅ | M | 0.63.0: colored by age; what it shows is a setting; 0.39.0: author and date per line (`git blame`), a toggle |

## 6. Not planned

| ID | Feature | Now | Effort | Notes |
|----|---------|-----|--------|-------|
| GT-50 | "Open file on GitHub", "Open file history on GitHub" | ✅ | S | 0.63.0: GitHub remotes (SSH or HTTPS), through the desktop's browser; Through the desktop's opener with the remote's URL |
| GT-51 | Mobile support and its isomorphic-git backend | ✗ | – | No mobile app; the `git` program is used |
| GT-52 | A commit message script (a shell command writes the message) | ✅ | S | 0.61.0: off unless set; Runs a command: off unless set, like upstream |

## 7. Suggested order

1. **Back up** (M): GT-01, GT-02, GT-10 … GT-14, GT-16, GT-20, GT-23,
   GT-25, GT-26, GT-34, and its settings page.
2. **See changes** (M): GT-30 … GT-33, GT-35, GT-18, GT-15.
3. **Repository chores** (S): GT-03 … GT-08, GT-17, GT-19, GT-21, GT-22,
   GT-24.
4. **In the editor** (M, with mdedit): GT-40 … GT-42.

## 8. Checked against

- [Git plugin README](https://github.com/Vinzent03/obsidian-git),
  [documentation](https://publish.obsidian.md/git-doc)
  ([features](https://publish.obsidian.md/git-doc/Features),
  [authentication](https://publish.obsidian.md/git-doc/Authentication)),
  its source: [`commands.ts`](https://github.com/Vinzent03/obsidian-git/blob/master/src/commands.ts)
  (every command) and [`constants.ts`](https://github.com/Vinzent03/obsidian-git/blob/master/src/constants.ts)
  (every setting's default) (2026-09-30).
