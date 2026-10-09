//! blackglass from the command line: the options, then the terminal (its
//! loop is here) or the window ([`crate::gui`]). The programs are thin:
//! `blackglass` passes its arguments, and on Windows `blackglass-window`
//! (no console) asks for the window.

use std::io;
use std::time::Duration;

use mdedit::terminal::Capabilities;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind, KeyboardEnhancementFlags, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;

use crate::cli::{self, Front};
use crate::startup;
use crate::ui;
use crate::vault_picker::Picked;
use crate::workspace::{Action, App};

/// Runs blackglass with these arguments (without the program's name).
pub fn main(args: Vec<String>) -> io::Result<()> {
    let cli = match cli::parse(args) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("blackglass: {e}");
            std::process::exit(2);
        }
    };
    if cli.help {
        print!("{}", cli::USAGE);
        return Ok(());
    }
    if cli.version {
        println!("blackglass {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let tty = std::io::IsTerminal::is_terminal(&io::stdin())
        && std::io::IsTerminal::is_terminal(&io::stdout());
    let front = match cli.front(tty, cfg!(feature = "gui")) {
        Ok(front) => front,
        Err(e) => {
            eprintln!("blackglass: {e}");
            std::process::exit(2);
        }
    };
    // The example vault: written out (once), then opened at its start.
    let mut path = cli.path.clone();
    if cli.example {
        let folder = cli
            .path
            .clone()
            .unwrap_or_else(crate::example::default_folder);
        match crate::example::install(&folder) {
            Ok(welcome) => path = Some(welcome),
            Err(e) => {
                eprintln!("blackglass: {e}");
                std::process::exit(1);
            }
        }
    }
    match front {
        Front::Terminal => run_with(path, cli.mouse),
        #[cfg(feature = "gui")]
        Front::Window => crate::gui::run(path).or_else(|e| {
            eprintln!("blackglass: cannot open a window: {e} (--terminal runs in the terminal)");
            std::process::exit(1)
        }),
        #[cfg(not(feature = "gui"))]
        Front::Window => unreachable!("the window is asked for only when it's built in"),
    }
}

/// Opens `path` (a vault or a note; `None`: choose a vault first) and runs
/// in the terminal. The very first run without a folder opens the example
/// vault; a folder that can't be opened goes back to choosing one, saying
/// why.
fn run_with(path: Option<std::path::PathBuf>, mouse: bool) -> io::Result<()> {
    let (mut path, mut problem) = startup::first(path);
    let (vault, note) = loop {
        // Without a folder: choose a vault (recent ones, or any folder).
        let chosen = match path.take() {
            Some(path) => path,
            None => match choose_vault(problem.take())? {
                Some(path) => path,
                None => return Ok(()),
            },
        };
        match startup::open(&chosen) {
            Ok(opened) => break opened,
            Err(e) => problem = Some(e),
        }
    };
    let mut app = startup::app(vault, note);
    app.shared.caps = Capabilities::from_env();

    let mut terminal = ratatui::init();
    // Pasted text arrives as one event instead of keystrokes, so newlines
    // don't continue lists and tabs don't indent.
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    if mouse {
        let _ = execute!(io::stdout(), EnableMouseCapture);
    }
    // Lets Ctrl+Shift+S (Save As) and Ctrl+Shift+F be told apart, in
    // terminals that support the kitty keyboard protocol.
    let enhanced = supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok();
    // How images are drawn: asks the terminal, so after raw mode.
    app.shared.picker = mdedit::images::picker_for(app.shared.config.images);
    // A panic must not leave the terminal in these modes (ratatui's own
    // hook restores the rest).
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        leave_modes(enhanced, mouse);
        hook(info);
    }));
    let result = run(&mut terminal, &mut app);
    leave_modes(enhanced, mouse);
    ratatui::restore();
    result
}

/// The vault picker on its own screen, before blackglass opens a vault;
/// `None` if it was left with Esc. `problem` (why the last folder couldn't
/// be opened) is shown in it.
fn choose_vault(problem: Option<String>) -> io::Result<Option<std::path::PathBuf>> {
    let (mut picker, palette) = startup::picker(problem);
    let caps = Capabilities::from_env();
    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    // The terminal is restored however the loop ends (an I/O error too).
    let chosen = (|| -> io::Result<Option<std::path::PathBuf>> {
        loop {
            terminal.draw(|f| ui::draw_launcher(f, &picker, caps, palette))?;
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match picker.key(key) {
                    Picked::Open(path) => return Ok(Some(path)),
                    Picked::Cancel => return Ok(None),
                    Picked::None => {}
                },
                Event::Paste(text) => picker.paste(&text),
                _ => {}
            }
        }
    })();
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    chosen
}

/// Turns off the terminal modes `main` turned on.
fn leave_modes(enhanced: bool, mouse: bool) {
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    if mouse {
        let _ = execute!(io::stdout(), DisableMouseCapture);
    }
    if enhanced {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut draw = true;
    loop {
        if draw {
            terminal.draw(|f| ui::draw(f, app))?;
        }
        // Idle: the plugins' timers and background work every half second;
        // drawn again only when something happened.
        while !event::poll(Duration::from_millis(500))? {
            if app.tick() {
                if app.quit_requested {
                    return Ok(());
                }
                terminal.draw(|f| ui::draw(f, app))?;
            }
        }
        // Handle every event that's already waiting before drawing again, so
        // a held key or a fast typist never waits for frames.
        let mut next = event::read()?;
        // Mouse moves draw again only when they change something.
        draw = false;
        loop {
            draw |= !matches!(&next, Event::Mouse(m) if m.kind == MouseEventKind::Moved);
            let action = match next {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Moved => {
                    draw |= app.mouse_moved(mouse);
                    Action::Continue
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Paste(text) => {
                    app.handle_paste(&text);
                    Action::Continue
                }
                _ => Action::Continue,
            };
            if action == Action::Quit || app.quit_requested {
                return Ok(());
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
            next = event::read()?;
        }
    }
}
