//! Command-line options: `blackglass [OPTIONS] [FOLDER | NOTE]`.

use std::path::PathBuf;

/// The help text for `--help`.
pub const USAGE: &str = "\
blackglass: a note vault in the terminal, or in a window

Usage: blackglass [OPTIONS] [FOLDER | NOTE]
       blackglass --example [FOLDER]

  FOLDER          the vault to open (without one, blackglass asks: a
                  recent vault, or any folder, made if it's new; the
                  very first time, it opens the example vault;
                  'blackglass .' opens the current folder)
  NOTE            a note to open, in the nearest folder above it with
                  .blackglass/ (else in its own folder)

Options:
  --example       write the example vault (a tour of every feature) to
                  FOLDER, or ~/blackglass-example, and open it; a copy
                  already there is opened as it is
  --no-mouse      leave the mouse to the terminal (select text as usual)
  --gui           open in a window of its own (the default when started
                  from a launcher, not a terminal)
  --terminal      run in the terminal (the default in a terminal)
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
~/.config/blackglass/; %APPDATA%\\blackglass\\ on Windows): config.toml
has the editor's settings, which are
mdedit's (until it exists, mdedit's config.toml is read; see `mdedit
--help`), and keys.toml the changed shortcuts (new-note = \"Alt+N\").
";

/// What the command line asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cli {
    pub path: Option<PathBuf>,
    /// `--example`: write the example vault (to `path`, if given).
    pub example: bool,
    pub mouse: bool,
    pub help: bool,
    pub version: bool,
    /// `--gui` / `--terminal`: which front end (`None`: by where it runs).
    pub asked: Option<Front>,
}

/// Where blackglass runs: in the terminal, or in a window of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Front {
    Terminal,
    Window,
}

impl Cli {
    /// The front end: as asked, else the terminal when it was started in
    /// one (`tty`) and the window when it wasn't (a launcher); the window
    /// only if it's built in (`built`).
    pub fn front(&self, tty: bool, built: bool) -> Result<Front, String> {
        match self.asked {
            Some(Front::Window) if !built => {
                Err("this blackglass is built without the window (the gui feature)".into())
            }
            Some(front) => Ok(front),
            None if tty || !built => Ok(Front::Terminal),
            None => Ok(Front::Window),
        }
    }
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
            "--example" => cli.example = true,
            "--gui" | "--terminal" => {
                let front = if arg == "--gui" {
                    Front::Window
                } else {
                    Front::Terminal
                };
                if cli.asked.is_some_and(|f| f != front) {
                    return Err("--gui or --terminal, not both".into());
                }
                cli.asked = Some(front);
            }
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
        let cli = args(&["--example", "tour"]).unwrap();
        assert!(cli.example);
        assert_eq!(cli.path, Some(PathBuf::from("tour")));
    }

    #[test]
    fn the_window_or_the_terminal() {
        use Front::{Terminal, Window};
        let front = |list: &[&str], tty: bool, built: bool| args(list).unwrap().front(tty, built);
        // Without an option: a terminal keeps it, else the window.
        assert_eq!(front(&[], true, true), Ok(Terminal));
        assert_eq!(front(&[], false, true), Ok(Window));
        assert_eq!(front(&[], false, false), Ok(Terminal), "no window built in");
        assert_eq!(front(&["--gui"], true, true), Ok(Window));
        assert_eq!(front(&["--terminal"], false, true), Ok(Terminal));
        assert!(
            front(&["--gui"], true, false).is_err(),
            "asked, not built in"
        );
        assert!(args(&["--gui", "--terminal"]).is_err());
    }

    #[test]
    fn help_fits_in_80_columns() {
        for line in USAGE.lines() {
            assert!(line.chars().count() <= 80, "too wide: {line:?}");
        }
    }
}
