# Git

The Git plugin backs a vault up with Git: it commits, pulls and pushes for
you, on a timer if you like, using your own `git` in the background.

It's not on in this vault (this folder is part of blackglass's own
repository). In your vault: **Alt+,** → Plugins → Git, then "Git:
Initialize a new repo" (or "Clone an existing remote repo") and add a
remote with "Git: Edit remotes".

- **"Git: Commit-and-sync"**: commit every change, pull, push. Settings:
  every N minutes, a pull at start, the commit message
  (`vault backup: {{date}}`).
- The status bar shows the branch and what's waiting.
- A **Git** tab in the sidebar lists the changes: **s** stages, **d**
  discards, **c** commits.
- "Show the diff of this note", "History": read-only tabs.
- The open note's changed lines are marked in a margin (`+`, `~`, `-`);
  "Toggle line author information" shows who wrote each line.
- Branches, remotes, `.gitignore`, aborting a merge: each a command
  (Ctrl+P, "Git:").

Notes a pull changed are shown again at once.
