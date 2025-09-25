use crate::{
    ACCOUNTS_DIR, Config, ensure_accounts_dir, get_account_store_path, list_accounts, open_store,
};
use anyhow::{Result, anyhow};
use presage::Manager;
use crate::contacts::initial_sync;

pub async fn create_account_cli(account_name: String, device_name: String) -> Result<()> {
    use futures::{channel::oneshot, future};
    use presage::libsignal_service::configuration::SignalServers;
    use qr2term;

    println!("Creating account '{}'...", account_name);

    let accounts = list_accounts()?;
    if accounts.contains(&account_name) {
        return Err(anyhow::anyhow!("Account '{}' already exists", account_name));
    }
    ensure_accounts_dir()?;

    let account_dir = format!("{}/{}", ACCOUNTS_DIR, account_name);
    let store_path = get_account_store_path(&account_name);

    tokio::fs::create_dir_all(&account_dir).await?;

    let _ = std::fs::remove_file(&store_path);
    let store = open_store(&store_path).await?;

    let (tx, rx) = oneshot::channel();
    let name = account_name.clone();
    let (manager_result, _err) = future::join(
        Manager::link_secondary_device(store, SignalServers::Production, device_name, tx),
        async move {
            match rx.await {
                Ok(url) => {
                    println!("Scan the QR code to link the device to account '{}'!", name);
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
        Err(e) => return Err(anyhow!("Error while linking device: {e}")),
    };

    initial_sync(&mut manager).await?;
    let mut config = Config::load();
    config.set_current_account(account_name.clone());
    config
        .save()
        .map_err(|e| anyhow::anyhow!("Failed to save config: {}", e))?;

    println!("Account '{}' created and set as current!", account_name);
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
        let marker = if current.map_or(false, |c| c == &account) {
            " (current)"
        } else {
            ""
        };
        println!("  - {}{}", account, marker);
    }
    Ok(())
}

pub async fn switch_account_cli(account_name: String) -> Result<()> {
    let accounts = list_accounts()?;
    if !accounts.contains(&account_name) {
        return Err(anyhow::anyhow!("Account '{}' does not exist", account_name));
    }

    let mut config = Config::load();
    config.set_current_account(account_name.clone());
    config
        .save()
        .map_err(|e| anyhow::anyhow!("Failed to save config: {}", e))?;

    println!("Switched to account '{}'", account_name);
    Ok(())
}

pub async fn get_current_account_cli() -> Result<()> {
    let config = Config::load();
    match config.get_current_account() {
        Some(account) => println!("Current account: {}", account),
        None => println!("No current account set"),
    }
    Ok(())
}
