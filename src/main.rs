//! blackglass: an Obsidian-style note vault in the terminal.
//!
//! Usage: blackglass [FOLDER | NOTE]

use std::io;
use std::time::Duration;

use mdedit::emoji::{self, Recent};
use mdedit::terminal::Capabilities;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind, KeyboardEnhancementFlags, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;

use blackglass::cli;
use blackglass::config;
use blackglass::ui;
use blackglass::vault::Vault;
use blackglass::vault_picker::{Picked, VaultPicker};
use blackglass::workspace::{Action, App};

fn main() -> io::Result<()> {
    // `args_os`: a non-UTF-8 argument must not panic (it's converted lossily).
    let args = std::env::args_os().skip(1).map(|a| {
        a.into_string()
            .unwrap_or_else(|a| a.to_string_lossy().into_owned())
    });
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
    // The example vault: written out (once), then opened at its start.
    if cli.example {
        let folder = cli
            .path
            .clone()
            .unwrap_or_else(blackglass::example::default_folder);
        match blackglass::example::install(&folder) {
            Ok(welcome) => {
                return run_with(Some(welcome), cli.mouse);
            }
            Err(e) => {
                eprintln!("blackglass: {e}");
                std::process::exit(1);
            }
        }
    }
    run_with(cli.path, cli.mouse)
}

/// Opens `path` (a vault or a note; `None`: choose a vault first) and runs.
fn run_with(path: Option<std::path::PathBuf>, mouse: bool) -> io::Result<()> {
    // Without a folder: choose a vault first (recent ones, or any folder).
    let path = match path {
        Some(path) => path,
        None => match choose_vault()? {
            Some(path) => path,
            None => return Ok(()),
        },
    };
    let (folder, note) = config::vault_and_note(Some(&path));
    let vault = match Vault::open(&folder) {
        Ok(vault) => vault,
        Err(e) => {
            eprintln!("blackglass: cannot open {}: {e}", folder.display());
            std::process::exit(1);
        }
    };

    let mut app = App::new(vault);
    app.shared.caps = Capabilities::from_env();
    // Recent emoji are shared with mdedit.
    if let Some(recent) = emoji::default_recent_path() {
        app.shared.recent = Recent::load(recent);
    }
    // The editor settings (blackglass's config.toml, else mdedit's) and
    // the keyboard shortcuts, from the user's config folder.
    if let Some(path) = config::settings_path() {
        match std::fs::read_to_string(&path) {
            Ok(text) => app.message = app.use_editor_config(&text).join("; "),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => app.message = format!("Cannot read {}: {e}", path.display()),
        }
    }
    app.config_dir = config::config_dir();
    app.load_user_config();
    app.remember_vault();
    if let Some(note) = note
        && let Ok(note) = note.canonicalize()
    {
        app.open(&note);
        app.sidebar.reveal(&note, &app.vault);
    }

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
/// `None` if it was left with Esc.
fn choose_vault() -> io::Result<Option<std::path::PathBuf>> {
    let recent = config::config_dir()
        .map(|dir| config::recent_vaults(&dir))
        .unwrap_or_default();
    let mut picker = VaultPicker::new(recent, config::home());
    let caps = Capabilities::from_env();
    let dir = config::config_dir();
    let palette = dir
        .as_deref()
        .and_then(|d| ui::theme::saved(&d.join(blackglass::workspace::APPEARANCE_FILE)))
        .and_then(|id| ui::theme::load(dir.as_deref(), &id))
        .map(|(palette, _)| palette)
        .unwrap_or_default();
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
