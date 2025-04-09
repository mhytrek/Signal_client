use anyhow::Result;
use std::{io, thread};
use std::sync::mpsc;
// use std::thread;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::sync::Arc;
use tokio::sync::Mutex;
// use tokio::sync::mpsc;
use crate::{app::{self, App}, create_registered_manager};
use crate::AsyncRegisteredManager;


pub async fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;
    let manager: AsyncRegisteredManager = Arc::new(Mutex::new(create_registered_manager().await?));

    let mut app = App::new();

    let (tx_thread, rx_tui) = mpsc::channel();

    let (tx_tui, rx_thread) = mpsc::channel();

    let tx_key_events = tx_thread.clone();
    tokio::spawn(async move {
        app::handle_key_input_events(tx_key_events).await;
    });
    
    let tx_contacts_events = tx_thread.clone();
    // let new_manager = Arc::clone(&manager);
    tokio::spawn(async move {
        // tokio::runtime::Runtime::new().unwrap().block_on(async {
            app::handle_contacts(tx_contacts_events).await;
        // });
        // let x = tx_contacts_events; // doesn't shout when only this is used
    });

    let new_manager = Arc::clone(&manager);
    tokio::spawn(async move {
        // tokio::runtime::Runtime::new().unwrap().block_on(async {
            app::handle_sending_messages(rx_thread, new_manager).await;
        // });
        // let x = rx_thread;
    });

    let res = app.run(&mut terminal, rx_tui, tx_tui).await;

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
