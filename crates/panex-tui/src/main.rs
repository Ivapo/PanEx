mod app;
mod input;
mod layout;
mod sort;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

fn print_help() {
    println!(
        "panex {version}
A multi-pane terminal file explorer.

USAGE:
    panex [OPTIONS]

OPTIONS:
    -h, -H, --help       Print this help
    -V, -v, --version    Print version

CONFIG:
    ~/.panex/config.toml — favorites, and per-extension openers
    under [open.tui], e.g. \".md\" = \"nvim\".

Opens in the current directory. Press ? inside the app for keyboard shortcuts.",
        version = env!("CARGO_PKG_VERSION"),
    );
}

/// Handles `--help`/`--version` and rejects anything else. The app takes no
/// arguments, so whatever comes first is terminal either way — every branch
/// exits the process. Must run before the terminal is put into raw mode.
fn parse_args() {
    let Some(arg) = std::env::args().nth(1) else {
        return;
    };
    match arg.as_str() {
        "-h" | "-H" | "--help" => {
            print_help();
            std::process::exit(0);
        }
        "-V" | "-v" | "--version" => {
            println!("panex {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        other => {
            eprintln!("panex: unrecognized argument '{other}'");
            eprintln!("Try 'panex --help' for usage.");
            std::process::exit(2);
        }
    }
}

/// Put the terminal back the way we found it. Safe to call more than once.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    let _ = execute!(io::stdout(), crossterm::cursor::Show);
}

/// Without this, a panic unwinds straight past the cleanup at the end of
/// `main`, leaving the user on the alternate screen in raw mode — the shell
/// looks dead and the panic message is never seen.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    parse_args();
    install_panic_hook();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = match app::App::new() {
        Ok(a) => a,
        Err(e) => {
            // Restore terminal before printing error
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), DisableMouseCapture, LeaveAlternateScreen)?;
            terminal.show_cursor()?;
            eprintln!("Failed to initialize: {}", e);
            std::process::exit(1);
        }
    };

    // Event loop — render on demand only. Idle = no CPU.
    let status_ttl = Duration::from_secs(3);
    let idle_timeout = Duration::from_secs(60);
    terminal.draw(|frame| ui::draw(frame, &mut app))?;

    loop {
        // Wake either for the next event or when the status message is due to expire.
        let timeout = match app.status_message_at {
            Some(at) => status_ttl.saturating_sub(at.elapsed()),
            None => idle_timeout,
        };

        let mut dirty = false;

        if event::poll(timeout)? {
            // Drain every pending event before redrawing so bursts
            // (e.g. trackpad scrolling) coalesce into a single frame.
            loop {
                match event::read()? {
                    Event::Key(key) => {
                        input::handle_key_event(&mut app, key);
                        dirty = true;
                    }
                    Event::Mouse(mouse) => {
                        if input::handle_mouse_event(&mut app, mouse) {
                            dirty = true;
                        }
                    }
                    Event::Resize(_, _) => dirty = true,
                    _ => {}
                }
                if app.should_quit || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        // Auto-clear status message after 3 seconds
        if let Some(at) = app.status_message_at {
            if at.elapsed() >= status_ttl {
                app.status_message = None;
                app.status_message_at = None;
                dirty = true;
            }
        }

        if app.should_quit {
            break;
        }

        if dirty {
            terminal.draw(|frame| ui::draw(frame, &mut app))?;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableMouseCapture, LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
