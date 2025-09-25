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

use crate::app::CurrentScreen::AccountSelector;
use crate::config::Config;
use crate::{
    app::{App, LinkingStatus},
    create_registered_manager_for_account,
    devices::is_registered,
    list_accounts,
};

pub async fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let accounts = list_accounts()?;
    let config = Config::load();

    let linking_status = if !accounts.is_empty() {
        if let Some(current) = config.get_current_account() {
            if accounts.contains(&current.to_string()) {
                match create_registered_manager_for_account(current).await {
                    Ok(_) => LinkingStatus::Linked,
                    Err(_) => LinkingStatus::Unlinked,
                }
            } else {
                let mut config = Config::load();
                config.clear_current_account();
                let _ = config.save();
                LinkingStatus::Unlinked
            }
        } else if accounts.len() == 1 {
            let mut config = Config::load();
            config.set_current_account(accounts[0].clone());
            let _ = config.save();
            LinkingStatus::Linked
        } else {
            LinkingStatus::Unlinked
        }
    } else {
        match is_registered().await? {
            true => LinkingStatus::Linked,
            false => LinkingStatus::Unlinked,
        }
    };

    let mut app = App::new(linking_status);

    if !accounts.is_empty() && config.get_current_account().is_none() && accounts.len() > 1 {
        app.current_screen = AccountSelector;
    }

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
