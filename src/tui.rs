use anyhow::Result;
use std::io;
use std::sync::mpsc;
use std::thread;

use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};

use crate::{app, app::App};

pub fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let (tx, rx) = mpsc::channel::<app::EventApp>();

    let tx_key_events = tx.clone();
    thread::spawn(move || app::handle_key_input_events(tx_key_events));

    let tx_contacts_events = tx.clone();
    thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            app::handle_contacts(tx_contacts_events).await;
        });
    });

    let res = app.run(&mut terminal, rx);

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
