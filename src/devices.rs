use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{Result, anyhow};
use futures::{channel::oneshot, future};
use presage::manager::Registered;
use presage::{Manager, libsignal_service::configuration::SignalServers};
use presage_store_sqlite::SqliteStore;
use tokio::fs;
use tracing::error;

use crate::contacts::initial_sync;
use crate::open_store;
use crate::paths::{self, ASSETS, QRCODE};

// / Links a new device to the Signal account using the given name.
// / Generates a url and waits for the user to use it to complete the linking process.
pub async fn link_new_device_tui(device_name: String) -> Result<Manager<SqliteStore, Registered>> {
    if let Err(e) = std::fs::remove_file(paths::STORE) {
        error!("Couldn't remove store: {e}");
    }
    let store = open_store(paths::SQL_STORE).await?;

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

    if Path::new(ASSETS).exists() {
        fs::remove_dir_all(ASSETS).await?;
    }

    url_handler_result?;
    Ok(manager_result?)
}

/// Links a new device to the Signal account using the given name.
/// Generates a QR code and prints it in the terminal, then waits for the user to scan it to complete the linking process.
pub async fn link_new_device_cli(device_name: String) -> Result<()> {
    if let Err(e) = std::fs::remove_file(paths::STORE) {
        error!("Couldn't remove store: {e}");
    }
    let store = open_store(paths::SQL_STORE).await?;

    let (tx, rx) = oneshot::channel();
    let (manager_result, _err) = future::join(
        Manager::link_secondary_device(store, SignalServers::Production, device_name, tx),
        async move {
            match rx.await {
                Ok(url) => {
                    println!("Scan the QR code to link the device!");
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
    Ok(())
}

/// return true if the device is registered and false otherwise
pub async fn is_registered() -> Result<bool> {
    let store = open_store(paths::SQL_STORE).await?;

    match Manager::load_registered(store).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
