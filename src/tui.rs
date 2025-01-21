use anyhow::Result;
use std::io;

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};

use crate::{
    app::{App, CurrentScreen},
    ui::ui,
};

pub fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Release {
                continue;
            }
            match app.current_screen {
                CurrentScreen::Main => match key.code {
                    KeyCode::Right | KeyCode::Char('d') => {
                        app.current_screen = CurrentScreen::Writing;
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.current_screen = CurrentScreen::Exiting;
                    }
                    KeyCode::Char('e') => {
                        app.current_screen = CurrentScreen::Options;
                    }
                    KeyCode::Down | KeyCode::Char('s') => {
                        if app.selected < app.contacts.len() - 1 {
                            app.selected += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('w') => {
                        if app.selected > 0 {
                            app.selected -= 1;
                        }
                    }
                    _ => {}
                },
                CurrentScreen::Exiting => match key.code {
                    KeyCode::Char('y') | KeyCode::Esc | KeyCode::Char('q') => {
                        return Ok(true);
                    }
                    KeyCode::Char('n') => {
                        app.current_screen = CurrentScreen::Main;
                    }
                    _ => {}
                },
                CurrentScreen::Writing => match key.code {
                    KeyCode::Esc | KeyCode::Left | KeyCode::Char('a') => {
                        app.current_screen = CurrentScreen::Main;
                    }
                    _ => {}
                },
                CurrentScreen::Options => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.current_screen = CurrentScreen::Main;
                    }
                    _ => {}
                },
            }
        }
    }
}
