use std::io;

use anyhow::Result;
// use std::thread;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};

use crate::config::Config;
use crate::{
    app::{App, LinkingStatus},
    create_registered_manager_for_account,
    list_accounts,
};
use crate::app::CurrentScreen;

pub async fn run_tui() -> Result<()> {
    if let Ok(removed_accounts) = crate::cleanup_invalid_accounts().await {
        if !removed_accounts.is_empty() {
            println!("Removed {} invalid account(s): {:?}", removed_accounts.len(), removed_accounts);
        }
    }
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    // Check for existing accounts
    let accounts = list_accounts()?;
    let config = Config::load();

    let (linking_status, initial_screen) = if accounts.is_empty() {
        // No accounts exist - show account creation screen
        (LinkingStatus::Unlinked, CurrentScreen::CreatingAccount)
    } else {
        // We have accounts, determine linking status
        let status = if let Some(current) = config.get_current_account() {
            if accounts.contains(&current.to_string()) {
                match create_registered_manager_for_account(current).await {
                    Ok(_) => LinkingStatus::Linked,
                    Err(_) => LinkingStatus::Unlinked,
                }
            } else {
                // Current account doesn't exist, clear it
                let mut config = Config::load();
                config.clear_current_account();
                let _ = config.save();
                LinkingStatus::Unlinked
            }
        } else if accounts.len() == 1 {
            // Auto-select single account
            let mut config = Config::load();
            config.set_current_account(accounts[0].clone());
            let _ = config.save();
            LinkingStatus::Linked
        } else {
            LinkingStatus::Unlinked
        };

        // Determine initial screen based on accounts
        let screen = if accounts.len() > 1 && config.get_current_account().is_none() {
            CurrentScreen::AccountSelector
        } else if status == LinkingStatus::Linked {
            CurrentScreen::Main
        } else {
            CurrentScreen::AccountSelector
        };

        (status, screen)
    };

    let mut app = App::new(linking_status);
    app.current_screen = initial_screen;

    let res = app.run(&mut terminal).await;

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
