//! Command-line options: `blackglass [OPTIONS] [FOLDER | NOTE]`.

use std::path::PathBuf;

/// The help text for `--help`.
pub const USAGE: &str = "\
blackglass: a note vault in the terminal

Usage: blackglass [OPTIONS] [FOLDER | NOTE]

  FOLDER          the vault to open (without one, blackglass asks: a
                  recent vault, or any folder, made if it's new;
                  'blackglass .' opens the current folder)
  NOTE            a note to open, in the nearest folder above it with
                  .blackglass/ (else in its own folder)

Options:
  --no-mouse      leave the mouse to the terminal (select text as usual)
  -h, --help      show this help
  -V, --version   show the version

Workspace keys (the defaults: Settings, Keyboard shortcuts changes them;
the editor's own keys are in `mdedit --help`):
  F1 / Ctrl+H     help, with every command and its keys
  Alt+, / Ctrl+,  settings: editor, keyboard shortcuts, plugins
  Ctrl+P          command palette: every command, including the
                  editor's, 'Insert hyperlink' and 'Open plugins'
  Ctrl+O          go to a note (quick switcher; can create one)
  Ctrl+N          new note, in the folder selected in the file explorer;
                  in its window, Tab picks a template (Templater plugin)
  Ctrl+W          close the tab (Ctrl+X works too)
  Ctrl+X          with text selected: extract it to a new note (the new
                  note window; the text moves there, a link replaces it)
  Alt+Left/Right  previous / next tab (also Ctrl+PgUp / Ctrl+PgDn)
  Alt+1 ... Alt+9 go to tab 1 to 8, or the last tab
  Ctrl+Alt+Left/Right  back / forward through the notes you were on (also
                  the mouse's back / forward buttons, where the terminal
                  reports them: scripts/mouse_probe.py shows)
  Ctrl+B          the next open pane: sidebar, calendar, note, backlinks
  Alt+B           hide / show the sidebar
  Alt+V           the note's mode: live preview, source, view (read-only:
                  Tab to a link or query result, Enter follows, Esc edits)
  Alt+;           edit the note's properties (a add, r rename, Delete
                  remove)
  Alt+O           the note's outline: its headings, Enter jumps
  Alt+P           preview the linked note under the cursor (Enter opens,
                  Esc closes)
  F2              rename the note (the explorer's, or the open one); the
                  links to it are updated everywhere
  Alt+L           show / hide the note's links under it: backlinks,
                  unlinked mentions (l links one), outgoing links; arrows
                  choose, Enter opens
  Alt+D           today's daily note (Periodic Notes)
  Alt+C           show / hide the calendar (Periodic Notes): arrows move,
                  PgUp / PgDn change month, Enter opens the day's note
                  (asks before creating one),
                  w / m the week's / month's, t today, Esc back
  Ctrl+G          search the vault (Ctrl+Shift+F where the terminal has it)
  F5              scan the vault again (after changes made elsewhere)
  Ctrl+Q          quit (asks about unsaved changes)

Typing [[ in a note lists the 5 latest notes, then fuzzy matches as you
type: Up / Down choose, Enter or Tab insert the link, Esc closes the list.

Sidebar keys:
  Tab, Shift+Tab  next / previous panel: Files, Search, Tags
  Up, Down, PgUp, PgDn, Home, End   move
  Enter           open a note, open or close a folder, search for a tag
  Right, Left     open / close a folder (Left on a file: its folder)
  s               sort files: A-Z, Z-A, newest, oldest
  Esc             back to the editor
  In Search: type to search (a capital makes it case-sensitive),
  tag:name finds the notes with a tag, Ctrl+U clears. Several words all
  match; \"a phrase\", -word (not), OR, ( ); file:, path:, line:(a b),
  task:, task-todo:, task-done:, [property], [property:value].

Settings window (Alt+,): a search box and the pages on the left (Options:
Editor, Keyboard shortcuts, Plugins; Plugin options: each plugin's), the
page on the right. Up / Down choose, Enter opens a page or changes a
setting, Left / Right choose a value, typing searches, Esc goes back.
  On Plugins (also 'Open plugins' in Ctrl+P):
  Enter           install / uninstall the plugin
  Space           enable / disable it
  Right           its settings (also 'Plugin: Settings' in Ctrl+P)
  Plugins are saved per vault in .blackglass/plugins.toml. Dataview renders
  ```dataview blocks: LIST, TABLE and TASK queries with FROM, WHERE, SORT
  and LIMIT over frontmatter, key:: value fields, tags, links and tasks;
  ```dataviewjs blocks run JavaScript with Dataview's dv object (in a
  sandbox; a Dataview setting turns them off). Templater runs <%* %>
  JavaScript commands. Recent Files adds a Recent tab to the sidebar: the
  notes visited lately (Enter opens one, Delete takes it off).

Settings (Alt+,) are saved in $XDG_CONFIG_HOME/blackglass/ (or
~/.config/blackglass/): config.toml has the editor's settings, which are
mdedit's (until it exists, mdedit's config.toml is read; see `mdedit
--help`), and keys.toml the changed shortcuts (new-note = \"Alt+N\").
";

/// What the command line asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cli {
    pub path: Option<PathBuf>,
    pub mouse: bool,
    pub help: bool,
    pub version: bool,
}

/// Parses the arguments (without the program name).
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Cli, String> {
    let mut cli = Cli {
        mouse: true,
        ..Cli::default()
    };
    for arg in args {
        match arg.as_str() {
            "--no-mouse" => cli.mouse = false,
            "-h" | "--help" => cli.help = true,
            "-V" | "--version" => cli.version = true,
            s if s.starts_with('-') && s.len() > 1 => {
                return Err(format!("unknown option {s} (try --help)"));
            }
            _ if cli.path.is_some() => return Err("only one folder or note, please".into()),
            _ => cli.path = Some(PathBuf::from(arg)),
        }
    }
    Ok(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Cli, String> {
        parse(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn options_and_a_path() {
        assert_eq!(
            args(&[]),
            Ok(Cli {
                mouse: true,
                ..Cli::default()
            })
        );
        let cli = args(&["--no-mouse", "notes"]).unwrap();
        assert!(!cli.mouse);
        assert_eq!(cli.path, Some(PathBuf::from("notes")));
        assert!(args(&["-h"]).unwrap().help);
        assert!(args(&["--version"]).unwrap().version);
        assert!(args(&["--nope"]).is_err());
        assert!(args(&["a", "b"]).is_err());
    }

    #[test]
    fn help_fits_in_80_columns() {
        for line in USAGE.lines() {
            assert!(line.chars().count() <= 80, "too wide: {line:?}");
        }
    }
}
