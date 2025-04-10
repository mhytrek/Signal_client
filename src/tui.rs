use anyhow::Result;
use std::sync::mpsc;
use std::{io, thread};
use tokio::runtime::Builder;
// use std::thread;
use crate::AsyncRegisteredManager;
use crate::{
    app::{self, App},
    create_registered_manager,
};
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
use tokio::sync::RwLock;

pub async fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;
    let manager: AsyncRegisteredManager = Arc::new(RwLock::new(create_registered_manager().await?));

    let mut app = App::new();

    let (tx_thread, rx_tui) = mpsc::channel();

    let (tx_tui, rx_thread) = mpsc::channel();

    let tx_key_events = tx_thread.clone();
    thread::spawn(move || {
        app::handle_key_input_events(tx_key_events);
    });

    let tx_contacts_events = tx_thread.clone();
    let new_manager = Arc::clone(&manager);
    // thread::spawn(move || {
    thread::Builder::new()
        .name(String::from("contacts_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("contacts_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                app::handle_contacts(tx_contacts_events, new_manager).await;
            })
        })
        .unwrap();

    let new_manager = Arc::clone(&manager);
    // thread::spawn(move || {
    thread::Builder::new()
        .name(String::from("sending_thread"))
        .stack_size(1024 * 1024 * 8)
        .spawn(move || {
            let runtime = Builder::new_multi_thread()
                .thread_name("sending_runtime")
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                app::handle_sending_messages(rx_thread, new_manager).await;
            })
            // let x = rx_thread;
        })
        .unwrap();

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
