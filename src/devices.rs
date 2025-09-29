use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::account_management::{ensure_accounts_dir, get_account_store_path};
use crate::contacts::initial_sync;
use crate::open_store;
use crate::paths::ACCOUNTS_DIR;
use crate::paths::{self, ASSETS, QRCODE};
use anyhow::Result;
use futures::{channel::oneshot, future};
use presage::manager::Registered;
use presage::{Manager, libsignal_service::configuration::SignalServers};
use presage_store_sqlite::SqliteStore;
use tokio::fs;
use tracing::error;
use tracing::log::warn;
// Links a new device to the Signal account using the given name.
// Generates a QR code and prints it in the terminal, then waits for the user to scan it to complete the linking process.
// pub async fn link_new_device_cli(device_name: String) -> Result<()> {
//     let _ = std::fs::remove_file(paths::STORE);
//     let store = open_store(paths::STORE).await?;
//
//     let (tx, rx) = oneshot::channel();
//     let (manager_result, _err) = future::join(
//         Manager::link_secondary_device(store, SignalServers::Production, device_name, tx),
//         async move {
//             match rx.await {
//                 Ok(url) => {
//                     println!("Scan the QR code to link the device!");
//                     qr2term::print_qr(url.as_ref()).expect("QR generation failed");
//                     println!("You can also use the URL: {url}");
//                 }
//                 Err(err) => println!("Error while linking device: {err}"),
//             }
//         },
//     )
//     .await;
//
//     // TODO: Handle initial synchronization for CLI
//     #[allow(unused_variables)]
//     #[allow(unused_mut)]
//     let mut manager = match manager_result {
//         Ok(manager) => {
//             println!("Device linked successfully! Syncing contacts...");
//             manager
//         }
//         Err(e) => return Err(anyhow!("Error while linking device: {e}")),
//     };
//     Ok(())
// }

/// return true if the device is registered and false otherwise
pub async fn is_registered() -> Result<bool> {
    let store = open_store(paths::STORE).await?;

    match Manager::load_registered(store).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub async fn link_new_device_for_account(
    account_name: String,
    device_name: String,
) -> Result<Manager<SqliteStore, Registered>> {
    ensure_accounts_dir()?;

    let account_dir = format!("{ACCOUNTS_DIR}/{account_name}");
    let store_path = get_account_store_path(&account_name);

    fs::create_dir_all(&account_dir).await?;

    match std::fs::remove_file(&store_path) {
        Ok(_) => {}
        Err(e) => error!("Error: {}", e),
    }
    let store = open_store(&store_path).await?;

    if !Path::new(ASSETS).exists() {
        fs::create_dir(ASSETS).await?;
    }

    let (tx, rx) = oneshot::channel();
    let manager_task =
        Manager::link_secondary_device(store, SignalServers::Production, device_name, tx);

    let url_handler_task = async move {
        match rx.await {
            Ok(url) => {
                let mut file = File::create(QRCODE)
                    .map_err(|e| anyhow::anyhow!("Failed to create QRcode file: {}", e))?;

                file.write_all(url.as_ref().as_bytes())
                    .map_err(|e| anyhow::anyhow!("Failed to save url to qr code: {}", e))?;
                Ok(())
            }
            Err(err) => Err(anyhow::anyhow!("Login error: {}", err)),
        }
    };

    let (manager_result, url_handler_result) = future::join(manager_task, url_handler_task).await;

    if Path::new(QRCODE).exists()
        && let Err(e) = std::fs::remove_file(QRCODE)
    {
        error!(error = %e, "Failed to remove QR code file");
    }

    if Path::new(ASSETS).exists() {
        fs::remove_dir_all(ASSETS).await?;
    }

    url_handler_result?;

    let mut manager = manager_result?;
    if let Err(e) = initial_sync(&mut manager).await {
        warn!("Warning: Initial sync failed: {e}, but account is created");
    }

    Ok(manager)
}
