use crate::contacts::initial_sync;
use crate::paths::ACCOUNTS_DIR;
use crate::{Config, open_store, paths};
use anyhow::{Error, Result, anyhow, bail};
use presage::Manager;
use presage::manager::Registered;
use presage_store_sqlite::SqliteStore;
use std::io::Write;
use std::path::Path;
use std::{fs, io};
use tracing::error;

pub async fn link_account_cli(account_name: String, device_name: String) -> Result<()> {
    use futures::{channel::oneshot, future};
    use presage::libsignal_service::configuration::SignalServers;
    use qr2term;

    println!("Creating account '{account_name}'...");

    let accounts = list_accounts()?;
    if accounts.contains(&account_name) {
        bail!("Account '{}' already exists", account_name);
    }
    ensure_accounts_dir()?;

    let account_dir = format!("{ACCOUNTS_DIR}/{account_name}");
    let store_path = get_account_store_path(&account_name);

    match std::fs::create_dir_all(&account_dir) {
        Ok(_) => {}
        Err(e) => error!("Error: {}", e),
    }

    match std::fs::remove_file(&store_path) {
        Ok(_) => {}
        Err(e) => error!("Error: {}", e),
    }
    let store = open_store(&store_path).await?;

    let (tx, rx) = oneshot::channel();
    let name = account_name.clone();
    let (manager_result, _err) = future::join(
        Manager::link_secondary_device(store, SignalServers::Production, device_name, tx),
        async move {
            match rx.await {
                Ok(url) => {
                    println!("Scan the QR code to link the device to account '{name}'!");
                    qr2term::print_qr(url.as_ref()).expect("QR generation failed");
                    println!("You can also use the URL: {url}");
                }
                Err(err) => println!("Error while linking device: {err}"),
            }
        },
    )
    .await;

    let mut manager = match manager_result {
        Ok(manager) => {
            println!("Device linked successfully! Syncing contacts...");
            manager
        }
        Err(e) => bail!("Error while linking device: {e}"),
    };

    initial_sync(&mut manager).await?;
    let mut config = Config::load();
    config.set_current_account(account_name.clone());
    config
        .save()
        .map_err(|e| anyhow!("Failed to save config: {e}"))?;

    println!("Account '{account_name}' created and set as current!");
    Ok(())
}

pub async fn list_accounts_cli() -> Result<()> {
    let accounts = list_accounts()?;
    let config = Config::load();
    let current = config.get_current_account();

    if accounts.is_empty() {
        println!("No accounts found.");
        return Ok(());
    }

    println!("Available accounts:");
    for account in accounts {
        let marker = match (current, &account) {
            (Some(curr), acc) if curr == acc => " (current)",
            _ => "",
        };
        println!("  - {account}{marker}");
    }
    Ok(())
}

pub async fn switch_account_cli(account_name: String) -> Result<()> {
    let accounts = list_accounts()?;
    if !accounts.contains(&account_name) {
        bail!("Account '{}' does not exist", account_name);
    }

    let mut config = Config::load();
    config.set_current_account(account_name.clone());
    config
        .save()
        .map_err(|e| anyhow!("Failed to save config: {e}"))?;

    println!("Switched to account '{account_name}'");
    Ok(())
}

pub async fn get_current_account_cli() -> Result<()> {
    let config = Config::load();
    match config.get_current_account() {
        Some(account) => println!("Current account: {account}"),
        None => println!("No current account set"),
    }
    Ok(())
}

pub async fn unlink_account_cli(account_name: String) -> Result<()> {
    let accounts = list_accounts()?;
    if !accounts.contains(&account_name) {
        bail!("Account '{}' does not exist", account_name);
    }

    let config = Config::load();
    let is_current = config.get_current_account() == Some(&account_name);

    loop {
        print!(
            "Are you sure you want to delete account '{}'? This action cannot be undone! [y/N]: ",
            account_name
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "y" => break,
            "n" | "" => {
                println!("Deletion cancelled.");
                return Ok(());
            }
            _ => println!("Invalid input. Please enter 'y' or 'n'."),
        }
    }

    let account_dir = format!("{ACCOUNTS_DIR}/{account_name}");
    if Path::new(&account_dir).exists() {
        std::fs::remove_dir_all(&account_dir)?;
    }

    if is_current {
        let mut config = Config::load();
        config.clear_current_account();

        let remaining_accounts = list_accounts()?;
        if !remaining_accounts.is_empty() {
            config.set_current_account(remaining_accounts[0].clone());
            println!(
                "Set '{}' as the new current account.",
                remaining_accounts[0]
            );
        }

        config
            .save()
            .map_err(|e| anyhow!("Failed to save config: {e}"))?;
    }

    println!("Account '{account_name}' deleted successfully.");
    Ok(())
}

/// Creates new manager in registered state
pub async fn create_registered_manager() -> Result<Manager<SqliteStore, Registered>> {
    let config = Config::load();

    match config.get_current_account() {
        Some(account_name) => create_registered_manager_for_account(account_name).await,
        None => {
            let store = open_store(paths::STORE).await?;
            match Manager::load_registered(store).await {
                Ok(manager) => Ok(manager),
                Err(err) => Err(Error::new(err)),
            }
        }
    }
}

pub fn get_account_store_path(account_name: &str) -> String {
    format!("{ACCOUNTS_DIR}/{account_name}/store.db")
}

pub fn ensure_accounts_dir() -> Result<()> {
    if !Path::new(ACCOUNTS_DIR).exists() {
        fs::create_dir_all(ACCOUNTS_DIR)?;
    }
    Ok(())
}

pub fn list_accounts() -> Result<Vec<String>> {
    ensure_accounts_dir()?;
    let mut accounts = Vec::new();

    if let Ok(entries) = fs::read_dir(ACCOUNTS_DIR) {
        for entry in entries {
            if let Ok(entry) = entry
                && entry.path().is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                let store_path = get_account_store_path(name);
                if Path::new(&store_path).exists() {
                    accounts.push(name.to_string());
                }
            }
        }
    }

    Ok(accounts)
}

/// Creates new manager for specific account
pub async fn create_registered_manager_for_account(
    account_name: &str,
) -> Result<Manager<SqliteStore, Registered>> {
    let store_path = get_account_store_path(account_name);
    let store = open_store(&store_path).await?;

    match Manager::load_registered(store).await {
        Ok(manager) => Ok(manager),
        Err(err) => Err(Error::new(err)),
    }
}

pub async fn cleanup_invalid_accounts() -> Result<Vec<String>> {
    let mut invalid_accounts = Vec::new();
    let accounts = list_accounts()?;

    for account_name in accounts {
        let store_path = get_account_store_path(&account_name);
        match open_store(&store_path).await {
            Ok(store) => match Manager::load_registered(store).await {
                Ok(_) => {}
                Err(_) => {
                    invalid_accounts.push(account_name.clone());
                    let account_dir = format!("{ACCOUNTS_DIR}/{account_name}");
                    if Path::new(&account_dir).exists() {
                        let _ = std::fs::remove_dir_all(&account_dir);
                    }
                }
            },
            Err(_) => {
                invalid_accounts.push(account_name.clone());
                let account_dir = format!("{ACCOUNTS_DIR}/{account_name}");
                if Path::new(&account_dir).exists() {
                    let _ = std::fs::remove_dir_all(&account_dir);
                }
            }
        }
    }

    Ok(invalid_accounts)
}
