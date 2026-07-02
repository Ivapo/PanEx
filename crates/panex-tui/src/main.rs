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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
